//! The root the main view of an IDE showed last (openspec/specs/repository/ide-embedding/spec.md, "Repositories"): `brd
//! mcp` shows the Git roots of the project of the IDE, and the user picks one; it is kept by the set of the roots in
//! `ideRootPicks` of `repository-state.json`, so the next start of the project shows it again, and another project
//! with other roots keeps its own. Rust alone reads and writes it, never the page.

use crate::file::IDE_ROOT_PICKS_KEY;
use crate::service::{SettingsError, SettingsService};
use domain_types::settings::{SettingsChange, SettingsPatch};
use serde_json::{Map, Value};

/// The origin of the changes of the picks
const ORIGIN: &str = "rust";

/// The key of a set of roots: the roots in order of their paths, one per line, so the order the IDE names them in does
/// not matter
fn key_of(roots: &[String]) -> String {
  let mut sorted: Vec<&str> = roots.iter().map(String::as_str).collect();
  sorted.sort_unstable();
  sorted.dedup();
  sorted.join("\n")
}

/// The root of `roots` the user picked last for this set of roots; none when the user picked none, or a path that is no
/// root of them any more
#[must_use]
pub fn picked(service: &SettingsService, roots: &[String]) -> Option<String> {
  let picks = service.value(IDE_ROOT_PICKS_KEY);
  let picked = picks.get(key_of(roots))?.as_str()?;
  roots.iter().find(|root| root.as_str() == picked).cloned()
}

/// The user picked `root` of `roots`: the next start of a project with these roots shows it
///
/// # Errors
/// The service refused the change
pub fn remember(service: &SettingsService, roots: &[String], root: &str) -> Result<SettingsChange, SettingsError> {
  let mut fields = Map::new();
  fields.insert(key_of(roots), Value::String(root.to_owned()));
  service.set(
    SettingsPatch::Fields {
      key: IDE_ROOT_PICKS_KEY.to_owned(),
      fields,
    },
    ORIGIN,
  )
}
