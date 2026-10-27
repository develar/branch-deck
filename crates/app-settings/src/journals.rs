//! The journals Activity reads, by the recent projects (spec/repository/activity.spec.md, "Log"): `journalPaths` keeps the
//! journal of the repository of each project by its path, written by Rust when a window opens the session of the
//! project (`remember_journal`, the host resolves the repository's `$GIT_COMMON_DIR` then), so a read of Activity runs
//! no git process (`activity_projects`). A `Fields` patch of one path changes no other, and no window writes the key.

use crate::file::{JOURNAL_PATHS_KEY, RECENT_PROJECTS_KEY};
use crate::service::{SettingsError, SettingsService};
use domain_types::settings::{SettingsChange, SettingsPatch};
use serde_json::{Map, Value};

/// The recent projects (`recentProjects`, most recent first) by their paths, each with the journal remembered for it;
/// one opened by no version that remembers it has none
#[must_use]
pub fn activity_projects(service: &SettingsService) -> Vec<(String, Option<String>)> {
  let (_, values) = service.snapshot();
  let journals = values.get(JOURNAL_PATHS_KEY).and_then(Value::as_object);
  let Some(Value::Array(projects)) = values.get(RECENT_PROJECTS_KEY) else {
    return Vec::new();
  };
  projects
    .iter()
    .filter_map(|project| project.get("path").and_then(Value::as_str))
    .map(|path| {
      let journal = journals.and_then(|journals| journals.get(path)).and_then(Value::as_str).map(str::to_owned);
      (path.to_owned(), journal)
    })
    .collect()
}

/// The journal `journal` of the repository of the project at `path`, remembered unless it is already; the change for
/// the windows when it was not
///
/// # Errors
/// The writer refused the patch
pub fn remember_journal(service: &SettingsService, path: &str, journal: &str) -> Result<Option<SettingsChange>, SettingsError> {
  let (_, values) = service.snapshot();
  let known = values.get(JOURNAL_PATHS_KEY).and_then(|journals| journals.get(path)).and_then(Value::as_str);
  if known == Some(journal) {
    return Ok(None);
  }
  let mut fields = Map::new();
  fields.insert(path.to_owned(), Value::String(journal.to_owned()));
  let patch = SettingsPatch::Fields {
    key: JOURNAL_PATHS_KEY.to_owned(),
    fields,
  };
  service.set(patch, "session").map(Some)
}
