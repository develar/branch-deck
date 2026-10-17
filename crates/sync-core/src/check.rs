//! A trial sync of a regroup plan: the plan is applied in memory, the branches are placed as a stacked sync places them
//! and built in dependency order with `build_branch`, which writes commit objects only. No ref, note, remote status,
//! HEAD or working tree changes, and no repository lock. Merges and commit trees go through the object cache, so trying
//! variants of a plan repeats no `merge-tree`.
//!
//! `stack_independence` asks one question of the check: does a stacked branch build on the baseline without its
//! parent? It builds that branch alone, as Unstack would make it, and names the commits of its ancestors it needs.

use crate::autosquash::plan_autosquash;
use crate::branch_processor::{BranchJob, BranchSyncContext, BuildOutcome, build_branch};
use crate::commit_grouper::{CommitGrouper, GroupedCommits, parse_subject};
use crate::commit_ids::{IdMatch, match_commit_id};
use crate::dangling_references::{DanglingInput, DanglingReference, SyntaxMemo, find_dangling_references, read_commit_lines};
use crate::dependencies::compute_dependencies;
use crate::diff_scan::{CommitDiff, read_commit_diffs_cached};
use crate::grouping::branch_tips;
use crate::missing_commits::CommitOwners;
use crate::regroup::{RegroupError, RegroupOutcome, RegroupPlan, Snapshot, plan_in_memory, read_guarded_snapshot};
use crate::stack::{StackForest, describe_problem};
use crate::stack::{empty_parent_states, place_branches, read_only_detection};
use crate::symbol_scan::{SymbolMode, read_symbol_mode};
use crate::sync::seeded_tree_cache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::Commit;
use git_ops::merge_memo::{MemoizedMerge, MergeMemo, MergeTrees};
use git_ops::model::{BranchError, BranchKind, strip_branch_namespace};
use object_cache::cache::ObjectCache;
use object_cache::commit_lines::FileLines;
use object_cache::merge_result::{MergeKey, MergeResult};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use sync_types::stack::{BlockReason, StackProblem};
use sync_types::{ProgressReporter, SyncEvent};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct CheckReport {
  pub head: String,
  pub baseline_branch: String,
  /// The commit the branches without a parent are built on; `None` without commits ahead of the baseline
  pub base: Option<String>,
  pub stacks: bool,
  /// The plan applied in memory; nothing changed without a plan
  pub regroup: RegroupOutcome,
  /// In build order: parents before their children
  pub branches: Vec<BranchCheck>,
  pub unassigned: usize,
  /// By branch: the uses of names a built branch removes, in its virtual tree but not in HEAD's
  /// (`dangling_references.rs`); empty for a check of one branch
  pub dangling: BTreeMap<String, Vec<DanglingReference>>,
}

#[derive(Debug, Clone)]
pub struct BranchCheck {
  pub name: String,
  /// Oldest first
  pub commits: Vec<String>,
  pub declared_parent: Option<String>,
  /// The branch it is built on; `None` for `base`
  pub built_on: Option<String>,
  pub outcome: CheckOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
  /// The branch builds; `head` is the commit object a sync would write
  Ok {
    head: String,
  },
  /// The cherry-pick of `commit` conflicts in `files`
  Conflict {
    commit: String,
    files: Vec<String>,
  },
  Error {
    message: String,
  },
  /// Not built: the parent can't be built on
  Blocked {
    parent: String,
    reason: BlockReason,
  },
  /// Not built: the branch can't be placed in its stack
  Problems(Vec<StackProblem>),
}

/// Whether a stacked branch builds on the baseline without its parent (`stack_independence`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StackIndependence {
  /// Built as a root, the branch applies cleanly. Lines only: a compile dependency on the parent is not seen
  Builds {
    /// The declared parent the branch is stacked on
    parent: String,
  },
  /// Built as a root, a commit of the branch conflicts
  Needs {
    parent: String,
    /// The first commit of the branch that conflicts
    child_commit: StackCommit,
    /// The files it conflicts in
    files: Vec<String>,
    /// The commits of the ancestors the branch needs, and the commits of the ancestors these need, oldest first, each
    /// with its fixup unit (a move takes a target with its fixups): what a move into the branch moves. By the line
    /// dependencies (`dependencies.rs`), so empty when no commit of an ancestor explains the conflict
    parent_commits: Vec<StackCommit>,
    /// `parent_commits` can move into the branch: no commit left in a branch whose build holds one of them (the
    /// ancestors that own one, and the branches built on those, but not this branch and the branches built on it)
    /// depends on them. Otherwise a move makes that branch need this one
    movable: bool,
  },
}

/// A commit ahead of the baseline, as `StackIndependence` names it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackCommit {
  pub id: String,
  /// The subject without its prefix; an autosquash marker stays
  pub subject: String,
  /// The branch the commit is in
  pub branch: String,
}

/// Does the stacked `branch` build on the baseline without its parent? A trial build of the branch alone, as Unstack
/// would make it a root; writes commit objects only
#[instrument(skip(git_executor, cache))]
pub fn stack_independence(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  branch: &str,
  cache: &ObjectCache,
) -> Result<StackIndependence, RegroupError> {
  CheckSession::open(git_executor, repository_path, branch_prefix, cache)?.stack_independence(branch)
}

/// Check `plan` (or the current grouping without one): would every branch build?
#[instrument(skip(git_executor, plan, cache))]
pub fn check_plan(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  plan: Option<&RegroupPlan>,
  cache: &ObjectCache,
) -> Result<CheckReport, RegroupError> {
  CheckSession::open(git_executor, repository_path, branch_prefix, cache)?.check(plan)
}

/// What every check of a repository reads once: the commits ahead of the baseline as they group now, the baseline and
/// the branch tips. A `check` of a plan then runs git only for the merges the object cache misses and, with stacked
/// branches on, for the detection notes of parents without commits; so trying many plans repeats no `log`
pub struct CheckSession {
  git_executor: GitCommandExecutor,
  repository_path: String,
  branch_prefix: String,
  cache: ObjectCache,
  snapshot: Snapshot,
  /// Full name (without `refs/heads/`) -> tip of the branches under the prefix; empty without commits
  tips: HashMap<String, String>,
  /// Check every mapping note against the tree of its virtual commit (`bd --no-cache`)
  verify_mapping_notes: bool,
  /// The changed lines of the commits ahead of the baseline and the baseline tip, read by the first check
  dangling_input: OnceLock<Result<DanglingFacts, String>>,
  /// `branchdeck.symbols`
  symbols: SymbolMode,
  /// The syntax trees of the blobs the dangling-reference checks read, shared with the sessions of `with_rewrite`
  syntax: Arc<SyntaxMemo>,
}

/// What every dangling-reference check of a session reads once: the changed lines of the commits ahead of the baseline,
/// and the baseline tip
type DanglingFacts = (HashMap<String, Vec<FileLines>>, Option<String>);

impl CheckSession {
  /// Read the snapshot after the guards of a regroup (no operation in progress, HEAD on a branch) and the branch tips
  #[instrument(skip(git_executor, cache))]
  pub fn open(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str, cache: &ObjectCache) -> Result<Self, RegroupError> {
    let snapshot = read_guarded_snapshot(git_executor, repository_path)?;
    let tips = if snapshot.commits.is_empty() {
      HashMap::new()
    } else {
      branch_tips(git_executor, repository_path, branch_prefix)?
    };
    let symbols = read_symbol_mode(git_executor, repository_path).map_err(RegroupError::from)?;
    Ok(Self {
      git_executor: git_executor.clone(),
      repository_path: repository_path.to_string(),
      branch_prefix: branch_prefix.to_string(),
      cache: cache.clone(),
      snapshot,
      tips,
      verify_mapping_notes: false,
      dangling_input: OnceLock::new(),
      symbols,
      syntax: Arc::default(),
    })
  }

  /// Check every mapping note of the trial builds against the tree of its virtual commit, as a sync with
  /// `SyncOptions::verify_mapping_notes` does
  #[must_use]
  pub const fn verifying_mapping_notes(mut self, verify: bool) -> Self {
    self.verify_mapping_notes = verify;
    self
  }

  /// The commits ahead of the baseline, oldest first, and their groups now
  pub const fn snapshot(&self) -> &Snapshot {
    &self.snapshot
  }

  /// Full name (without `refs/heads/`) -> tip of the branches under the prefix
  pub const fn tips(&self) -> &HashMap<String, String> {
    &self.tips
  }

  /// A session on the history after a rewrite that was written as objects only (a `MoveHunkMode::Trial`, which keeps
  /// HEAD's tree): each commit of `commits` (old id -> new id, oldest first, the tree of each in `trees`) replaces its
  /// old version, with its parent following and without its mapping note, so its branch is built anew; the groups, the
  /// stack forest and the branch tips stay
  #[must_use]
  pub fn with_rewrite(&self, commits: &[(String, String)], trees: &[String]) -> Self {
    let new_ids: HashMap<&str, &str> = commits.iter().map(|(old, new)| (old.as_str(), new.as_str())).collect();
    let new_trees: HashMap<&str, &str> = commits.iter().zip(trees).map(|((old, _), tree)| (old.as_str(), tree.as_str())).collect();
    let renamed = |id: &str| new_ids.get(id).map_or_else(|| id.to_string(), |new| (*new).to_string());
    let mut snapshot = self.snapshot.clone();
    snapshot.head = renamed(&snapshot.head);
    for commit in &mut snapshot.commits {
      let Some(tree) = new_trees.get(commit.id.as_str()) else { continue };
      commit.id = renamed(&commit.id);
      commit.tree_id = (*tree).to_string();
      commit.parent_id = commit.parent_id.as_deref().map(renamed);
      commit.note = None;
      commit.mapped_commit_id = None;
      commit.mapped_source = None;
    }
    snapshot.groups = snapshot.groups.into_iter().map(|(id, group)| (renamed(id.as_str()), group)).collect();
    Self {
      git_executor: self.git_executor.clone(),
      repository_path: self.repository_path.clone(),
      branch_prefix: self.branch_prefix.clone(),
      cache: self.cache.clone(),
      snapshot,
      tips: self.tips.clone(),
      verify_mapping_notes: self.verify_mapping_notes,
      dangling_input: OnceLock::new(),
      symbols: self.symbols,
      syntax: Arc::clone(&self.syntax),
    }
  }

  /// `plan` for the session of `with_rewrite(commits, …)`: its commit ids, abbreviated ones too, name the new commits
  pub fn rewritten_plan(&self, plan: &RegroupPlan, commits: &[(String, String)]) -> RegroupPlan {
    let new_ids: HashMap<&str, &str> = commits.iter().map(|(old, new)| (old.as_str(), new.as_str())).collect();
    let renamed = |id: &String| match match_commit_id(id, self.snapshot.commits.iter().map(|commit| commit.id.as_str())) {
      IdMatch::One(full) => new_ids.get(full).map_or_else(|| id.clone(), |new| (*new).to_string()),
      IdMatch::None | IdMatch::Ambiguous => id.clone(),
    };
    RegroupPlan {
      branches: plan.branches.iter().map(|(path, ids)| (path.clone(), ids.iter().map(renamed).collect())).collect(),
      unassign: plan.unassign.iter().map(renamed).collect(),
      ..plan.clone()
    }
  }

  /// Check `plan` (or the current grouping without one) against the snapshot
  #[instrument(skip_all)]
  pub fn check(&self, plan: Option<&RegroupPlan>) -> Result<CheckReport, RegroupError> {
    self.check_only(plan, None, None)
  }

  /// `check`, with the dangling references of `branches` only: a trial of a fix that concerns them
  #[instrument(skip_all)]
  pub fn check_with_dangling_of(&self, plan: Option<&RegroupPlan>, branches: &[String]) -> Result<CheckReport, RegroupError> {
    self.check_only(plan, None, Some(branches))
  }

  /// The check of `plan`; with `only`, of that branch and the branches it is built on, the others are not built and
  /// not in the report
  pub(crate) fn check_only(&self, plan: Option<&RegroupPlan>, only: Option<&str>, dangling_of: Option<&[String]>) -> Result<CheckReport, RegroupError> {
    let (git_executor, repository_path, branch_prefix) = (&self.git_executor, self.repository_path.as_str(), self.branch_prefix.as_str());
    let no_plan = RegroupPlan::default();
    let regroup = plan_in_memory(&self.snapshot, plan.unwrap_or(&no_plan))?;
    let baseline_branch = self.snapshot.baseline.clone();
    let messages: HashMap<&str, &str> = regroup.commits.iter().map(|commit| (commit.id.as_str(), commit.message.as_str())).collect();
    // the issue groups as the plan leaves them
    let mut grouper = CommitGrouper::new_with(&regroup.grouping);
    for commit in &self.snapshot.commits {
      let mut commit = commit.clone();
      if let Some(message) = messages.get(commit.id.as_str()) {
        apply_message(&mut commit, message);
      }
      grouper.add_commit(commit);
    }
    let base = grouper.first_parent_id.take();
    let (GroupedCommits { branches, unassigned, .. }, forest) = grouper.finish_with_stacks();

    let placements = match &forest {
      Some(forest) => Some(place_branches(
        forest,
        &empty_parent_states(git_executor, repository_path, branch_prefix, &self.tips, forest, read_only_detection(&baseline_branch))?,
      )),
      None => None,
    };
    let order: Vec<String> = forest.as_ref().map_or_else(|| branches.keys().cloned().collect(), |forest| forest.order.clone());
    // `only` and the branches it is built on
    let built: Option<HashSet<String>> = only.map(|name| {
      let mut built = HashSet::new();
      let mut current = Some(name.to_string());
      while let Some(name) = current.filter(|name| !built.contains(name)) {
        current = placements
          .as_ref()
          .and_then(|placements| placements.get(&name))
          .and_then(|stack| stack.effective_base.clone());
        built.insert(name);
      }
      built
    });

    let progress = CollectingReporter::default();
    let context = BranchSyncContext {
      repository_path: repository_path.to_string(),
      branch_prefix: branch_prefix.to_string(),
      baseline_branch: baseline_branch.clone(),
      total_branches: branches.len(),
      progress: progress.clone(),
      git_executor: git_executor.clone(),
      tree_id_cache: seeded_tree_cache(TreeIdCache::with_merge_memo(Arc::new(CacheMemo { cache: self.cache.clone() })), branches.values().flatten()),
      existing_tips: self
        .tips
        .iter()
        .filter(|(name, _)| strip_branch_namespace(name, branch_prefix, BranchKind::Virtual).is_some())
        .map(|(name, tip)| (name.clone(), tip.clone()))
        .collect(),
      // no remote status
      ancestor_keys: HashMap::new(),
      verify_mapping_notes: self.verify_mapping_notes,
      // conflicts are light, without missing commits: `missing_commits` finds them in the report
      owners: CommitOwners::default(),
    };

    let mut checks: Vec<BranchCheck> = Vec::with_capacity(order.len());
    for (branch_idx, name) in order.iter().enumerate() {
      let Some(commits) = branches.get(name) else { continue };
      if built.as_ref().is_some_and(|built| !built.contains(name)) {
        continue;
      }
      let placement = placements.as_ref().and_then(|placements| placements.get(name));
      let built_on = placement.and_then(|stack| stack.effective_base.clone());
      let parent_outcome = built_on
        .as_ref()
        .and_then(|parent| checks.iter().find(|check| &check.name == parent))
        .map(|check| &check.outcome);
      let outcome = if let Some(stack) = placement.filter(|stack| !stack.problems.is_empty()) {
        CheckOutcome::Problems(stack.problems.clone())
      } else if let Some(block) = placement.and_then(|stack| stack.blocked_by.as_ref()) {
        CheckOutcome::Blocked {
          parent: block.parent.clone(),
          reason: block.reason,
        }
      } else {
        match (parent_outcome, &built_on) {
          (Some(CheckOutcome::Ok { head }), Some(_)) => build(&context, &progress, name, commits, branch_idx, head, true),
          (Some(outcome), Some(parent)) => CheckOutcome::Blocked {
            parent: parent.clone(),
            reason: match outcome {
              CheckOutcome::Conflict { .. } => BlockReason::Conflict,
              CheckOutcome::Blocked { .. } => BlockReason::Blocked,
              _ => BlockReason::Error,
            },
          },
          _ => match &base {
            Some(base) => build(&context, &progress, name, commits, branch_idx, base, false),
            None => CheckOutcome::Error {
              message: "The oldest commit ahead of the baseline has no parent".to_string(),
            },
          },
        }
      };
      checks.push(BranchCheck {
        name: name.clone(),
        commits: commits.iter().map(|commit| commit.id.clone()).collect(),
        declared_parent: placement.and_then(|stack| stack.declared_parent.clone()),
        built_on,
        outcome,
      });
    }

    let dangling = if only.is_some() {
      BTreeMap::new()
    } else {
      self.dangling_references(&regroup.previous_head, base.as_deref(), &checks, dangling_of)?
    };
    Ok(CheckReport {
      head: regroup.previous_head.clone(),
      baseline_branch,
      base,
      stacks: regroup.grouping.stacks,
      regroup,
      branches: checks,
      unassigned: unassigned.len(),
      dangling,
    })
  }

  /// The changed lines of the commits ahead of the baseline and the baseline tip, read once per session
  fn dangling_facts(&self) -> Result<&DanglingFacts, RegroupError> {
    let commits: Vec<String> = self.snapshot.commits.iter().map(|commit| commit.id.clone()).collect();
    let read = self.dangling_input.get_or_init(|| {
      let lines = read_commit_lines(&self.git_executor, &self.repository_path, &commits, &self.cache).map_err(|e| format!("{e:#}"))?;
      // the local baseline branch itself has no tip ahead of the base
      let tip = self
        .git_executor
        .execute_command_with_status(
          &["rev-parse", "--verify", "--quiet", &format!("{}^{{commit}}", self.snapshot.baseline)],
          &self.repository_path,
        )
        .ok()
        .filter(|(_, exit_code)| *exit_code == 0)
        .map(|(tip, _)| tip);
      Ok((lines, tip))
    });
    read.as_ref().map_err(|e| RegroupError::Other(anyhow::anyhow!("Cannot read the changed lines: {e}")))
  }

  /// The changed lines of each commit ahead of the baseline, in the files of the known languages
  pub(crate) fn changed_lines(&self) -> Result<&HashMap<String, Vec<FileLines>>, RegroupError> {
    self.dangling_facts().map(|(lines, _)| lines)
  }

  /// The changes of the commits ahead of the baseline, oldest first, from the object cache and one `log` for the misses
  pub(crate) fn commit_diffs(&self) -> Result<Vec<CommitDiff>, RegroupError> {
    let ids: Vec<String> = self.snapshot.commits.iter().map(|commit| commit.id.clone()).collect();
    read_commit_diffs_cached(&self.git_executor, &self.repository_path, &self.cache, &ids).map_err(RegroupError::from)
  }

  /// The dangling references of the built branches
  fn dangling_references(&self, head: &str, base: Option<&str>, checks: &[BranchCheck], of: Option<&[String]>) -> Result<BTreeMap<String, Vec<DanglingReference>>, RegroupError> {
    let commits: Vec<String> = self.snapshot.commits.iter().map(|commit| commit.id.clone()).collect();
    let (lines, baseline_tip) = self.dangling_facts()?;
    let input = DanglingInput {
      head,
      base,
      baseline_tip: baseline_tip.as_deref(),
      commits: &commits,
      lines,
      of,
      syntax: (self.symbols != SymbolMode::Off).then_some(&*self.syntax),
    };
    find_dangling_references(&self.git_executor, &self.repository_path, &input, checks).map_err(RegroupError::from)
  }
}

impl CheckSession {
  /// `stack_independence` of `branch` against the snapshot
  #[instrument(skip(self))]
  pub fn stack_independence(&self, branch: &str) -> Result<StackIndependence, RegroupError> {
    let forest = self.snapshot.forest.as_ref();
    let Some(parent) = forest.and_then(|forest| forest.declared_parent(branch)).map(str::to_string) else {
      return Err(RegroupError::InvalidPlan {
        message: format!("{branch} is not stacked"),
        commits: Vec::new(),
      });
    };
    let plan = RegroupPlan {
      unstack: vec![branch.to_string()],
      ..RegroupPlan::default()
    };
    let report = self.check_only(Some(&plan), Some(branch), None)?;
    let outcome = report.branches.into_iter().find(|check| check.name == branch).map(|check| check.outcome);
    match outcome {
      Some(CheckOutcome::Ok { .. }) => Ok(StackIndependence::Builds { parent }),
      Some(CheckOutcome::Conflict { commit, files }) => self.needed_commits(branch, parent, &commit, files),
      Some(CheckOutcome::Error { message }) => Err(RegroupError::Other(anyhow::anyhow!(message))),
      Some(CheckOutcome::Problems(problems)) => Err(RegroupError::Other(anyhow::anyhow!(problems.iter().map(describe_problem).collect::<Vec<_>>().join("; ")))),
      Some(CheckOutcome::Blocked { parent, .. }) => Err(RegroupError::Other(anyhow::anyhow!("{branch} is blocked by {parent}"))),
      None => Err(RegroupError::InvalidPlan {
        message: format!("No branch named {branch:?} among the commits ahead of the baseline"),
        commits: Vec::new(),
      }),
    }
  }

  /// The commits of the ancestors of `branch` that its commits need by their lines, and what those need in turn, from
  /// the changes of every commit ahead of the baseline (the object cache, one `log` for the misses). Generated files
  /// count: the conflict of a trial build is real in any file
  fn needed_commits(&self, branch: &str, parent: String, conflicting: &str, files: Vec<String>) -> Result<StackIndependence, RegroupError> {
    let commits = &self.snapshot.commits;
    let ids: Vec<String> = commits.iter().map(|commit| commit.id.clone()).collect();
    let diffs = read_commit_diffs_cached(&self.git_executor, &self.repository_path, &self.cache, &ids)?;
    let group_of = |position: usize| self.snapshot.groups.get(&commits[position].id).and_then(Option::as_deref);
    let forest = self.snapshot.forest.as_ref();
    let ancestors = forest.map_or_default(|forest| ancestors_of(forest, branch));
    let mut needs: Vec<Vec<usize>> = vec![Vec::new(); commits.len()];
    for dependency in compute_dependencies(&diffs) {
      needs[dependency.from].push(dependency.to);
    }

    // a move takes a fixup unit whole: the fixups follow their target, and a fixup can't leave it
    let position_of: HashMap<&str, usize> = commits.iter().enumerate().map(|(position, commit)| (commit.id.as_str(), position)).collect();
    let mut unit_of: Vec<Vec<usize>> = vec![Vec::new(); commits.len()];
    for group in plan_autosquash(self.snapshot.head.clone(), commits, &self.snapshot.grouping).groups {
      let unit: Vec<usize> = std::iter::once(group.target.as_str())
        .chain(group.folded.iter().map(|fixup| fixup.commit_id.as_str()))
        .filter_map(|id| position_of.get(id).copied())
        .collect();
      for &member in &unit {
        unit_of[member].clone_from(&unit);
      }
    }

    // from the commits of the branch through the commits of its ancestors, each with its fixup unit
    let in_ancestor = |position: usize| group_of(position).is_some_and(|group| ancestors.contains(&group));
    let mut needed: BTreeSet<usize> = BTreeSet::new();
    let mut pending: Vec<usize> = (0..commits.len()).filter(|&position| group_of(position) == Some(branch)).collect();
    while let Some(position) = pending.pop() {
      let unit = unit_of[position].iter().copied().filter(|&member| member != position);
      for to in needs[position].iter().copied().chain(unit) {
        if in_ancestor(to) && needed.insert(to) {
          pending.push(to);
        }
      }
    }
    // a commit left in a branch whose build holds a needed commit (an ancestor that owns one, and the branches built
    // on it) would need this branch after a move; the branches built on this one get the commits anyway, and the other
    // stacks never had them
    let built_on = |group: &str, base: &str| group == base || forest.is_some_and(|forest| ancestors_of(forest, group).contains(&base));
    let owners: BTreeSet<&str> = needed.iter().filter_map(|&position| group_of(position)).collect();
    let holds_needed = |group: &str| !built_on(group, branch) && owners.iter().any(|owner| built_on(group, owner));
    let movable = !needed.is_empty()
      && (0..commits.len())
        .filter(|position| !needed.contains(position))
        .filter(|&position| group_of(position).is_some_and(holds_needed))
        .all(|position| needs[position].iter().all(|to| !needed.contains(to)));

    let named = |position: usize| StackCommit {
      id: commits[position].id.clone(),
      subject: subject_without_prefix(&commits[position].subject),
      branch: group_of(position).unwrap_or_default().to_string(),
    };
    let child_commit = commits.iter().position(|commit| commit.id == conflicting).map_or_else(
      || StackCommit {
        id: conflicting.to_string(),
        subject: String::new(),
        branch: branch.to_string(),
      },
      named,
    );
    Ok(StackIndependence::Needs {
      parent,
      child_commit,
      files,
      parent_commits: needed.iter().map(|&position| named(position)).collect(),
      movable,
    })
  }
}

/// The subject without its `(prefix)`, an autosquash marker kept: `fixup! (a) X` gives `fixup! X`
fn subject_without_prefix(subject: &str) -> String {
  let parsed = parse_subject(subject);
  format!("{}{}", parsed.autosquash, parsed.rest)
}

/// The declared ancestors of `branch`, nearest first
fn ancestors_of<'a>(forest: &'a StackForest, branch: &str) -> Vec<&'a str> {
  let mut ancestors: Vec<&str> = Vec::new();
  let mut current = forest.declared_parent(branch);
  while let Some(parent) = current.filter(|parent| *parent != branch && !ancestors.contains(parent)) {
    ancestors.push(parent);
    current = forest.declared_parent(parent);
  }
  ancestors
}

/// The commit as the plan rewords it: grouping reads the subject, the copy writes the message
fn apply_message(commit: &mut Commit, message: &str) {
  commit.subject = message.lines().next().unwrap_or_default().to_string();
  commit.stripped_subject.clone_from(&commit.subject);
  message.clone_into(&mut commit.message);
}

/// Build one branch on `base_commit`, objects only. `on_parent`: `base_commit` is the parent's head, as in a sync; it only
/// drives the stack note and the remote status of `finish_branch`, which a check never runs
fn build(
  context: &BranchSyncContext<CollectingReporter>,
  progress: &CollectingReporter,
  name: &str,
  commits: &[Commit],
  branch_idx: usize,
  base_commit: &str,
  on_parent: bool,
) -> CheckOutcome {
  progress.clear();
  let job = BranchJob {
    branch_name: name.to_string(),
    commits: commits.to_vec(),
    my_email: None,
    branch_idx,
    base_commit: base_commit.to_string(),
    on_parent,
  };
  match build_branch(context, job) {
    Ok(BuildOutcome::Built(built)) => CheckOutcome::Ok { head: built.head },
    Ok(BuildOutcome::Failed(_)) => progress.commit_error(name).unwrap_or_else(|| CheckOutcome::Error {
      message: format!("Branch {name} failed to build"),
    }),
    Err(e) => CheckOutcome::Error { message: format!("{e:#}") },
  }
}

/// Keeps the commit errors of the branch being built; the other events of a build are not needed
#[derive(Clone, Default)]
struct CollectingReporter {
  errors: Arc<Mutex<Vec<(String, String, BranchError)>>>,
}

impl CollectingReporter {
  fn clear(&self) {
    if let Ok(mut errors) = self.errors.lock() {
      errors.clear();
    }
  }

  fn commit_error(&self, branch: &str) -> Option<CheckOutcome> {
    let errors = self.errors.lock().ok()?;
    let (_, commit, error) = errors.iter().find(|(name, _, _)| name == branch)?;
    Some(match error {
      BranchError::MergeConflict(info) => CheckOutcome::Conflict {
        commit: commit.clone(),
        files: info.conflicting_files.iter().map(|file| file.file.clone()).collect(),
      },
      BranchError::Generic(message) => CheckOutcome::Error { message: message.clone() },
    })
  }
}

impl ProgressReporter for CollectingReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    if let SyncEvent::CommitError { branch_name, commit_hash, error } = event {
      self
        .errors
        .lock()
        .map_err(|e| anyhow::anyhow!("the error collector is poisoned: {e}"))?
        .push((branch_name, commit_hash, error));
    }
    Ok(())
  }
}

/// Merges and commit trees in the object cache. Conflicts are reported light: the files without the analysis
struct CacheMemo {
  cache: ObjectCache,
}

fn merge_key(trees: &MergeTrees) -> MergeKey {
  MergeKey {
    base_tree: trees.base.clone(),
    commit_tree: trees.commit.clone(),
    target_tree: trees.target.clone(),
  }
}

impl MergeMemo for CacheMemo {
  fn merge(&self, trees: &MergeTrees) -> Option<MemoizedMerge> {
    self.cache.get_merge_results(&[merge_key(trees)]).pop().flatten().map(|result| match result {
      MergeResult::Clean { tree } => MemoizedMerge::Clean { tree },
      MergeResult::Conflict { files } => MemoizedMerge::Conflict { files },
    })
  }

  fn remember_merge(&self, trees: &MergeTrees, result: &MemoizedMerge) {
    let result = match result {
      MemoizedMerge::Clean { tree } => MergeResult::Clean { tree: tree.clone() },
      MemoizedMerge::Conflict { files } => MergeResult::Conflict { files: files.clone() },
    };
    self.cache.put_merge_results(&[(merge_key(trees), result)]);
  }

  fn commit_tree(&self, commit: &str) -> Option<String> {
    self.cache.get_commit_trees(&[commit.to_string()]).remove(commit)
  }

  fn remember_commit_tree(&self, commit: &str, tree: &str) {
    self.cache.put_commit_trees(&[(commit.to_string(), tree.to_string())]);
  }

  fn light_conflicts(&self) -> bool {
    true
  }
}

#[cfg(test)]
#[path = "check_test.rs"]
mod check_test;
