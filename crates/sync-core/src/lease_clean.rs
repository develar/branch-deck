//! The clean rules of a slot (`bd lease return`, `bd lease clean`, spec/cli/lease.spec.md): the `[lease]` table of
//! `.branchdeck.toml`, committed with the build layout it cleans. Two kinds only, and no command: a return must not run
//! arbitrary code.
//!
//! ```toml
//! [lease]
//! clean = [
//!   { cargo = "target", keep = 8, age = "1h" },  # the stale units of every profile dir
//!   { delete = "tests/results" },
//! ]
//! ```
//!
//! A path of a rule is relative and normalized, stays inside the slot, never names `.git`, has no symlink on its way,
//! and git tracks nothing under it (one `ls-files` for all rules); a rule that breaks one of these is skipped and
//! reported, never applied. The walk follows no symlink.

use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::tracked_under;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use tracing::debug;

/// The file of the rules, at the root of a checkout
pub const BRANCH_DECK_FILE: &str = ".branchdeck.toml";

/// How many units of a group the cargo rule keeps by default, whatever their age
const DEFAULT_KEEP: usize = 8;
/// How young a unit is that the cargo rule keeps by default
const DEFAULT_AGE: Duration = Duration::from_hours(1);
/// How deep below the target dir the cargo rule looks for profile dirs (`debug`, `<triple>/release`, a nested target)
const PROFILE_DEPTH: usize = 4;

/// A rule of `[lease] clean`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanRule {
  /// The units of the profile dirs under `dir` (a dir with `.cargo-lock`): `build/<package>/<hash>` and
  /// `incremental/<crate>-<hash>` stay when they are among the `keep` newest of their group or younger than `age`
  Cargo { dir: String, keep: usize, age: Duration },
  /// Remove `path`, a file or a directory
  Delete { path: String },
}

impl CleanRule {
  /// The rule as the output names it: `cargo target`, `delete tests/results`
  pub fn describe(&self) -> String {
    match self {
      Self::Cargo { dir, .. } => format!("cargo {dir}"),
      Self::Delete { path } => format!("delete {path}"),
    }
  }

  const fn path(&self) -> &str {
    match self {
      Self::Cargo { dir, .. } => dir.as_str(),
      Self::Delete { path } => path.as_str(),
    }
  }
}

/// A rule of the file, or the reason it is not one (an unsafe path); the reason is reported, never applied
pub type ParsedRule = Result<CleanRule, (String, String)>;

/// The rules of `.branchdeck.toml` in `checkout`; none without the file. A file that does not parse, an unknown key or
/// an unknown kind is an error
pub fn read_clean_rules(checkout: &Path) -> Result<Vec<ParsedRule>> {
  let path = checkout.join(BRANCH_DECK_FILE);
  match std::fs::read_to_string(&path) {
    Ok(text) => parse_clean_rules(&text).map_err(|error| error.context(format!("Cannot read {}", path.display()))),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
    Err(error) => Err(anyhow!(error).context(format!("Cannot read {}", path.display()))),
  }
}

/// The rules of the text of a `.branchdeck.toml`
pub fn parse_clean_rules(text: &str) -> Result<Vec<ParsedRule>> {
  let table: toml::Table = text.parse()?;
  if let Some(key) = table.keys().find(|key| *key != "lease") {
    bail!("unknown key {key:?}; the file has only a [lease] table");
  }
  let Some(lease) = table.get("lease") else { return Ok(Vec::new()) };
  let lease = lease.as_table().ok_or_else(|| anyhow!("`lease` is not a table"))?;
  if let Some(key) = lease.keys().find(|key| *key != "clean") {
    bail!("unknown key lease.{key}; [lease] has only `clean`");
  }
  let Some(clean) = lease.get("clean") else { return Ok(Vec::new()) };
  let clean = clean.as_array().ok_or_else(|| anyhow!("`lease.clean` is not an array"))?;
  clean
    .iter()
    .enumerate()
    .map(|(index, rule)| parse_rule(rule).map_err(|error| error.context(format!("lease.clean[{index}]"))))
    .collect()
}

fn parse_rule(rule: &toml::Value) -> Result<ParsedRule> {
  let rule = rule
    .as_table()
    .ok_or_else(|| anyhow!("a rule is a table, `{{ cargo = \"target\" }}` or `{{ delete = \"<path>\" }}`"))?;
  let string = |key: &str| -> Result<String> { rule[key].as_str().map(str::to_string).ok_or_else(|| anyhow!("`{key}` is not a string")) };
  let only = |keys: &[&str]| -> Result<()> {
    match rule.keys().find(|key| !keys.contains(&key.as_str())) {
      Some(key) => bail!("unknown key {key:?}; the rule takes {}", keys.join(", ")),
      None => Ok(()),
    }
  };
  let (kind, raw) = if rule.contains_key("cargo") {
    only(&["cargo", "keep", "age"])?;
    let keep = match rule.get("keep") {
      Some(keep) => usize::try_from(keep.as_integer().ok_or_else(|| anyhow!("`keep` is not an integer"))?).map_err(|error| anyhow!("`keep` is negative ({error})"))?,
      None => DEFAULT_KEEP,
    };
    let age = match rule.get("age") {
      Some(age) => parse_age(age.as_str().ok_or_else(|| anyhow!("`age` is not a string such as \"1h\""))?)?,
      None => DEFAULT_AGE,
    };
    let dir = string("cargo")?;
    (CleanRule::Cargo { dir: dir.clone(), keep, age }, dir)
  } else if rule.contains_key("delete") {
    only(&["delete"])?;
    let path = string("delete")?;
    (CleanRule::Delete { path: path.clone() }, path)
  } else {
    bail!("unknown kind: a rule has `cargo` or `delete`");
  };
  Ok(match normalize(&raw) {
    Ok(path) => Ok(match kind {
      CleanRule::Cargo { keep, age, .. } => CleanRule::Cargo { dir: path, keep, age },
      CleanRule::Delete { .. } => CleanRule::Delete { path },
    }),
    Err(reason) => Err((kind.describe(), reason)),
  })
}

/// The parameters of a call that change the rules of the file without an edit of it (`bd lease clean --keep 2`)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CleanOverrides {
  /// `keep` of every cargo rule
  pub keep: Option<usize>,
  /// `age` of every cargo rule
  pub age: Option<Duration>,
}

impl CleanOverrides {
  /// `rules` with these parameters in every cargo rule
  pub fn apply(self, rules: Vec<ParsedRule>) -> Vec<ParsedRule> {
    rules
      .into_iter()
      .map(|rule| match rule {
        Ok(CleanRule::Cargo { dir, keep, age }) => Ok(CleanRule::Cargo {
          dir,
          keep: self.keep.unwrap_or(keep),
          age: self.age.unwrap_or(age),
        }),
        other => other,
      })
      .collect()
  }
}

/// `30s`, `15m`, `1h`, `2d`
pub fn parse_age(text: &str) -> Result<Duration> {
  let split = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
  let (number, unit) = text.split_at(split);
  let number: u64 = number
    .parse()
    .map_err(|error| anyhow!("`age` {text:?} is not a number and a unit, such as \"1h\" ({error})"))?;
  let seconds = match unit {
    "s" => 1,
    "m" => 60,
    "h" => 60 * 60,
    "d" => 24 * 60 * 60,
    _ => bail!("`age` {text:?} has no unit s, m, h or d"),
  };
  Ok(Duration::from_secs(number * seconds))
}

/// `path` as a normalized relative path (`./a//b/` is `a/b`); an absolute path, `..`, `.git`, a backslash or nothing
/// at all is refused with the reason
fn normalize(path: &str) -> Result<String, String> {
  if path.contains('\\') {
    return Err(format!("{path:?} has a backslash; write `/`"));
  }
  if Path::new(path).has_root() || path.starts_with('/') {
    return Err(format!("{path:?} is absolute; a path of a rule is relative to the checkout"));
  }
  let mut parts = Vec::new();
  for component in Path::new(path).components() {
    match component {
      Component::CurDir => {}
      Component::Normal(name) if name == ".git" => return Err(format!("{path:?} names .git")),
      Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
      Component::ParentDir => return Err(format!("{path:?} leaves the checkout with ..")),
      Component::RootDir | Component::Prefix(_) => return Err(format!("{path:?} is absolute; a path of a rule is relative to the checkout")),
    }
  }
  if parts.is_empty() {
    return Err(format!("{path:?} names the checkout itself"));
  }
  Ok(parts.join("/"))
}

/// What one rule did in one checkout
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cleaned {
  /// `cargo target`, `delete tests/results`
  pub rule: String,
  /// The bytes on disk it freed, or would free in a dry run
  pub freed_bytes: u64,
  pub elapsed: Duration,
  /// Why it was skipped (an unsafe path, a tracked file, a symlink) or failed partway
  pub error: Option<String>,
}

/// Apply `rules` to `checkout`, or only measure them (`apply` false). One `ls-files` for the paths of all rules; a rule
/// whose path is unsafe is reported and skipped. A failed rule never stops the others
pub fn apply_clean_rules(git_executor: &GitCommandExecutor, checkout: &str, rules: &[ParsedRule], apply: bool) -> Vec<Cleaned> {
  let valid: Vec<&CleanRule> = rules.iter().filter_map(|rule| rule.as_ref().ok()).collect();
  let paths: Vec<&str> = valid.iter().map(|rule| rule.path()).collect();
  let tracked = match tracked_under(git_executor, checkout, &paths) {
    Ok(tracked) => Ok(tracked),
    Err(error) => Err(format!("{error:#}")),
  };
  rules
    .iter()
    .map(|rule| {
      let started = Instant::now();
      let outcome = match rule {
        Err((rule, reason)) => Cleaned {
          rule: rule.clone(),
          freed_bytes: 0,
          elapsed: Duration::ZERO,
          error: Some(reason.clone()),
        },
        Ok(rule) => {
          let result = match &tracked {
            Err(error) => Err(anyhow!("cannot tell what git tracks: {error}")),
            Ok(tracked) => check_path(Path::new(checkout), rule.path(), tracked).and_then(|target| run_rule(rule, &target, apply)),
          };
          let (freed_bytes, error) = match result {
            Ok(freed) => (freed, None),
            Err(error) => (0, Some(format!("{error:#}"))),
          };
          Cleaned {
            rule: rule.describe(),
            freed_bytes,
            elapsed: started.elapsed(),
            error,
          }
        }
      };
      debug!(checkout = %checkout, rule = %outcome.rule, freed = outcome.freed_bytes, error = ?outcome.error, apply, "clean rule");
      outcome
    })
    .collect()
}

/// The target of a rule in `checkout`: no symlink on its way (the last component included), nothing under it that git
/// tracks
fn check_path(checkout: &Path, path: &str, tracked: &[String]) -> Result<PathBuf> {
  if let Some(file) = tracked.iter().find(|file| file.as_str() == path || file.starts_with(&format!("{path}/"))) {
    bail!("git tracks {file}");
  }
  let mut target = checkout.to_path_buf();
  for part in path.split('/') {
    target.push(part);
    if target.symlink_metadata().is_ok_and(|metadata| metadata.file_type().is_symlink()) {
      bail!("{} is a symlink, which a rule never follows", target.display());
    }
  }
  Ok(target)
}

fn run_rule(rule: &CleanRule, target: &Path, apply: bool) -> Result<u64> {
  let Ok(metadata) = target.symlink_metadata() else { return Ok(0) };
  match rule {
    CleanRule::Delete { .. } => {
      let size = disk_size(target);
      if apply {
        if metadata.is_dir() {
          std::fs::remove_dir_all(target)
        } else {
          std::fs::remove_file(target)
        }
        .map_err(|error| anyhow!("cannot remove {}: {error}", target.display()))?;
      }
      Ok(size)
    }
    CleanRule::Cargo { keep, age, .. } => {
      if !metadata.is_dir() {
        return Ok(0);
      }
      let cutoff = SystemTime::now().checked_sub(*age).unwrap_or(SystemTime::UNIX_EPOCH);
      let mut freed = 0;
      for profile in profile_dirs(target, PROFILE_DEPTH) {
        freed += prune_units(&profile.join("build"), true, *keep, cutoff, apply)?;
        freed += prune_units(&profile.join("incremental"), false, *keep, cutoff, apply)?;
      }
      Ok(freed)
    }
  }
}

/// The profile dirs under `dir`: the ones with `.cargo-lock`, at most `depth` levels down, symlinks not followed
fn profile_dirs(dir: &Path, depth: usize) -> Vec<PathBuf> {
  if dir.join(".cargo-lock").symlink_metadata().is_ok() {
    return vec![dir.to_path_buf()];
  }
  if depth == 0 {
    return Vec::new();
  }
  subdirs(dir).into_iter().flat_map(|sub| profile_dirs(&sub, depth - 1)).collect()
}

/// The directories in `dir`, not the symlinks to one
fn subdirs(dir: &Path) -> Vec<PathBuf> {
  std::fs::read_dir(dir).map_or_default(|entries| {
    entries
      .flatten()
      .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
      .map(|entry| entry.path())
      .collect()
  })
}

/// Remove the stale units of `dir`: `build/<package>/<hash>` (`nested`) or `incremental/<crate>-<hash>`, grouped by
/// package or crate (a crate name may hold `-`). A unit stays when it is among the `keep` last built of its group or
/// was built after `cutoff`: a slot builds a dozen variants an hour, so age alone keeps all
fn prune_units(dir: &Path, nested: bool, keep: usize, cutoff: SystemTime, apply: bool) -> Result<u64> {
  let mut groups: HashMap<String, Vec<(SystemTime, PathBuf)>> = HashMap::new();
  for entry in subdirs(dir) {
    let name = entry.file_name().map_or_default(|name| name.to_string_lossy().into_owned());
    if nested {
      for unit in subdirs(&entry) {
        groups.entry(name.clone()).or_default().push((last_built(&unit), unit));
      }
    } else {
      let crate_name = name.rfind('-').map_or(name.as_str(), |dash| &name[..dash]).to_string();
      groups.entry(crate_name).or_default().push((last_built(&entry), entry));
    }
  }
  let mut freed = 0;
  for units in groups.values_mut() {
    units.sort_by_key(|(built, _)| std::cmp::Reverse(*built));
    for (built, unit) in units.iter().skip(keep) {
      if *built < cutoff {
        freed += disk_size(unit);
        if apply {
          std::fs::remove_dir_all(unit).map_err(|error| anyhow!("cannot remove {}: {error}", unit.display()))?;
        }
      }
    }
  }
  Ok(freed)
}

/// When a unit was last built: a rebuild rewrites its fingerprint files (`build/`) or adds a session
/// (`incremental/`), not always the directory itself, so the newest mtime of the directory and of the direct children
/// of its `fingerprint` dir (else of itself) counts
fn last_built(unit: &Path) -> SystemTime {
  let modified = |path: &Path| path.symlink_metadata().and_then(|metadata| metadata.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
  let fingerprint = unit.join("fingerprint");
  let children = if fingerprint.is_dir() { fingerprint } else { unit.to_path_buf() };
  std::fs::read_dir(&children)
    .map_or(SystemTime::UNIX_EPOCH, |entries| {
      entries.flatten().map(|entry| modified(&entry.path())).max().unwrap_or(SystemTime::UNIX_EPOCH)
    })
    .max(modified(unit))
}

/// The bytes on disk of `path` and of everything below it, symlinks counted as themselves
fn disk_size(path: &Path) -> u64 {
  let Ok(metadata) = path.symlink_metadata() else { return 0 };
  let own = allocated(&metadata);
  if !metadata.is_dir() {
    return own;
  }
  own + std::fs::read_dir(path).map_or(0, |entries| entries.flatten().map(|entry| disk_size(&entry.path())).sum())
}

#[cfg(unix)]
fn allocated(metadata: &std::fs::Metadata) -> u64 {
  use std::os::unix::fs::MetadataExt;
  metadata.blocks() * 512
}

#[cfg(not(unix))]
fn allocated(metadata: &std::fs::Metadata) -> u64 {
  metadata.len()
}

#[cfg(test)]
#[path = "lease_clean_test.rs"]
mod lease_clean_test;
