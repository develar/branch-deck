//! The finished robots the main window of an older version kept in `robotHistory`, by the path of their repository,
//! before the sessions kept them in the object cache of each repository (`repository_model::robot_history`): read once
//! (`legacy_robot_history`) and removed (`forget_robot_history`), which the first session that opens does
//! (`app_api::app::services`). The move is by robot id, so a second one adds nothing.

use crate::service::{SettingsError, SettingsService};
use domain_types::patronus::{FinishedRobot, SampleProfile};
use domain_types::settings::{SettingsChange, SettingsPatch};
use serde_json::Value;
use std::collections::BTreeMap;

/// The key the main window kept the history in
pub const ROBOT_HISTORY_KEY: &str = "robotHistory";

/// The robots of `robotHistory` by the path of their repository, each entry the page wrote (`id`, `repositoryPath`,
/// `profileId`: a string, `null` for no profile, absent while not known, `pushMode`, `targetBranch`, `startTime`,
/// `finishTime`, `status`); an entry another version wrote otherwise is left out. None without the key
#[must_use]
pub fn legacy_robot_history(service: &SettingsService) -> Option<BTreeMap<String, Vec<FinishedRobot>>> {
  let Value::Array(entries) = service.value(ROBOT_HISTORY_KEY) else {
    return None;
  };
  let mut by_repository: BTreeMap<String, Vec<FinishedRobot>> = BTreeMap::new();
  for entry in &entries {
    if let Some((repository, robot)) = legacy_entry(entry) {
      by_repository.entry(repository).or_default().push(robot);
    }
  }
  Some(by_repository)
}

/// The repository and the robot of an entry of `robotHistory`; `None` for one of another form
fn legacy_entry(entry: &Value) -> Option<(String, FinishedRobot)> {
  let text = |field: &str| entry.get(field)?.as_str().map(str::to_owned);
  let time = |field: &str| u32::try_from(entry.get(field)?.as_u64()?).ok();
  let profile = match entry.get("profileId") {
    None => SampleProfile::Unknown,
    Some(Value::Null) => SampleProfile::None,
    Some(Value::String(id)) => SampleProfile::Id(id.clone()),
    Some(_) => return None,
  };
  let robot = FinishedRobot {
    id: text("id")?,
    status: serde_json::from_value(entry.get("status")?.clone()).ok()?,
    push_mode: serde_json::from_value(entry.get("pushMode")?.clone()).ok()?,
    target_branch: text("targetBranch")?,
    start_time: time("startTime")?,
    finish_time: time("finishTime")?,
    profile,
  };
  Some((text("repositoryPath")?, robot))
}

/// Removes `robotHistory` once its robots are moved; the change for the windows, none without the key
///
/// # Errors
/// The writer refused the patch
pub fn forget_robot_history(service: &SettingsService) -> Result<Option<SettingsChange>, SettingsError> {
  if service.value(ROBOT_HISTORY_KEY).is_null() {
    return Ok(None);
  }
  let patch = SettingsPatch::Value {
    key: ROBOT_HISTORY_KEY.to_owned(),
    value: Value::Null,
  };
  service.set(patch, "session").map(Some)
}
