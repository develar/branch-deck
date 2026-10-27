//! The stack branch of a stack that forks (`{prefix}/stack/<root>`): every commit of its branches in the order of the
//! checked out branch's history, on the base of the sync, as one more virtual branch, which a Safe Push of the whole
//! stack lands in one robot (spec/branch/whole-stack.spec.md, docs/design/git-operations.md, "Stack branch").
//!
//! - **Which stacks**: a stack is a root and the branches built on it, by `effective_base` (or the parent that blocks
//!   them): every branch's virtual branch has its ancestors' commits, so the stack branch has every piece. One forks
//!   when a branch of it has two children or more; a straight chain has none, its leaf is the whole stack.
//! - **No mapping notes, no branch events**: the note of an original names the copy of its own branch. The build is a
//!   function of the base and the original commits (the copies are deterministic), so it is remembered by a hash of
//!   them (`StackBuilds`: the memory of the process, else the object cache): a stack with the same commits on the same
//!   base runs no git to know its stack branch is built.
//! - **One chain**: a build runs one `merge-tree --stdin` and one `fast-import` (`CommitChain`); a conflict is reported
//!   by its commit and files, without the analysis of a branch's conflict view.
//! - **The ref**: moved with compare-and-swap on the tip the sync listed, only when every member was built; else it
//!   keeps its tip, which no push may send (`StackBranchState`). A stack branch whose stack no longer forks is deleted
//!   with compare-and-swap in one transaction, unless it has a remote branch or is checked out.

use crate::branch_processor::{BranchMove, move_branch};
use crate::grouping::BranchGrouping;
use crate::remote_status::RemoteStatusMemo;
use anyhow::Result;
use branch_integration::common::ListedBranch;
use domain_types::branch_names::{BranchKind, strip_branch_namespace, to_stack_branch_name};
use domain_types::commit::Commit;
use domain_types::stack::{BlockReason, BranchStack, StackBranchConflict, StackBranchState, StackBranchUpdate, StackMemberBlock};
use domain_types::{BranchError, BranchSyncStatus, RemoteStatusUpdate};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_chain::CommitChain;
use git_ops::copy_commit::{CopyCommitError, CreateCommitParams, ProgressInfo, create_or_update_commit};
use git_ops::merge_memo::MergeMemo;
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::short_id;
use git_ops::tree_id_cache::TreeIdCache;
use git_ops::word_resolve::ResolveMode;
use indexmap::{IndexMap, IndexSet};
use object_cache::cache::ObjectCache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{debug, info, instrument, warn};

/// The version of a remembered build: bump it when the copies change (their message, ident or dates), so no build of
/// the old copies is read
const STACK_BUILD_VERSION: u32 = 2;

/// A stack that forks and what its stack branch is built from
#[derive(Debug, Clone)]
pub struct StackBranchPlan {
  /// The root branch of the stack
  pub root: String,
  /// `{prefix}/stack/<root>`
  pub full_name: String,
  /// The branches of the stack, the root first, each after its parent (the stack order)
  pub members: Vec<String>,
  /// The original commits of all members, in the order of the checked out branch's history, oldest first
  pub commits: Vec<Commit>,
  /// The most frequent author email of the commits, of a tie the author seen first: the user's commits of its remote
  /// status
  pub my_email: Option<String>,
  /// The first member, in stack order, that the placement already keeps from being built: a stack problem (`Error`)
  /// or a block by its parent (`Blocked`)
  pub blocked_by: Option<StackMemberBlock>,
}

/// The branch a stack member is built on: its effective base, else the ancestor that blocks it, else for a branch with
/// a stack problem the parent it declares; `None` for a root
fn stack_parent<'a>(name: &str, placement: &'a BranchStack, has_commits: impl Fn(&str) -> bool) -> Option<&'a str> {
  if let Some(base) = &placement.effective_base {
    return Some(base);
  }
  if let Some(block) = &placement.blocked_by {
    return Some(&block.parent);
  }
  placement
    .declared_parent
    .as_deref()
    .filter(|parent| !placement.problems.is_empty() && *parent != name && has_commits(parent))
}

/// The stacks that fork, by their roots in stack order: `order` is the stack order (`StackForest::order`, parents
/// first), `placements` where each branch goes, `commits` the commits of each branch (any order), `positions` the
/// index of each commit in the checked out branch's history. A branch whose parents lead into a cycle belongs to no
/// stack. No git
pub fn plan_stack_branches(
  branch_prefix: &str,
  order: &[String],
  placements: &IndexMap<String, BranchStack>,
  commits: &HashMap<&str, &[Commit]>,
  positions: &HashMap<&str, usize>,
) -> Vec<StackBranchPlan> {
  let parent_of = |name: &str| {
    placements
      .get(name)
      .and_then(|placement| stack_parent(name, placement, |parent| commits.contains_key(parent)))
  };
  // the root of each branch, `None` for one whose parents lead into a cycle
  let root_of = |name: &str| {
    let mut seen = HashSet::from([name]);
    let mut current = name;
    while let Some(parent) = parent_of(current) {
      if !seen.insert(parent) {
        return None;
      }
      current = parent;
    }
    Some(current.to_string())
  };
  let mut stacks: IndexMap<String, Vec<String>> = IndexMap::new();
  let mut child_counts: HashMap<&str, usize> = HashMap::new();
  for name in order.iter().filter(|name| commits.contains_key(name.as_str())) {
    let Some(root) = root_of(name) else { continue };
    if let Some(parent) = parent_of(name) {
      *child_counts.entry(parent).or_default() += 1;
    }
    stacks.entry(root).or_default().push(name.clone());
  }

  stacks
    .into_iter()
    .filter(|(_, members)| members.iter().any(|member| child_counts.get(member.as_str()).is_some_and(|count| *count > 1)))
    .filter_map(|(root, members)| {
      let full_name = to_stack_branch_name(branch_prefix, &root)
        .inspect_err(|e| warn!(root = %root, error = %e, "a stack root names no stack branch"))
        .ok()?;
      let mut union: Vec<&Commit> = members.iter().flat_map(|member| commits.get(member.as_str()).copied().unwrap_or_default()).collect();
      union.sort_by_key(|commit| positions.get(commit.id.as_str()).copied().unwrap_or(usize::MAX));
      let blocked_by = members.iter().find_map(|member| {
        let placement = placements.get(member)?;
        let reason = if !placement.problems.is_empty() {
          BlockReason::Problem
        } else if placement.blocked_by.is_some() {
          BlockReason::Blocked
        } else {
          return None;
        };
        Some(StackMemberBlock { member: member.clone(), reason })
      });
      Some(StackBranchPlan {
        my_email: most_frequent_email(&union),
        commits: union.into_iter().cloned().collect(),
        root,
        full_name,
        members,
        blocked_by,
      })
    })
    .collect()
}

/// The email of the most commits; of a tie the author seen first
fn most_frequent_email(commits: &[&Commit]) -> Option<String> {
  let mut counts: IndexMap<&str, usize> = IndexMap::new();
  for commit in commits.iter().filter(|commit| !commit.author_email.is_empty()) {
    *counts.entry(&commit.author_email).or_default() += 1;
  }
  let mut best: Option<(&str, usize)> = None;
  for (email, count) in counts {
    if best.is_none_or(|(_, best_count)| count > best_count) {
      best = Some((email, count));
    }
  }
  best.map(|(email, _)| email.to_string())
}

/// What the build of a stack branch gave, a function of its base and its original commits
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackBuild {
  Built {
    tip: String,
  },
  /// The original commit whose copy conflicts, and the conflicting paths
  Conflict {
    commit: String,
    files: Vec<String>,
  },
}

/// The key of a build: a hash of the base, the ids of the original commits in their order and the resolve mode. A
/// commit id covers its tree, its parent, its author, its committer and its message, everything its copy is made of; the
/// resolve by words decides whether a conflict of git stays one
pub fn build_key(base: &str, commits: &[Commit], resolve: ResolveMode) -> [u8; 32] {
  let mut hasher = Sha256::new();
  hasher.update(b"stack_branch\0");
  hasher.update(base.as_bytes());
  for commit in commits {
    hasher.update([0]);
    hasher.update(commit.id.as_bytes());
  }
  hasher.update(resolve.key().to_be_bytes());
  hasher.finalize().into()
}

/// Builds of this process, by key; cleared when full
static REMEMBERED: LazyLock<Mutex<HashMap<[u8; 32], StackBuild>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
const REMEMBERED_LIMIT: usize = 1024;

fn remembered() -> std::sync::MutexGuard<'static, HashMap<[u8; 32], StackBuild>> {
  REMEMBERED.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The builds of stack branches one operation knows: from the memory of the process, else from the object cache; the
/// new ones go to both, to the object cache in one write when the memo is dropped. The builds are those of one resolve
/// mode, which their keys hold
pub struct StackBuilds {
  cache: ObjectCache,
  resolve: ResolveMode,
  pending: Mutex<Vec<([u8; 32], StackBuild)>>,
}

impl StackBuilds {
  pub fn new(cache: ObjectCache, resolve: ResolveMode) -> Self {
    Self {
      cache,
      resolve,
      pending: Mutex::default(),
    }
  }

  /// The build of `key`, when one is remembered
  pub fn get(&self, key: &[u8; 32]) -> Option<StackBuild> {
    let known = remembered().get(key).cloned();
    if known.is_some() {
      return known;
    }
    let build = self.cache.get_stack_builds::<StackBuild>(STACK_BUILD_VERSION, &[*key]).pop().flatten()?;
    remembered().insert(*key, build.clone());
    Some(build)
  }

  fn remember(&self, key: [u8; 32], build: &StackBuild) {
    let mut memory = remembered();
    if memory.len() >= REMEMBERED_LIMIT {
      memory.clear();
    }
    memory.insert(key, build.clone());
    drop(memory);
    self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((key, build.clone()));
  }
}

impl Drop for StackBuilds {
  fn drop(&mut self) {
    let pending = std::mem::take(&mut *self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    self.cache.put_stack_builds(STACK_BUILD_VERSION, &pending);
  }
}

/// A merge memo that remembers nothing and reports a conflict by its files only (`MergeConflictInfo::light`): a stack
/// branch shows no conflict view, so its conflict costs no analysis
struct LightConflicts;

impl MergeMemo for LightConflicts {
  fn light_conflicts(&self) -> bool {
    true
  }
}

/// Copies `commits` onto `base` in one `CommitChain`, as a flat virtual branch is built (`create_or_update_commit`,
/// the deterministic committer dates), without its notes and events. `tree_id_cache` has the trees of the commits and
/// the date of the base
#[instrument(skip(git_executor, tree_id_cache, commits), fields(commits = commits.len()))]
pub fn build_stack_branch(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  full_name: &str,
  tree_id_cache: &TreeIdCache,
  base: &str,
  commits: &[Commit],
  resolve: ResolveMode,
) -> Result<StackBuild> {
  let tree_id_cache = tree_id_cache.with_other_memo(Arc::new(LightConflicts));
  let chain = RefCell::new(CommitChain::new(git_executor, repository_path));
  let mut parent = base.to_string();
  let mut committer_timestamp = tree_id_cache
    .get_committer_timestamp(git_executor, repository_path, base)
    .map_err(CopyCommitError::into_anyhow)?;
  let on_status = |_: BranchSyncStatus| {};
  for (index, commit) in commits.iter().enumerate() {
    committer_timestamp = committer_timestamp.max(commit.committer_timestamp);
    let progress_info = ProgressInfo {
      branch_name: full_name,
      current_commit_idx: index,
      total_commits_in_branch: commits.len(),
      current_branch_idx: 0,
      total_branches: 1,
    };
    let copied = create_or_update_commit(CreateCommitParams {
      commit,
      new_parent_oid: &parent,
      reuse_if_possible: false,
      repo_path: repository_path,
      on_status: &on_status,
      progress_info: &progress_info,
      git_executor,
      tree_id_cache: &tree_id_cache,
      chain: &chain,
      existing_virtual_commits: None,
      verify_notes: false,
      committer_timestamp,
      // a stack branch remembers its whole build (`StackBuilds`), not the copies before a conflict
      remembered: None,
      resolve,
    });
    match copied {
      // its mapping note names the copy of its own branch
      Ok(copied) => parent = copied.hash,
      Err(CopyCommitError::BranchError(BranchError::MergeConflict(info))) => {
        chain.into_inner().finish()?;
        return Ok(StackBuild::Conflict {
          commit: commit.id.clone(),
          files: info.conflicting_files.into_iter().map(|file| file.file).collect(),
        });
      }
      Err(CopyCommitError::BranchError(BranchError::Generic(message))) => {
        chain.into_inner().finish()?;
        anyhow::bail!(message);
      }
      Err(CopyCommitError::Other(e)) => return Err(e),
    }
  }
  chain.into_inner().finish()?;
  Ok(StackBuild::Built { tip: parent })
}

/// How a build of a stack branch ended in a sync: remembered, built now, or failed
#[derive(Debug)]
pub(crate) enum BuildResult {
  Remembered(StackBuild),
  Built(StackBuild),
  Failed(anyhow::Error),
}

/// The stack branches of one sync: what it plans, what it keeps and what it deletes, from the listing it read at its
/// start
#[derive(Debug, Default)]
pub struct StackBranchSync {
  pub plans: Vec<StackBranchPlan>,
  /// Full name -> tip of every stack branch the sync listed
  pub tips: HashMap<String, String>,
  /// The stack branches of no stack that forks that stay: `(root, full name, why)`
  pub kept: Vec<(String, String, &'static str)>,
  /// The stack branches of no stack that forks to delete: `(full name, tip)`
  pub obsolete: Vec<(String, String)>,
}

impl StackBranchSync {
  /// What a sync does with the stack branches: `plans` are the stacks that fork; every other stack branch among
  /// `listed` is deleted, unless `remote_tips` has its remote branch or a worktree has it checked out
  pub fn new(plans: Vec<StackBranchPlan>, listed: &[ListedBranch], branch_prefix: &str, remote_tips: &HashMap<String, String>) -> Self {
    let planned: HashSet<&str> = plans.iter().map(|plan| plan.full_name.as_str()).collect();
    let mut sync = Self {
      plans: Vec::new(),
      tips: HashMap::new(),
      kept: Vec::new(),
      obsolete: Vec::new(),
    };
    for branch in listed {
      let Some(root) = strip_branch_namespace(&branch.name, branch_prefix, BranchKind::Stack) else {
        continue;
      };
      sync.tips.insert(branch.name.clone(), branch.tip.clone());
      if planned.contains(branch.name.as_str()) {
        continue;
      }
      if remote_tips.contains_key(&branch.name) {
        sync.kept.push((root.to_string(), branch.name.clone(), "the ref stays for its remote branch"));
      } else if branch.checked_out {
        sync.kept.push((root.to_string(), branch.name.clone(), "the ref stays while a worktree has it checked out"));
      } else {
        sync.obsolete.push((branch.name.clone(), branch.tip.clone()));
      }
    }
    sync.plans = plans;
    sync
  }

  /// The roots of the stack branches the sync reports, the planned ones first
  pub fn reported_roots(&self) -> Vec<String> {
    let roots: IndexSet<String> = self
      .plans
      .iter()
      .map(|plan| plan.root.clone())
      .chain(self.kept.iter().map(|(root, ..)| root.clone()))
      .collect();
    roots.into_iter().collect()
  }

  /// The full names of the reported stack branches, whose remote tips the remote statuses need
  pub(crate) fn full_names(&self) -> impl Iterator<Item = &str> {
    self
      .plans
      .iter()
      .map(|plan| plan.full_name.as_str())
      .chain(self.kept.iter().map(|(_, full_name, _)| full_name.as_str()))
  }
}

/// The build of a plan without a known block, on the blocking pool: the remembered build of its commits, else a build
/// now; `None` for a plan the placement blocks. A remembered tip counts only while the ref is at it (`listed_tip`):
/// elsewhere its commits may be gone (pruned since a delete of the ref, or written in another repository with the same
/// commits), and they are built again
pub(crate) fn build_planned(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  tree_id_cache: &TreeIdCache,
  builds: &StackBuilds,
  base: &str,
  plan: &StackBranchPlan,
  listed_tip: Option<&str>,
) -> Option<BuildResult> {
  if plan.blocked_by.is_some() {
    return None;
  }
  let key = build_key(base, &plan.commits, builds.resolve);
  match builds.get(&key) {
    Some(StackBuild::Built { tip }) if listed_tip != Some(tip.as_str()) => {
      debug!(stack = %plan.full_name, "the remembered build is not where the ref is, building it again");
    }
    Some(build) => {
      debug!(stack = %plan.full_name, "the build of the same commits is remembered");
      return Some(BuildResult::Remembered(build));
    }
    None => {}
  }
  Some(
    match build_stack_branch(git_executor, repository_path, &plan.full_name, tree_id_cache, base, &plan.commits, builds.resolve) {
      Ok(build) => {
        builds.remember(key, &build);
        BuildResult::Built(build)
      }
      Err(e) => {
        warn!(stack = %plan.full_name, error = %format!("{e:#}"), "cannot build the stack branch");
        BuildResult::Failed(e)
      }
    },
  )
}

/// The member that keeps the stack branch from being built: the first, in stack order, that failed or is blocked in the
/// sync (`failed`), a failure before a block
fn failed_member(plan: &StackBranchPlan, failed: &HashMap<String, BlockReason>) -> Option<StackMemberBlock> {
  let block = |member: &String| {
    failed.get(member).map(|reason| StackMemberBlock {
      member: member.clone(),
      reason: *reason,
    })
  };
  plan
    .members
    .iter()
    .find_map(|member| block(member).filter(|block| block.reason != BlockReason::Blocked))
    .or_else(|| plan.members.iter().find_map(block))
}

/// What the finish of the stack branches works with
pub(crate) struct StackFinish<'a> {
  pub git_executor: &'a GitCommandExecutor,
  pub repository_path: &'a str,
  /// The remote statuses of the sync; `None` without one
  pub remote_statuses: Option<&'a RemoteStatusMemo>,
  /// Branch -> why it was not built in this sync
  pub failed: &'a HashMap<String, BlockReason>,
}

/// What the finish of the stack branches reported
pub(crate) struct FinishedStacks {
  /// The update of each reported stack branch, in the order of `StackBranchSync::reported_roots`
  pub updates: Vec<StackBranchUpdate>,
  /// Another process moved (or deleted) a stack branch while the sync built it: its tip is not this sync's build, a
  /// sync again rebuilds it (`SyncReport::retry`)
  pub moved_elsewhere: bool,
}

/// Moves each planned stack branch whose members all built to its build, with compare-and-swap on the listed tip,
/// deletes the obsolete ones in one transaction, and returns the update of each reported stack branch. `results` are
/// the builds of `build_planned`, in the order of the plans
pub(crate) fn finish_stack_branches(finish: &StackFinish<'_>, sync: &StackBranchSync, results: Vec<Option<BuildResult>>) -> FinishedStacks {
  let StackBranchSync { plans, tips, kept, obsolete } = sync;
  let mut updates = Vec::with_capacity(plans.len() + kept.len());
  let mut moved_elsewhere = false;
  for (plan, result) in plans.iter().zip(results) {
    let old_tip = tips.get(&plan.full_name).cloned();
    let mut update = stale_update(plan, old_tip.clone());
    // the stack branch's own conflict comes before a member's failure: `bd status` knows it without a sync, and the
    // member's row says its own
    let blocked = |update: &mut StackBranchUpdate, block: StackMemberBlock| {
      update.state = StackBranchState::Blocked;
      update.blocked_by = Some(block);
    };
    match (plan.blocked_by.clone(), result) {
      (Some(block), _) => blocked(&mut update, block),
      (None, Some(BuildResult::Remembered(StackBuild::Conflict { commit, files }) | BuildResult::Built(StackBuild::Conflict { commit, files }))) => {
        update.state = StackBranchState::Conflict;
        update.conflict = Some(StackBranchConflict { commit, files });
      }
      (None, result) => match (failed_member(plan, finish.failed), result) {
        (Some(block), _) => blocked(&mut update, block),
        (None, Some(BuildResult::Remembered(StackBuild::Built { tip }) | BuildResult::Built(StackBuild::Built { tip }))) => {
          let (state, tip, error) = move_stack_branch(finish, &plan.full_name, tip, old_tip.as_deref());
          moved_elsewhere |= matches!(error, Some(StackMoveError::MovedElsewhere(_)));
          let error = error.map(StackMoveError::into_message);
          update.state = state;
          update.tip = tip;
          update.error = error;
        }
        (None, Some(BuildResult::Failed(e))) => update.error = Some(format!("Not rebuilt: {e:#}")),
        (None, _) => update.error = Some("Not rebuilt".to_string()),
      },
    }
    update.remote = remote_status(finish, &plan.full_name, update.tip.as_deref(), plan.my_email.as_deref());
    updates.push(update);
  }

  for (root, full_name, why) in kept {
    let mut update = kept_update(root, full_name, why, tips);
    update.remote = remote_status(finish, full_name, update.tip.as_deref(), None);
    updates.push(update);
  }
  delete_obsolete(finish.git_executor, finish.repository_path, obsolete);
  FinishedStacks { updates, moved_elsewhere }
}

/// Why a stack branch was not moved to its build
enum StackMoveError {
  /// Another process moved or deleted it since the sync listed it
  MovedElsewhere(String),
  /// The move failed otherwise
  Failed(String),
}

impl StackMoveError {
  fn into_message(self) -> String {
    match self {
      Self::MovedElsewhere(message) | Self::Failed(message) => message,
    }
  }
}

/// The update of a planned stack branch at `tip`, stale until its state is known
fn stale_update(plan: &StackBranchPlan, tip: Option<String>) -> StackBranchUpdate {
  StackBranchUpdate {
    root: plan.root.clone(),
    branch_name: plan.full_name.clone(),
    members: plan.members.clone(),
    commit_count: u32::try_from(plan.commits.len()).unwrap_or(u32::MAX),
    my_email: plan.my_email.clone(),
    state: StackBranchState::Stale,
    tip,
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  }
}

/// A stack branch whose stack no longer forks, kept for its remote branch or a worktree: stale, without members
fn kept_update(root: &str, full_name: &str, why: &str, tips: &HashMap<String, String>) -> StackBranchUpdate {
  StackBranchUpdate {
    root: root.to_string(),
    branch_name: full_name.to_string(),
    members: Vec::new(),
    commit_count: 0,
    my_email: None,
    state: StackBranchState::Stale,
    tip: tips.get(full_name).cloned(),
    blocked_by: None,
    conflict: None,
    error: Some(format!("Not rebuilt: {root} heads no stack that forks; {why}")),
    remote: None,
  }
}

/// Moves the stack branch to `tip` unless it is there: `(state, tip after, why it is stale)`
fn move_stack_branch(finish: &StackFinish<'_>, full_name: &str, tip: String, old_tip: Option<&str>) -> (StackBranchState, Option<String>, Option<StackMoveError>) {
  if old_tip == Some(tip.as_str()) {
    return (StackBranchState::Built, Some(tip), None);
  }
  match move_branch(finish.git_executor, finish.repository_path, full_name, &tip, old_tip) {
    Ok(BranchMove::Written) => {
      info!(stack = %full_name, tip = %short_id(&tip), "moved the stack branch");
      (StackBranchState::Built, Some(tip), None)
    }
    Ok(BranchMove::MovedElsewhere(moved)) => {
      warn!(stack = %full_name, expected = ?old_tip, actual = ?moved, "another process moved the stack branch during the sync");
      let error = match &moved {
        Some(moved) => format!("Not updated: another process moved {full_name} to {} while this sync built it; sync again", short_id(moved)),
        None => format!("Not updated: another process deleted {full_name} while this sync built it; sync again"),
      };
      (StackBranchState::Stale, moved, Some(StackMoveError::MovedElsewhere(error)))
    }
    Err(e) => {
      warn!(stack = %full_name, error = %format!("{e:#}"), "cannot move the stack branch");
      (
        StackBranchState::Stale,
        old_tip.map(str::to_string),
        Some(StackMoveError::Failed(format!("Not updated: {e:#}"))),
      )
    }
  }
}

/// The remote status of the stack branch at `tip`, by content, as for a virtual branch; `None` without a ref, without
/// the remote statuses of a sync, or when it can't be computed
fn remote_status(finish: &StackFinish<'_>, full_name: &str, tip: Option<&str>, my_email: Option<&str>) -> Option<RemoteStatusUpdate> {
  let (tip, statuses) = (tip?, finish.remote_statuses?);
  statuses
    .status(full_name, full_name, tip, my_email, None)
    .inspect_err(|e| warn!(stack = %full_name, error = %format!("{e:#}"), "cannot compute the remote status of the stack branch"))
    .ok()
}

/// Deletes the stack branches of stacks that no longer fork in one transaction, with compare-and-swap on their listed
/// tips; best effort: on a failure they stay for the next sync
fn delete_obsolete(git_executor: &GitCommandExecutor, repository_path: &str, obsolete: &[(String, String)]) {
  if obsolete.is_empty() {
    return;
  }
  let mut transaction = RefTransaction::new();
  for (full_name, tip) in obsolete {
    transaction.delete(&format!("refs/heads/{full_name}"), Some(tip));
  }
  match transaction.commit(git_executor, repository_path, "delete stack branch") {
    Ok(()) => info!(count = obsolete.len(), "deleted the stack branches of stacks that no longer fork"),
    Err(e) => warn!(error = %format!("{e:#}"), "cannot delete the stack branches of stacks that no longer fork"),
  }
}

/// The stack branches a sync of `grouping` would report, as they stand without one (`bd status`), without their remote
/// statuses: a stack blocked by its placement is `Blocked`; else what the build of its commits gave when it is
/// remembered (the memory of the process, `cache`) and the ref is at it; else `Stale`, the ref is not known to be the
/// build of these commits (or there is none). No git
pub fn read_stack_branches(branch_prefix: &str, grouping: &BranchGrouping, cache: &ObjectCache, resolve: ResolveMode) -> Vec<StackBranchUpdate> {
  let (Some(forest), Some(placements), Some(base)) = (&grouping.forest, &grouping.placements, &grouping.base) else {
    return Vec::new();
  };
  let commits: HashMap<&str, &[Commit]> = grouping.branches.iter().map(|branch| (branch.name.as_str(), branch.commits.as_slice())).collect();
  let positions: HashMap<&str, usize> = grouping.commit_ids.iter().enumerate().map(|(index, id)| (id.as_str(), index)).collect();
  let plans = plan_stack_branches(branch_prefix, &forest.order, placements, &commits, &positions);
  let sync = StackBranchSync::new(plans, &grouping.listed, branch_prefix, &grouping.remote_tips);
  let builds = StackBuilds::new(cache.clone(), resolve);
  let mut updates: Vec<StackBranchUpdate> = sync
    .plans
    .iter()
    .map(|plan| {
      let tip = sync.tips.get(&plan.full_name).cloned();
      let mut update = stale_update(plan, tip);
      if let Some(block) = &plan.blocked_by {
        update.state = StackBranchState::Blocked;
        update.blocked_by = Some(block.clone());
        return update;
      }
      match builds.get(&build_key(base, &plan.commits, resolve)) {
        Some(StackBuild::Built { tip }) if update.tip.as_ref() == Some(&tip) => update.state = StackBranchState::Built,
        Some(StackBuild::Conflict { commit, files }) => {
          update.state = StackBranchState::Conflict;
          update.conflict = Some(StackBranchConflict { commit, files });
        }
        _ => update.error = Some("Not the build of these commits: `bd sync` builds it or says why not".to_string()),
      }
      update
    })
    .collect();
  updates.extend(sync.kept.iter().map(|(root, full_name, why)| kept_update(root, full_name, why, &sync.tips)));
  updates
}

/// Forgets the builds of this process, as a new process starts without them
#[cfg(test)]
pub(crate) fn forget_builds() {
  remembered().clear();
}

#[cfg(test)]
#[path = "stack_branch_test.rs"]
mod stack_branch_test;
