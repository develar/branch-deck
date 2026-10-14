//! `bd sync`: `sync_core::sync::sync_branches` under the repository lock, its events collected into one compact
//! summary. It rebuilds the virtual branches, runs the integration detection of archived branches and the update check
//! of the checked out branch, as the app's sync does

use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::output::{SchemaVersion, ShortIds};
use git_ops::model::{BranchError, BranchSyncStatus, to_final_branch_name};
use git_ops::update_branch::{UpdateKind, UpdateState};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use sync_core::repository_lock::{RepositoryLocked, lock_repository};
use sync_core::sync::{SyncOptions, sync_branches};
use sync_types::branch_integration::BranchIntegrationStatus;
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
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub archived: Vec<ArchivedBranch>,
  /// Whether the checked out branch can be updated onto the last fetched baseline; omitted for a local baseline
  #[serde(skip_serializing_if = "Option::is_none")]
  pub update: Option<UpdateSummary>,
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
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub blocked: Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub remote: Option<RemoteSummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ConflictSummary {
  /// The commit whose cherry-pick conflicts
  pub commit: String,
  pub subject: String,
  pub files: Vec<String>,
  /// Commits of other branches that the conflicting change likely needs
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub missing_commits: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RemoteSummary {
  pub exists: bool,
  /// Commits of the virtual branch the remote doesn't have: all of them while there is no remote branch
  pub unpushed: usize,
  /// Commits the remote has that the virtual branch doesn't
  pub behind: u32,
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
  /// The files a conflicting update touches, or the local changes that block it
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub files: Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
}

/// Collects the events of a sync
#[derive(Clone, Default)]
struct CollectingReporter {
  events: Arc<Mutex<Vec<SyncEvent>>>,
}

impl ProgressReporter for CollectingReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    self.events.lock().map_err(|_| anyhow::anyhow!("the event collector is poisoned"))?.push(event);
    Ok(())
  }
}

pub fn sync(context: &RepoContext) -> Result<SyncOutput, CliError> {
  let _lock = lock_repository(&context.git, &context.repository, context.lock_timeout).map_err(|e| match e.downcast_ref::<RepositoryLocked>() {
    Some(locked) => CliError::new(ErrorCode::Locked, locked.to_string()),
    None => CliError::from(e),
  })?;
  let reporter = CollectingReporter::default();
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .map_err(|e| CliError::new(ErrorCode::GitError, format!("Cannot start the sync runtime: {e}")))?;
  runtime.block_on(sync_branches(&context.git, &context.repository, &context.prefix, reporter.clone(), SyncOptions::default()))?;
  let events = std::mem::take(&mut *reporter.events.lock().map_err(|_| CliError::new(ErrorCode::GitError, "the event collector is poisoned"))?);
  Ok(summarize(context, events))
}

/// One summary of the events, in the order they came
fn summarize(context: &RepoContext, events: Vec<SyncEvent>) -> SyncOutput {
  let mut baseline = String::new();
  let mut grouped = Vec::new();
  let mut unassigned_count = 0;
  let mut statuses: HashMap<String, (BranchSyncStatus, Option<BranchError>)> = HashMap::new();
  let mut tips: HashMap<String, String> = HashMap::new();
  let mut blocked: HashMap<String, Vec<String>> = HashMap::new();
  let mut remotes: HashMap<String, RemoteStatusUpdate> = HashMap::new();
  let mut archived: BTreeMap<String, ArchivedBranch> = BTreeMap::new();
  let mut update = None;
  for event in events {
    match event {
      SyncEvent::BranchesGrouped { branches, baseline_branch, .. } => {
        baseline = baseline_branch;
        grouped = branches.into_iter().map(|branch| (branch.name, branch.commits.len())).collect();
      }
      SyncEvent::UnassignedCommits { commits } => unassigned_count = commits.len(),
      // the newest commit comes last; its new hash is the tip
      SyncEvent::CommitSynced { branch_name, new_hash, .. } => {
        tips.insert(branch_name, new_hash);
      }
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
      SyncEvent::BaselineStatus { update: check, check_error, .. } => {
        update = match (check, check_error) {
          (Some(check), _) => {
            let (state, files) = match check.state {
              UpdateState::UpToDate => ("up_to_date", Vec::new()),
              UpdateState::Clean { kind: UpdateKind::FastForward } => ("fast_forward", Vec::new()),
              UpdateState::Clean { kind: UpdateKind::Rebase } => ("rebase", Vec::new()),
              UpdateState::Conflict { files, .. } => ("conflict", files),
              UpdateState::Blocked { paths } => ("blocked", paths),
            };
            Some(UpdateSummary {
              upstream: Some(check.upstream),
              state: state.to_string(),
              files,
              error: None,
            })
          }
          (None, Some(error)) => Some(UpdateSummary {
            upstream: None,
            state: "error".to_string(),
            files: Vec::new(),
            error: Some(error),
          }),
          (None, None) => None,
        };
      }
      SyncEvent::IssueNavigationConfig { .. } | SyncEvent::CommitError { .. } | SyncEvent::ArchivedBranchesFound { .. } => {}
    }
  }

  let mut ids: Vec<&str> = tips.values().map(String::as_str).collect();
  ids.extend(blocked.values().flatten().map(String::as_str));
  let conflicts: Vec<&str> = statuses
    .values()
    .filter_map(|(_, error)| match error {
      Some(BranchError::MergeConflict(info)) => Some(info.commit.hash.as_str()),
      _ => None,
    })
    .collect();
  ids.extend(conflicts);
  let short = context.short_ids(ids);

  let branches = grouped
    .into_iter()
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
          .then(|| tips.get(&name).map(|tip| short.get(tip).to_string()))
          .flatten(),
        reference,
        status,
        commit_count,
        error,
        conflict,
        blocked: blocked.remove(&name).unwrap_or_default().iter().map(|id| short.get(id).to_string()).collect(),
        remote: remotes.remove(&name).map(|remote| RemoteSummary {
          exists: remote.remote_exists,
          // without a remote branch every commit is unpushed
          unpushed: if remote.remote_exists { remote.unpushed_commits.len() } else { commit_count },
          behind: remote.commits_behind,
        }),
        name,
      }
    })
    .collect();

  SyncOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    baseline,
    branches,
    unassigned_count,
    archived: archived.into_values().collect(),
    update,
  }
}

fn conflict_summary(info: &git_ops::model::MergeConflictInfo, short: &ShortIds) -> ConflictSummary {
  ConflictSummary {
    commit: short.get(&info.commit.hash).to_string(),
    subject: info.commit.message.lines().next().unwrap_or_default().to_string(),
    files: info.conflicting_files.iter().map(|file| file.file.clone()).collect(),
    missing_commits: info.conflict_analysis.missing_commits.iter().map(|commit| short.get(&commit.hash).to_string()).collect(),
  }
}
