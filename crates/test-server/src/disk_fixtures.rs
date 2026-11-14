//! The disk of the lease pools of a test repository (the Disk window, openspec/specs/repository/disk-usage/spec.md): its pool
//! lives in the fake home of the repository (`AppHost::tool_environment` of `TestHost`), where a test adds free slots
//! by real gets and returns (`POST /repositories/{id}/lease-slots`), so `disk_usage` and `trim_pools` run their real
//! cores; the hosts of `bd-host` are the fixture of the test (`POST /repositories/{id}/host-disks`,
//! `AppHost::host_disks`), as no test reaches another machine. A test that opens a slot of the pool as its repository
//! leases one (`POST /repositories/{id}/lease-slot`), then returns it and leases it again (`{"again": true}`): a new
//! epoch of the same path (`lease_pool::epoch::worktree_epoch`), which the page then opens.

use crate::state::AppState;
use app_api::app::disk::HostDisk;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{RepositoryDirs, repository_dirs};
use lease_pool::lease::{LeaseGetParams, LeaseReturnParams, lease_get_core, lease_return_core};
use lease_pool::lease_clean::CleanOverrides;
use pool_disk::report::DiskReport;
use serde::Deserialize;
use std::sync::{Arc, PoisonError};
use std::time::Duration;
use sync_utils::span::in_current_span;

/// A host of `bd-host` as the test has it: its report, or why it has none
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostDiskFixture {
  pub alias: String,
  /// As `bd-host disk --json` carries it (`pool_disk::report::DiskReport`)
  #[serde(default)]
  pub report: Option<DiskReport>,
  #[serde(default)]
  pub error: Option<String>,
}

impl HostDiskFixture {
  pub fn host_disk(&self) -> HostDisk {
    HostDisk {
      alias: self.alias.clone(),
      report: match (&self.report, &self.error) {
        (Some(report), _) => Ok(report.clone()),
        (None, error) => Err(error.clone().unwrap_or_else(|| "the host answered no report".to_owned())),
      },
    }
  }
}

/// The hosts from now on
pub async fn set_host_disks(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(hosts): Json<Vec<HostDiskFixture>>) -> Result<StatusCode, StatusCode> {
  let repository = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  *repository.host_disks.write().unwrap_or_else(PoisonError::into_inner) = hosts;
  Ok(StatusCode::OK)
}

/// How many free slots to add
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeaseSlotsFixture {
  pub count: u32,
}

/// `count` free slots in the pool of the repository in its fake home: that many gets held at once, then their returns,
/// the first returned first; answers their paths in that order
pub async fn add_lease_slots(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<LeaseSlotsFixture>) -> Result<Json<Vec<String>>, StatusCode> {
  let (path, home) = pool_of(&state, &id)?;
  let git = state.git_executor.clone();
  let added = tokio::task::spawn_blocking(in_current_span(move || -> anyhow::Result<Vec<String>> {
    let dirs = repository_dirs(&git, &path)?.ok_or_else(|| anyhow::anyhow!("{path} is no repository"))?;
    let mut slots = Vec::new();
    for n in 0..fixture.count {
      slots.push(get_slot(&git, &dirs, home.as_deref(), format!("fixture-{n}"), None)?);
    }
    for (slot, lease_id) in &slots {
      return_slot(&git, slot, Some(lease_id))?;
    }
    Ok(slots.into_iter().map(|(slot, _)| slot).collect())
  }))
  .await
  .map_err(|error| {
    tracing::warn!(error = %error, "the fixture of the lease slots panicked");
    StatusCode::INTERNAL_SERVER_ERROR
  })?
  .map_err(|error| {
    tracing::warn!(error = %format!("{error:#}"), "cannot add the lease slots of the fixture");
    StatusCode::INTERNAL_SERVER_ERROR
  })?;
  Ok(Json(added))
}

/// The lease of a slot of the pool of the repository that a test opens as its repository: the first one, or the slot
/// returned and leased again (`again`), the same path under a new lease
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeaseSlotFixture {
  #[serde(default)]
  pub again: bool,
}

/// Leases a slot of the pool of the repository in its fake home, as an agent's `brd lease get` does, and names the slot
/// as the repository (`AppState::path_to_id`) and as the selected project of the app, so its page and its session are of
/// the slot; `again` returns the lease of that slot and leases it again, which takes the same slot, the free one returned
/// last. Answers the path of the slot and the id of its lease
pub async fn lease_slot(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<LeaseSlotFixture>) -> Result<Json<serde_json::Value>, StatusCode> {
  let (path, home) = pool_of(&state, &id)?;
  let leased: Vec<String> = state
    .path_to_id
    .iter()
    .filter(|entry| entry.value() == &id && entry.key() != &path)
    .map(|entry| entry.key().clone())
    .collect();
  let git = state.git_executor.clone();
  let got = tokio::task::spawn_blocking(in_current_span(move || -> anyhow::Result<(String, String)> {
    let dirs = repository_dirs(&git, &path)?.ok_or_else(|| anyhow::anyhow!("{path} is no repository"))?;
    if fixture.again {
      let slot = leased.first().ok_or_else(|| anyhow::anyhow!("no slot of {path} is leased"))?;
      return_slot(&git, slot, None)?;
    }
    // a branch of its own, as an agent leases one: a sync refuses a detached HEAD
    let branch = if fixture.again { "e2e-lease-again" } else { "e2e-lease" };
    get_slot(&git, &dirs, home.as_deref(), "e2e".to_owned(), Some(branch.to_owned()))
  }))
  .await
  .map_err(|error| {
    tracing::warn!(error = %error, "the fixture of the lease of a slot panicked");
    StatusCode::INTERNAL_SERVER_ERROR
  })?
  .map_err(|error| {
    tracing::warn!(error = %format!("{error:#}"), "cannot lease a slot of the fixture");
    StatusCode::INTERNAL_SERVER_ERROR
  })?;
  let (slot, lease_id) = got;
  // the window opens the slot: the selected project of the app
  let store = Arc::clone(&state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?.store);
  crate::set_fixture_value(&store, "recentProjects", serde_json::json!([{ "path": slot }]))?;
  state.path_to_id.insert(slot.clone(), id);
  Ok(Json(serde_json::json!({ "path": slot, "leaseId": lease_id })))
}

/// `brd lease get` of the repository of `dirs` by `holder` in the pool of `home`, on `branch` or detached: the path of
/// the slot and the id of its lease
fn get_slot(git: &GitCommandExecutor, dirs: &RepositoryDirs, home: Option<&std::path::Path>, holder: String, branch: Option<String>) -> anyhow::Result<(String, String)> {
  let got = lease_get_core(
    git,
    &LeaseGetParams {
      source: dirs,
      home,
      holder: Some(holder),
      branch,
      min_free: Some(0),
      server_timeout: Duration::from_secs(5),
      bazel: None,
      lock_timeout: Duration::from_secs(10),
      progress: None,
      space: None,
    },
  )?;
  let lease_id = got.slot.lease.as_ref().map_or_else(String::new, |lease| lease.id.clone());
  Ok((got.slot.path, lease_id))
}

/// `brd lease return` of the slot at `slot`, of the lease `lease_id` (the lease the slot holds when none, its work
/// discarded): the slot is free again
fn return_slot(git: &GitCommandExecutor, slot: &str, lease_id: Option<&str>) -> anyhow::Result<()> {
  let workspace = repository_dirs(git, slot)?.ok_or_else(|| anyhow::anyhow!("{slot} is no worktree"))?;
  lease_return_core(
    git,
    &LeaseReturnParams {
      workspace: &workspace,
      lease_id,
      find_by_lease_id: false,
      confirm_preserved: lease_id.is_none(),
      server_timeout: Duration::from_secs(5),
      bazel: None,
      clean: Some(CleanOverrides::default()),
      lock_timeout: Duration::from_secs(10),
    },
  )?;
  Ok(())
}

/// The path of the repository `id` and the fake home its pool lives in
fn pool_of(state: &AppState, id: &str) -> Result<(String, Option<std::path::PathBuf>), StatusCode> {
  let repository = state.repositories.get(id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  Ok((repository.path.clone(), repository.cli_tools.environment().home))
}
