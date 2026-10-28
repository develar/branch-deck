//! Rewrites of the checked out branch that drop commits or replace them with new ones (squash), without touching the
//! working tree (docs/design/git-operations.md). The kept commits between the changed ones are replayed with
//! `git replay`, which writes only objects; the branch moves in one ref transaction, and the index follows only where
//! HEAD's tree changed.

use crate::commit_chain::CommitChain;
use crate::commit_utils::empty_tree;
use crate::commit_writer::{NewCommit, NewParent, RawCommit, WrittenCommits, current_committer, discard_temp_refs, read_raw_commits, write_commits};
use crate::copy_commit::CopyCommitError;
use crate::history::History;
use crate::merge_memo::MergeOutcome;
use crate::ref_transaction::RefTransaction;
use crate::replay::replay_onto;
use crate::revs::{resolve_tree_id, short_id};
use crate::rewrite_guard::{IndexUpdate, RewriteTarget};
use anyhow::anyhow;
use domain_types::BranchError;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{BTreeMap, HashMap};
use tracing::{debug, instrument};

/// A rewrite of the checked out branch whose new commits exist, but whose branch has not moved yet
#[derive(Debug)]
pub struct PreparedRewrite {
  pub(crate) target: RewriteTarget,
  pub(crate) new_head: String,
  pub(crate) index_update: IndexUpdate,
  /// The commits written on temporary refs, which keep the new objects reachable until the branch moves
  pub(crate) written: Vec<WrittenCommits>,
  /// `(replaced commit, its replacement)` of every `Replace` step, oldest first
  pub(crate) replacements: Vec<(String, String)>,
}

impl PreparedRewrite {
  /// The tip the branch moves to
  #[must_use]
  pub fn new_head(&self) -> &str {
    &self.new_head
  }

  /// The tip the branch moves from
  #[must_use]
  pub fn old_head(&self) -> &str {
    &self.target.head
  }

  /// The checked out branch it moves
  #[must_use]
  pub fn branch(&self) -> &str {
    &self.target.branch
  }

  /// The id of the commit that replaces `commit_id` (a `Replace` step)
  #[must_use]
  pub fn replacement_of(&self, commit_id: &str) -> Option<&str> {
    self.replacements.iter().find(|(old, _)| old == commit_id).map(|(_, new)| new.as_str())
  }

  /// Move the branch in one transaction with `transaction` (other ref changes of the same operation), then let the
  /// index follow. Nothing changes if a staged change would be lost or the branch moved meanwhile.
  #[instrument(skip(self, git_executor, transaction), fields(new_head = %self.new_head))]
  pub fn apply(&self, git_executor: &GitCommandExecutor, repo_path: &str, mut transaction: RefTransaction, operation: &str) -> Result<(), CopyCommitError> {
    if let Err(e) = self.index_update.preflight(git_executor, repo_path) {
      self.discard(git_executor, repo_path);
      return Err(CopyCommitError::BranchError(BranchError::Generic(format!("{e:#}"))));
    }

    transaction.update(&self.target.full_ref(), &self.new_head, Some(&self.target.head));
    for written in &self.written {
      written.delete_temp_refs(&mut transaction);
    }
    if let Err(e) = transaction.commit(git_executor, repo_path, operation) {
      self.discard(git_executor, repo_path);
      return Err(CopyCommitError::Other(anyhow!(
        "Branch {} changed while rewriting, sync and try again: {e:#}",
        self.target.branch
      )));
    }

    self.index_update.apply(git_executor, repo_path).map_err(CopyCommitError::Other)
  }

  /// Abandon the rewrite: delete the temporary refs, leaving the unreferenced objects to `git gc`
  pub fn discard(&self, git_executor: &GitCommandExecutor, repo_path: &str) {
    delete_temp_refs(git_executor, repo_path, &self.written);
  }
}

/// Delete the temporary refs of an abandoned rewrite in one transaction; nothing runs without any
fn delete_temp_refs(git_executor: &GitCommandExecutor, repo_path: &str, written: &[WrittenCommits]) {
  let names: Vec<&str> = written.iter().map(|written| written.temp_ref.as_str()).collect();
  discard_temp_refs(git_executor, repo_path, &names.join(" "), |transaction| {
    for written in written {
      written.delete_temp_refs(transaction);
    }
  });
}

/// The guards of a rewrite of the commits of `history` from `oldest_position` up to HEAD (`History::check_rewrite`),
/// and HEAD still the tip the history was read from
pub(crate) fn check_target(git_executor: &GitCommandExecutor, repo_path: &str, history: &History, oldest_position: usize) -> Result<RewriteTarget, CopyCommitError> {
  let target = history.check_rewrite(git_executor, repo_path, oldest_position).map_err(|e| refusal(format!("{e:#}")))?;
  if history.head() != Some(target.head.as_str()) {
    return Err(CopyCommitError::Other(anyhow!("Branch {} changed meanwhile, sync and try again", target.branch)));
  }
  Ok(target)
}

/// What a rewrite does with one commit; commits without a step are kept and replayed
#[derive(Debug)]
pub enum PlanStep {
  Drop,
  Replace(Replacement),
}

/// The commit is replaced by one with its changes followed by the changes of `folded` (as cherry-picks, oldest
/// first), keeping its author. The folded commits leave the history.
#[derive(Debug)]
pub struct Replacement {
  /// History positions of the folded commits, all newer than the replaced one
  pub folded: Vec<usize>,
  /// The message of the new commit, stored as UTF-8; `None` keeps the message and the encoding of the replaced commit
  /// byte for byte
  pub message: Option<Vec<u8>>,
}

/// Steps by History position
#[derive(Debug, Default)]
pub struct RewritePlan {
  pub steps: BTreeMap<usize, PlanStep>,
}

/// Checks of the rewritten history before the branch can move
#[derive(Debug, Clone, Copy, Default)]
pub struct PlanChecks {
  /// HEAD keeps its tree, so the index and the working tree are not affected (squash)
  pub keep_head_tree: bool,
  /// No commit is lost on the way: `git replay` silently drops commits that become empty
  pub check_commit_count: bool,
}

#[derive(Clone, Copy)]
enum Role<'a> {
  Drop,
  Replace(&'a Replacement),
  Folded,
}

/// Prepare the rewrite of the checked out branch described by `plan`, with positions in `history`. The new commits
/// are written and the kept ones replayed, oldest first; the branch is not moved, `PreparedRewrite::apply` does it.
/// A refusal (conflict, empty result, failed check) leaves no temporary refs.
#[instrument(skip(git_executor, history, plan))]
pub fn prepare_plan(git_executor: &GitCommandExecutor, repo_path: &str, history: &History, plan: &RewritePlan, checks: PlanChecks) -> Result<PreparedRewrite, CopyCommitError> {
  let roles = plan_roles(history, plan)?;
  let Some((&oldest_position, _)) = roles.last_key_value() else {
    return Err(CopyCommitError::Other(anyhow!("Nothing to rewrite")));
  };
  let target = check_target(git_executor, repo_path, history, oldest_position)?;

  let mut writer = PlanWriter {
    git_executor,
    repo_path,
    history,
    target: &target,
    raw_commits: HashMap::new(),
    trees: HashMap::new(),
    merger: CommitChain::new(git_executor, repo_path),
    committer: String::new(),
    written: Vec::new(),
    replacements: Vec::new(),
  };
  let outcome = writer.write(&roles, oldest_position, checks).and_then(|new_head| {
    let new_tree = writer.tree(&new_head)?;
    Ok((new_head, new_tree))
  });
  let PlanWriter {
    written, replacements, merger, ..
  } = writer;
  let finished = merger.finish().map_err(CopyCommitError::Other);
  let (new_head, new_tree) = match outcome.and_then(|outcome| finished.map(|()| outcome)) {
    Ok(outcome) => outcome,
    Err(e) => {
      delete_temp_refs(git_executor, repo_path, &written);
      return Err(e);
    }
  };

  // `check_target` saw HEAD at the history's head
  let old_tree = history.commit(0).tree.clone();
  debug!(%new_head, "prepared rewrite");
  Ok(PreparedRewrite {
    target,
    new_head,
    index_update: IndexUpdate::TwoWay { old_tree, new_tree },
    written,
    replacements,
  })
}

/// The role of every commit the plan changes, by position; validates the plan
fn plan_roles<'a>(history: &History, plan: &'a RewritePlan) -> Result<BTreeMap<usize, Role<'a>>, CopyCommitError> {
  let mut roles = BTreeMap::new();
  let mut add = |position: usize, role: Role<'a>| {
    let Some(commit) = history.commits().get(position) else {
      return Err(CopyCommitError::Other(anyhow!("Commit position {position} is outside the history of HEAD")));
    };
    if roles.insert(position, role).is_some() {
      return Err(CopyCommitError::Other(anyhow!("Commit {} is rewritten twice", short_id(&commit.id))));
    }
    Ok(())
  };
  for (&position, step) in &plan.steps {
    match step {
      PlanStep::Drop => add(position, Role::Drop)?,
      PlanStep::Replace(replacement) => {
        add(position, Role::Replace(replacement))?;
        for &folded in &replacement.folded {
          if folded >= position {
            return Err(CopyCommitError::Other(anyhow!("A commit can only be squashed into an older commit")));
          }
          add(folded, Role::Folded)?;
        }
      }
    }
  }
  Ok(roles)
}

const fn refusal(message: String) -> CopyCommitError {
  CopyCommitError::BranchError(BranchError::Generic(message))
}

/// Writes the commits of one plan, recording the temporary refs to delete on success or refusal
struct PlanWriter<'a> {
  git_executor: &'a GitCommandExecutor,
  repo_path: &'a str,
  history: &'a History,
  target: &'a RewriteTarget,
  /// The replaced commits: their authors and messages
  raw_commits: HashMap<String, RawCommit>,
  /// The trees of the commits this plan wrote or replayed; the history has the others
  trees: HashMap<String, String>,
  /// One `merge-tree --stdin` for every merge of the plan
  merger: CommitChain,
  committer: String,
  /// The replacements, each on its temporary ref
  written: Vec<WrittenCommits>,
  replacements: Vec<(String, String)>,
}

impl PlanWriter<'_> {
  /// Write the new history; returns the new HEAD
  fn write(&mut self, roles: &BTreeMap<usize, Role<'_>>, oldest_position: usize, checks: PlanChecks) -> Result<String, CopyCommitError> {
    if roles.values().any(|role| matches!(role, Role::Replace(_))) {
      self.read_replaced_commits(roles)?;
      self.committer = current_committer(self.git_executor, self.repo_path)?;
    }

    let mut new_tip = self.history.parent(oldest_position).map(str::to_string);
    // the previous changed commit, older than the current one
    let mut previous: Option<(usize, Role<'_>)> = None;
    for (&position, &role) in roles.iter().rev() {
      if let Some(previous) = previous
        && previous.0 > position + 1
      {
        new_tip = Some(self.replay_segment(new_tip.as_deref(), previous, position + 1)?);
      }
      match role {
        Role::Drop if new_tip.is_none() => {
          return Err(CopyCommitError::Other(anyhow!("Cannot drop the root commit {}", short_id(self.history.id(position)))));
        }
        Role::Drop | Role::Folded => {}
        Role::Replace(replacement) => new_tip = Some(self.write_replacement(new_tip.as_deref(), position, replacement)?),
      }
      previous = Some((position, role));
    }

    let new_head = match previous {
      Some(previous) if previous.0 > 0 => self.replay_segment(new_tip.as_deref(), previous, 0)?,
      // HEAD itself changed; only a dropped root would leave no commit, and that is refused above
      _ => new_tip.ok_or_else(|| CopyCommitError::Other(anyhow!("The rewrite leaves no commits")))?,
    };

    if checks.keep_head_tree && self.tree(&new_head)? != self.history.commit(0).tree {
      return Err(refusal(format!(
        "The rewrite would change the files of {}: later commits depend on the order of the changes",
        self.target.branch
      )));
    }
    if checks.check_commit_count {
      let removed = roles.values().filter(|role| !matches!(role, Role::Replace(_))).count();
      let expected = oldest_position + 1 - removed;
      let actual = self.count_commits(&new_head, self.history.parent(oldest_position))?;
      if actual != expected {
        return Err(refusal(format!(
          "The rewrite would leave {} later commits of {} empty: their changes are already in the squashed commit",
          expected.abs_diff(actual),
          self.target.branch
        )));
      }
    }
    Ok(new_head)
  }

  /// Replay the kept commits between the changed commit `previous` and `end_position` (inclusive) onto `new_tip`
  fn replay_segment(&self, new_tip: Option<&str>, (previous_position, previous_role): (usize, Role<'_>), end_position: usize) -> Result<String, CopyCommitError> {
    let previous_id = self.history.id(previous_position);
    let end = self.history.id(end_position);
    let new_tip = new_tip.ok_or_else(|| CopyCommitError::Other(anyhow!("No commit to replay {} onto", short_id(end))))?;
    replay_onto(self.git_executor, self.repo_path, new_tip, previous_id, end)?.ok_or_else(|| {
      if matches!(previous_role, Role::Drop) {
        return refusal(format!(
          "Removing commit {} conflicts with later commits on {}: they depend on its changes",
          short_id(previous_id),
          self.target.branch
        ));
      }
      let start = self.history.id(previous_position - 1);
      let segment = if start == end {
        format!("commit {}", short_id(end))
      } else {
        format!("commits {} to {}", short_id(start), short_id(end))
      };
      refusal(format!(
        "Squashing conflicts with {segment} on {}: they change the same lines as the squashed commits",
        self.target.branch
      ))
    })
  }

  /// Read every replaced commit with one `cat-file --batch`: its author and message; the trees come from the history
  fn read_replaced_commits(&mut self, roles: &BTreeMap<usize, Role<'_>>) -> Result<(), CopyCommitError> {
    let ids: Vec<&str> = roles
      .iter()
      .filter(|(_, role)| matches!(role, Role::Replace(_)))
      .map(|(&position, _)| self.history.id(position))
      .collect();
    self.raw_commits = read_raw_commits(self.git_executor, self.repo_path, &ids)?;
    Ok(())
  }

  /// Write the replacement of the commit at `position` onto `new_tip`; returns its id
  fn write_replacement(&mut self, new_tip: Option<&str>, position: usize, replacement: &Replacement) -> Result<String, CopyCommitError> {
    let (history, target) = (self.history, self.target);
    let commit_id = history.id(position);
    let base_tree = match new_tip {
      Some(tip) => self.tree(tip)?,
      None => self.empty_tree(),
    };

    // the commit's own changes, as a cherry-pick onto the rewritten history
    let own_tree = &history.commit(position).tree;
    let mut tree = if new_tip == history.parent(position) {
      own_tree.clone()
    } else {
      let parent_tree = self.parent_tree(position)?;
      self.merge(&parent_tree, &base_tree, own_tree, || {
        format!("Commit {} conflicts with the commits rewritten before it on {}", short_id(commit_id), target.branch)
      })?
    };

    let mut folded = replacement.folded.clone();
    // oldest first
    folded.sort_unstable_by(|a, b| b.cmp(a));
    for folded_position in folded {
      let folded_id = history.id(folded_position);
      // a folded commit is newer than the replaced one, so its parent is in the history
      let parent_tree = self.parent_tree(folded_position)?;
      tree = self.merge(&parent_tree, &tree, &history.commit(folded_position).tree, || {
        format!(
          "Squashing commit {} into {} conflicts: the commits in between change the same lines",
          short_id(folded_id),
          short_id(commit_id)
        )
      })?;
    }
    if tree == base_tree {
      return Err(refusal(format!("Squashing into commit {} leaves no changes", short_id(commit_id))));
    }

    let replaced = self
      .raw_commits
      .get(commit_id)
      .ok_or_else(|| CopyCommitError::Other(anyhow!("Commit {commit_id} not found")))?;
    let (message, encoding) = match &replacement.message {
      Some(message) => (message.as_slice(), None),
      None => (replaced.message.as_slice(), replaced.encoding.as_deref()),
    };
    let written = write_commits(
      self.git_executor,
      self.repo_path,
      &[NewCommit {
        tree: &tree,
        parent: new_tip.map(NewParent::Existing),
        author: &replaced.author,
        committer: &self.committer,
        encoding,
        message,
      }],
    )?;
    let new_id = written.ids.first().cloned().ok_or_else(|| CopyCommitError::Other(anyhow!("no commit was written")))?;
    self.written.push(written);
    self.replacements.push((commit_id.to_string(), new_id.clone()));
    self.trees.insert(new_id.clone(), tree);
    Ok(new_id)
  }

  /// A merge of the plan's `merge-tree --stdin`, refused with `conflict_message` on a conflict
  fn merge(&mut self, base: &str, ours: &str, theirs: &str, conflict_message: impl FnOnce() -> String) -> Result<String, CopyCommitError> {
    match self.merger.merge(base, ours, theirs)? {
      MergeOutcome::Clean { tree } => Ok(tree),
      MergeOutcome::Conflict { .. } => Err(refusal(conflict_message())),
    }
  }

  /// The tree of a commit of the history or of this plan; one `rev-parse` for a replayed commit
  fn tree(&mut self, id: &str) -> Result<String, CopyCommitError> {
    if let Some(tree) = self.history.tree_of(id).or_else(|| self.trees.get(id).map(String::as_str)) {
      return Ok(tree.to_string());
    }
    let tree = resolve_tree_id(self.git_executor, self.repo_path, id)?;
    self.trees.insert(id.to_string(), tree.clone());
    Ok(tree)
  }

  /// The tree of the first parent of the commit at `position`; the empty tree for the root commit
  fn parent_tree(&mut self, position: usize) -> Result<String, CopyCommitError> {
    match self.history.parent(position) {
      Some(parent) => self.tree(parent),
      None => Ok(self.empty_tree()),
    }
  }

  /// The empty tree, in the hash algorithm of the repository; no process
  fn empty_tree(&self) -> String {
    empty_tree(&self.target.head).to_string()
  }

  /// Number of first-parent commits from `tip` down to, not including, `base` (all of them without a base)
  fn count_commits(&self, tip: &str, base: Option<&str>) -> Result<usize, CopyCommitError> {
    let exclude = base.map(|base| format!("^{base}"));
    let mut args = vec!["rev-list", "--count", "--first-parent", tip];
    args.extend(exclude.as_deref());
    let count = self.git_executor.execute_command(&args, self.repo_path)?;
    count.parse().map_err(|e| CopyCommitError::Other(anyhow!("Unexpected rev-list count {count:?}: {e}")))
  }
}

/// Prepare dropping `commit_ids_to_drop` from the checked out branch: their descendants are replayed onto the
/// remaining history. The branch is not moved; `PreparedRewrite::apply` does it.
/// The working tree keeps the dropped content, so it shows up as unstaged changes (files the dropped commits added as
/// untracked); staged changes on other paths stay staged.
#[instrument(skip(git_executor))]
pub fn prepare_drop_commits(git_executor: &GitCommandExecutor, repo_path: &str, commit_ids_to_drop: &[String]) -> Result<PreparedRewrite, CopyCommitError> {
  if commit_ids_to_drop.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No commits specified to drop")));
  }

  let ids: Vec<&str> = commit_ids_to_drop.iter().map(String::as_str).collect();
  let history = History::read_to(git_executor, repo_path, &ids)?;
  let mut plan = RewritePlan::default();
  for id in commit_ids_to_drop {
    plan.steps.insert(history.position(id)?, PlanStep::Drop);
  }
  prepare_plan(git_executor, repo_path, &history, &plan, PlanChecks::default())
}

/// Drop `commit_ids_to_drop` from the checked out branch; returns the new HEAD
#[instrument(skip(git_executor))]
pub fn drop_commits_from_head(git_executor: &GitCommandExecutor, repo_path: &str, commit_ids_to_drop: &[String]) -> Result<String, CopyCommitError> {
  let prepared = prepare_drop_commits(git_executor, repo_path, commit_ids_to_drop)?;
  prepared.apply(git_executor, repo_path, RefTransaction::new(), &format!("drop {} commits", commit_ids_to_drop.len()))?;
  Ok(prepared.new_head)
}
