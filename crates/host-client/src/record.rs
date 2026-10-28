//! The local files of the runs of a checkout, in `branch-deck/host/runs/` of its git directory (spec/cli/host.spec.md,
//! "The files"): `<run>.json`, the record of what the client asked of the run, and `<run>.patch`, the patch of its
//! changes. The files older than `KEEP` go at the next run of the checkout.
//!
//! The client writes the record before the start of the run. A fetch takes only what the record asked for: the staged
//! files below its fetch directories, the files of its outputs, and the changes when it asked for them. So the host
//! cannot choose which local paths it replaces. The outputs come from `[host]` of the shipped commit, through the
//! parser that the agent uses too

use crate::config::write_private;
use host_link::identity::create_private_dir;
use host_link::refusal::{Code, Refusal};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long the records and the patches stay in the git directory of a checkout
const KEEP: Duration = Duration::from_hours(7 * 24);

/// What the client asked of a run
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunRecord {
  /// The shipped commit
  pub(crate) commit: String,
  /// The directories relative to the slot root whose staged files come back
  pub(crate) fetch: Vec<String>,
  /// The outputs of `[host]` of the commit, when the client asked for them
  pub(crate) outputs: Vec<String>,
  /// The client asked for the changes of the command
  pub(crate) changes: bool,
}

impl RunRecord {
  /// Whether the run brings anything back
  pub(crate) const fn asks_for_files(&self) -> bool {
    !self.fetch.is_empty() || !self.outputs.is_empty() || self.changes
  }
}

/// The directory of the run files of a checkout
pub(crate) struct RunFiles(PathBuf);

impl RunFiles {
  /// The directory `dir`, made private when it is missing. Its records and patches older than `KEEP` go
  pub(crate) fn new(dir: &Path) -> Result<Self, Refusal> {
    let dir = dir.to_path_buf();
    create_private_dir(&dir).map_err(|error| Refusal::new(Code::Config, format!("cannot make {}: {error}", dir.display())))?;
    if let Ok(entries) = std::fs::read_dir(&dir) {
      for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let old = entry
          .metadata()
          .and_then(|metadata| metadata.modified())
          .ok()
          .and_then(|modified| modified.elapsed().ok())
          .is_some_and(|age| age >= KEEP);
        if old && (name.ends_with(".patch") || name.ends_with(".json")) {
          let _ = sync_utils::fs_retry::remove_file(entry.path());
        }
      }
    }
    Ok(Self(dir))
  }

  /// The file of the patch of the changes of `run`
  pub(crate) fn patch(&self, run: &str) -> PathBuf {
    self.0.join(format!("{run}.patch"))
  }

  fn record(&self, run: &str) -> PathBuf {
    self.0.join(format!("{run}.json"))
  }

  pub(crate) fn write(&self, run: &str, record: &RunRecord) -> Result<(), Refusal> {
    let path = self.record(run);
    let mut text = serde_json::to_string_pretty(record).map_err(|error| Refusal::new(Code::Config, format!("cannot write the run record {}: {error}", path.display())))?;
    text.push('\n');
    write_private(&path, &text)
  }

  /// The record of `run`, or `None` when this checkout has none
  pub(crate) fn read(&self, run: &str) -> Result<Option<RunRecord>, Refusal> {
    let path = self.record(run);
    let text = match std::fs::read_to_string(&path) {
      Ok(text) => text,
      Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
      Err(error) => return Err(Refusal::new(Code::Config, format!("cannot read the run record {}: {error}", path.display()))),
    };
    serde_json::from_str(&text)
      .map(Some)
      .map_err(|error| Refusal::new(Code::Config, format!("{} is not a run record: {error}", path.display())))
  }

  pub(crate) fn dir(&self) -> &Path {
    &self.0
  }
}

#[cfg(test)]
#[path = "record_test.rs"]
mod record_test;
