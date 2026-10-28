//! The recent projects as Rust reads them without a window: the selected one, the first of `recentProjects`, which the
//! start of the app shows from the frame of its session before its window loads (`selected_project`), and whether the
//! syncs recorded one in `repositoryState` (`synced_before`), which the session of a project syncs by as it opens.

use crate::file::{RECENT_PROJECTS_KEY, REPOSITORY_STATE_KEY};
use crate::service::SettingsService;
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
    .and_then(|state| state.get("lastSyncTime"))
    .is_some_and(Value::is_number)
}
