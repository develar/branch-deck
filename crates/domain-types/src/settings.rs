//! The settings of the app that Rust reads or must agree on with the page (`appSettings` of `settings.json`), and the
//! branch prefix: what git config sets (`GitBranchPrefix`) and the prefix a command names its branches with, with where
//! it came from (`ResolvedBranchPrefix`). The file, its writer and the order of the prefixes are `app-settings`'.

use crate::branch_names::normalize_branch_prefix;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

/// The key of `AppSettings` in `settings.json`
pub const APP_SETTINGS_KEY: &str = "appSettings";

/// `appSettings` of `settings.json`: the fields Rust reads, with the defaults of the page (`src/stores/settings/appSettings.ts`),
/// and the fields only the page reads, kept as they are. Deserializing is tolerant per field: a field of the wrong type
/// or out of range is its default, so one bad field never costs the others
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
  /// General → Branch prefix: the prefix of every repository whose own git config sets none (`resolve_branch_prefix`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub global_user_branch_prefix: Option<String>,
  /// Settings → Patronus: the new API of Patronus (the progress of a running robot in one request, the merge profile
  /// in the robot list); off by default
  pub patronus_new_api: bool,
  /// The zoom of the pages in every window, positive; 1 by default
  pub zoom: f64,
  /// A focus of the main window syncs when what a sync reads changed; on by default
  pub auto_sync_on_focus: bool,
  /// Settings → Appearance → Title bar (Windows, Linux): the system title bar and menu bar; off by default
  pub system_title_bar: bool,
  /// The fields only the page reads (the notifications, the review sidebar, the Setup skips, …), raw
  #[serde(flatten)]
  #[cfg_attr(feature = "specta", specta(skip))]
  pub other: Map<String, Value>,
}

impl Default for AppSettings {
  fn default() -> Self {
    Self {
      global_user_branch_prefix: None,
      patronus_new_api: false,
      zoom: 1.0,
      auto_sync_on_focus: true,
      system_title_bar: false,
      other: Map::new(),
    }
  }
}

impl AppSettings {
  /// The settings of a stored `appSettings` object: each known field that is valid, else its default; every other
  /// field into `other`
  #[must_use]
  pub fn from_stored(mut stored: Map<String, Value>) -> Self {
    let defaults = Self::default();
    let mut take_bool = |key: &str, default: bool| stored.remove(key).and_then(|value| value.as_bool()).unwrap_or(default);
    let patronus_new_api = take_bool("patronusNewApi", defaults.patronus_new_api);
    let auto_sync_on_focus = take_bool("autoSyncOnFocus", defaults.auto_sync_on_focus);
    let system_title_bar = take_bool("systemTitleBar", defaults.system_title_bar);
    let zoom = stored
      .remove("zoom")
      .and_then(|value| value.as_f64())
      .filter(|zoom| zoom.is_finite() && *zoom > 0.0)
      .unwrap_or(defaults.zoom);
    let global_user_branch_prefix = match stored.remove("globalUserBranchPrefix") {
      Some(Value::String(prefix)) => Some(prefix),
      _ => None,
    };
    Self {
      global_user_branch_prefix,
      patronus_new_api,
      zoom,
      auto_sync_on_focus,
      system_title_bar,
      other: stored,
    }
  }

  /// The stored form: only the fields that differ from their defaults, and `other` as it was read (decision 5 of
  /// `docs/design/distributed-settings.md`); empty when every field is its default
  #[must_use]
  pub fn to_stored(&self) -> Map<String, Value> {
    let defaults = Self::default();
    let mut stored = self.other.clone();
    if let Some(prefix) = &self.global_user_branch_prefix {
      stored.insert("globalUserBranchPrefix".to_owned(), Value::String(prefix.clone()));
    }
    let mut put_bool = |key: &str, value: bool, default: bool| {
      if value != default {
        stored.insert(key.to_owned(), Value::Bool(value));
      }
    };
    put_bool("patronusNewApi", self.patronus_new_api, defaults.patronus_new_api);
    put_bool("autoSyncOnFocus", self.auto_sync_on_focus, defaults.auto_sync_on_focus);
    put_bool("systemTitleBar", self.system_title_bar, defaults.system_title_bar);
    #[expect(clippy::float_cmp, reason = "the default is the exact value 1, which the page writes as is")]
    let default_zoom = self.zoom == defaults.zoom;
    if !default_zoom && let Some(zoom) = json_number(self.zoom) {
      stored.insert("zoom".to_owned(), Value::Number(zoom));
    }
    stored
  }
}

/// `value` as JavaScript writes it: a whole number without a fraction (`2`, not `2.0`), so a stored zoom reads back as
/// the page wrote it
fn json_number(value: f64) -> Option<serde_json::Number> {
  const SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
  if value.fract() == 0.0 && value.abs() <= SAFE_INTEGER {
    #[expect(clippy::cast_possible_truncation, reason = "a whole number within the safe integers of JavaScript")]
    return Some((value as i64).into());
  }
  serde_json::Number::from_f64(value)
}

impl<'de> Deserialize<'de> for AppSettings {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    Map::deserialize(deserializer).map(Self::from_stored)
  }
}

/// `branchdeck.branchPrefix` as git resolves it (system, global, local, worktree, command: the last one wins), and
/// whether it is the repository's own
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitBranchPrefix {
  /// Trimmed, empty if no config sets it
  pub prefix: String,
  /// Set in the local or the worktree config of the repository, or by `git -c` / `GIT_CONFIG_*`: it wins over the app's
  /// prefix (`resolve_branch_prefix`)
  pub repository_scoped: bool,
}

/// Where the prefix of the branches came from
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum PrefixSource {
  /// `--prefix` of `bd`
  Cli,
  /// `branchdeck.branchPrefix` of the repository's local or worktree config, or of `git -c` / `GIT_CONFIG_*`
  RepositoryConfig,
  /// General → Branch prefix of the app (`globalUserBranchPrefix`)
  AppSetting,
  /// `branchdeck.branchPrefix` of the global or system git config
  GlobalConfig,
  /// Nothing sets a prefix
  None,
}

/// The prefix the branches are named with, and where it came from; `source` is `None` exactly when `prefix` is empty
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ResolvedBranchPrefix {
  /// Normalized (`normalize_branch_prefix`): trimmed, no trailing slash
  pub prefix: String,
  pub source: PrefixSource,
}

impl ResolvedBranchPrefix {
  /// No prefix
  #[must_use]
  pub const fn none() -> Self {
    Self {
      prefix: String::new(),
      source: PrefixSource::None,
    }
  }

  /// `prefix` of `source`, normalized (`jane/` is `jane`); no prefix when nothing is left of it
  #[must_use]
  pub fn new(prefix: &str, source: PrefixSource) -> Self {
    let prefix = normalize_branch_prefix(prefix);
    if prefix.is_empty() {
      Self::none()
    } else {
      Self {
        prefix: prefix.to_owned(),
        source,
      }
    }
  }
}
