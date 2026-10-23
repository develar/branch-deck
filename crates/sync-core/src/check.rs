//! A trial sync of a regroup plan: the plan is applied in memory, the branches are placed as a stacked sync places them
//! and built with `build_branch`, which writes commit objects only, in parallel as a sync builds them: a branch as soon
//! as the one it is built on has its head (`run_builds`). No ref, note, remote status,
//! HEAD or working tree changes, and no repository lock. Merges and commit trees go through the object cache, so trying
//! variants of a plan repeats no `merge-tree`.
//!
//! `stack_independence` asks one question of the check: does a stacked branch build on the baseline without its
//! parent? It builds that branch alone, as Unstack would make it, and names the commits of its ancestors it needs.

use crate::branch_processor::{BranchJob, BranchSyncContext, BuildOutcome, ExistingCommits, build_branch};
use crate::cache_memo::CacheMemo;
use crate::commit_owners::CommitOwners;
use crate::config_snapshot::ConfigSnapshot;
use crate::config_snapshot::SymbolMode;
use crate::dangling_references::{DanglingInput, DanglingReference, SyntaxMemo, find_dangling_references, read_commit_lines};
use crate::dependencies::compute_dependencies;
use crate::diff_scan::{CommitDiff, read_commit_diffs_cached};
use crate::grouping::branch_tips;
use crate::symbol_dependencies::declaration_checks::{AddedKeys, DeclarationFacts};
use crate::symbol_dependencies::{SymbolGraph, compute_symbol_dependencies};
use crate::symbol_scan::{SymbolInput, read_symbol_input};
use crate::sync::{SyncReport, seeded_tree_cache};
use code_symbols::language::language_of as symbols_language_of;
use domain_types::branch_names::{BranchKind, strip_branch_namespace};
use domain_types::commit::Commit;
use domain_types::landing::StackCommit;
use domain_types::stack::{BlockReason, ParentState, StackProblemView};
use domain_types::{BranchError, ProgressReporter, SyncEvent};
use file_diffs::service::FileDiffs;
use git_executor::concurrency::ProcessPermits;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::get_commit_list;
use git_ops::revs::resolve_commit;
use git_ops::tree_id_cache::TreeIdCache;
use git_ops::word_resolve::ResolveMode;
use history_rewrite::autosquash::plan_autosquash;
use history_rewrite::commit_grouper::{CommitGrouper, GroupedCommits, parse_subject};
use history_rewrite::commit_ids::{IdMatch, match_commit_id};
use history_rewrite::regroup::{RegroupError, RegroupOutcome, RegroupPlan, plan_in_memory};
use history_rewrite::snapshot::{Grouped, Snapshot, group_commits};
use history_rewrite::stack::{PendingDetection, empty_parent_states, place_branches, read_only_detection};
use history_rewrite::stack::{StackForest, describe_problem};
use object_cache::cache::ObjectCache;
use object_cache::commit_lines::FileLines;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use sync_utils::span::in_current_span;
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

impl CheckReport {
  /// Whether `name` builds: it applies cleanly and has no dangling reference a commit proves
  pub fn builds_cleanly(&self, name: &str) -> bool {
    self
      .branches
      .iter()
      .find(|check| check.name == name)
      .is_some_and(|check| matches!(check.outcome, CheckOutcome::Ok { .. }))
      && self
        .dangling
        .get(name)
        .is_none_or(|references| references.iter().all(|reference| !reference.is_confirmed()))
  }
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
  Problems(Vec<StackProblemView>),
}

/// Whether a stacked branch builds on the baseline without its parent (`stack_independence`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StackIndependence {
  /// Built as a root, the branch applies cleanly; with `branchdeck.symbols on` the uses of names its ancestors add are
  /// named, else a compile dependency on the parent is not seen
  Builds {
    /// The declared parent the branch is stacked on
    parent: String,
    /// The names commits of the branch use that commits of its declared ancestors add (`symbol_dependencies`): applies
    /// alone, but would not compile; empty when none, or the symbols are not on
    uses: Vec<StackUse>,
    /// The symbols were read (`branchdeck.symbols on`): an empty `uses` says that no name of an ancestor is used
    symbols: bool,
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

/// A use of a name that a commit of an ancestor adds, by a commit of the branch
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackUse {
  /// The commit of the branch
  pub commit: StackCommit,
  pub name: String,
  /// The commit of the ancestor that adds it
  pub declared_by: StackCommit,
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

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackIndependenceParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// A stacked branch
  pub branch_name: String,
}

/// Whether a stacked branch builds on the baseline without its parent, and else which commits of its ancestors it
/// needs: a trial build of the branch as a root (`stack_independence`), objects only, without the repository lock
#[instrument(skip(git_executor, cache, params), fields(branch = %params.branch_name))]
pub fn get_stack_independence(git_executor: &GitCommandExecutor, cache: &ObjectCache, params: &StackIndependenceParams) -> anyhow::Result<StackIndependence> {
  Ok(stack_independence(
    git_executor,
    &params.repository_path,
    &params.branch_prefix,
    &params.branch_name,
    cache,
  )?)
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
/// the branch tips; and, by the first check that needs them, the states of the parents without commits (once for each
/// set of them), the commit the branches start on and the changes of the commits. A `check` of a plan then runs git to
/// build the branches (the merges the object cache misses, the commit objects, the virtual commits a copy may reuse) and
/// for their dangling references
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
  /// `branchdeck.resolveByWords`: the trial builds resolve the conflicts of git by words as the sync does
  resolve: ResolveMode,
  /// The syntax trees of the blobs the dangling-reference checks read, shared with the sessions of `with_rewrite`
  syntax: Arc<SyntaxMemo>,
  /// The summaries of the files the commits ahead of the baseline change, with the module structure above them, read by
  /// the first that asks, with `branchdeck.symbols on`; shared with the sessions of `with_rewrite`, whose rewritten
  /// commits keep HEAD's tree (a blob only they have lacks its summary there, and its file is not searched)
  symbol_input: Arc<OnceLock<Result<Option<SymbolInput>, String>>>,
  /// The keys each commit declares anew, for the declaration searches of its checks, shared with the sessions of
  /// `with_rewrite`
  added_keys: Arc<AddedKeys>,
  /// The dependencies by symbols of the commits ahead of the baseline, computed by the first that asks, with
  /// `branchdeck.symbols on`
  symbol_graph: OnceLock<Result<Option<SymbolGraph>, String>>,
  /// The changes of the commits ahead of the baseline, read by the first that asks
  commit_diffs: OnceLock<Result<Vec<CommitDiff>, String>>,
  /// The blocks and the blobs of the scans (`FileDiffs::for_scans`): the changes and the changed lines of the commits
  /// read the blobs once; shared with the sessions of `with_rewrite`
  file_diffs: FileDiffs,
  /// Their line dependencies and fixup units, computed by the first that asks
  line_graph: OnceLock<Result<LineGraph, String>>,
  /// The states of the declared parents without commits, by the set of them, shared with the sessions of
  /// `with_rewrite`: a plan changes which parents have commits, never the branch tips or the baseline the detection of
  /// a pending parent reads, so the checks of a session detect each set once
  parent_states: Arc<Mutex<HashMap<BTreeSet<String>, ParentStates>>>,
  /// The tree and committer date of the commit the branches without a parent are built on, learned by the first check
  /// that builds on it: every check starts a tree cache of its own, which would read them again
  base_facts: Arc<Mutex<Option<BaseFacts>>>,
  /// The commits of the existing virtual branches down to a base, read by the first check that builds on it, shared
  /// with the sessions of `with_rewrite`: a plan never changes the branch tips, so one `log` serves every check
  existing_commits: Arc<Mutex<HashMap<String, Arc<ExistingCommits>>>>,
  /// The commit of the baseline, resolved by the first that asks: the detection of each set of parents without commits
  /// and the dangling-reference checks compare with it; `None` when it does not resolve (the local baseline branch)
  baseline_commit: Arc<OnceLock<Option<String>>>,
}

/// Parent -> its state, of the declared parents without commits (`empty_parent_states`)
type ParentStates = HashMap<String, ParentState>;

/// What a build reads of the commit it starts on (`TreeIdCache::get_committer_timestamp`)
struct BaseFacts {
  commit: String,
  tree: String,
  committer_timestamp: u32,
}

/// The line dependencies of the commits ahead of the baseline with their fixup units, by position in
/// `snapshot().commits`: the base of the graphs that `stack_independence` and the conflict fix decide by
pub(crate) struct LineGraph {
  /// Position -> the older commits it depends on by its lines (`dependencies.rs`)
  pub(crate) needs: Vec<Vec<usize>>,
  /// Position -> the other members of its fixup unit, empty outside one: a move takes a unit whole, the fixups follow
  /// their target and a fixup can't leave it
  pub(crate) unit: Vec<Vec<usize>>,
}

impl LineGraph {
  fn new(snapshot: &Snapshot, diffs: &[CommitDiff]) -> Self {
    let commits = &snapshot.commits;
    let mut needs: Vec<Vec<usize>> = vec![Vec::new(); commits.len()];
    for dependency in compute_dependencies(diffs) {
      needs[dependency.from].push(dependency.to);
    }
    let position_of: HashMap<&str, usize> = commits.iter().enumerate().map(|(position, commit)| (commit.id.as_str(), position)).collect();
    let mut unit: Vec<Vec<usize>> = vec![Vec::new(); commits.len()];
    for group in plan_autosquash(snapshot.head.clone(), commits, &snapshot.grouping).groups {
      let members: Vec<usize> = std::iter::once(group.target.as_str())
        .chain(group.folded.iter().map(|fixup| fixup.commit_id.as_str()))
        .filter_map(|id| position_of.get(id).copied())
        .collect();
      for &member in &members {
        unit[member] = members.iter().copied().filter(|other| *other != member).collect();
      }
    }
    Self { needs, unit }
  }
}

/// What every dangling-reference check of a session reads once: the changed lines of the commits ahead of the baseline,
/// and the baseline tip
type DanglingFacts = (HashMap<String, Vec<FileLines>>, Option<String>);

impl CheckSession {
  /// Read the snapshot after the guards of a regroup (no operation in progress, HEAD on a branch) and the branch tips
  #[instrument(skip(git_executor, cache))]
  pub fn open(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str, cache: &ObjectCache) -> Result<Self, RegroupError> {
    let snapshot = Snapshot::read(git_executor, repository_path)?;
    let config = ConfigSnapshot::read(git_executor, repository_path).map_err(RegroupError::from)?;
    let symbols = config.symbol_mode().map_err(RegroupError::from)?;
    let resolve = config.resolve_mode().map_err(RegroupError::from)?;
    Self::open_with(git_executor, repository_path, branch_prefix, cache, snapshot, (symbols, resolve))
  }

  /// The session after the sync of `report`, on what it read (`snapshot_of_sync`, its config): the commits with the
  /// mapping notes it wrote and the branch tips it left are read again, nothing else
  #[instrument(skip(git_executor, cache, report))]
  pub fn of_sync(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str, cache: &ObjectCache, report: &SyncReport) -> Result<Self, RegroupError> {
    let symbols = report.config.symbol_mode().map_err(RegroupError::from)?;
    let resolve = report.config.resolve_mode().map_err(RegroupError::from)?;
    let snapshot = snapshot_of_sync(git_executor, repository_path, report)?;
    Self::open_with(git_executor, repository_path, branch_prefix, cache, snapshot, (symbols, resolve))
  }

  fn open_with(
    git_executor: &GitCommandExecutor,
    repository_path: &str,
    branch_prefix: &str,
    cache: &ObjectCache,
    snapshot: Snapshot,
    (symbols, resolve): (SymbolMode, ResolveMode),
  ) -> Result<Self, RegroupError> {
    let tips = if snapshot.commits.is_empty() {
      HashMap::new()
    } else {
      branch_tips(git_executor, repository_path, branch_prefix)?
    };
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
      resolve,
      syntax: Arc::default(),
      symbol_input: Arc::default(),
      added_keys: Arc::default(),
      symbol_graph: OnceLock::new(),
      commit_diffs: OnceLock::new(),
      file_diffs: FileDiffs::for_scans(),
      line_graph: OnceLock::new(),
      parent_states: Arc::default(),
      base_facts: Arc::default(),
      existing_commits: Arc::default(),
      baseline_commit: Arc::default(),
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

  pub(crate) const fn git_executor(&self) -> &GitCommandExecutor {
    &self.git_executor
  }

  pub(crate) fn repository_path(&self) -> &str {
    &self.repository_path
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
      resolve: self.resolve,
      syntax: Arc::clone(&self.syntax),
      symbol_input: Arc::clone(&self.symbol_input),
      added_keys: Arc::clone(&self.added_keys),
      symbol_graph: OnceLock::new(),
      commit_diffs: OnceLock::new(),
      file_diffs: self.file_diffs.clone(),
      line_graph: OnceLock::new(),
      parent_states: Arc::clone(&self.parent_states),
      existing_commits: Arc::clone(&self.existing_commits),
      baseline_commit: Arc::clone(&self.baseline_commit),
      base_facts: Arc::clone(&self.base_facts),
    }
  }

  /// `branchdeck.symbols`
  pub const fn symbol_mode(&self) -> SymbolMode {
    self.symbols
  }

  /// The session with the summaries a caller read already for the changes of the same commits (`bd suggest`), so that
  /// its checks and `symbol_graph` don't read them again; ignored unless `branchdeck.symbols` is `on`
  #[must_use]
  pub fn with_symbol_input(self, input: SymbolInput) -> Self {
    if self.symbols == SymbolMode::On {
      let _ = self.symbol_input.set(Ok(Some(input)));
    }
    self
  }

  /// The summaries of the files the commits ahead of the baseline change, `None` unless `branchdeck.symbols` is `on`,
  /// read once per session
  pub fn symbol_input(&self) -> Result<Option<&SymbolInput>, RegroupError> {
    if self.symbols != SymbolMode::On {
      return Ok(None);
    }
    self
      .symbol_input
      .get_or_init(|| {
        let diffs = self.commit_diffs().map_err(|e| format!("{e:#}"))?;
        let input = read_symbol_input(&self.git_executor, &self.repository_path, &self.cache, diffs).map_err(|e| format!("{e:#}"))?;
        Ok(Some(input))
      })
      .as_ref()
      .map(Option::as_ref)
      .map_err(|e| RegroupError::Other(anyhow::anyhow!("{e}")))
  }

  /// The dependencies by symbols of the commits ahead of the baseline (positions of `snapshot().commits`), `None` unless
  /// `branchdeck.symbols` is `on`: what decides by them reads them here, once per session
  pub fn symbol_graph(&self) -> Result<Option<&SymbolGraph>, RegroupError> {
    let Some(input) = self.symbol_input()? else { return Ok(None) };
    let diffs = self.commit_diffs()?;
    Ok(
      self
        .symbol_graph
        .get_or_init(|| Ok(Some(compute_symbol_dependencies(diffs, input))))
        .as_ref()
        .ok()
        .and_then(Option::as_ref),
    )
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

  /// The check of the current grouping of `branch` and the branches it is built on, the others not built, with the
  /// dangling references of `branches` among them: what a Safe Push of `branch` lands
  #[instrument(skip(self))]
  pub fn check_landing(&self, branch: &str, branches: &[String]) -> Result<CheckReport, RegroupError> {
    self.check_only(None, Some(branch), Some(branches))
  }

  /// The check of `plan`; with `only`, of that branch and the branches it is built on, the others are not built and
  /// not in the report, and without `dangling_of` no dangling reference is looked for
  pub(crate) fn check_only(&self, plan: Option<&RegroupPlan>, only: Option<&str>, dangling_of: Option<&[String]>) -> Result<CheckReport, RegroupError> {
    let report = self.check_until(plan, only, dangling_of, &|| false)?;
    Ok(report.expect("a check that is never stopped has a report"))
  }

  /// `check_only` that `stop` ends before the next branch build and before the dangling references: `None` then, the
  /// builds so far are thrown away (their merges stay in the memo). An empty `dangling_of` skips the dangling references
  pub(crate) fn check_until(
    &self,
    plan: Option<&RegroupPlan>,
    only: Option<&str>,
    dangling_of: Option<&[String]>,
    stop: &(dyn Fn() -> bool + Sync),
  ) -> Result<Option<CheckReport>, RegroupError> {
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
      Some(forest) => Some(place_branches(forest, &self.empty_parent_states(forest)?)),
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
    let memo = Arc::new(CacheMemo::new(self.cache.clone(), git_executor.clone(), repository_path.to_string()));
    let existing_tips: HashMap<String, String> = self
      .tips
      .iter()
      .filter(|(name, _)| strip_branch_namespace(name, branch_prefix, BranchKind::Virtual).is_some())
      .map(|(name, tip)| (name.clone(), tip.clone()))
      .collect();
    let context = BranchSyncContext {
      repository_path: repository_path.to_string(),
      branch_prefix: branch_prefix.to_string(),
      baseline_branch: baseline_branch.clone(),
      total_branches: branches.len(),
      progress: progress.clone(),
      git_executor: git_executor.clone(),
      tree_id_cache: self.seeded_base(
        seeded_tree_cache(TreeIdCache::with_merge_memo(Arc::<CacheMemo>::clone(&memo)), branches.values().flatten()),
        base.as_deref(),
      ),
      remote_statuses: None,
      conflict_chains: None,
      conflict_tips: HashMap::new(),
      // the commits of every copy a build may reuse, by one `log` for all branches and all checks of the session
      existing_commits: base.as_deref().map(|base| self.existing_commits_on(base, &existing_tips)),
      existing_tips,
      // no remote status
      ancestor_keys: HashMap::new(),
      verify_mapping_notes: self.verify_mapping_notes,
      // conflicts are light, without missing commits: `missing_commits` finds them in the report
      owners: CommitOwners::default(),
      resolve: self.resolve,
    };

    // the branches to check in stack order, a parent before its children
    let mut planned: Vec<Planned> = Vec::with_capacity(order.len());
    let mut checks: Vec<BranchCheck> = Vec::with_capacity(order.len());
    let mut jobs: Vec<(usize, &[Commit])> = Vec::with_capacity(order.len());
    for (branch_idx, name) in order.iter().enumerate() {
      let Some(commits) = branches.get(name) else { continue };
      if built.as_ref().is_some_and(|built| !built.contains(name)) {
        continue;
      }
      let placement = placements.as_ref().and_then(|placements| placements.get(name));
      let built_on = placement.and_then(|stack| stack.effective_base.clone());
      let immediate = if let Some(stack) = placement.filter(|stack| !stack.problems.is_empty()) {
        Some(CheckOutcome::Problems(stack.problems.clone()))
      } else {
        placement.and_then(|stack| stack.blocked_by.as_ref()).map(|block| CheckOutcome::Blocked {
          parent: block.parent.clone(),
          reason: block.reason,
        })
      };
      planned.push(Planned {
        name: name.clone(),
        // a parent that is not checked here (no commits) leaves the branch on the base
        parent: built_on.as_ref().and_then(|parent| planned.iter().position(|planned| &planned.name == parent)),
        immediate,
      });
      jobs.push((branch_idx, commits.as_slice()));
      checks.push(BranchCheck {
        name: name.clone(),
        commits: commits.iter().map(|commit| commit.id.clone()).collect(),
        declared_parent: placement.and_then(|stack| stack.declared_parent.clone()),
        built_on,
        outcome: CheckOutcome::Error { message: String::new() },
      });
    }
    // every branch built on the base copies its first commit with the base's date: one read before the parallel
    // builds, not one per build (none when an earlier check of the session learned it); a failed read is the error of
    // each build that reads it again
    if let Some(base) = &base
      && planned.iter().any(|branch| branch.parent.is_none() && branch.immediate.is_none())
    {
      let _ = context.tree_id_cache.get_committer_timestamp(git_executor, repository_path, base);
    }
    let built_on_base = AtomicBool::new(false);
    let stopped = AtomicBool::new(false);
    let outcomes = run_builds(&planned, base.as_deref(), ProcessPermits::for_operation().capacity(), |index, base_commit, on_parent| {
      if stopped.load(Ordering::Relaxed) || stop() {
        stopped.store(true, Ordering::Relaxed);
        return CheckOutcome::Error { message: "Stopped".to_string() };
      }
      if !on_parent {
        built_on_base.store(true, Ordering::Relaxed);
      }
      let (branch_idx, commits) = jobs[index];
      build(&context, &progress, &planned[index].name, commits, branch_idx, base_commit, on_parent)
    });
    for (check, outcome) in checks.iter_mut().zip(outcomes) {
      check.outcome = outcome;
    }
    let built_on_base = built_on_base.into_inner();
    // before the dangling references, so that a parallel `bd` reuses the merges at once
    memo.flush();
    if let Some(base) = base.as_deref().filter(|_| built_on_base) {
      self.remember_base(&context.tree_id_cache, base);
    }

    if stopped.into_inner() || stop() {
      return Ok(None);
    }
    let dangling = if dangling_of.map_or_else(|| only.is_some(), <[String]>::is_empty) {
      BTreeMap::new()
    } else {
      self.dangling_references(&regroup.previous_head, base.as_deref(), &checks, dangling_of)?
    };
    Ok(Some(CheckReport {
      head: regroup.previous_head.clone(),
      baseline_branch,
      base,
      stacks: regroup.grouping.stacks,
      regroup,
      branches: checks,
      unassigned: unassigned.len(),
      dangling,
    }))
  }

  /// The dangling references of the built branches of `report`, a check that looked for none (an empty `dangling_of`):
  /// no build again
  pub(crate) fn dangling_of_report(&self, report: &CheckReport) -> Result<BTreeMap<String, Vec<DanglingReference>>, RegroupError> {
    self.dangling_references(&report.head, report.base.as_deref(), &report.branches, None)
  }

  /// `tree_id_cache` with the tree and committer date of `base`, when an earlier check learned them
  fn seeded_base(&self, tree_id_cache: TreeIdCache, base: Option<&str>) -> TreeIdCache {
    let facts = self.base_facts.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(facts) = facts.as_ref().filter(|facts| Some(facts.commit.as_str()) == base) {
      tree_id_cache.insert(&facts.commit, &facts.tree);
      tree_id_cache.insert_committer_timestamp(&facts.commit, facts.committer_timestamp);
    }
    tree_id_cache
  }

  /// Keep what a check that built on `base` read of it; the cache of that check answers without git
  fn remember_base(&self, tree_id_cache: &TreeIdCache, base: &str) {
    let mut facts = self.base_facts.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if facts.as_ref().is_some_and(|facts| facts.commit == base) {
      return;
    }
    let (git, repository) = (&self.git_executor, self.repository_path.as_str());
    if let (Ok(tree), Ok(committer_timestamp)) = (
      tree_id_cache.get_tree_id(git, repository, base),
      tree_id_cache.get_committer_timestamp(git, repository, base),
    ) {
      *facts = Some(BaseFacts {
        commit: base.to_string(),
        tree,
        committer_timestamp,
      });
    }
  }

  /// The commits of the virtual branches at `tips` down to `base`, read by the first check of the session that asks
  fn existing_commits_on(&self, base: &str, tips: &HashMap<String, String>) -> Arc<ExistingCommits> {
    let mut known = self.existing_commits.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    Arc::clone(
      known
        .entry(base.to_string())
        .or_insert_with(|| Arc::new(ExistingCommits::new(base, tips.values().cloned()))),
    )
  }

  /// How the declared parents without commits of `forest` stand (`empty_parent_states`), detected once per session for
  /// each set of them
  fn empty_parent_states(&self, forest: &StackForest) -> Result<ParentStates, RegroupError> {
    let empty: BTreeSet<String> = forest.nodes.iter().filter(|(_, node)| !node.has_commits).map(|(name, _)| name.clone()).collect();
    let memo = || self.parent_states.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(states) = memo().get(&empty) {
      return Ok(states.clone());
    }
    let detection = PendingDetection {
      baseline_commit: self.baseline_commit(),
      ..read_only_detection(&self.snapshot.baseline)
    };
    let states = empty_parent_states(&self.git_executor, &self.repository_path, &self.branch_prefix, &self.tips, forest, detection)?;
    memo().insert(empty, states.clone());
    Ok(states)
  }

  /// The commit of the baseline, one `rev-parse` per session (and the sessions of `with_rewrite`); `None` when it does
  /// not resolve
  fn baseline_commit(&self) -> Option<&str> {
    self
      .baseline_commit
      .get_or_init(|| resolve_commit(&self.git_executor, &self.repository_path, &self.snapshot.baseline).ok().flatten())
      .as_deref()
  }

  /// The changed lines of the commits ahead of the baseline and the baseline tip, read once per session
  fn dangling_facts(&self) -> Result<&DanglingFacts, RegroupError> {
    let commits: Vec<String> = self.snapshot.commits.iter().map(|commit| commit.id.clone()).collect();
    let read = self.dangling_input.get_or_init(|| {
      let lines = read_commit_lines(&self.git_executor, &self.repository_path, &commits, &self.cache, &self.file_diffs).map_err(|e| format!("{e:#}"))?;
      Ok((lines, self.baseline_commit().map(str::to_string)))
    });
    read.as_ref().map_err(|e| RegroupError::Other(anyhow::anyhow!("Cannot read the changed lines: {e}")))
  }

  /// The changed lines of each commit ahead of the baseline, in the files of the known languages
  pub(crate) fn changed_lines(&self) -> Result<&HashMap<String, Vec<FileLines>>, RegroupError> {
    self.dangling_facts().map(|(lines, _)| lines)
  }

  /// The changes of the commits ahead of the baseline, oldest first (positions of `snapshot().commits`), from the object
  /// cache and one `log` for the misses, read once per session
  pub fn commit_diffs(&self) -> Result<&[CommitDiff], RegroupError> {
    self
      .commit_diffs
      .get_or_init(|| {
        let ids: Vec<String> = self.snapshot.commits.iter().map(|commit| commit.id.clone()).collect();
        read_commit_diffs_cached(&self.git_executor, &self.repository_path, &self.cache, &self.file_diffs, &ids).map_err(|e| format!("{e:#}"))
      })
      .as_ref()
      .map(Vec::as_slice)
      .map_err(|e| RegroupError::Other(anyhow::anyhow!("Cannot read the changes of the commits: {e}")))
  }

  /// The line dependencies of the commits ahead of the baseline with their fixup units, computed once per session
  pub(crate) fn line_graph(&self) -> Result<&LineGraph, RegroupError> {
    self
      .line_graph
      .get_or_init(|| {
        let diffs = self.commit_diffs().map_err(|e| format!("{e:#}"))?;
        Ok(LineGraph::new(&self.snapshot, diffs))
      })
      .as_ref()
      .map_err(|e| RegroupError::Other(anyhow::anyhow!("{e}")))
  }

  /// The dangling references of the built branches
  fn dangling_references(&self, head: &str, base: Option<&str>, checks: &[BranchCheck], of: Option<&[String]>) -> Result<BTreeMap<String, Vec<DanglingReference>>, RegroupError> {
    let commits: Vec<String> = self.snapshot.commits.iter().map(|commit| commit.id.clone()).collect();
    let (lines, baseline_tip) = self.dangling_facts()?;
    // the summaries only where a commit changes a source file of a grammar
    let sources = lines.values().flatten().any(|file| symbols_language_of(&file.path).is_some());
    let declarations = match if sources { self.symbol_input()? } else { None } {
      Some(input) => Some(DeclarationFacts {
        input,
        diffs: self.commit_diffs()?,
        added: &self.added_keys,
      }),
      None => None,
    };
    let input = DanglingInput {
      head,
      base,
      baseline_tip: baseline_tip.as_deref(),
      commits: &commits,
      lines,
      of,
      syntax: (self.symbols != SymbolMode::Off).then_some(&*self.syntax),
      declarations,
    };
    find_dangling_references(&self.git_executor, &self.repository_path, &input, checks).map_err(RegroupError::from)
  }

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
      Some(CheckOutcome::Ok { .. }) => {
        let graph = self.symbol_graph()?;
        Ok(StackIndependence::Builds {
          parent,
          uses: graph.map_or_else(Vec::new, |graph| self.ancestor_uses(branch, graph)),
          symbols: graph.is_some(),
        })
      }
      Some(CheckOutcome::Conflict { commit, files }) => self.needed_commits(branch, parent, &commit, files),
      Some(CheckOutcome::Error { message }) => Err(RegroupError::Other(anyhow::anyhow!(message))),
      Some(CheckOutcome::Problems(problems)) => Err(RegroupError::Other(anyhow::anyhow!(
        problems.iter().map(|problem| describe_problem(branch, problem)).collect::<Vec<_>>().join("; ")
      ))),
      Some(CheckOutcome::Blocked { parent, .. }) => Err(RegroupError::Other(anyhow::anyhow!("{branch} is blocked by {parent}"))),
      None => Err(RegroupError::InvalidPlan {
        message: format!("No branch named {branch:?} among the commits ahead of the baseline"),
        commits: Vec::new(),
      }),
    }
  }

  /// The commit at `position` of the snapshot, as `StackIndependence` names it
  fn stack_commit(&self, position: usize) -> StackCommit {
    let commit = &self.snapshot.commits[position];
    StackCommit {
      id: commit.id.clone(),
      subject: subject_without_prefix(&commit.subject),
      branch: self.snapshot.groups.get(&commit.id).and_then(Option::as_deref).unwrap_or_default().to_string(),
    }
  }

  /// The commit `id` of the snapshot, as `StackIndependence` names it; `None` for a commit not ahead of the baseline
  pub(crate) fn stack_commit_of(&self, id: &str) -> Option<StackCommit> {
    self.snapshot.commits.iter().position(|commit| commit.id == id).map(|position| self.stack_commit(position))
  }

  /// The uses, by commits of `branch`, of names that commits of its declared ancestors add, one per name
  fn ancestor_uses(&self, branch: &str, graph: &SymbolGraph) -> Vec<StackUse> {
    let commits = &self.snapshot.commits;
    let group_of = |position: usize| commits.get(position).and_then(|commit| self.snapshot.groups.get(&commit.id)).and_then(Option::as_deref);
    let ancestors = self.snapshot.forest.as_ref().map_or_default(|forest| ancestors_of(forest, branch));
    let named = |position: usize| self.stack_commit(position);
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    graph
      .edges
      .iter()
      .filter(|edge| group_of(edge.from) == Some(branch) && group_of(edge.to).is_some_and(|group| ancestors.contains(&group)))
      .flat_map(|edge| edge.links.iter().map(move |link| (edge, link)))
      .filter(|(_, link)| seen.insert(link.name.as_str()))
      .map(|(edge, link)| StackUse {
        commit: named(edge.from),
        name: link.name.clone(),
        declared_by: named(edge.to),
      })
      .collect()
  }

  /// The commits of the ancestors of `branch` that its commits need by their lines, and what those need in turn, from
  /// the changes of every commit ahead of the baseline (`line_graph`). Generated files count: the conflict of a trial
  /// build is real in any file
  fn needed_commits(&self, branch: &str, parent: String, conflicting: &str, files: Vec<String>) -> Result<StackIndependence, RegroupError> {
    let commits = &self.snapshot.commits;
    let LineGraph { needs, unit } = self.line_graph()?;
    let group_of = |position: usize| self.snapshot.groups.get(&commits[position].id).and_then(Option::as_deref);
    let forest = self.snapshot.forest.as_ref();
    let ancestors = forest.map_or_default(|forest| ancestors_of(forest, branch));

    // from the commits of the branch through the commits of its ancestors, each with its fixup unit
    let in_ancestor = |position: usize| group_of(position).is_some_and(|group| ancestors.contains(&group));
    let mut needed: BTreeSet<usize> = BTreeSet::new();
    let mut pending: Vec<usize> = (0..commits.len()).filter(|&position| group_of(position) == Some(branch)).collect();
    while let Some(position) = pending.pop() {
      for &to in needs[position].iter().chain(&unit[position]) {
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

    let named = |position: usize| self.stack_commit(position);
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

/// A branch of a check as `run_builds` schedules it
pub(crate) struct Planned {
  pub name: String,
  /// The index of the branch it is built on, when that one is checked too; else it is built on the base
  pub parent: Option<usize>,
  /// Its outcome without a build: the branch can't be placed in its stack
  pub immediate: Option<CheckOutcome>,
}

/// What `run_builds` knows while the branches build
struct Schedule {
  outcomes: Vec<Option<CheckOutcome>>,
  /// `(branch, the commit it is built on, whether that is its parent's head)`
  ready: VecDeque<(usize, String, bool)>,
  running: usize,
}

impl Schedule {
  /// The children of `index` now that its outcome is known: on the head of a branch that builds they are ready, a
  /// failed branch blocks them, and their own children in turn
  fn release(&mut self, index: usize, planned: &[Planned], children: &[Vec<usize>]) {
    for &child in &children[index] {
      if self.outcomes[child].is_some() {
        continue;
      }
      match &self.outcomes[index] {
        Some(CheckOutcome::Ok { head }) => self.ready.push_back((child, head.clone(), true)),
        Some(outcome) => {
          let reason = match outcome {
            CheckOutcome::Conflict { .. } => BlockReason::Conflict,
            CheckOutcome::Blocked { .. } => BlockReason::Blocked,
            CheckOutcome::Problems(_) => BlockReason::Problem,
            _ => BlockReason::Error,
          };
          self.outcomes[child] = Some(CheckOutcome::Blocked {
            parent: planned[index].name.clone(),
            reason,
          });
          self.release(child, planned, children);
        }
        None => {}
      }
    }
  }
}

/// The outcome of each of `planned` (a parent before its children): the branches build on `threads` threads, each as
/// soon as the branch it is built on has its head, the roots on `base`. `build(index, base commit, on parent)` builds
/// one; a branch placed with a problem is not built, and a failed branch blocks the branches built on it, as a check
/// one by one decides
pub(crate) fn run_builds(planned: &[Planned], base: Option<&str>, threads: usize, build: impl Fn(usize, &str, bool) -> CheckOutcome + Sync) -> Vec<CheckOutcome> {
  let mut children = vec![Vec::new(); planned.len()];
  for (index, branch) in planned.iter().enumerate() {
    if let Some(parent) = branch.parent {
      children[parent].push(index);
    }
  }
  let mut schedule = Schedule {
    outcomes: planned.iter().map(|branch| branch.immediate.clone()).collect(),
    ready: VecDeque::new(),
    running: 0,
  };
  for index in 0..planned.len() {
    if schedule.outcomes[index].is_some() {
      schedule.release(index, planned, &children);
    }
  }
  for (index, branch) in planned.iter().enumerate() {
    if schedule.outcomes[index].is_some() || branch.parent.is_some() {
      continue;
    }
    if let Some(base) = base {
      schedule.ready.push_back((index, base.to_string(), false));
    } else {
      schedule.outcomes[index] = Some(CheckOutcome::Error {
        message: "The oldest commit ahead of the baseline has no parent".to_string(),
      });
      schedule.release(index, planned, &children);
    }
  }

  let state = Mutex::new(schedule);
  let woken = Condvar::new();
  let lock = || state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
  std::thread::scope(|scope| {
    for _ in 0..threads.clamp(1, planned.len().max(1)) {
      scope.spawn(in_current_span(|| {
        loop {
          let job = {
            let mut schedule = lock();
            loop {
              if let Some(job) = schedule.ready.pop_front() {
                schedule.running += 1;
                break Some(job);
              }
              if schedule.running == 0 {
                break None;
              }
              schedule = woken.wait(schedule).unwrap_or_else(std::sync::PoisonError::into_inner);
            }
          };
          let Some((index, base_commit, on_parent)) = job else {
            woken.notify_all();
            return;
          };
          let outcome = build(index, &base_commit, on_parent);
          let mut schedule = lock();
          schedule.running -= 1;
          schedule.outcomes[index] = Some(outcome);
          schedule.release(index, planned, &children);
          drop(schedule);
          woken.notify_all();
        }
      }));
    }
  });
  state
    .into_inner()
    .unwrap_or_else(std::sync::PoisonError::into_inner)
    .outcomes
    .into_iter()
    .map(|outcome| outcome.unwrap_or_else(|| CheckOutcome::Error { message: "Not built".to_string() }))
    .collect()
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
    Ok(BuildOutcome::Failed(..)) => progress.commit_error(name).unwrap_or_else(|| CheckOutcome::Error {
      message: format!("Branch {name} failed to build"),
    }),
    Err(e) => CheckOutcome::Error { message: format!("{e:#}") },
  }
}

/// Keeps the commit errors of the branches of a check, each built once; the other events of a build are not needed
#[derive(Clone, Default)]
struct CollectingReporter {
  errors: Arc<Mutex<Vec<(String, String, BranchError)>>>,
}

impl CollectingReporter {
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

/// The snapshot after the sync of `report`, on what it read at its start (its guards passed, HEAD, the grouping, the
/// baseline): one `log` of the commits ahead of the baseline, for the mapping notes the sync wrote. Under the
/// repository lock the sync and the checks after it hold, no rewrite moved HEAD since
fn snapshot_of_sync(git_executor: &GitCommandExecutor, repository_path: &str, report: &SyncReport) -> anyhow::Result<Snapshot> {
  let commits = get_commit_list(git_executor, repository_path, &report.baseline)?;
  let Grouped { groups, forest, .. } = group_commits(&commits, &report.grouping);
  Ok(Snapshot {
    head: report.head.clone(),
    branch: report.branch.clone(),
    grouping: report.grouping.clone(),
    baseline: report.baseline.clone(),
    commits,
    groups,
    forest,
  })
}

#[cfg(test)]
#[path = "check_test.rs"]
mod check_test;
