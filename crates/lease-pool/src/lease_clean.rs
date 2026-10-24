//! The clean rules of a slot (`bd lease return`, `bd lease clean`, spec/cli/lease.spec.md): the `[lease]` table of
//! `.branchdeck.toml`, committed with the build layout it cleans. Two kinds only, and no command: a return must not run
//! arbitrary code. A get leaves out of a new slot what the `delete` rules delete, when it clones one (`clone_dir`).
//!
//! ```toml
//! [lease]
//! clean = [
//!   { cargo = "target", keep = 8, age = "1h" },  # the stale units of every profile dir
//!   { delete = "tests/results" },
//!   { delete = "out/dev-run", except = ["jar-cache"] },  # what is in it, but for these entries
//! ]
//! ```
//!
//! A path of a rule is relative and normalized, stays inside the slot, never names `.git`, has no symlink on its way
//! (a `delete` may name a symlink itself, and removes the link), and git tracks nothing under it (one `ls-files` for
//! all rules); a rule that breaks one of these is skipped and reported, never applied. The walk follows no symlink. A
//! return and `bd lease clean` move what a `delete` removes into a trash directory first, a rename, and delete it once
//! the slot is free again (`Trash`).

use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::tracked_under;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use sync_utils::process_scan::{is_running, process_started};
use tracing::{debug, warn};

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
  /// Remove `path`, a file, a directory or a symlink; with `except`, the entries of the directory but the ones named
  Delete { path: String, except: Vec<String> },
}

impl CleanRule {
  /// The rule as the output names it: `cargo target`, `delete tests/results`, `delete out/dev-run except jar-cache`
  pub fn describe(&self) -> String {
    match self {
      Self::Cargo { dir, .. } => format!("cargo {dir}"),
      Self::Delete { path, except } if except.is_empty() => format!("delete {path}"),
      Self::Delete { path, except } => format!("delete {path} except {}", except.join(", ")),
    }
  }

  const fn path(&self) -> &str {
    match self {
      Self::Cargo { dir, .. } => dir.as_str(),
      Self::Delete { path, .. } => path.as_str(),
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
  let rules: Vec<ParsedRule> = clean
    .iter()
    .enumerate()
    .map(|(index, rule)| parse_rule(rule).map_err(|error| error.context(format!("lease.clean[{index}]"))))
    .collect::<Result<_>>()?;
  Ok(refuse_deletes_of_cargo_dirs(rules))
}

/// A `delete` whose path holds the dir of a `cargo` rule, without the entry on its way in `except`, would remove what
/// the cargo rule keeps: it is reported, never applied
fn refuse_deletes_of_cargo_dirs(rules: Vec<ParsedRule>) -> Vec<ParsedRule> {
  let cargo_dirs: Vec<String> = rules
    .iter()
    .filter_map(|rule| match rule {
      Ok(CleanRule::Cargo { dir, .. }) => Some(dir.clone()),
      _ => None,
    })
    .collect();
  rules
    .into_iter()
    .map(|rule| match rule {
      Ok(CleanRule::Delete { path, except }) => {
        let covered = cargo_dirs.iter().find(|dir| {
          *dir == &path
            || dir
              .strip_prefix(&format!("{path}/"))
              .is_some_and(|rest| !except.iter().any(|kept| rest.split('/').next() == Some(kept.as_str())))
        });
        match covered {
          Some(dir) => {
            let rule = CleanRule::Delete { path, except };
            Err((rule.describe(), format!("it would delete {dir}, the dir of a cargo rule; name its entry in `except`")))
          }
          None => Ok(CleanRule::Delete { path, except }),
        }
      }
      other => other,
    })
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
    only(&["delete", "except"])?;
    let path = string("delete")?;
    let except = match rule.get("except") {
      Some(except) => except
        .as_array()
        .ok_or_else(|| anyhow!("`except` is not an array of names"))?
        .iter()
        .map(|name| name.as_str().map(str::to_string).ok_or_else(|| anyhow!("an entry of `except` is not a string")))
        .collect::<Result<Vec<_>>>()?,
      None => Vec::new(),
    };
    (CleanRule::Delete { path: path.clone(), except }, path)
  } else {
    bail!("unknown kind: a rule has `cargo` or `delete`");
  };
  let path = normalize(&raw).and_then(|path| match &kind {
    CleanRule::Delete { except, .. } => except.iter().try_for_each(|name| check_entry_name(name)).map(|()| path),
    CleanRule::Cargo { .. } => Ok(path),
  });
  Ok(match path {
    Ok(path) => Ok(match kind {
      CleanRule::Cargo { keep, age, .. } => CleanRule::Cargo { dir: path, keep, age },
      CleanRule::Delete { except, .. } => CleanRule::Delete { path, except },
    }),
    Err(reason) => Err((kind.describe(), reason)),
  })
}

/// An entry of `except`: the name of one entry of the directory, no path
fn check_entry_name(name: &str) -> Result<(), String> {
  if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
    return Err(format!("`except` entry {name:?} is not the name of an entry of the directory"));
  }
  Ok(())
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

/// A size: a number and a binary unit, `200G` or `200GiB` (K, M, G, T, P; `B` or none for bytes), as `df -h` counts
pub fn parse_size(text: &str) -> Result<u64> {
  let text = text.trim();
  let split = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
  let (number, unit) = text.split_at(split);
  let number: u64 = number
    .parse()
    .map_err(|error| anyhow!("the size {text:?} is not a number and a unit, such as \"200G\" ({error})"))?;
  let unit = unit.trim();
  let unit = unit.strip_suffix("iB").or_else(|| unit.strip_suffix('B').filter(|rest| !rest.is_empty())).unwrap_or(unit);
  let shift = match unit {
    "" | "B" => 0,
    "K" | "k" => 10,
    "M" => 20,
    "G" => 30,
    "T" => 40,
    "P" => 50,
    _ => bail!("the size {text:?} has no unit K, M, G, T or P"),
  };
  number.checked_mul(1 << shift).ok_or_else(|| anyhow!("the size {text:?} is too large"))
}

/// `bytes` in GiB with one decimal, as a message names a size: `123.4 GiB`
#[expect(clippy::cast_precision_loss, reason = "one decimal of a size in a message")]
pub fn describe_size(bytes: u64) -> String {
  format!("{:.1} GiB", bytes as f64 / f64::from(1u32 << 30))
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
  /// The bytes on disk it freed, or would free in a dry run. Blocks that an APFS clone shares with another checkout
  /// count too, though only the last of the files that hold them frees them
  pub freed_bytes: u64,
  pub elapsed: Duration,
  /// Why it was skipped (an unsafe path, a tracked file, a symlink) or failed partway
  pub error: Option<String>,
  /// What it moved into the trash: `Trash::empty` deletes it and counts its bytes
  pub trashed: Vec<PathBuf>,
}

/// How `apply_clean_rules` applies the rules
#[derive(Clone, Copy)]
pub enum CleanMode<'a> {
  /// Measure only (`--dry-run`)
  Measure,
  /// Delete in place
  Delete,
  /// Move what a `delete` removes into the trash, a rename (in place on another volume); `Trash::empty` deletes it
  Trash(&'a Trash),
}

impl CleanMode<'_> {
  const fn applies(self) -> bool {
    !matches!(self, Self::Measure)
  }
}

/// Apply `rules` to `checkout`, or only measure them. One `ls-files` for the paths of all rules; a rule whose path is
/// unsafe is reported and skipped. A failed rule never stops the others
pub fn apply_clean_rules(git_executor: &GitCommandExecutor, checkout: &str, rules: &[ParsedRule], mode: CleanMode<'_>) -> Vec<Cleaned> {
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
          trashed: Vec::new(),
        },
        Ok(rule) => {
          let mut trashed = Vec::new();
          let result = match &tracked {
            Err(error) => Err(anyhow!("cannot tell what git tracks: {error}")),
            Ok(tracked) => check_path(Path::new(checkout), rule, tracked).and_then(|target| run_rule(rule, &target, mode, &mut trashed)),
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
            trashed,
          }
        }
      };
      debug!(checkout = %checkout, rule = %outcome.rule, freed = outcome.freed_bytes, trashed = outcome.trashed.len(), error = ?outcome.error, apply = mode.applies(), "clean rule");
      outcome
    })
    .collect()
}

/// The target of a rule in `checkout`: no symlink on its way, nothing under it that git tracks. The last component may
/// be a symlink for a `delete`, which removes the link and never follows it
fn check_path(checkout: &Path, rule: &CleanRule, tracked: &[String]) -> Result<PathBuf> {
  let path = rule.path();
  if let Some(file) = tracked.iter().find(|file| file.as_str() == path || file.starts_with(&format!("{path}/"))) {
    bail!("git tracks {file}");
  }
  let parts: Vec<&str> = path.split('/').collect();
  let mut target = checkout.to_path_buf();
  for (index, part) in parts.iter().enumerate() {
    target.push(part);
    let last = index + 1 == parts.len();
    if target.symlink_metadata().is_ok_and(|metadata| metadata.file_type().is_symlink()) && !(last && matches!(rule, CleanRule::Delete { .. })) {
      bail!("{} is a symlink, which a rule never follows", target.display());
    }
  }
  Ok(target)
}

fn run_rule(rule: &CleanRule, target: &Path, mode: CleanMode<'_>, trashed: &mut Vec<PathBuf>) -> Result<u64> {
  let Ok(metadata) = target.symlink_metadata() else { return Ok(0) };
  match rule {
    CleanRule::Delete { except, .. } => {
      if except.is_empty() || !metadata.is_dir() {
        return remove_entry(target, mode, trashed);
      }
      let entries = std::fs::read_dir(target).map_err(|error| anyhow!("cannot read {}: {error}", target.display()))?;
      let mut freed = 0;
      for entry in entries.flatten() {
        if !except.iter().any(|kept| entry.file_name() == kept.as_str()) {
          freed += remove_entry(&entry.path(), mode, trashed)?;
        }
      }
      Ok(freed)
    }
    CleanRule::Cargo { keep, age, .. } => {
      if !metadata.is_dir() {
        return Ok(0);
      }
      let cutoff = SystemTime::now().checked_sub(*age).unwrap_or(SystemTime::UNIX_EPOCH);
      let mut freed = 0;
      for profile in profile_dirs(target, PROFILE_DEPTH) {
        let mut groups = classic_units(&profile);
        add_build_dir_units(&mut groups, &profile.join("build"));
        add_incremental_units(&mut groups, &profile.join("incremental"));
        freed += remove_stale(groups, *keep, cutoff, mode.applies())?;
      }
      Ok(freed)
    }
  }
}

/// Remove `path` (a file, a symlink, a directory with what is in it) as `mode` says: measure it, delete it in place, or
/// move it into the trash, where its bytes are counted once it is deleted. On another volume than the trash it is
/// deleted in place
fn remove_entry(path: &Path, mode: CleanMode<'_>, trashed: &mut Vec<PathBuf>) -> Result<u64> {
  match mode {
    CleanMode::Measure => Ok(disk_size(path)),
    CleanMode::Delete => remove_measured(path).map_err(|error| anyhow!("cannot remove {}: {error}", path.display())),
    CleanMode::Trash(trash) => match trash.move_in(path) {
      Ok(moved) => {
        trashed.push(moved);
        Ok(0)
      }
      Err(error) => {
        debug!(path = %path.display(), error = %error, "no move into the trash, deleting in place");
        remove_measured(path).map_err(|error| anyhow!("cannot remove {}: {error}", path.display()))
      }
    },
  }
}

/// Delete `path` and everything below it, symlinks as themselves, in one walk that counts the bytes on disk it freed
pub fn remove_measured(path: &Path) -> std::io::Result<u64> {
  let metadata = match path.symlink_metadata() {
    Ok(metadata) => metadata,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
    Err(error) => return Err(error),
  };
  let own = allocated(&metadata);
  if !metadata.is_dir() {
    std::fs::remove_file(path)?;
    return Ok(own);
  }
  let mut freed = own;
  for entry in std::fs::read_dir(path)? {
    freed += remove_measured(&entry?.path())?;
  }
  std::fs::remove_dir(path)?;
  Ok(freed)
}

/// The directory a return or a clean moves what it deletes into, one per run: `<root>/<pid>-<start>-<uuid>` on the
/// volume of the repository. A rename there is instant, so the slot is freed before the long delete; `empty` deletes
/// it, and also the directories that runs whose process is gone left behind
pub struct Trash {
  root: PathBuf,
  dir: PathBuf,
}

impl Trash {
  /// The trash of this process under `root`, made on the first move
  pub fn new(root: &Path) -> Self {
    let pid = std::process::id();
    let started = process_started(pid).unwrap_or(0);
    Self {
      root: root.to_path_buf(),
      dir: root.join(format!("{pid}-{started}-{}", uuid::Uuid::new_v4())),
    }
  }

  /// Move `path` into the trash, under a number of its own
  fn move_in(&self, path: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(&self.dir)?;
    let count = std::fs::read_dir(&self.dir)?.count();
    let target = self.dir.join(count.to_string());
    std::fs::rename(path, &target)?;
    Ok(target)
  }

  /// Delete what `cleaned` moved into the trash, counting the bytes of each rule, then the directories of runs whose
  /// process is gone. A path that can't be deleted stays in the trash and is named in the error of its rule, and in the
  /// paths returned
  pub fn empty(&self, cleaned: &mut [Cleaned]) -> Vec<String> {
    let mut left = Vec::new();
    for rule in cleaned.iter_mut() {
      for path in std::mem::take(&mut rule.trashed) {
        match remove_measured(&path) {
          Ok(freed) => rule.freed_bytes += freed,
          Err(error) => {
            rule.error.get_or_insert_with(|| format!("cannot delete {} in the trash: {error}", path.display()));
            left.push(path.display().to_string());
          }
        }
      }
    }
    if left.is_empty() {
      let _ = std::fs::remove_dir(&self.dir);
    }
    self.sweep();
    left
  }

  /// Delete the trash directories of runs whose process is gone (killed by a timeout before it emptied its trash)
  fn sweep(&self) {
    let Ok(entries) = std::fs::read_dir(&self.root) else { return };
    for entry in entries.flatten() {
      let name = entry.file_name().to_string_lossy().into_owned();
      let mut parts = name.splitn(3, '-');
      let (Some(pid), Some(started)) = (parts.next().and_then(|pid| pid.parse().ok()), parts.next().and_then(|started| started.parse().ok())) else {
        continue;
      };
      if entry.path() == self.dir || is_running(pid, Some(started).filter(|started| *started != 0)) {
        continue;
      }
      match remove_measured(&entry.path()) {
        Ok(freed) => debug!(trash = %entry.path().display(), freed, "deleted the trash of a run that is gone"),
        Err(error) => warn!(trash = %entry.path().display(), error = %error, "cannot delete the trash of a run that is gone"),
      }
    }
  }
}

/// The `delete` rules as a get leaves them out of a clone (`clone_dir`): each path with the entries it keeps
pub fn clone_skips(rules: &[ParsedRule]) -> Vec<(String, Vec<String>)> {
  rules
    .iter()
    .filter_map(|rule| match rule {
      Ok(CleanRule::Delete { path, except }) => Some((path.clone(), except.clone())),
      _ => None,
    })
    .collect()
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

/// The files of one unit, removed together, and when the unit was last built (the newest of `last_built` of its files)
#[derive(Debug, Default)]
struct Unit {
  built: Option<SystemTime>,
  paths: Vec<PathBuf>,
}

impl Unit {
  fn add(&mut self, path: PathBuf) {
    let built = last_built(&path);
    self.built = Some(self.built.map_or(built, |known| known.max(built)));
    self.paths.push(path);
  }
}

/// The units of each group (a package or a crate, `-` and `_` the same), by an id unique in the profile dir
type Groups = HashMap<String, HashMap<String, Unit>>;

/// The units of the classic layout of a profile dir: `<name>-<16 hex>` in `deps/` (`lib<name>-<hash>.rlib`,
/// `.rmeta`, `<name>-<hash>.d`, the executable, its `.dSYM`, `.pdb` or `.exe`, the object files), `.fingerprint/` and
/// `build/` (a build script and its run, `out/`). One name and hash is one unit, whose files go together. The uplifted
/// artifacts at the top of the profile dir and `.cargo-lock` are never part of one
fn classic_units(profile: &Path) -> Groups {
  let mut groups = Groups::new();
  for dir in ["deps", ".fingerprint", "build"] {
    let Ok(entries) = std::fs::read_dir(profile.join(dir)) else { continue };
    for entry in entries.flatten() {
      // a symlink is never part of a unit: its target may be out of the slot
      if entry.file_type().is_ok_and(|file_type| file_type.is_symlink()) {
        continue;
      }
      let name = entry.file_name().to_string_lossy().into_owned();
      if let Some((group, hash)) = classic_unit_name(&name) {
        groups.entry(group).or_default().entry(hash).or_default().add(entry.path());
      }
    }
  }
  groups
}

/// The group and the hash of a file or a dir of the classic layout: the name before `-<16 hex digits>`, which the end
/// or a `.` follows, without the `lib` of a library file, `-` as `_`
pub fn classic_unit_name(file_name: &str) -> Option<(String, String)> {
  let bytes = file_name.as_bytes();
  let dash = file_name.match_indices('-').map(|(index, _)| index).find(|&index| {
    let hash = &bytes[index + 1..];
    hash.len() >= 16 && hash[..16].iter().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte)) && hash.get(16).is_none_or(|&next| next == b'.')
  })?;
  let rest = &file_name[dash + 17..];
  let mut name = &file_name[..dash];
  if [".rlib", ".rmeta", ".so", ".dylib", ".a"].contains(&rest) {
    name = name.strip_prefix("lib").unwrap_or(name);
  }
  (!name.is_empty()).then(|| (name.replace('-', "_"), file_name[dash + 1..dash + 17].to_string()))
}

/// The units of the build-dir layout: `build/<package>/<hash>/`; a classic `build/<pkg>-<hash>/` belongs to
/// `classic_units`
fn add_build_dir_units(groups: &mut Groups, build: &Path) {
  for package in subdirs(build) {
    let name = package.file_name().map_or_default(|name| name.to_string_lossy().into_owned());
    if classic_unit_name(&name).is_some() {
      continue;
    }
    for unit in subdirs(&package) {
      let id = format!("build/{name}/{}", unit.file_name().map_or_default(|hash| hash.to_string_lossy().into_owned()));
      groups.entry(name.replace('-', "_")).or_default().entry(id).or_default().add(unit);
    }
  }
}

/// The units of `incremental/<crate>-<hash>/`, grouped by crate (a crate name may hold `-`); their hash is not the one
/// of the other files, so each is a unit of its own
fn add_incremental_units(groups: &mut Groups, incremental: &Path) {
  for unit in subdirs(incremental) {
    let name = unit.file_name().map_or_default(|name| name.to_string_lossy().into_owned());
    let crate_name = name.rfind('-').map_or(name.as_str(), |dash| &name[..dash]).replace('-', "_");
    groups.entry(format!("incremental {crate_name}")).or_default().entry(name).or_default().add(unit);
  }
}

/// Remove the stale units of `groups`: a unit stays when it is among the `keep` last built of its group or was built
/// after `cutoff`, since a slot builds a dozen variants an hour and age alone keeps all. The files of a unit go together
fn remove_stale(groups: Groups, keep: usize, cutoff: SystemTime, apply: bool) -> Result<u64> {
  let mut freed = 0;
  for units in groups.into_values() {
    let mut units: Vec<Unit> = units.into_values().collect();
    units.sort_by_key(|unit| std::cmp::Reverse(unit.built));
    for unit in units.iter().skip(keep).filter(|unit| unit.built.is_some_and(|built| built < cutoff)) {
      for path in &unit.paths {
        freed += disk_size(path);
        if apply {
          let is_dir = path.symlink_metadata().is_ok_and(|metadata| metadata.is_dir());
          if is_dir { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) }.map_err(|error| anyhow!("cannot remove {}: {error}", path.display()))?;
        }
      }
    }
  }
  Ok(freed)
}

/// When a file or a dir of a unit was last built: a rebuild rewrites its fingerprint files (`build/`) or adds a
/// session (`incremental/`), not always the directory itself, so the newest mtime of the directory and of the direct
/// children of its `fingerprint` dir (else of itself) counts; of a file its own mtime
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
pub fn disk_size(path: &Path) -> u64 {
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
