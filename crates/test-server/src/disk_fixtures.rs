//! The disk of the lease pools of a test repository (the Disk window, spec/repository/disk-usage.spec.md): its pool
//! lives in the fake home of the repository (`AppHost::tool_environment` of `TestHost`), where a test adds free slots
//! by real gets and returns (`POST /repositories/{id}/lease-slots`), so `disk_usage` and `trim_pools` run their real
//! cores; the hosts of `bd-host` are the fixture of the test (`POST /repositories/{id}/host-disks`,
//! `AppHost::host_disks`), as no test reaches another machine.

use crate::state::AppState;
use app_api::app::disk::HostDisk;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use git_ops::revs::repository_dirs;
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
  let (path, home) = {
    let repository = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    (repository.path.clone(), repository.cli_tools.environment().home)
  };
  let git = state.git_executor.clone();
  let added = tokio::task::spawn_blocking(in_current_span(move || -> anyhow::Result<Vec<String>> {
    let dirs = repository_dirs(&git, &path)?.ok_or_else(|| anyhow::anyhow!("{path} is no repository"))?;
    let mut slots = Vec::new();
    for n in 0..fixture.count {
      let got = lease_get_core(
        &git,
        &LeaseGetParams {
          source: &dirs,
          home: home.as_deref(),
          holder: Some(format!("fixture-{n}")),
          branch: None,
          min_free: Some(0),
          server_timeout: Duration::from_secs(5),
          bazel: None,
          lock_timeout: Duration::from_secs(10),
          progress: None,
          available: None,
        },
      )?;
      let lease_id = got.slot.lease.as_ref().map_or_else(String::new, |lease| lease.id.clone());
      slots.push((got.slot.path, lease_id));
    }
    for (slot, lease_id) in &slots {
      let workspace = repository_dirs(&git, slot)?.ok_or_else(|| anyhow::anyhow!("{slot} is no worktree"))?;
      lease_return_core(
        &git,
        &LeaseReturnParams {
          workspace: &workspace,
          lease_id: Some(lease_id),
          find_by_lease_id: false,
          confirm_preserved: false,
          server_timeout: Duration::from_secs(5),
          bazel: None,
          clean: Some(CleanOverrides::default()),
          lock_timeout: Duration::from_secs(10),
        },
      )?;
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
