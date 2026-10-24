//! `settings.json`: one JSON object of the page's stores by key (`appSettings`, `recentProjects`, the diff view, …).
//! Rust knows `appSettings` (`AppSettings`); every other key is kept raw, as the page wrote it. `repository-state.json`
//! beside it has the same form with the one key `repositoryState`: what each sync of a repository records
//! (`REPOSITORY_STATE_FIELDS`), so a sync never rewrites the file of the user's settings, which `bd` reads. A load never fails: a
//! missing file is the defaults, a file that does not parse is the defaults with a warning, a field of the wrong type
//! is its default. A save writes the stored form (only the fields that differ from their defaults, decision 5 of
//! `docs/design/distributed-settings.md`) to a temporary file beside it and renames it over the file, so a reader never
//! sees a cut file.

use domain_types::settings::{APP_SETTINGS_KEY, AppSettings};
use serde_json::{Map, Value};
use std::io::Write as _;
use std::path::Path;
use tracing::warn;

/// The name of the settings file in the app data directory (the bundle identifier's): the app's writer opens it there,
/// `bd` reads it there
pub const SETTINGS_FILE_NAME: &str = "settings.json";

/// The name of the file beside `settings.json` that keeps `repositoryState` (`REPOSITORY_STATE_KEY`)
pub const REPOSITORY_STATE_FILE_NAME: &str = "repository-state.json";

/// The key of what the syncs and the updates record of each repository, by its path: the one key of
/// `repository-state.json`
pub const REPOSITORY_STATE_KEY: &str = "repositoryState";

/// The fields of a repository in `repositoryState`: when it was synced last, how many branches that sync had, how long
/// the steps of Update Project and of a sync took. A settings file of before kept them in each project of
/// `recentProjects` (`SettingsFile::take_repository_state`)
pub const REPOSITORY_STATE_FIELDS: &[&str] = &["lastSyncTime", "lastBranchCount", "updateTimings"];

/// The key of the recent projects, whose projects kept the fields of `repositoryState` before it
const RECENT_PROJECTS_KEY: &str = "recentProjects";

/// Why the text of a settings file is no settings
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
  #[error(transparent)]
  Json(#[from] serde_json::Error),
  #[error("not a JSON object but {0}")]
  NotAnObject(&'static str),
}

/// The content of the settings file
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsFile {
  /// `appSettings`
  pub app: AppSettings,
  /// Every other key of the file, raw: the stores only the page reads
  pub other: Map<String, Value>,
}

impl SettingsFile {
  /// The settings of `path`: the defaults when the file is missing, or with a warning when it can't be read or does
  /// not parse
  #[must_use]
  pub fn load(path: &Path) -> Self {
    match std::fs::read_to_string(path) {
      Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
        warn!(file = %path.display(), %error, "the settings file does not parse, using the defaults");
        Self::default()
      }),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
      Err(error) => {
        warn!(file = %path.display(), %error, "cannot read the settings file, using the defaults");
        Self::default()
      }
    }
  }

  /// The settings of the text of a settings file; an error when it is no JSON object. An `appSettings` that is no
  /// object is the defaults with a warning, and is dropped from the file by the next save
  pub fn parse(text: &str) -> Result<Self, ParseError> {
    let mut other = match serde_json::from_str::<Value>(text)? {
      Value::Object(object) => object,
      value => return Err(ParseError::NotAnObject(json_kind(&value))),
    };
    let app = match other.remove(APP_SETTINGS_KEY) {
      None => AppSettings::default(),
      Some(Value::Object(stored)) => AppSettings::from_stored(stored),
      Some(value) => {
        warn!(kind = json_kind(&value), "{APP_SETTINGS_KEY} of the settings file is no object, using the defaults");
        AppSettings::default()
      }
    };
    Ok(Self { app, other })
  }

  /// The stored form: `appSettings` without its default fields, and no `appSettings` when all of them are defaults;
  /// every other key as it was read
  #[must_use]
  pub fn to_stored(&self) -> Map<String, Value> {
    let mut stored = self.other.clone();
    let app = self.app.to_stored();
    if !app.is_empty() {
      stored.insert(APP_SETTINGS_KEY.to_owned(), Value::Object(app));
    }
    stored
  }

  /// The fields of `repositoryState` (`REPOSITORY_STATE_FIELDS`) that the projects of `recentProjects` still have, as
  /// a settings file of before kept them, by the path of each project; taken out of the projects, so the next save of
  /// the file has them no more. A project without a path keeps them, and nothing else of the file changes
  pub fn take_repository_state(&mut self) -> Map<String, Value> {
    let mut taken = Map::new();
    let Some(Value::Array(projects)) = self.other.get_mut(RECENT_PROJECTS_KEY) else {
      return taken;
    };
    for project in projects.iter_mut().filter_map(Value::as_object_mut) {
      let Some(path) = project.get("path").and_then(Value::as_str).map(str::to_owned) else {
        continue;
      };
      let fields: Map<String, Value> = REPOSITORY_STATE_FIELDS
        .iter()
        .filter_map(|field| project.remove(*field).map(|value| ((*field).to_owned(), value)))
        .collect();
      if !fields.is_empty() {
        taken.insert(path, Value::Object(fields));
      }
    }
    taken
  }

  /// Write the stored form to `path` atomically: a temporary file in the same directory, flushed, renamed over `path`.
  /// The directory is created when missing
  pub fn save(&self, path: &Path) -> std::io::Result<()> {
    let directory = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(directory)?;
    let mut text = serde_json::to_vec_pretty(&Value::Object(self.to_stored()))?;
    text.push(b'\n');
    let mut file = tempfile::Builder::new().prefix(".settings-").suffix(".tmp").tempfile_in(directory)?;
    file.write_all(&text)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
  }
}

const fn json_kind(value: &Value) -> &'static str {
  match value {
    Value::Null => "null",
    Value::Bool(_) => "a boolean",
    Value::Number(_) => "a number",
    Value::String(_) => "a string",
    Value::Array(_) => "an array",
    Value::Object(_) => "an object",
  }
}
