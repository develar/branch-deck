//! The recent projects as Rust reads them without a window: the selected one, the first of `recentProjects`, which the
//! start of the app shows from the frame of its session before its window loads (`selected_project`), and whether the
//! syncs recorded one in `repositoryState` (`synced_before`), which the session of a project syncs by as it opens, and
//! the record of such a sync, by the session itself under the lease it opened under (`record_sync`).

use crate::file::{RECENT_PROJECTS_KEY, REPOSITORY_STATE_KEY};
use crate::service::{SYNC_TIME_FIELD, SettingsError, SettingsService};
use domain_types::settings::SettingsChange;
use domain_types::worktree_epoch::WorktreeEpoch;
use serde_json::Value;

/// The path of the selected project, the first of the recent projects; none without one
#[must_use]
pub fn selected_project(service: &SettingsService) -> Option<String> {
  let projects = service.value(RECENT_PROJECTS_KEY);
  projects.get(0)?.get("path")?.as_str().filter(|path| !path.is_empty()).map(str::to_owned)
}

/// Whether a sync of the repository at `path` ended before: its `lastSyncTime` in `repositoryState`
#[must_use]
pub fn synced_before(service: &SettingsService, path: &str) -> bool {
  service
    .value(REPOSITORY_STATE_KEY)
    .get(path)
    .and_then(|state| state.get(SYNC_TIME_FIELD))
    .is_some_and(Value::is_number)
}

/// The origin of a record of a sync by its session
pub const SESSION_ORIGIN: &str = "session";

/// Records that a sync of the session of the worktree at `path` ended now (`SYNC_TIME_FIELD`), the session opened
/// under the epoch `epoch` of that worktree: `None` when the worktree holds another known epoch now, which records
/// nothing (`SettingsService::set_repository_field`)
///
/// # Errors
/// As `SettingsService::set`
pub fn record_sync(service: &SettingsService, path: &str, epoch: &WorktreeEpoch, at_ms: u64) -> Result<Option<SettingsChange>, SettingsError> {
  service.set_repository_field(path, SYNC_TIME_FIELD, Value::from(at_ms), epoch, SESSION_ORIGIN)
}
