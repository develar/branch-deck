//! `bd sync`: `sync_core::sync::sync_branches` under the repository lock, its events collected into one compact
//! summary. It rebuilds the virtual branches, runs the integration detection of archived branches and the update check
//! of the checked out branch, as the app's sync does

use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::install::shell_word;
use crate::output::{SchemaVersion, ShortIds};
use git_ops::model::{BranchError, BranchSyncStatus, to_final_branch_name};
use git_ops::update_branch::{UpdateKind, UpdateState};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use sync_core::repository_lock::{RepositoryLock, RepositoryLocked, lock_repository};
use sync_core::sync::{SyncOptions, SyncReport, sync_branches};
use sync_types::branch_integration::BranchIntegrationStatus;
use sync_types::landed::LandedBranch;
use sync_types::{ProgressReporter, RemoteStatusUpdate, SyncEvent};

/// `bd sync`
#[derive(Debug, Serialize, JsonSchema)]
pub struct SyncOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub baseline: String,
  /// In the order of `bd status`
  pub branches: Vec<SyncedBranch>,
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
}

impl SyncOutput {
  pub fn has_conflicts(&self) -> bool {
    self.branches.iter().any(|branch| branch.status == SyncState::MergeConflict)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
  Created,
  Updated,
  Unchanged,
  Error,
  MergeConflict,
  /// A stacked branch whose parent can't be built on: not rebuilt this sync
  Blocked,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SyncedBranch {
  pub name: String,
  #[serde(rename = "ref")]
  pub reference: String,
  pub status: SyncState,
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
  #[serde(skip_serializing_if = "Option::is_none")]
  pub remote: Option<RemoteSummary>,
  /// The `bd push` that brings the remote branch up to date, to run only after the user approves: with
  /// `--force --expect-remote <remote.head>` when the push drops `dropped_commits`, with `--force` alone when a parent
  /// it takes along has commits for the remote too; omitted when the remote branch is up to date or the branch didn't
  /// sync
  #[serde(skip_serializing_if = "Option::is_none")]
  pub push: Option<String>,
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
  /// commit the branch has is not one of them (`amended`, `restacked`)
  pub behind: u32,
  /// Remote commits that are older versions of commits amended here (same author, author time and subject): a push
  /// replaces them without --force
  #[serde(default, skip_serializing_if = "is_zero")]
  pub amended: u32,
  /// Remote commits that are older versions of commits of the branches a stacked branch is built on, amended or rebuilt
  /// since it was pushed: a push replaces them without --force
  #[serde(default, skip_serializing_if = "is_zero")]
  pub restacked: u32,
  /// The remote-tracking commit the counts compare with; `null` without a remote branch. `bd push --force` leases on
  /// it, as `--expect-remote` does
  pub head: Option<String>,
  /// The newest of the `behind` commits, newest first, at most ten
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub dropped_commits: Vec<DroppedCommitSummary>,
  /// `head` in full, for the lease
  #[serde(skip)]
  pub head_id: Option<String>,
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature serde's skip_serializing_if needs")]
pub(crate) const fn is_zero(value: &u32) -> bool {
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

impl RemoteSummary {
  /// `commit_count`: the commits of the branch, all of them unpushed without a remote branch
  pub fn new(remote: &RemoteStatusUpdate, commit_count: usize, short: &ShortIds) -> Self {
    Self {
      exists: remote.remote_exists,
      unpushed: if remote.remote_exists { remote.unpushed_commits.len() } else { commit_count },
      behind: remote.commits_behind,
      amended: remote.commits_amended,
      restacked: remote.commits_restacked,
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
      head_id: remote.remote_tip.clone(),
    }
  }

  /// The ids `new` shortens, for `RepoContext::short_ids`
  pub fn ids(remote: &RemoteStatusUpdate) -> impl Iterator<Item = &str> {
    remote
      .remote_tip
      .as_deref()
      .into_iter()
      .chain(remote.dropped_commits.iter().map(|commit| commit.id.as_str()))
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ArchivedBranch {
  pub name: String,
  /// `integrated`, `not_integrated` or `partial`
  pub status: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub integrated_at: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateSummary {
  pub upstream: Option<String>,
  /// `up_to_date`, `fast_forward`, `rebase`, `conflict` or `blocked`; `error` when the check failed
  pub state: String,
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

/// Collects the events of a sync
#[derive(Clone, Default)]
struct CollectingReporter {
  events: Arc<Mutex<Vec<SyncEvent>>>,
}

impl ProgressReporter for CollectingReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    self.events.lock().map_err(|e| anyhow::anyhow!("the event collector is poisoned: {e}"))?.push(event);
    Ok(())
  }
}

pub fn sync(context: &RepoContext) -> Result<SyncOutput, CliError> {
  let _lock = lock(context)?;
  sync_locked(context)
}

/// The repository lock, `locked` after the timeout
pub fn lock(context: &RepoContext) -> Result<RepositoryLock, CliError> {
  lock_repository(&context.git, &context.repository, context.lock_timeout).map_err(|e| match e.downcast_ref::<RepositoryLocked>() {
    Some(locked) => CliError::new(ErrorCode::Locked, locked.to_string()),
    None => CliError::from(e),
  })
}

/// `bd sync` for a caller that holds the repository lock
pub fn sync_locked(context: &RepoContext) -> Result<SyncOutput, CliError> {
  let reporter = CollectingReporter::default();
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .map_err(|e| CliError::new(ErrorCode::GitError, format!("Cannot start the sync runtime: {e}")))?;
  // `--no-cache` trusts no mapping note either: each is checked against the tree of its virtual commit
  let options = SyncOptions {
    verify_mapping_notes: !context.use_cache,
    ..SyncOptions::default()
  };
  let report = runtime.block_on(sync_branches(&context.git, &context.repository, &context.prefix, reporter.clone(), options))?;
  let events = std::mem::take(
    &mut *reporter
      .events
      .lock()
      .map_err(|e| CliError::new(ErrorCode::GitError, format!("the event collector is poisoned: {e}")))?,
  );
  Ok(summarize(context, report, events))
}

/// One summary of the report and of the events, in the order they came
fn summarize(context: &RepoContext, report: SyncReport, events: Vec<SyncEvent>) -> SyncOutput {
  // branch -> the branch it is built on
  let bases: HashMap<String, String> = report
    .branches
    .iter()
    .filter_map(|branch| Some((branch.name.clone(), branch.stack.as_ref()?.effective_base.clone()?)))
    .collect();
  let tip_of = |name: &str| report.tips.get(&to_final_branch_name(&context.prefix, name).ok()?);
  let mut unassigned_count = 0;
  let mut statuses: HashMap<String, (BranchSyncStatus, Option<BranchError>)> = HashMap::new();
  let mut blocked: HashMap<String, Vec<String>> = HashMap::new();
  let mut remotes: HashMap<String, RemoteStatusUpdate> = HashMap::new();
  let mut archived: BTreeMap<String, ArchivedBranch> = BTreeMap::new();
  let mut update = None;
  let (mut landing, mut landed) = (Vec::new(), Vec::new());
  for event in events {
    match event {
      SyncEvent::UnassignedCommits { commits } => unassigned_count = commits.len(),
      SyncEvent::CommitsBlocked {
        branch_name,
        blocked_commit_hashes,
      } => blocked.entry(branch_name).or_default().extend(blocked_commit_hashes),
      SyncEvent::BranchStatusUpdate { branch_name, status, error } => {
        statuses.insert(branch_name, (status, error));
      }
      SyncEvent::RemoteStatusUpdate(remote) => {
        remotes.insert(remote.branch_name.clone(), remote);
      }
      SyncEvent::BranchIntegrationDetected { info } => {
        let (status, integrated_at) = match info.status {
          BranchIntegrationStatus::Integrated { integrated_at, .. } => ("integrated", integrated_at),
          BranchIntegrationStatus::NotIntegrated { integrated_at, .. } => ("not_integrated", integrated_at),
          BranchIntegrationStatus::Partial { .. } => ("partial", None),
        };
        archived.insert(
          info.name.clone(),
          ArchivedBranch {
            name: info.name,
            status: status.to_string(),
            integrated_at,
          },
        );
      }
      SyncEvent::BaselineStatus {
        update: check,
        check_error,
        landing: incoming,
        landed: report,
        ..
      } => {
        landing = incoming;
        landed = report;
        update = match (check, check_error) {
          (Some(check), _) => {
            let (state, files, count) = match check.state {
              UpdateState::UpToDate => ("up_to_date", Vec::new(), 0),
              UpdateState::Clean { kind: UpdateKind::FastForward } => ("fast_forward", Vec::new(), 0),
              UpdateState::Clean { kind: UpdateKind::Rebase } => ("rebase", Vec::new(), 0),
              UpdateState::Conflict { files, file_count, .. } => ("conflict", files, file_count),
              UpdateState::Blocked { paths, path_count } => ("blocked", paths, path_count),
            };
            Some(UpdateSummary {
              upstream: Some(check.upstream),
              state: state.to_string(),
              files_omitted: count.saturating_sub(u32::try_from(files.len()).unwrap_or(u32::MAX)),
              files,
              error: None,
              landing: Vec::new(),
            })
          }
          (None, Some(error)) => Some(UpdateSummary {
            upstream: None,
            state: "error".to_string(),
            files: Vec::new(),
            files_omitted: 0,
            error: Some(error),
            landing: Vec::new(),
          }),
          (None, None) => None,
        };
      }
      SyncEvent::BranchesGrouped { .. }
      | SyncEvent::CommitSynced { .. }
      | SyncEvent::IssueNavigationConfig { .. }
      | SyncEvent::CommitError { .. }
      | SyncEvent::ArchivedBranchesFound { .. } => {}
    }
  }

  let mut ids: Vec<&str> = report.tips.values().map(String::as_str).collect();
  ids.extend(blocked.values().flatten().map(String::as_str));
  let conflicts: Vec<&str> = statuses
    .values()
    .filter_map(|(_, error)| match error {
      Some(BranchError::MergeConflict(info)) => Some(info.commit.hash.as_str()),
      _ => None,
    })
    .collect();
  ids.extend(conflicts);
  ids.extend(remotes.values().flat_map(RemoteSummary::ids));
  ids.extend(landed_ids(&landing));
  ids.extend(landed_ids(&landed));
  let short = context.short_ids(ids);
  if let Some(update) = &mut update {
    update.landing = landed_summary(std::mem::take(&mut landing), &short);
  }
  let landed = landed_summary(landed, &short);

  let branches: Vec<SyncedBranch> = report
    .branches
    .iter()
    .map(|branch| (branch.name.clone(), branch.commits.len()))
    .map(|(name, commit_count)| {
      let (status, error) = statuses.remove(&name).unwrap_or((BranchSyncStatus::Unchanged, None));
      let status = match status {
        BranchSyncStatus::Created => SyncState::Created,
        BranchSyncStatus::Updated => SyncState::Updated,
        BranchSyncStatus::Unchanged => SyncState::Unchanged,
        BranchSyncStatus::Error => SyncState::Error,
        BranchSyncStatus::Blocked => SyncState::Blocked,
        BranchSyncStatus::MergeConflict | BranchSyncStatus::AnalyzingConflict => SyncState::MergeConflict,
      };
      let (error, conflict) = match error {
        Some(BranchError::Generic(message)) => (Some(message), None),
        Some(BranchError::MergeConflict(info)) => (None, Some(conflict_summary(&info, &short))),
        None => (None, None),
      };
      let reference = to_final_branch_name(&context.prefix, &name).map_or_else(|_| String::new(), |name| format!("refs/heads/{name}"));
      SyncedBranch {
        tip: matches!(status, SyncState::Created | SyncState::Updated | SyncState::Unchanged)
          .then(|| tip_of(&name).map(|tip| short.get(tip).to_string()))
          .flatten(),
        reference,
        status,
        commit_count,
        error,
        conflict,
        blocked: blocked.remove(&name).unwrap_or_default().iter().map(|id| short.get(id).to_string()).collect(),
        remote: remotes.remove(&name).map(|remote| RemoteSummary::new(&remote, commit_count, &short)),
        push: None,
        name,
      }
    })
    .collect();
  let branches = with_push_commands(branches, &bases, &context.bd);

  SyncOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    baseline: report.baseline.clone(),
    branches,
    unassigned_count,
    archived: archived.into_values().collect(),
    update,
    landed,
    report,
  }
}

/// Whether a push of the branch would change its remote branch
const fn has_work_for_remote(remote: &RemoteSummary) -> bool {
  !remote.exists || remote.unpushed > 0 || remote.behind > 0 || remote.amended > 0 || remote.restacked > 0
}

/// `branches` with the `bd push` each needs. `--expect-remote` pushes the named branch alone, so it pins the lease only
/// when no branch it is built on has commits for the remote
fn with_push_commands(mut branches: Vec<SyncedBranch>, bases: &HashMap<String, String>, bd: &str) -> Vec<SyncedBranch> {
  let work: HashMap<&str, bool> = branches
    .iter()
    .map(|branch| (branch.name.as_str(), branch.tip.is_some() && branch.remote.as_ref().is_some_and(has_work_for_remote)))
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

fn conflict_summary(info: &git_ops::model::MergeConflictInfo, short: &ShortIds) -> ConflictSummary {
  ConflictSummary {
    commit: short.get(&info.commit.hash).to_string(),
    subject: info.commit.message.lines().next().unwrap_or_default().to_string(),
    files: info.conflicting_files.iter().map(|file| file.file.clone()).collect(),
    missing_commits: info.conflict_analysis.missing_commits.iter().map(|commit| short.get(&commit.hash).to_string()).collect(),
  }
}
