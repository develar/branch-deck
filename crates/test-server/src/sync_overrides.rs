//! What a test changes of the next sync of its repository (`POST /repositories/{id}/sync-overrides`,
//! docs/design/e2e-testing.md): what the server can't compute from a template, which has no remote of its own (the
//! remote statuses of the branches and the stack branches, as a push or a pull request left them) and the issue links of
//! a repository of a YouTrack user. The next run of the repository's session takes them
//! (`TestSessionEnvironment::reporter` puts `OverridingReporter` around the run's reporter), so the table of the
//! session, its patches to the page and the table of the views are what a sync of such a repository makes; the runs
//! after it are the server's as they are, unless `every` keeps the overrides for each of them. `GET /repositories/{id}/revision?name=<rev>` answers the commit a ref of the
//! repository is at, which such a test compares with what the page shows (the copy of a commit the sync built).
//! `POST /repositories/{id}/rebase-in-progress` (`{"inProgress": bool}`) leaves the repository as an interrupted rebase
//! does (`.git/rebase-merge`), so its syncs are deferred (`SyncDeferred`) by the real check, until it is set back.
//! `POST /repositories/{id}/branch-prefix` (`{"prefix": "team"}`) sets `branchdeck.branchPrefix` of `.git/config` as
//! `git config` in a terminal does: no command of the app was asked, so the server's prefixes still name the old one

use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use domain_policy::push;
use domain_types::issue_navigation::IssueNavigationConfig;
use domain_types::{ProgressReporter, PushAction, RemoteStatusUpdate, SyncEvent};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Arc;
use sync_utils::span::in_current_span;

/// The overrides of the next sync; a branch or a root not named is reported as the sync computed it
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncOverrides {
  /// Branch -> the fields of its `RemoteStatusUpdate` that replace the computed ones; null: the sync reports no remote
  /// status of the branch, which keeps the one it had (none before its first)
  #[serde(default)]
  remote: HashMap<String, Option<RemoteOverride>>,
  /// Stack root -> the fields of its `StackBranchUpdate` that replace the reported ones (`remote` as a whole)
  #[serde(default)]
  stacks: HashMap<String, Map<String, Value>>,
  /// The issue links the sync reports instead of the repository's (`.idea/vcs.xml`)
  #[serde(default)]
  issue_links: Option<IssueNavigationConfig>,
  /// Every run from now on takes them, not only the next: a test of several starts, whatever syncs run between them,
  /// until a test sets others
  #[serde(default)]
  pub every: bool,
}

/// The remote status of a branch as a test sets it
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteOverride {
  /// Only the newest `n` of the commits the sync found the remote lacking stay unpushed: the remote has the others, ids
  /// a test can't know before the sync built the copies
  keep_unpushed: Option<usize>,
  #[serde(flatten)]
  fields: Map<String, Value>,
}

impl SyncOverrides {
  /// `event` as the overrides change it; `None` leaves it out
  fn apply(&self, event: SyncEvent) -> anyhow::Result<Option<SyncEvent>> {
    Ok(Some(match event {
      SyncEvent::RemoteStatusUpdate(status) => match self.remote.get(&status.branch_name) {
        None => SyncEvent::RemoteStatusUpdate(status),
        Some(None) => return Ok(None),
        Some(Some(remote)) => {
          let mut status = status;
          if let Some(keep) = remote.keep_unpushed {
            status.unpushed_commits.truncate(keep);
          }
          let mut status = with_fields(&status, &remote.fields)?;
          // the counts are the test's, the action is the rule's on them, as the app's sync derives it
          status.action = push::action(&status);
          SyncEvent::RemoteStatusUpdate(status)
        }
      },
      SyncEvent::StackBranchUpdate(update) => match self.stacks.get(&update.root) {
        None => SyncEvent::StackBranchUpdate(update),
        Some(fields) => {
          let mut fields = fields.clone();
          if let Some(remote) = fields.get_mut("remote").filter(|remote| remote.is_object()) {
            *remote = serde_json::to_value(test_remote_status(remote.take())?)?;
          }
          SyncEvent::StackBranchUpdate(Box::new(with_fields(&*update, &fields)?))
        }
      },
      SyncEvent::IssueNavigationConfig { config } => SyncEvent::IssueNavigationConfig {
        config: self.issue_links.clone().or(config),
      },
      event => event,
    }))
  }
}

/// A remote status as a test writes it, its counts without the `action`, which the server derives from them as the app's
/// sync does (`domain_policy::push::action`): a test states what the remote has, never the rule's answer
pub fn test_remote_status(mut json: Value) -> anyhow::Result<RemoteStatusUpdate> {
  let object = json.as_object_mut().ok_or_else(|| anyhow::anyhow!("A remote status is no JSON object"))?;
  object.insert("action".to_string(), serde_json::to_value(PushAction::UpToDate)?);
  let mut status: RemoteStatusUpdate = serde_json::from_value(json)?;
  status.action = push::action(&status);
  Ok(status)
}

/// `value` with `fields` in place of its own, as the JSON of the app names them
fn with_fields<T: serde::Serialize + DeserializeOwned>(value: &T, fields: &Map<String, Value>) -> anyhow::Result<T> {
  let mut json = serde_json::to_value(value)?;
  let object = json.as_object_mut().ok_or_else(|| anyhow::anyhow!("An event of the sync is no JSON object"))?;
  object.extend(fields.iter().map(|(name, value)| (name.clone(), value.clone())));
  Ok(serde_json::from_value(json)?)
}

/// The reporter of a run that took the overrides of its repository: each event as they change it, to the run's own
pub struct OverridingReporter {
  pub inner: Arc<dyn ProgressReporter>,
  pub overrides: SyncOverrides,
}

impl ProgressReporter for OverridingReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    match self.overrides.apply(event)? {
      Some(event) => self.inner.send(event),
      None => Ok(()),
    }
  }
}

/// Sets the overrides the next sync of the repository takes (each one, with `every`), replacing the ones it hasn't taken
/// yet
pub async fn set_sync_overrides(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(overrides): Json<SyncOverrides>) -> StatusCode {
  let Some(repository) = state.repositories.get(&id) else {
    return StatusCode::UNPROCESSABLE_ENTITY;
  };
  *repository.sync_overrides.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(overrides);
  StatusCode::OK
}

#[derive(Deserialize)]
pub struct RevisionQuery {
  name: String,
}

/// The commit `name` (a branch, `user-name/virtual/<group>`) of the repository is at: `{"id": <sha>}`
pub async fn get_revision(Path(id): Path<String>, Query(query): Query<RevisionQuery>, State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
  let path = state
    .repositories
    .get(&id)
    .map(|repository| repository.path.clone())
    .ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let git = state.git_executor.clone();
  let resolved = tokio::task::spawn_blocking(in_current_span(move || git_ops::revs::resolve_commit(&git, &path, &query.name)))
    .await
    .map_err(anyhow::Error::from)
    .and_then(|resolved| resolved);
  match resolved {
    Ok(Some(commit)) => Ok(Json(serde_json::json!({ "id": commit }))),
    Ok(None) => Err(StatusCode::NOT_FOUND),
    Err(error) => {
      tracing::warn!("Cannot resolve the revision: {error:#}");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebaseRequest {
  in_progress: bool,
}

#[derive(Deserialize)]
pub struct BranchPrefixRequest {
  prefix: String,
}

/// Sets `branchdeck.branchPrefix` in `.git/config` of the repository behind the app's back
pub async fn set_branch_prefix_in_terminal(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(request): Json<BranchPrefixRequest>) -> StatusCode {
  let Some(path) = state.repositories.get(&id).map(|repository| repository.path.clone()) else {
    return StatusCode::UNPROCESSABLE_ENTITY;
  };
  let git = state.git_executor.clone();
  let written = tokio::task::spawn_blocking(in_current_span(move || {
    git.execute_command(&["config", "--local", "branchdeck.branchPrefix", &request.prefix], &path)
  }))
  .await;
  match written {
    Ok(Ok(_)) => StatusCode::OK,
    Ok(Err(error)) => {
      tracing::warn!("Cannot set the branch prefix of the repository: {error:#}");
      StatusCode::INTERNAL_SERVER_ERROR
    }
    Err(error) => {
      tracing::warn!("The write of the branch prefix failed: {error}");
      StatusCode::INTERNAL_SERVER_ERROR
    }
  }
}

/// Creates or removes `.git/rebase-merge` of the repository: git's mark of a rebase it stopped in
pub async fn set_rebase_in_progress(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(request): Json<RebaseRequest>) -> StatusCode {
  let Some(path) = state.repositories.get(&id).map(|repository| repository.path.clone()) else {
    return StatusCode::UNPROCESSABLE_ENTITY;
  };
  let marker = std::path::Path::new(&path).join(".git/rebase-merge");
  let changed = if request.in_progress {
    std::fs::create_dir_all(&marker)
  } else if marker.exists() {
    sync_utils::fs_retry::remove_dir_all(&marker)
  } else {
    Ok(())
  };
  match changed {
    Ok(()) => StatusCode::OK,
    Err(error) => {
      tracing::warn!("Cannot change the rebase of the repository: {error}");
      StatusCode::INTERNAL_SERVER_ERROR
    }
  }
}
