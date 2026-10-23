//! `settings.json`: one JSON object of the page's stores by key (`appSettings`, `recentProjects`, the diff view, …).
//! Rust knows `appSettings` (`AppSettings`); every other key is kept raw, as the page wrote it. A load never fails: a
//! missing file is the defaults, a file that does not parse is the defaults with a warning, a field of the wrong type
//! is its default. A save writes the stored form (only the fields that differ from their defaults, decision 5 of
//! `docs/design/distributed-settings.md`) to a temporary file beside it and renames it over the file, so a reader never
//! sees a cut file.

use domain_types::settings::{APP_SETTINGS_KEY, AppSettings};
use serde_json::{Map, Value};
use std::io::Write as _;
use std::path::Path;
use tracing::warn;

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
