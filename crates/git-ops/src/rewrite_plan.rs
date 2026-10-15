//! Rewrites of the checked out branch that drop commits or replace them with new ones (squash), without touching the
//! working tree (docs/design/git-operations.md). The kept commits between the changed ones are replayed with
//! `git replay`, which writes only objects; the branch moves in one ref transaction, and the index follows only where
//! HEAD's tree changed.

use crate::commit_utils::empty_tree_id;
use crate::commit_writer::{NewCommit, NewParent, RawCommit, current_committer, read_raw_commits, write_commits};
use crate::copy_commit::CopyCommitError;
use crate::model::BranchError;
use crate::ref_transaction::RefTransaction;
use crate::rewrite_guard::{IndexUpdate, RewriteTarget, check_rewrite};
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::{debug, instrument};

/// `git replay --ref-action=print` reports the new tip as an update of this ref; the ref itself is never written
const REPLAY_REF: &str = "refs/branch-deck/replay";

/// A rewrite of the checked out branch whose new commits exist, but whose branch has not moved yet
#[derive(Debug)]
pub struct PreparedRewrite {
  pub(crate) target: RewriteTarget,
  pub(crate) new_head: String,
  pub(crate) index_update: IndexUpdate,
  /// Refs keeping the new objects reachable until the branch moves
  pub(crate) temp_refs: Vec<String>,
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
    for temp_ref in &self.temp_refs {
      transaction.delete(temp_ref, None);
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
    delete_temp_refs(git_executor, repo_path, &self.temp_refs);
  }
}

fn delete_temp_refs(git_executor: &GitCommandExecutor, repo_path: &str, temp_refs: &[String]) {
  for temp_ref in temp_refs {
    if let Err(e) = git_executor.execute_command(&["update-ref", "-d", temp_ref], repo_path) {
      tracing::warn!(temp_ref = %temp_ref, error = %e, "failed to delete temporary ref");
    }
  }
}

/// First-parent history of HEAD, newest first, with each commit's position
#[derive(Debug)]
pub struct History {
  pub(crate) commits: Vec<String>,
  positions: HashMap<String, usize>,
}

impl History {
  pub fn read(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Self, CopyCommitError> {
    let commits = git_executor.execute_command_lines(&["rev-list", "--first-parent", "HEAD"], repo_path)?;
    let positions = commits.iter().enumerate().map(|(index, id)| (id.clone(), index)).collect();
    Ok(Self { commits, positions })
  }

  /// The tip the history was read from; `None` for an unborn branch
  #[must_use]
  pub fn head(&self) -> Option<&str> {
    self.commits.first().map(String::as_str)
  }

  /// Position of the commit, 0 for HEAD
  pub fn position(&self, commit_id: &str) -> Result<usize, CopyCommitError> {
    self
      .positions
      .get(commit_id)
      .copied()
      .ok_or_else(|| CopyCommitError::Other(anyhow!("Commit {} is not on the current branch, sync and try again", short_id(commit_id))))
  }

  /// First parent of the commit at `position`; `None` for the root commit
  pub(crate) fn parent(&self, position: usize) -> Option<&str> {
    self.commits.get(position + 1).map(String::as_str)
  }

  /// Read the rewrite target and check that HEAD is still the tip this history was read from
  pub(crate) fn check_rewrite(&self, git_executor: &GitCommandExecutor, repo_path: &str, oldest_position: usize) -> Result<RewriteTarget, CopyCommitError> {
    let target = check_rewrite(git_executor, repo_path, &self.commits[oldest_position]).map_err(|e| CopyCommitError::BranchError(BranchError::Generic(format!("{e:#}"))))?;
    if self.commits.first() != Some(&target.head) {
      return Err(CopyCommitError::Other(anyhow!("Branch {} changed meanwhile, sync and try again", target.branch)));
    }
    Ok(target)
  }
}

/// The abbreviated commit id used in messages and in the output of the agent CLI
pub fn short_id(id: &str) -> &str {
  id.get(..8).unwrap_or(id)
}

/// Replay the commits of `revs` (a revision range of linear history, e.g. `A..B`) onto `onto` with `git replay`;
/// returns the new tip. The replayed commits keep their authors; the committer is the current user. Commits that
/// become empty are dropped. `None` on a conflict.
pub fn replay_range(git_executor: &GitCommandExecutor, repo_path: &str, onto: &str, revs: &[&str]) -> Result<Option<String>, CopyCommitError> {
  replay_range_with_env(git_executor, repo_path, onto, revs, &[])
}

/// `replay_range` with extra environment variables, e.g. a fixed `GIT_COMMITTER_DATE` for a replay whose commits are
/// only inspected: the same input then writes the same objects
#[instrument(skip(git_executor))]
pub fn replay_range_with_env(git_executor: &GitCommandExecutor, repo_path: &str, onto: &str, revs: &[&str], env: &[(&str, &str)]) -> Result<Option<String>, CopyCommitError> {
  let onto_arg = format!("--onto={onto}");
  let ref_arg = format!("--ref={REPLAY_REF}");
  let mut args = vec!["replay", "--ref-action=print", &onto_arg, &ref_arg];
  args.extend_from_slice(revs);
  let (output, exit_code) = git_executor.execute_command_with_status_env(&args, repo_path, env)?;
  match exit_code {
    0 => {
      // `update <ref> <new> <old>`
      let new_tip = output
        .lines()
        .find_map(|line| line.strip_prefix("update ")?.strip_prefix(REPLAY_REF)?.split_whitespace().next())
        .ok_or_else(|| CopyCommitError::Other(anyhow!("git replay printed no new tip: {output}")))?;
      Ok(Some(new_tip.to_string()))
    }
    1 => Ok(None),
    _ => Err(CopyCommitError::Other(anyhow!("git replay failed: {output}"))),
  }
}

/// Replay the commits of `from..to` onto `onto`, see `replay_range`
pub(crate) fn replay_onto(git_executor: &GitCommandExecutor, repo_path: &str, onto: &str, from: &str, to: &str) -> Result<Option<String>, CopyCommitError> {
  replay_range(git_executor, repo_path, onto, &[&format!("{from}..{to}")])
}

/// Three-way merge of trees without a working tree; `None` on a conflict
pub(crate) fn merge_trees(git_executor: &GitCommandExecutor, repo_path: &str, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, CopyCommitError> {
  let merge_base = format!("--merge-base={base}");
  let (output, exit_code) = git_executor.execute_command_with_status(&["merge-tree", "--write-tree", "--no-messages", &merge_base, ours, theirs], repo_path)?;
  match exit_code {
    0 => Ok(Some(output.lines().next().unwrap_or_default().to_string())),
    1 => Ok(None),
    _ => Err(CopyCommitError::Other(anyhow!("git merge-tree failed: {output}"))),
  }
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
  pub message: Vec<u8>,
  pub encoding: Option<String>,
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
  let target = history.check_rewrite(git_executor, repo_path, oldest_position)?;

  let mut writer = PlanWriter {
    git_executor,
    repo_path,
    history,
    target: &target,
    raw_commits: HashMap::new(),
    committer: String::new(),
    temp_refs: Vec::new(),
    replacements: Vec::new(),
  };
  let new_head = match writer.write(&roles, oldest_position, checks) {
    Ok(new_head) => new_head,
    Err(e) => {
      delete_temp_refs(git_executor, repo_path, &writer.temp_refs);
      return Err(e);
    }
  };
  let PlanWriter { temp_refs, replacements, .. } = writer;

  let old_tree = git_executor.resolve_tree_id(repo_path, &target.head)?;
  let new_tree = git_executor.resolve_tree_id(repo_path, &new_head)?;
  debug!(%new_head, "prepared rewrite");
  Ok(PreparedRewrite {
    target,
    new_head,
    index_update: IndexUpdate::TwoWay { old_tree, new_tree },
    temp_refs,
    replacements,
  })
}

/// The role of every commit the plan changes, by position; validates the plan
fn plan_roles<'a>(history: &History, plan: &'a RewritePlan) -> Result<BTreeMap<usize, Role<'a>>, CopyCommitError> {
  let mut roles = BTreeMap::new();
  let mut add = |position: usize, role: Role<'a>| {
    let Some(id) = history.commits.get(position) else {
      return Err(CopyCommitError::Other(anyhow!("Commit position {position} is outside the history of HEAD")));
    };
    if roles.insert(position, role).is_some() {
      return Err(CopyCommitError::Other(anyhow!("Commit {} is rewritten twice", short_id(id))));
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

fn refusal(message: String) -> CopyCommitError {
  CopyCommitError::BranchError(BranchError::Generic(message))
}

/// Writes the commits of one plan, recording the temporary refs to delete on success or refusal
struct PlanWriter<'a> {
  git_executor: &'a GitCommandExecutor,
  repo_path: &'a str,
  history: &'a History,
  target: &'a RewriteTarget,
  /// Replaced and folded commits and their parents
  raw_commits: HashMap<String, RawCommit>,
  committer: String,
  temp_refs: Vec<String>,
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
          return Err(CopyCommitError::Other(anyhow!("Cannot drop the root commit {}", short_id(&self.history.commits[position]))));
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

    if checks.keep_head_tree && self.tree(&new_head)? != self.tree(&self.target.head)? {
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
    let previous_id = &self.history.commits[previous_position];
    let end = &self.history.commits[end_position];
    let new_tip = new_tip.ok_or_else(|| CopyCommitError::Other(anyhow!("No commit to replay {} onto", short_id(end))))?;
    replay_onto(self.git_executor, self.repo_path, new_tip, previous_id, end)?.ok_or_else(|| {
      if let Role::Drop = previous_role {
        return refusal(format!(
          "Removing commit {} conflicts with later commits on {}: they depend on its changes",
          short_id(previous_id),
          self.target.branch
        ));
      }
      let start = &self.history.commits[previous_position - 1];
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

  /// Read every replaced and folded commit and their parents with one `cat-file --batch`
  fn read_replaced_commits(&mut self, roles: &BTreeMap<usize, Role<'_>>) -> Result<(), CopyCommitError> {
    let mut ids = HashSet::new();
    for (&position, role) in roles {
      if matches!(role, Role::Replace(_) | Role::Folded) {
        ids.insert(self.history.commits[position].as_str());
        ids.extend(self.history.parent(position));
      }
    }
    let ids: Vec<&str> = ids.into_iter().collect();
    self.raw_commits = read_raw_commits(self.git_executor, self.repo_path, &ids)?;
    Ok(())
  }

  fn raw(&self, id: &str) -> Result<&RawCommit, CopyCommitError> {
    self.raw_commits.get(id).ok_or_else(|| CopyCommitError::Other(anyhow!("Commit {id} not found")))
  }

  /// Write the replacement of the commit at `position` onto `new_tip`; returns its id
  fn write_replacement(&mut self, new_tip: Option<&str>, position: usize, replacement: &Replacement) -> Result<String, CopyCommitError> {
    let history = self.history;
    let commit_id = &history.commits[position];
    let replaced = self.raw(commit_id)?;
    let base_tree = match new_tip {
      Some(tip) => self.tree(tip)?,
      None => self.empty_tree()?,
    };

    // the commit's own changes, as a cherry-pick onto the rewritten history
    let mut tree = if new_tip == history.parent(position) {
      replaced.tree.clone()
    } else {
      let parent_tree = match history.parent(position) {
        Some(parent) => self.raw(parent)?.tree.clone(),
        None => self.empty_tree()?,
      };
      self.merge(&parent_tree, &base_tree, &replaced.tree, || {
        format!("Commit {} conflicts with the commits rewritten before it on {}", short_id(commit_id), self.target.branch)
      })?
    };

    let mut folded = replacement.folded.clone();
    // oldest first
    folded.sort_unstable_by(|a, b| b.cmp(a));
    for folded_position in folded {
      let folded_id = &history.commits[folded_position];
      let parent = history.parent(folded_position).expect("a folded commit is newer than the replaced one");
      tree = self.merge(&self.raw(parent)?.tree, &tree, &self.raw(folded_id)?.tree, || {
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

    let written = write_commits(
      self.git_executor,
      self.repo_path,
      &[NewCommit {
        tree: &tree,
        parent: new_tip.map(NewParent::Existing),
        author: &replaced.author,
        committer: &self.committer,
        encoding: replacement.encoding.as_deref(),
        message: &replacement.message,
      }],
    )?;
    self.temp_refs.push(written.temp_ref);
    let new_id = written.ids.into_iter().next().ok_or_else(|| CopyCommitError::Other(anyhow!("no commit was written")))?;
    self.replacements.push((commit_id.clone(), new_id.clone()));
    Ok(new_id)
  }

  /// `merge_trees`, refused with `conflict_message` on a conflict
  fn merge(&self, base: &str, ours: &str, theirs: &str, conflict_message: impl FnOnce() -> String) -> Result<String, CopyCommitError> {
    merge_trees(self.git_executor, self.repo_path, base, ours, theirs)?.ok_or_else(|| refusal(conflict_message()))
  }

  fn tree(&self, rev: &str) -> Result<String, CopyCommitError> {
    Ok(self.git_executor.resolve_tree_id(self.repo_path, rev)?)
  }

  /// Id of the empty tree in the repository's hash algorithm; the object is not written
  fn empty_tree(&self) -> Result<String, CopyCommitError> {
    Ok(empty_tree_id(self.git_executor, self.repo_path)?)
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

  let history = History::read(git_executor, repo_path)?;
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
