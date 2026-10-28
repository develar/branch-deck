//! The prepare steps of `[host]`. A step runs before the command when its fingerprint differs from the one of its last
//! success in the host slot, or when a path that it creates is missing. The fingerprint covers the command of the step,
//! the paths of its inputs and their object ids at the commit of the run, so a step runs again when one of its inputs
//! changed. The fingerprints are kept per slot ([`Prepared`]), by its lease id and the generation of its files, so a new
//! slot, or a slot whose files a give-back or a clean removed, runs each step once: the agent cannot know what was done
//! to it before

use crate::files::{private_dir, write_private};
use host_link::config::Step;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// Why a step runs, or `None` when it is current
pub(crate) fn reason(step: &Step, fingerprint: &str, prepared: &BTreeMap<String, String>, slot: &Path) -> Option<String> {
  if let Some(missing) = step.creates.iter().find(|path| !slot.join(path).exists()) {
    return Some(format!("{missing} is missing"));
  }
  match prepared.get(&key(step)) {
    None => Some("the first run in this slot".to_owned()),
    Some(recorded) if recorded != fingerprint => Some(if step.inputs.is_empty() {
      "its command changed".to_owned()
    } else {
      format!("{} changed", step.inputs.join(", "))
    }),
    Some(_) => None,
  }
}

/// The key of a step among the fingerprints of a slot: a hash of its name
pub(crate) fn key(step: &Step) -> String {
  format!("{:016x}", fnv(step.name.as_bytes(), FNV_OFFSET))
}

/// The fingerprint of a step: its command, and each input path with its object id at the commit, `-` for a path that the
/// commit lacks
pub(crate) fn fingerprint(step: &Step, ids: &[Option<String>]) -> String {
  let mut hash = FNV_OFFSET;
  for word in &step.run {
    hash = fnv(word.as_bytes(), hash);
    hash = fnv(&[0], hash);
  }
  for (path, id) in step.inputs.iter().zip(ids) {
    hash = fnv(&[1], hash);
    hash = fnv(path.as_bytes(), hash);
    hash = fnv(&[0], hash);
    hash = fnv(id.as_deref().unwrap_or("-").as_bytes(), hash);
  }
  format!("{hash:016x}")
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a over `bytes`, from `hash`. Stable across builds and platforms
fn fnv(bytes: &[u8], hash: u64) -> u64 {
  bytes.iter().fold(hash, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}

/// The fingerprints of the prepare steps that succeeded in one slot, in `<slots>/<lease id>.prepared`: a `generation=`
/// line and `prepared.<key>=<fingerprint>` lines. Another lease id is another slot, and another generation means that a
/// give-back discarded changes or a clean ran since, which can have deleted what a step made: both start empty
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Prepared {
  path: PathBuf,
  generation: u64,
  pub(crate) steps: BTreeMap<String, String>,
}

impl Prepared {
  /// The fingerprints of the slot `lease_id` in the generation `generation` of its files, in the directory `dir`
  pub(crate) fn load(dir: &Path, lease_id: &str, generation: u64) -> Self {
    let path = dir.join(format!("{lease_id}.prepared"));
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut recorded = None;
    let mut steps = BTreeMap::new();
    for line in text.lines() {
      if let Some(value) = line.strip_prefix("generation=") {
        recorded = value.parse().ok();
      } else if let Some((key, fingerprint)) = line.strip_prefix("prepared.").and_then(|rest| rest.split_once('=')) {
        steps.insert(key.to_owned(), fingerprint.to_owned());
      }
    }
    if recorded != Some(generation) {
      steps.clear();
    }
    Self { path, generation, steps }
  }

  pub(crate) fn save(&self) -> io::Result<()> {
    if let Some(dir) = self.path.parent() {
      private_dir(dir)?;
    }
    let mut text = format!("generation={}\n", self.generation);
    for (key, fingerprint) in &self.steps {
      let _ = writeln!(text, "prepared.{key}={fingerprint}");
    }
    write_private(&self.path, text.as_bytes())
  }
}

#[cfg(test)]
#[path = "prepare_test.rs"]
mod prepare_test;
