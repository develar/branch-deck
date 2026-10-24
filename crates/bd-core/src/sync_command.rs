//! `bd sync`: `sync_core::sync::sync_branches`, its events folded into the table of the app's session as they come
//! (`repository_model::table::TableState`), and that table with the report in one compact summary. It rebuilds the
//! virtual branches, runs the integration detection of archived branches and the update check of the checked out
//! branch, as the app's sync does, and like it without the repository lock: its refs move by compare-and-swap, so a
//! sync never makes a rewrite, Update Project or the app wait

use crate::bd_command::shell_word;
use crate::branch_view::placement;
use crate::context::RepoContext;
use crate::error::CliError;
use crate::output::{ResolvedByWords, SchemaVersion, ShortIds};
use crate::stack_summary::StackSummary;
use domain_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus};
use domain_types::branch_names::to_final_branch_name;
use domain_types::commit::Commit;
use domain_types::conflict::{MergeConflictInfo, WordResolutionSummary};
use domain_types::error::ErrorCode;
use domain_types::landed::LandedBranch;
use domain_types::session::{BaselineStatus, BranchStatus, BranchSync, BranchView};
use domain_types::stack::BranchStack;
use domain_types::update_project::{UpdateKind, UpdateState};
use domain_types::{BranchError, ProgressReporter, RemoteStatusUpdate, SyncEvent};
use history_rewrite::repository_lock::{RepositoryLock, RepositoryLocked, lock_repository_in};
use repository_model::table::TableState;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use sync_core::sync::{SyncOptions, SyncReport, sync_branches};

/// `bd sync`
#[derive(Debug, Serialize, JsonSchema)]
pub struct SyncOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub baseline: String,
  /// In the order of `bd status`
  pub branches: Vec<SyncedBranch>,
  /// With stacked branches on: the stack branch of each stack that forks (`{prefix}/stack/<root>`, all its commits in
  /// one branch), then those kept for their remote branch after their stack stopped forking
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub stacks: Vec<StackSummary>,
  pub unassigned_count: usize,
  /// The archived branches that integration detection checked
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub archived: Vec<ArchivedBranch>,
  /// Whether the checked out branch can be updated onto the last fetched baseline; omitted for a local baseline
  #[serde(skip_serializing_if = "Option::is_none")]
  pub update: Option<UpdateSummary>,
  /// Commits that landed in the baseline with an update (here or elsewhere) and that the app hasn't dismissed
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub landed: Vec<LandedSummary>,
  /// What the sync read and the tips it left, for a command that goes on after the sync (`bd push`, `bd safe-push`)
  #[serde(skip)]
  #[schemars(skip)]
  pub report: SyncReport,
  /// The remote statuses the table's rows show, one per branch the sync computed one of, for the remote facts of its
  /// table (`repository_model::branch_table::remote_facts`)
  #[serde(skip)]
  #[schemars(skip)]
  pub remote_statuses: Vec<RemoteStatusUpdate>,
}

impl SyncOutput {
  /// A branch the sync could not build (`SyncState::is_built`): the exit code 1 of `bd sync` and of a rewrite with --sync
  pub fn has_unbuilt(&self) -> bool {
    self.branches.iter().any(|branch| !branch.state.is_built())
  }

  /// Parents first: the stack order with stacked branches, else the order of `branches`
  pub fn order(&self) -> Vec<&str> {
    self.report.forest.as_ref().map_or_else(
      || self.branches.iter().map(|branch| branch.name.as_str()).collect(),
      |forest| forest.order.iter().map(String::as_str).collect(),
    )
  }

  /// Where the sync placed the branch in the stack; `None` with stacked branches off
  pub fn placement(&self, name: &str) -> Option<&BranchStack> {
    placement(&self.report.branches, name)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
  Created,
  Updated,
  Unchanged,
  Error,
  Conflict,
  /// A stacked branch whose parent can't be built on: not rebuilt this sync
  Blocked,
}

impl SyncState {
  /// The virtual branch is what its commits build: not a conflict, an error, or a stacked branch whose parent can't be
  /// built on
  pub const fn is_built(self) -> bool {
    matches!(self, Self::Created | Self::Updated | Self::Unchanged)
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SyncedBranch {
  pub name: String,
  #[serde(rename = "ref")]
  pub reference: String,
  pub state: SyncState,
  pub commit_count: usize,
  /// The virtual branch after the sync; `null` if it was not written
  pub tip: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub conflict: Option<ConflictSummary>,
  /// Commits not synced because an earlier commit of the branch failed
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub blocked: Vec<String>,
  /// The commits whose conflict of git the sync resolved by words, oldest first: the branch built where git stopped
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub resolved: Vec<ResolvedCommit>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub remote: Option<RemoteSummary>,
  /// The `bd push` that brings the remote branch up to date, to run only after the user approves: with
  /// `--force --expect-remote <remote.head>` when the push drops `dropped_commits`, with `--force` alone when a parent
  /// it takes along has commits for the remote too; omitted when the remote branch is up to date or the branch didn't
  /// sync
  #[serde(skip_serializing_if = "Option::is_none")]
  pub push: Option<String>,
}

/// A commit whose conflict of git the sync resolved by words (`branchdeck.resolveByWords`)
#[derive(Debug, Serialize, JsonSchema)]
pub struct ResolvedCommit {
  /// The original commit
  pub commit: String,
  #[serde(flatten)]
  pub resolved: ResolvedByWords,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ConflictSummary {
  /// The commit whose cherry-pick conflicts
  pub commit: String,
  pub subject: String,
  pub files: Vec<String>,
  /// Commits of other branches that the conflicting change likely needs
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub missing_commits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RemoteSummary {
  pub exists: bool,
  /// Commits of the virtual branch the remote doesn't have: all of them while there is no remote branch
  pub unpushed: usize,
  /// Commits the remote has that the virtual branch doesn't: a push with --force drops them. An older version of a
  /// commit the branch or the baseline has is not one of them (`amended`, `restacked`, `landed`)
  pub behind: u32,
  /// Remote commits that are older versions of commits amended here (same author, author time and subject): a push
  /// replaces them without --force
  #[serde(default, skip_serializing_if = "is_zero")]
  pub amended: u32,
  /// Remote commits that are older versions of commits of the branches a stacked branch is built on, amended or rebuilt
  /// since it was pushed: a push replaces them without --force
  #[serde(default, skip_serializing_if = "is_zero")]
  pub restacked: u32,
  /// Remote commits that are older versions of commits the baseline has (same author, author time and subject; the
  /// changes may differ, as when a Safe Push landed them on a newer target): they landed, a push replaces them without
  /// --force
  #[serde(default, skip_serializing_if = "is_zero")]
  pub landed: u32,
  /// Commits of the virtual branch the remote has by content in another version (a reword, another author): not
  /// unpushed, but a push replaces their remote copies, which a Safe Push of the remote branch would land as they are
  #[serde(default, skip_serializing_if = "is_zero")]
  pub reworded: u32,
  /// Commits of the branches a stacked branch is built on that its remote branch lacks (a new commit of the parent since
  /// it was pushed): a Safe Push of the remote branch would land an older piece, so a push replaces it
  #[serde(default, skip_serializing_if = "is_zero")]
  pub ancestors_ahead: u32,
  /// The remote-tracking commit the counts compare with; `null` without a remote branch. `bd push --force` leases on
  /// it, as `--expect-remote` does
  pub head: Option<String>,
  /// The newest of the `behind` commits, newest first, at most ten
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub dropped_commits: Vec<DroppedCommitSummary>,
  /// The newest of the `landed` commits, newest first, at most ten, each with the baseline commit that has it
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub landed_commits: Vec<LandedRemoteCommitSummary>,
  /// `head` in full, for the lease
  #[serde(skip)]
  pub head_id: Option<String>,
}

/// By reference: the signature serde's `skip_serializing_if` needs
pub const fn is_zero(value: &u32) -> bool {
  *value == 0
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DroppedCommitSummary {
  pub commit: String,
  pub subject: String,
  pub author: String,
  /// Unix seconds
  pub author_time: u32,
}

/// A remote commit that landed in the baseline: a push replaces it
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LandedRemoteCommitSummary {
  pub commit: String,
  pub subject: String,
  /// The commit of the baseline with its author, author time and subject
  pub baseline_commit: String,
}

impl RemoteSummary {
  /// `commit_count`: the commits of the branch, all of them unpushed without a remote branch
  pub fn new(remote: &RemoteStatusUpdate, commit_count: usize, short: &ShortIds) -> Self {
    Self {
      exists: remote.remote_exists,
      unpushed: if remote.remote_exists { remote.unpushed_commits.len() } else { commit_count },
      behind: remote.commits_behind,
      amended: remote.commits_amended,
      restacked: remote.commits_restacked,
      landed: remote.commits_landed,
      reworded: remote.commits_reworded,
      ancestors_ahead: remote.ancestor_commits_ahead,
      head: remote.remote_tip.as_deref().map(|head| short.get(head).to_string()),
      dropped_commits: remote
        .dropped_commits
        .iter()
        .map(|commit| DroppedCommitSummary {
          commit: short.get(&commit.id).to_string(),
          subject: commit.subject.clone(),
          author: commit.author.clone(),
          author_time: commit.author_time,
        })
        .collect(),
      landed_commits: landed_commits(remote, short),
      head_id: remote.remote_tip.clone(),
    }
  }

  /// A push of the branch would change its remote branch: the remote lacks it or commits of it, or has commits a push
  /// replaces or drops
  pub const fn has_work(&self) -> bool {
    !self.exists || self.unpushed > 0 || self.behind > 0 || self.amended > 0 || self.restacked > 0 || self.landed > 0 || self.reworded > 0 || self.ancestors_ahead > 0
  }

  /// The ids `new` shortens, for `RepoContext::short_ids`
  pub fn ids(remote: &RemoteStatusUpdate) -> impl Iterator<Item = &str> {
    remote
      .remote_tip
      .as_deref()
      .into_iter()
      .chain(remote.dropped_commits.iter().map(|commit| commit.id.as_str()))
      .chain(remote.landed_commits.iter().flat_map(|commit| [commit.id.as_str(), commit.baseline_commit.as_str()]))
  }
}

/// The landed commits of `remote` (`RemoteSummary::landed_commits`), with the ids `RemoteSummary::ids` shortened
pub fn landed_commits(remote: &RemoteStatusUpdate, short: &ShortIds) -> Vec<LandedRemoteCommitSummary> {
  remote
    .landed_commits
    .iter()
    .map(|commit| LandedRemoteCommitSummary {
      commit: short.get(&commit.id).to_string(),
      subject: commit.subject.clone(),
      baseline_commit: short.get(&commit.baseline_commit).to_string(),
    })
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationState {
  Integrated,
  NotIntegrated,
  Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCheckState {
  UpToDate,
  FastForward,
  Rebase,
  Conflict,
  Blocked,
  Error,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ArchivedBranch {
  pub name: String,
  pub state: IntegrationState,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub integrated_at: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateSummary {
  pub upstream: Option<String>,
  /// `blocked`: local changes sit on paths the update writes; `error`: the check failed
  pub state: UpdateCheckState,
  /// The files a conflicting update touches (at most 50), or the local changes that block it (at most 100)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub files: Vec<String>,
  /// The files left out of `files`
  #[serde(default, skip_serializing_if = "is_zero")]
  pub files_omitted: u32,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
  /// Local commits the fetched baseline already has: the update drops them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub landing: Vec<LandedSummary>,
}

/// The landed commits of a branch
#[derive(Debug, Serialize, JsonSchema)]
pub struct LandedSummary {
  /// The virtual branch; omitted for unassigned commits
  #[serde(skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  pub commits: Vec<LandedCommitSummary>,
  /// The commits of the branch that stay local
  pub remaining: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LandedCommitSummary {
  /// The local commit
  pub commit: String,
  pub subject: String,
  /// The baseline commit that carries it; omitted when the baseline has the change in another commit (squashed)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub upstream: Option<String>,
}

pub fn landed_summary(branches: Vec<LandedBranch>, short: &ShortIds) -> Vec<LandedSummary> {
  branches
    .into_iter()
    .map(|branch| LandedSummary {
      branch: branch.name,
      commits: branch
        .commits
        .into_iter()
        .map(|commit| LandedCommitSummary {
          commit: short.get(&commit.id).to_string(),
          subject: commit.subject,
          upstream: commit.upstream.map(|upstream| short.get(&upstream.id).to_string()),
        })
        .collect(),
      remaining: branch.remaining,
    })
    .collect()
}

/// The ids a landed report names
pub fn landed_ids(branches: &[LandedBranch]) -> impl Iterator<Item = &str> {
  branches
    .iter()
    .flat_map(|branch| &branch.commits)
    .flat_map(|commit| std::iter::once(commit.id.as_str()).chain(commit.upstream.as_ref().map(|upstream| upstream.id.as_str())))
}

/// Folds each event of the sync into a table of the app's session at once (`TableState::apply`, the applier of the
/// app's rows): the summary reads that table, not the events once more
#[derive(Clone)]
struct TableReporter {
  table: Arc<Mutex<TableState>>,
}

impl TableReporter {
  fn new() -> Self {
    let mut table = TableState::new();
    table.begin_sync();
    Self {
      table: Arc::new(Mutex::new(table)),
    }
  }

  /// The table the sync left once it succeeded
  fn finish(&self) -> Result<TableState, CliError> {
    let mut table = std::mem::take(
      &mut *self
        .table
        .lock()
        .map_err(|e| CliError::new(ErrorCode::GitError, format!("the table of the sync is poisoned: {e}")))?,
    );
    table.finish_sync(Ok(()));
    Ok(table)
  }
}

impl ProgressReporter for TableReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    self.table.lock().map_err(|e| anyhow::anyhow!("the table of the sync is poisoned: {e}"))?.apply(event);
    Ok(())
  }
}

/// `bd sync`, without the repository lock: the app's sync takes none either, and both move each ref only from the tip
/// they listed
pub fn sync(context: &RepoContext) -> Result<SyncOutput, CliError> {
  sync_locked(context)
}

/// The repository lock, `locked` after the timeout
pub fn lock(context: &RepoContext) -> Result<RepositoryLock, CliError> {
  // another process holds it: it may change the config the command read before
  let on_wait = || context.forget_config();
  lock_repository_in(&context.git_common_dir, context.lock_timeout, &on_wait).map_err(|e| match e.downcast_ref::<RepositoryLocked>() {
    Some(locked) => CliError::new(ErrorCode::Locked, locked.to_string()),
    None => CliError::from(e),
  })
}

/// The sync of `bd sync`; `bd push` and `bd safe-push` run it under the lock they hold to the end of their push
pub fn sync_locked(context: &RepoContext) -> Result<SyncOutput, CliError> {
  let reporter = TableReporter::new();
  let runtime = context.runtime()?;
  // `--no-cache` trusts no mapping note either: each is checked against the tree of its virtual commit; its cache is
  // disabled, so every conflict is analysed again
  let options = SyncOptions {
    verify_mapping_notes: !context.use_cache,
    object_cache: context.cache(),
    config: Some(context.config()?),
    ..SyncOptions::default()
  };
  let report = runtime.block_on(sync_branches(&context.git, &context.repository, &context.prefix, reporter.clone(), options))?;
  Ok(summarize(context, report, &reporter.finish()?))
}

/// One summary of the report and of the table the sync left: the rows in the order of the table, each with its state,
/// its error, its blocked and resolved commits and its remote status, then the stack branches and the sections
fn summarize(context: &RepoContext, report: SyncReport, table: &TableState) -> SyncOutput {
  // branch -> the branch it is built on
  let bases: HashMap<String, String> = report
    .branches
    .iter()
    .filter_map(|branch| Some((branch.name.clone(), branch.stack.as_ref()?.effective_base.clone()?)))
    .collect();
  let tip_of = |name: &str| report.tips.get(&to_final_branch_name(&context.prefix, name).ok()?);
  let rows: Vec<&BranchView> = table.order().iter().filter_map(|name| table.branch(name)).collect();
  let sections = table.sections();
  let landed = &sections.landed;

  let mut ids: Vec<&str> = report.tips.values().map(String::as_str).collect();
  for row in &rows {
    ids.extend(table.blocked_commits(&row.head.name).iter().map(String::as_str));
    ids.extend(resolved_commits(row).map(|(commit, _)| commit.id.as_str()));
    ids.extend(error_of(&row.sync).1.map(|info| info.commit.hash.as_str()));
    ids.extend(row.remote.status.iter().flat_map(RemoteSummary::ids));
  }
  ids.extend(table.stacks().flat_map(StackSummary::ids));
  ids.extend(landed_ids(&landed.landing));
  ids.extend(landed_ids(&landed.landed));
  let short = context.short_ids(ids);

  let branches: Vec<SyncedBranch> = rows
    .iter()
    .map(|row| {
      let name = &row.head.name;
      let commit_count = row.commits.len();
      let state = sync_state(row.sync.status);
      let (error, conflict) = error_of(&row.sync);
      SyncedBranch {
        name: name.clone(),
        reference: to_final_branch_name(&context.prefix, name).map_or_else(|_| String::new(), |name| format!("refs/heads/{name}")),
        state,
        commit_count,
        tip: state.is_built().then(|| tip_of(name).map(|tip| short.get(tip).to_string())).flatten(),
        error: error.map(str::to_owned),
        conflict: conflict.map(|info| conflict_summary(info, &short)),
        blocked: table.blocked_commits(name).iter().map(|id| short.get(id).to_string()).collect(),
        resolved: resolved_commits(row)
          .map(|(commit, resolved)| ResolvedCommit {
            commit: short.get(&commit.id).to_string(),
            resolved: ResolvedByWords::of_summary(resolved),
          })
          .collect(),
        remote: row.remote.status.as_ref().map(|remote| RemoteSummary::new(remote, commit_count, &short)),
        push: None,
      }
    })
    .collect();
  let branches = with_push_commands(branches, &bases, &context.bd);
  let stacks = table
    .stacks()
    .map(|stack| StackSummary::new(stack, rows.iter().flat_map(|row| &row.commits), &short))
    .collect();

  SyncOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    baseline: report.baseline.clone(),
    branches,
    stacks,
    unassigned_count: sections.unassigned.len(),
    archived: sections.archived.integration.values().map(archived_branch).collect(),
    update: landed.baseline.as_ref().and_then(|baseline| update_summary(baseline, &landed.landing, &short)),
    landed: landed_summary(landed.landed.clone(), &short),
    report,
    remote_statuses: rows.iter().filter_map(|row| row.remote.status.clone()).collect(),
  }
}

/// The state of a row; a branch the sync reported no status of is unchanged
const fn sync_state(status: BranchStatus) -> SyncState {
  match status {
    BranchStatus::Created => SyncState::Created,
    BranchStatus::Updated => SyncState::Updated,
    BranchStatus::Syncing | BranchStatus::Unchanged => SyncState::Unchanged,
    BranchStatus::Error => SyncState::Error,
    BranchStatus::Blocked => SyncState::Blocked,
    BranchStatus::MergeConflict | BranchStatus::AnalyzingConflict => SyncState::Conflict,
  }
}

/// The error the sync reported with the status of a row (for a block: why its parent can't be built on), or its
/// conflict
fn error_of(sync: &BranchSync) -> (Option<&str>, Option<&MergeConflictInfo>) {
  match (sync.status, &sync.error_details) {
    (BranchStatus::Blocked, _) => (sync.blocked_reason.as_deref(), None),
    (_, Some(BranchError::Generic(message))) => (Some(message), None),
    (_, Some(BranchError::MergeConflict(info))) => (None, Some(info)),
    (_, None) => (None, None),
  }
}

/// The commits of a row whose conflict of git the sync resolved by words, oldest first
fn resolved_commits(row: &BranchView) -> impl Iterator<Item = (&Commit, &WordResolutionSummary)> {
  row
    .commits
    .iter()
    .zip(&row.copies)
    .rev()
    .filter_map(|(commit, copy)| Some((commit, copy.resolved.as_ref()?)))
}

fn archived_branch(info: &BranchIntegrationInfo) -> ArchivedBranch {
  let (state, integrated_at) = match &info.status {
    BranchIntegrationStatus::Integrated { integrated_at, .. } => (IntegrationState::Integrated, *integrated_at),
    BranchIntegrationStatus::NotIntegrated { integrated_at, .. } => (IntegrationState::NotIntegrated, *integrated_at),
    BranchIntegrationStatus::Partial { .. } => (IntegrationState::Partial, None),
  };
  ArchivedBranch {
    name: info.name.clone(),
    state,
    integrated_at,
  }
}

/// The update check of the baseline status with the local commits the update drops; `None` for a local baseline
fn update_summary(baseline: &BaselineStatus, landing: &[LandedBranch], short: &ShortIds) -> Option<UpdateSummary> {
  let (upstream, state, files, count, error) = match (&baseline.update, &baseline.check_error) {
    (Some(check), _) => {
      let (state, files, count) = match &check.state {
        UpdateState::UpToDate => (UpdateCheckState::UpToDate, Vec::new(), 0),
        UpdateState::Clean { kind: UpdateKind::FastForward } => (UpdateCheckState::FastForward, Vec::new(), 0),
        UpdateState::Clean { kind: UpdateKind::Rebase } => (UpdateCheckState::Rebase, Vec::new(), 0),
        UpdateState::Conflict { files, file_count, .. } => (UpdateCheckState::Conflict, files.clone(), *file_count),
        UpdateState::Blocked { paths, path_count } => (UpdateCheckState::Blocked, paths.clone(), *path_count),
      };
      (Some(check.upstream.clone()), state, files, count, None)
    }
    (None, Some(error)) => (None, UpdateCheckState::Error, Vec::new(), 0, Some(error.clone())),
    (None, None) => return None,
  };
  Some(UpdateSummary {
    upstream,
    state,
    files_omitted: count.saturating_sub(u32::try_from(files.len()).unwrap_or(u32::MAX)),
    files,
    error,
    landing: landed_summary(landing.to_vec(), short),
  })
}

/// `branches` with the `bd push` each needs. `--expect-remote` pushes the named branch alone, so it pins the lease only
/// when no branch it is built on has commits for the remote
fn with_push_commands(mut branches: Vec<SyncedBranch>, bases: &HashMap<String, String>, bd: &str) -> Vec<SyncedBranch> {
  let work: HashMap<&str, bool> = branches
    .iter()
    .map(|branch| (branch.name.as_str(), branch.tip.is_some() && branch.remote.as_ref().is_some_and(RemoteSummary::has_work)))
    .collect();
  // the chain of bases, bounded in case a broken stack had a cycle
  let ancestor_has_work = |name: &str| {
    std::iter::successors(bases.get(name), |base| bases.get(base.as_str()))
      .take(bases.len())
      .any(|base| work.get(base.as_str()).copied().unwrap_or(false))
  };
  let commands: Vec<Option<String>> = branches
    .iter()
    .map(|branch| {
      let remote = branch.remote.as_ref().filter(|_| work[branch.name.as_str()])?;
      let name = shell_word(&branch.name);
      Some(match (remote.behind > 0, remote.head.as_deref()) {
        (true, Some(head)) if !ancestor_has_work(&branch.name) => format!("{bd} push {name} --force --expect-remote {head}"),
        (true, _) => format!("{bd} push {name} --force"),
        (false, _) => format!("{bd} push {name}"),
      })
    })
    .collect();
  for (branch, push) in branches.iter_mut().zip(commands) {
    branch.push = push;
  }
  branches
}

fn conflict_summary(info: &MergeConflictInfo, short: &ShortIds) -> ConflictSummary {
  ConflictSummary {
    commit: short.get(&info.commit.hash).to_string(),
    subject: info.commit.message.lines().next().unwrap_or_default().to_string(),
    files: info.conflicting_files.iter().map(|file| file.file.clone()).collect(),
    missing_commits: info.conflict_analysis.missing_commits.iter().map(|commit| short.get(&commit.hash).to_string()).collect(),
  }
}
