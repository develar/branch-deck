//! The clean rules of a slot (`brd lease return`, `brd lease clean`, openspec/specs/cli/lease/spec.md): the `[lease]` table of
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
//! A `delete` of the dir of a `cargo` rule needs `except` (`{ delete = "target", except = ["debug", "release"] }`):
//! it removes every other entry of the target dir (the scratch an agent puts there), and the cargo rule then prunes only
//! the profile dirs at and below the kept entries (`cargo_scope`; of a dir that is a profile dir itself, the units in its
//! kept dirs), so the two rules never take the same path, whatever their order: a dry run counts each byte once, and
//! the trash gets each path from one rule.
//!
//! A path of a rule is relative and normalized, stays inside the slot, never names `.git`, has no symlink on its way
//! (a `delete` may name a symlink itself, and removes the link), and git tracks nothing under it (one `ls-files` for
//! all rules); a rule that breaks one of these is skipped and reported, never applied. The walk follows no symlink and
//! removes a link, a junction of Windows too, as the link (`fs_retry::remove_entry`), never what it points to, and
//! widens a directory it deletes that the owner may not write, as Bazel leaves the directories of its outputs. A
//! return and `brd lease clean` move what a rule removes into a trash directory first, a rename (a stale unit of a
//! `cargo` rule as a whole), and delete it once the slot is free again (`Trash`); a link they delete at once, which
//! costs no more than its move, so the trash holds no links. What a process keeps (on Windows one that works in it or
//! has a file of it open, which no rename moves) stays whole in the checkout and the error of its rule names it, never
//! half deleted: a later clean deletes it once it is free.
//!
//! A cargo rule that deletes takes the locks of cargo of each profile dir exclusive (`CargoLock`) while it moves the
//! stale units that it read before: no build, check or test of cargo works there meanwhile, and a build that starts
//! waits. A held lock skips that profile dir. After the release it reads the stale objects of rustc in the kept units
//! (`stale_objects`). A build may run meanwhile, and two guards keep what it reads, whatever the compile time. An
//! object that it compiles is younger than `age`, as the run started before it. An object that it links again from a
//! session of `incremental/` keeps its old mtime, but the sessions of every dir there hold its inode, read at the pass.
//! The objects go in place, never through the trash, when the caller empties its trash (`Cleaned::deferred`): a return
//! once the slot is free, the prune of a leased slot while the lease holds.

use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::tracked_under;
use history_rewrite::repository_lock::POLL_INTERVAL;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};
use sync_utils::fs_retry;
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
/// The lock files of cargo in a profile dir, in the order cargo takes them. A build holds `.cargo-build-lock` exclusive
/// and `.cargo-lock` shared, a cargo before 1.91 has only `.cargo-lock`, which it holds exclusive
const CARGO_LOCKS: [&str; 2] = [".cargo-build-lock", ".cargo-lock"];
/// How long a return waits for the locks of cargo of a profile dir (`LeaseReturnParams::cargo_lock_wait`): the return
/// found no process in the slot, so only another clean holds them. A clean holds them only while it moves its stale
/// units, one rename each, and reads the objects after the release
pub const RETURN_CARGO_LOCK_WAIT: Duration = Duration::from_secs(30);
/// The trash of a cargo rule in its profile dir, when the trash of the run is on another volume
const PROFILE_TRASH: &str = ".brd-trash";

/// A rule of `[lease] clean`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanRule {
  /// The units of the profile dirs under `dir` (a dir with `.cargo-lock`): `build/<package>/<hash>` stays when it is
  /// among the `keep` newest of its group or younger than `age`; `incremental/<crate>-<id>` when it is the newest of its
  /// crate, younger than `age`, or while the last built unit of a group of its crate (or a young one) wrote its newest
  /// session; in a kept unit the objects of rustc that no build reads go
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
  read_parsed(&checkout.join(BRANCH_DECK_FILE), parse_clean_rules).map(Option::unwrap_or_default)
}

/// The text of the file at `path` as `parse` reads it, `None` without the file; an error names the file
pub(crate) fn read_parsed<T>(path: &Path, parse: impl FnOnce(&str) -> Result<T>) -> Result<Option<T>> {
  match std::fs::read_to_string(path) {
    Ok(text) => parse(&text).map(Some).map_err(|error| error.context(format!("Cannot read {}", path.display()))),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(error) => Err(anyhow!(error).context(format!("Cannot read {}", path.display()))),
  }
}

/// The key of `[lease]` by which `brd lease pick` refuses a tree that no green build or test recorded (`check_record`)
const PICK_REQUIRES_CHECK: &str = "pick_requires_check";
/// The key of `[lease]` by which a record of an older tree carries over a clean update onto the target branch
/// (`check_record::check_tree`); true without it
const CHECK_CARRIES_OVER: &str = "check_carries_over";

/// The `[lease]` table of the text of a `.branchdeck.toml`, `None` without one. An unknown key of `[lease]` is an error;
/// another top-level table stays unread: its reader checks it (`[host]` of `host_link::config`), and a table of a
/// later version must not turn off the rules of this one
fn lease_table(text: &str) -> Result<Option<toml::Table>> {
  let mut table: toml::Table = text.parse()?;
  let Some(lease) = table.remove("lease") else { return Ok(None) };
  let toml::Value::Table(lease) = lease else { bail!("`lease` is not a table") };
  if let Some(key) = lease.keys().find(|key| *key != "clean" && *key != PICK_REQUIRES_CHECK && *key != CHECK_CARRIES_OVER) {
    bail!("unknown key lease.{key}; [lease] has only `clean`, `{PICK_REQUIRES_CHECK}` and `{CHECK_CARRIES_OVER}`");
  }
  Ok(Some(lease))
}

/// The gate of the check of the tree of a pick, as `[lease]` of `.branchdeck.toml` sets it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickCheck {
  /// `pick_requires_check`, false by default: a pick lands only a tree that a green build or test recorded
  pub required: bool,
  /// `check_carries_over`, true by default: a record of an older tree counts for a clean update of it onto the target
  pub carries_over: bool,
}

impl Default for PickCheck {
  fn default() -> Self {
    Self {
      required: false,
      carries_over: true,
    }
  }
}

/// The gate of the check of `.branchdeck.toml` in `checkout`: the defaults without the file or the keys. A file that
/// does not parse, an unknown key or a value that is no boolean is an error
pub fn read_pick_check(checkout: &Path) -> Result<PickCheck> {
  read_parsed(&checkout.join(BRANCH_DECK_FILE), parse_pick_check).map(Option::unwrap_or_default)
}

/// The gate of the check of the text of a `.branchdeck.toml`
pub fn parse_pick_check(text: &str) -> Result<PickCheck> {
  let lease = lease_table(text)?;
  let flag = |key: &str, default: bool| match lease.as_ref().and_then(|lease| lease.get(key)) {
    None => Ok(default),
    Some(value) => value.as_bool().ok_or_else(|| anyhow!("`lease.{key}` is not a boolean")),
  };
  let defaults = PickCheck::default();
  Ok(PickCheck {
    required: flag(PICK_REQUIRES_CHECK, defaults.required)?,
    carries_over: flag(CHECK_CARRIES_OVER, defaults.carries_over)?,
  })
}

/// The rules of the text of a `.branchdeck.toml`
pub fn parse_clean_rules(text: &str) -> Result<Vec<ParsedRule>> {
  let Some(lease) = lease_table(text)? else { return Ok(Vec::new()) };
  let Some(clean) = lease.get("clean") else { return Ok(Vec::new()) };
  let clean = clean.as_array().ok_or_else(|| anyhow!("`lease.clean` is not an array"))?;
  let rules: Vec<ParsedRule> = clean
    .iter()
    .enumerate()
    .map(|(index, rule)| parse_rule(rule).map_err(|error| error.context(format!("lease.clean[{index}]"))))
    .collect::<Result<_>>()?;
  Ok(refuse_repeated_deletes(refuse_deletes_of_cargo_dirs(rules)))
}

/// A `delete` of a path that an earlier one has is reported, never applied: a path has one whitelist, so a dry run
/// measures each of its entries once (two deletes of one path would both measure an entry that the first then removes),
/// a clone leaves out what the clean removes (`clone_skips`, one `except` a path), and a cargo rule of that dir prunes
/// the entries of one `except` (`cargo_scope`)
fn refuse_repeated_deletes(rules: Vec<ParsedRule>) -> Vec<ParsedRule> {
  let mut deleted = HashSet::new();
  rules
    .into_iter()
    .map(|rule| match rule {
      Ok(CleanRule::Delete { path, except }) if !deleted.insert(path.clone()) => {
        let reason = format!("an earlier rule deletes {path}; name what it keeps in the `except` of that one");
        Err((CleanRule::Delete { path, except }.describe(), reason))
      }
      other => other,
    })
    .collect()
}

/// A `delete` whose path holds the dir of a `cargo` rule, without the entry on its way in `except`, or that is the dir
/// itself without `except`, would remove what the cargo rule keeps: it is reported, never applied. One of the dir with
/// `except` removes the other entries, and the cargo rule prunes the kept ones (`cargo_scope`)
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
          (*dir == &path && except.is_empty())
            || dir
              .strip_prefix(&format!("{path}/"))
              .is_some_and(|rest| !except.iter().any(|kept| rest.split('/').next() == Some(kept.as_str())))
        });
        match covered {
          Some(dir) => {
            let fix = if *dir == path {
              "name the entries it keeps (its profile dirs) in `except`"
            } else {
              "name its entry in `except`"
            };
            let rule = CleanRule::Delete { path, except };
            Err((rule.describe(), format!("it would delete {dir}, the dir of a cargo rule; {fix}")))
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

/// The parameters of a call that change the rules of the file without an edit of it (`brd lease clean --keep 2`)
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
  /// What it deletes in place when the caller empties the trash: the stale objects of rustc in the kept units of a
  /// cargo rule, and the trash of a cargo rule in its profile dir (`.brd-trash/`). `Trash::empty` deletes it after
  /// `trashed` and counts its bytes; a delete that stops leaves the rest in place for the next clean. A return empties
  /// the trash once the slot is free, the prune of a leased slot while the lease holds
  pub deferred: Vec<PathBuf>,
  /// What a cargo rule did with its profile dirs
  pub profiles: Profiles,
}

/// What a cargo rule did with its profile dirs (`Cleaned::profiles`)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profiles {
  /// How many it pruned, or measured in a dry run
  pub pruned: usize,
  /// The ones it left as they are for a held lock of cargo, relative to the checkout: `target/debug`
  pub held: Vec<String>,
  /// It stopped under the locks, as `CleanOptions::still_holds` no longer held (the lease of the slot changed)
  pub changed: bool,
}

/// How `apply_clean_rules` applies the rules
#[derive(Clone, Copy)]
pub enum CleanMode<'a> {
  /// Measure only (`--dry-run`)
  Measure,
  /// Delete in place
  Delete,
  /// Move what a rule removes into the trash, a rename; `Trash::empty` deletes it. On another volume than the trash the
  /// paths of a `delete` go in place, the stale units of a cargo rule into `.brd-trash/` of their profile dir. A link
  /// is deleted in place
  Trash(&'a Trash),
}

impl CleanMode<'_> {
  pub(crate) const fn applies(self) -> bool {
    !matches!(self, Self::Measure)
  }
}

/// How a clean meets the locks of cargo (`apply_clean_rules_with`)
#[derive(Clone, Copy, Default)]
pub struct CleanOptions<'a> {
  /// How long a cargo rule waits for a held lock of cargo of a profile dir before it skips that dir; zero skips at once
  pub cargo_lock_wait: Duration,
  /// Only the cargo rules run, the others are neither applied nor reported: the prune of a leased slot, whose `delete`
  /// rules name the files of its holder. The whole list still gives the scope of a cargo rule (`cargo_scope`)
  pub cargo_only: bool,
  /// Asked under the locks of cargo of each profile dir, before its stale units move: false leaves the rest of the rule
  /// undone (`Profiles::changed`). The prune of a leased slot asks whether its lease is still the one it read
  pub still_holds: Option<&'a dyn Fn() -> bool>,
}

/// Apply `rules` to `checkout`, or only measure them. One `ls-files` for the paths of all rules; a rule whose path is
/// unsafe is reported and skipped. A failed rule never stops the others. A held lock of cargo skips its profile dir at
/// once
pub fn apply_clean_rules(git_executor: &GitCommandExecutor, checkout: &str, rules: &[ParsedRule], mode: CleanMode<'_>) -> Vec<Cleaned> {
  apply_clean_rules_with(git_executor, checkout, rules, mode, CleanOptions::default())
}

/// `apply_clean_rules` with `options`
pub fn apply_clean_rules_with(git_executor: &GitCommandExecutor, checkout: &str, rules: &[ParsedRule], mode: CleanMode<'_>, options: CleanOptions<'_>) -> Vec<Cleaned> {
  apply_rules(checkout, rules, &tracked_by_rules(git_executor, checkout, rules), mode, options)
}

/// What git tracks under the paths of the rules of a checkout: the git of `apply_clean_rules`, or why it failed
pub struct TrackedPaths(Result<Vec<String>, String>);

/// The one `ls-files` of `apply_clean_rules`, for the paths of all `rules` in `checkout`: a caller that must run its git
/// in one step and its walk in another (the give-back of a host slot) calls it, then `apply_tracked_rules`
pub fn tracked_by_rules(git_executor: &GitCommandExecutor, checkout: &str, rules: &[ParsedRule]) -> TrackedPaths {
  let paths: Vec<&str> = rules.iter().filter_map(|rule| rule.as_ref().ok()).map(CleanRule::path).collect();
  TrackedPaths(tracked_under(git_executor, checkout, &paths).map_err(|error| format!("{error:#}")))
}

/// `apply_clean_rules` with what git tracks (`tracked_by_rules`): no git, only the walk
pub fn apply_tracked_rules(checkout: &str, rules: &[ParsedRule], tracked: &TrackedPaths, mode: CleanMode<'_>) -> Vec<Cleaned> {
  apply_rules(checkout, rules, tracked, mode, CleanOptions::default())
}

fn apply_rules(checkout: &str, rules: &[ParsedRule], tracked: &TrackedPaths, mode: CleanMode<'_>, options: CleanOptions<'_>) -> Vec<Cleaned> {
  let tracked = &tracked.0;
  let applies = |rule: &ParsedRule| {
    !options.cargo_only
      || match rule {
        Ok(rule) => matches!(rule, CleanRule::Cargo { .. }),
        Err((described, _)) => described.starts_with("cargo "),
      }
  };
  rules
    .iter()
    .filter(|rule| applies(rule))
    .map(|rule| {
      let started = Instant::now();
      let outcome = match rule {
        Err((rule, reason)) => Cleaned {
          rule: rule.clone(),
          freed_bytes: 0,
          elapsed: Duration::ZERO,
          error: Some(reason.clone()),
          trashed: Vec::new(),
          deferred: Vec::new(),
          profiles: Profiles::default(),
        },
        Ok(rule) => {
          let mut moved = Moved::default();
          let result = match &tracked {
            Err(error) => Err(anyhow!("cannot tell what git tracks: {error}")),
            Ok(tracked) => check_path(Path::new(checkout), rule, tracked)
              .and_then(|target| run_rule(rule, cargo_scope(rules, rule).as_deref(), &target, (mode, options), &mut moved)),
          };
          let (freed_bytes, error, profiles) = match result {
            Ok(removals) => (removals.freed, removals.error(), removals.profiles),
            Err(error) => (0, Some(format!("{error:#}")), Profiles::default()),
          };
          Cleaned {
            rule: rule.describe(),
            freed_bytes,
            elapsed: started.elapsed(),
            error,
            trashed: moved.trashed,
            deferred: moved.deferred,
            profiles,
          }
        }
      };
      debug!(checkout = %checkout, rule = %outcome.rule, freed = outcome.freed_bytes, trashed = outcome.trashed.len(), deferred = outcome.deferred.len(), error = ?outcome.error, apply = mode.applies(), "clean rule");
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

/// What the removals of one rule freed, and the ones that failed: a failed one never stops the next
#[derive(Default)]
struct Removals {
  freed: u64,
  failed: Vec<String>,
  profiles: Profiles,
}

impl Removals {
  fn add(&mut self, removal: Result<u64>) {
    match removal {
      Ok(freed) => self.freed += freed,
      Err(error) => self.failed.push(format!("{error:#}")),
    }
  }

  /// The error of the rule: every removal that failed
  fn error(&self) -> Option<String> {
    (!self.failed.is_empty()).then(|| self.failed.join("; "))
  }
}

/// The entries of the dir of a cargo rule whose profile dirs it prunes, `None` for the whole dir: with the `delete` of the
/// same dir (one, `refuse_repeated_deletes`; `except` is not empty there, `refuse_deletes_of_cargo_dirs`), the entries it
/// keeps, since it removes the others whole; of a dir that is a profile dir itself, the dirs of its units among them
/// (`run_rule`). So the two rules never take the same path, in whatever order the file has them
fn cargo_scope<'a>(rules: &'a [ParsedRule], rule: &CleanRule) -> Option<Vec<&'a str>> {
  let CleanRule::Cargo { dir, .. } = rule else { return None };
  rules.iter().find_map(|other| match other {
    Ok(CleanRule::Delete { path, except }) if path == dir => Some(except.iter().map(String::as_str).collect()),
    _ => None,
  })
}

/// What a rule moved into the trash, and what it deletes in place once the slot is free (`Cleaned`)
#[derive(Default)]
struct Moved {
  trashed: Vec<PathBuf>,
  deferred: Vec<PathBuf>,
}

/// Apply `rule` at `target`; `scope` names the entries of a cargo rule's dir it prunes (`cargo_scope`)
fn run_rule(rule: &CleanRule, scope: Option<&[&str]>, target: &Path, (mode, options): (CleanMode<'_>, CleanOptions<'_>), moved: &mut Moved) -> Result<Removals> {
  let mut removals = Removals::default();
  let Ok(metadata) = target.symlink_metadata() else { return Ok(removals) };
  match rule {
    CleanRule::Delete { except, .. } => {
      if except.is_empty() || !metadata.is_dir() {
        removals.add(remove_together(&[target.to_path_buf()], mode, &mut moved.trashed));
        return Ok(removals);
      }
      let entries = std::fs::read_dir(target).map_err(|error| anyhow!("cannot read {}: {error}", target.display()))?;
      for entry in entries.flatten() {
        if !except.iter().any(|kept| entry.file_name() == kept.as_str()) {
          removals.add(remove_together(&[entry.path()], mode, &mut moved.trashed));
        }
      }
    }
    CleanRule::Cargo { dir, keep, age } => {
      if !metadata.is_dir() {
        return Ok(removals);
      }
      // each profile dir, with the dirs of it whose units the walk reads (`None` for all)
      let profiles: Vec<(PathBuf, Option<Vec<PathBuf>>)> = match scope {
        None => profile_dirs(target, PROFILE_DEPTH).into_iter().map(|profile| (profile, None)).collect(),
        // the dir is a profile dir itself: its kept dirs (`deps`, `build`, ...) alone, the delete removes the others
        Some(kept) if is_profile_dir(target) => vec![(target.to_path_buf(), Some(kept_dirs(target, kept)))],
        // a kept entry that is a directory, not a link to one, and the profile dirs at and below it
        Some(kept) => kept_dirs(target, kept)
          .iter()
          .flat_map(|entry| profile_dirs(entry, PROFILE_DEPTH - 1))
          .map(|profile| (profile, None))
          .collect(),
      };
      for (profile, walked) in profiles {
        if removals.profiles.changed {
          break;
        }
        // the profile dir as the messages name it, relative to the checkout: `target/debug`
        let named = Path::new(dir).join(profile.strip_prefix(target).unwrap_or(&profile));
        let pruned = ProfilePrune {
          profile: &profile,
          named: &named,
          walked: walked.as_deref(),
          keep: *keep,
          age: *age,
        };
        pruned.run(mode, options, &mut removals, moved);
      }
    }
  }
  Ok(removals)
}

/// One profile dir of a cargo rule
struct ProfilePrune<'a> {
  profile: &'a Path,
  /// The profile dir relative to the checkout, for the messages
  named: &'a Path,
  /// The dirs of it whose units the walk reads, `None` for all
  walked: Option<&'a [PathBuf]>,
  keep: usize,
  age: Duration,
}

impl ProfilePrune<'_> {
  /// Read the units, keep the `keep` newest of each group and the ones built within `age`; then, under the locks of cargo
  /// when the rule deletes, move the others, each one only while no build wrote it since the read. Then, after the
  /// release, the stale objects of the kept units (`stale_objects`). A held lock skips the dir, its error in the error
  /// of the rule
  fn run(&self, mode: CleanMode<'_>, options: CleanOptions<'_>, removals: &mut Removals, moved: &mut Moved) {
    let started = Instant::now();
    let cutoff = SystemTime::now().checked_sub(self.age).unwrap_or(SystemTime::UNIX_EPOCH);
    // the walk before the locks, which a build of a large profile dir would wait for: a build meanwhile only writes new
    // units or rewrites the fingerprint of one, which the check under the locks sees
    let walks = |name: &str| self.walked.is_none_or(|dirs| dirs.contains(&self.profile.join(name)));
    let mut units = classic_units(self.profile, &walks);
    if walks("build") {
      add_build_dir_units(&mut units, &self.profile.join("build"));
    }
    if walks("incremental") {
      add_incremental_units(&mut units, &self.profile.join("incremental"));
    }
    let (stale, kept) = split_units(units, self.keep, cutoff);
    let walk = started.elapsed();
    let lock = if mode.applies() {
      match CargoLock::take(self.profile, options.cargo_lock_wait) {
        Ok(lock) => Some(lock),
        Err(error) => {
          if matches!(error, NotLocked::Held) {
            removals.profiles.held.push(self.named.display().to_string());
          }
          removals.failed.push(format!("{error}: {}", self.named.display()));
          return;
        }
      }
    } else {
      None
    };
    // under the locks: no clean of another lease starts here once its holder got the slot
    if let Some(still_holds) = options.still_holds
      && !still_holds()
    {
      removals.profiles.changed = true;
      return;
    }
    removals.profiles.pruned += 1;
    let locked_at = Instant::now();
    let mut own_trash = None;
    let mut built_again = 0;
    for unit in &stale {
      if mode.applies() && unit.built_again() {
        built_again += 1;
        continue;
      }
      removals.add(self.remove_unit(&unit.paths, mode, &mut own_trash, &mut moved.trashed));
    }
    let held = locked_at.elapsed();
    // a build that waits starts now, while the objects are read: what it compiles is newer than `cutoff`, and what it
    // links again from a session `stale_objects` reads in every dir of `incremental/`
    drop(lock);
    let objects_started = Instant::now();
    let objects = stale_objects(&kept, &self.profile.join("incremental"), cutoff);
    let object_count = objects.len();
    match mode {
      CleanMode::Measure => removals.freed += measure_objects(&objects),
      CleanMode::Delete => {
        for object in &objects {
          removals.add(remove_measured(&object.path).map_err(|error| anyhow!("cannot remove {}: {error}", object.path.display())));
        }
      }
      CleanMode::Trash(_) => moved.deferred.extend(objects.into_iter().map(|object| object.path)),
    }
    moved.deferred.extend(own_trash.map(|trash| trash.dir().to_path_buf()));
    if mode.applies() {
      sweep_profile_trash(self.profile);
    }
    debug!(
      profile = %self.profile.display(),
      walk_ms = walk.as_millis(),
      held_ms = held.as_millis(),
      built_again,
      objects_ms = objects_started.elapsed().as_millis(),
      stale_units = stale.len(),
      kept_units = kept.len(),
      stale_objects = object_count,
      apply = mode.applies(),
      "pruned a profile dir under the locks of cargo"
    );
  }

  /// Remove the paths of one stale unit as `mode` says. Into the trash of the run while it is on the volume of the
  /// profile dir; else into `own_trash`, made in the profile dir on the first unit that crosses (`.brd-trash/`), so no
  /// delete runs under the locks. Its dir goes after the release (`Moved::deferred`)
  fn remove_unit(&self, paths: &[PathBuf], mode: CleanMode<'_>, own_trash: &mut Option<Trash>, trashed: &mut Vec<PathBuf>) -> Result<u64> {
    let CleanMode::Trash(trash) = mode else { return remove_together(paths, mode, trashed) };
    if let Some(own) = own_trash {
      return trash_together(paths, own, &mut Vec::new()).map_err(Together::into_error);
    }
    match trash_together(paths, trash, trashed) {
      Err(Together::CrossesDevices(error)) => {
        debug!(profile = %self.profile.display(), error = %error, "the trash is on another volume, moving the stale units into the profile dir");
        let own = own_trash.insert(Trash::new(&self.profile.join(PROFILE_TRASH)));
        trash_together(paths, own, &mut Vec::new()).map_err(Together::into_error)
      }
      other => other.map_err(Together::into_error),
    }
  }
}

/// Delete the trash dirs of runs that are gone in `.brd-trash/` of `profile`, where a cargo rule moves its stale units
/// when the trash of its run is on another volume; one look at a missing dir costs no more
fn sweep_profile_trash(profile: &Path) {
  let root = profile.join(PROFILE_TRASH);
  if root.symlink_metadata().is_ok() {
    Trash::new(&root).sweep();
  }
}

/// Why `CargoLock::take` did not take the locks of a profile dir
#[derive(Debug, thiserror::Error)]
enum NotLocked {
  #[error("the lock of cargo is held (a build or another clean)")]
  Held,
  #[error("cannot lock {path} of cargo: {error}")]
  Failed { path: String, error: std::io::Error },
}

/// The locks of cargo of one profile dir (`CARGO_LOCKS`), held exclusive: no cargo command builds there meanwhile, and
/// no other clean prunes it. The files are opened to read only, never created, written or deleted; the drop closes them,
/// which releases the locks
struct CargoLock {
  _files: Vec<std::fs::File>,
}

impl CargoLock {
  /// Take each lock of `profile` in the order of cargo, trying a held one again every `POLL_INTERVAL` for up to `wait`.
  /// A profile dir of a cargo before 1.91 has no `.cargo-build-lock`
  fn take(profile: &Path, wait: Duration) -> Result<Self, NotLocked> {
    let deadline = Instant::now() + wait;
    let mut files = Vec::with_capacity(CARGO_LOCKS.len());
    for name in CARGO_LOCKS {
      let path = profile.join(name);
      let failed = |error| NotLocked::Failed {
        path: path.display().to_string(),
        error,
      };
      let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && name != ".cargo-lock" => continue,
        Err(error) => return Err(failed(error)),
      };
      loop {
        match file.try_lock() {
          Ok(()) => break,
          Err(std::fs::TryLockError::WouldBlock) => {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
              return Err(NotLocked::Held);
            }
            std::thread::sleep(POLL_INTERVAL.min(left));
          }
          Err(std::fs::TryLockError::Error(error)) => return Err(failed(error)),
        }
      }
      files.push(file);
    }
    Ok(Self { _files: files })
  }
}

/// Remove `paths` (files, symlinks, directories with what is in them) together, as `mode` says: measure them, delete
/// them in place, or move them into the trash, where their bytes are counted once it is deleted. A path that can't be
/// moved (on Windows a process works in it or has a file of it open) puts back the ones moved before it, so what a
/// holder keeps stays whole in the checkout and the error names it, never half deleted; on another volume than the
/// trash they are deleted in place. A link (a symlink, a junction of Windows) never goes into the trash: once every
/// other path is moved it is deleted in place as the link (`fs_retry::remove_entry`), its target kept, and its bytes
/// are counted at once; a failed move leaves it too, since a deleted link can't be put back
pub(crate) fn remove_together(paths: &[PathBuf], mode: CleanMode<'_>, trashed: &mut Vec<PathBuf>) -> Result<u64> {
  let trash = match mode {
    CleanMode::Measure => return Ok(paths.iter().map(|path| fs_walk::tree_size(path)).sum()),
    CleanMode::Delete => return delete_together(paths),
    CleanMode::Trash(trash) => trash,
  };
  match trash_together(paths, trash, trashed) {
    Err(Together::CrossesDevices(error)) => {
      debug!(path = %paths.first().map_or_default(|path| path.display().to_string()), error = %error, "the trash is on another volume, deleting in place");
      delete_together(paths)
    }
    other => other.map_err(Together::into_error),
  }
}

/// Delete `paths` in place, counting the bytes they freed
fn delete_together(paths: &[PathBuf]) -> Result<u64> {
  paths.iter().try_fold(0, |freed, path| {
    Ok(freed + remove_measured(path).map_err(|error| anyhow!("cannot remove {}: {error}", path.display()))?)
  })
}

/// Why `trash_together` moved nothing
enum Together {
  /// The first path is on another volume than the trash: nothing moved
  CrossesDevices(std::io::Error),
  /// A move or the delete of a link failed; what moved before is back in place
  Failed(anyhow::Error),
}

impl Together {
  fn into_error(self) -> anyhow::Error {
    match self {
      Self::CrossesDevices(error) => anyhow!("cannot move into the trash on another volume: {error}"),
      Self::Failed(error) => error,
    }
  }
}

/// `remove_together` into `trash`: what moved goes into `trashed`, the links are deleted in place and counted
fn trash_together(paths: &[PathBuf], trash: &Trash, trashed: &mut Vec<PathBuf>) -> Result<u64, Together> {
  let mut links: Vec<(&PathBuf, std::fs::Metadata)> = Vec::new();
  let mut moved: Vec<(&PathBuf, PathBuf)> = Vec::new();
  for path in paths {
    if let Ok(metadata) = path.symlink_metadata()
      && metadata.file_type().is_symlink()
    {
      links.push((path, metadata));
      continue;
    }
    match trash.move_in(path) {
      Ok(target) => moved.push((path, target)),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices && moved.is_empty() => return Err(Together::CrossesDevices(error)),
      Err(error) => {
        let error = removal_error(path, error);
        for (path, target) in moved.into_iter().rev() {
          if let Err(back) = fs_retry::rename(&target, path) {
            warn!(path = %path.display(), trash = %target.display(), error = %back, "cannot move a path back from the trash");
          }
        }
        return Err(Together::Failed(anyhow!("cannot move {} into the trash: {error}{}", path.display(), held_note(&error))));
      }
    }
  }
  trashed.extend(moved.into_iter().map(|(_, target)| target));
  links
    .into_iter()
    .try_fold(0, |freed, (path, metadata)| match fs_retry::remove_entry(path, &metadata.file_type()) {
      Ok(()) => Ok(freed + allocated(&metadata)),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(freed),
      Err(error) => {
        let error = removal_error(path, error);
        Err(Together::Failed(anyhow!("cannot remove the link {}: {error}{}", path.display(), held_note(&error))))
      }
    })
}

/// What the error of a held path tells to do: no scan of Windows names the process, but its refusal of the move says
/// that one holds the path
fn held_note(error: &std::io::Error) -> &'static str {
  if fs_retry::is_held(error) {
    "; a process holds it (works in it or has a file of it open): once it is closed, `brd lease clean` deletes it"
  } else {
    ""
  }
}

#[cfg(target_os = "macos")]
const EPERM: i32 = 1;

#[derive(Debug)]
struct ProtectedAppRemoval {
  path: PathBuf,
  bundle: PathBuf,
  source: std::io::Error,
}

impl std::fmt::Display for ProtectedAppRemoval {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      formatter,
      "macOS denied removal of {} inside app bundle {}; move the whole bundle to the Trash in Finder to remove it: {}",
      self.path.display(),
      self.bundle.display(),
      self.source
    )
  }
}

impl std::error::Error for ProtectedAppRemoval {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(&self.source)
  }
}

#[cfg_attr(
  not(target_os = "macos"),
  expect(clippy::missing_const_for_fn, reason = "only macOS reads the path, to name a protected app bundle")
)]
fn removal_error(path: &Path, error: std::io::Error) -> std::io::Error {
  #[cfg(target_os = "macos")]
  if error.raw_os_error() == Some(EPERM)
    && let Some(bundle) = path.ancestors().find(|ancestor| ancestor.extension() == Some(std::ffi::OsStr::new("app")))
  {
    return std::io::Error::new(
      error.kind(),
      ProtectedAppRemoval {
        path: path.to_path_buf(),
        bundle: bundle.to_path_buf(),
        source: error,
      },
    );
  }
  #[cfg(not(target_os = "macos"))]
  let _ = path;
  error
}

fn trash_left_path(path: &Path, error: &std::io::Error) -> String {
  if error.get_ref().and_then(|source| source.downcast_ref::<ProtectedAppRemoval>()).is_some() {
    error.to_string()
  } else {
    path.display().to_string()
  }
}

/// Delete `path` and everything below it in one walk that counts the bytes on disk it freed (`unlink_frees`: a hard
/// link that another link outlives frees nothing, an APFS clone its private size only). A link, a junction or a
/// directory symlink of Windows too, is removed as the link (`fs_retry::remove_entry`): never followed, never gone into.
/// A directory the owner may not read, write and search, as Bazel leaves every directory of an output it wrote,
/// refuses the walk of it and the removal of what is in it: it is widened before the walk goes into it (`widen_dir`),
/// which no directory that stays ever sees
pub fn remove_measured(path: &Path) -> std::io::Result<u64> {
  remove_measured_until(path, &|| false).map(|removed| removed.freed)
}

/// What a delete that may stop did (`remove_measured_until`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Removed {
  /// The bytes on disk it freed
  pub freed: u64,
  /// It deleted all of the path; false: `stop` held first, and the rest stays where it was
  pub complete: bool,
}

/// How many entries a delete that may stop removes between two asks of its `stop` (`remove_measured_until`): one
/// `statfs` a time, a few thousand a second against the 3,000 entries a second one thread deletes under load
const STOP_EVERY: u32 = 256;

/// `remove_measured`, which asks `stop` before its first entry and then once every `STOP_EVERY` entries, and leaves what
/// it has not deleted yet where it is once `stop` holds: the delete of the shrink or the eviction of a get stops as soon
/// as the volume has its minimum, as a slot of idea-1 is 1.4 million files and takes minutes, every get behind it
/// waiting
pub fn remove_measured_until(path: &Path, stop: &dyn Fn() -> bool) -> std::io::Result<Removed> {
  remove_counted(path, stop, &mut 0)
}

fn remove_counted(path: &Path, stop: &dyn Fn() -> bool, seen: &mut u32) -> std::io::Result<Removed> {
  remove_counted_told(path, None, stop, seen)
}

/// `remove_counted` of `path`, whose private size the read of its directory told (`private_sizes`), else `None`
fn remove_counted_told(path: &Path, private: Option<u64>, stop: &dyn Fn() -> bool, seen: &mut u32) -> std::io::Result<Removed> {
  if seen.is_multiple_of(STOP_EVERY) && stop() {
    return Ok(Removed { freed: 0, complete: false });
  }
  *seen = seen.wrapping_add(1);
  let metadata = match path.symlink_metadata() {
    Ok(metadata) => metadata,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Removed { freed: 0, complete: true }),
    Err(error) => return Err(removal_error(path, error)),
  };
  if !metadata.is_dir() {
    let own = unlink_frees(&metadata, private);
    fs_retry::remove_entry(path, &metadata.file_type()).map_err(|error| removal_error(path, error))?;
    return Ok(Removed { freed: own, complete: true });
  }
  widen_dir(path, &metadata);
  let mut freed = allocated(&metadata);
  let private_sizes = private_sizes(path);
  for entry in std::fs::read_dir(path).map_err(|error| removal_error(path, error))? {
    let entry = entry.map_err(|error| removal_error(path, error))?;
    let private = private_sizes.get(&entry.file_name()).copied();
    let removed = remove_counted_told(&entry.path(), private, stop, seen)?;
    freed += removed.freed;
    if !removed.complete {
      return Ok(Removed { freed, complete: false });
    }
  }
  fs_retry::remove_dir(path).map_err(|error| removal_error(path, error))?;
  Ok(Removed { freed, complete: true })
}

/// The bytes the unlink of the file of `metadata` frees: nothing while another hard link keeps its blocks (cargo links
/// `deps/` into the profile dir), else its private size as the read of its directory told it (an APFS clone frees only
/// the blocks it shares with no other file), else its blocks. The private size is read before the deletes of the
/// directory, so a clone that a later delete leaves alone counts less than it frees, never more
fn unlink_frees(metadata: &std::fs::Metadata, private: Option<u64>) -> u64 {
  #[cfg(unix)]
  {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() > 1 {
      return 0;
    }
  }
  private.unwrap_or_else(|| allocated(metadata))
}

/// The private size of each file right in the directory `path` (`fs_walk::private_sizes`); none where it can't be read,
/// and the delete counts the blocks of each file instead
#[cfg(target_os = "macos")]
fn private_sizes(path: &Path) -> HashMap<std::ffi::OsString, u64> {
  fs_walk::private_sizes(path).unwrap_or_default()
}

/// Nothing shares the blocks of a file off macOS: the delete counts its blocks
#[cfg(not(target_os = "macos"))]
fn private_sizes(_path: &Path) -> HashMap<std::ffi::OsString, u64> {
  HashMap::new()
}

/// Give the owner of the directory `path`, which is to be deleted, the rights to read, write and search it, the ones a
/// removal of what is in it needs: Bazel takes them from every directory of an output it wrote (`dr-xr-xr-x`), and
/// without them a trash of such a tree stays and grows. The mode as it is costs no call, and a failure is none: the
/// caller reports the refusal of the removal, not of the mode
#[cfg(unix)]
fn widen_dir(path: &Path, metadata: &std::fs::Metadata) {
  use std::os::unix::fs::PermissionsExt;
  /// Read, write and search for the owner
  const OWNER_RWX: u32 = 0o700;
  let mode = metadata.permissions().mode();
  if mode & OWNER_RWX != OWNER_RWX {
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode | OWNER_RWX));
  }
}

/// Nothing on Windows: the read-only attribute of a directory refuses no removal of what is in it, and the one of the
/// directory itself `fs_retry` clears
#[cfg(not(unix))]
#[expect(clippy::missing_const_for_fn, reason = "the signature of the Unix variant")]
fn widen_dir(_path: &Path, _metadata: &std::fs::Metadata) {}

/// The directory a return or a clean moves what it deletes into, one per run: `<root>/<pid>-<start>-<uuid>` on the
/// volume of the repository. A rename there is instant, so the slot is freed before the long delete; `empty` deletes
/// it, and also the directories that runs whose process is gone left behind
pub struct Trash {
  root: PathBuf,
  dir: PathBuf,
  /// The name of the next path moved in: a path moved back out never frees its name for another
  next: AtomicUsize,
  /// What was moved in for no rule (`adopt`)
  adopted: Mutex<Vec<PathBuf>>,
  /// A test seam: every move fails as one onto another volume
  #[cfg(test)]
  crosses_devices: bool,
}

impl Trash {
  /// The trash of this process under `root`, made on the first move
  pub fn new(root: &Path) -> Self {
    let pid = std::process::id();
    let started = process_started(pid).unwrap_or(0);
    Self {
      root: root.to_path_buf(),
      dir: root.join(format!("{pid}-{started}-{}", uuid::Uuid::new_v4())),
      next: AtomicUsize::new(0),
      adopted: Mutex::default(),
      #[cfg(test)]
      crosses_devices: false,
    }
  }

  /// A trash under `root` whose every move fails as one onto another volume (`CrossesDevices`)
  #[cfg(all(test, unix))]
  pub(crate) fn on_another_volume(root: &Path) -> Self {
    Self {
      crosses_devices: true,
      ..Self::new(root)
    }
  }

  /// The paths `remove_together` moved in for no rule (the seeded paths of a return): `empty` deletes them too
  pub(crate) fn adopt(&self, moved: Vec<PathBuf>) {
    self.adopted.lock().unwrap_or_else(PoisonError::into_inner).extend(moved);
  }

  /// The directory of this run in the trash, made on the first move
  pub(crate) fn dir(&self) -> &Path {
    &self.dir
  }

  /// Move `path` into the trash, under a number of its own (`fs_retry::rename`: a holder that lets go soon is waited
  /// for)
  pub(crate) fn move_in(&self, path: &Path) -> std::io::Result<PathBuf> {
    #[cfg(test)]
    if self.crosses_devices {
      return Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices));
    }
    std::fs::create_dir_all(&self.dir)?;
    let target = self.dir.join(self.next.fetch_add(1, Ordering::Relaxed).to_string());
    fs_retry::rename(path, &target)?;
    Ok(target)
  }

  /// Delete what `cleaned` moved into the trash, counting the bytes of each rule, then what it defers
  /// (`Cleaned::deferred`), then what it adopted (`adopt`), then the directories of runs whose process is gone. A path
  /// that can't be deleted stays in the trash and is named in the error of its rule, and in the paths returned; a
  /// deferred one stays in place for the next clean, named in the error of its rule
  pub fn empty(&self, cleaned: &mut [Cleaned]) -> Vec<String> {
    self.empty_until(cleaned, &|| false).0
  }

  /// `empty`, which stops once `stop` holds (`remove_measured_until`): the rest of the trash of this run is left to the
  /// sweep of a later run (`abandon`), and so are the directories of the runs that are gone; the rest of the deferred
  /// paths stays in place for the next clean. One count of entries serves all deferred paths, so `stop` is asked once
  /// every `STOP_EVERY` of them, not once per stale object. Whether it deleted all
  pub fn empty_until(&self, cleaned: &mut [Cleaned], stop: &dyn Fn() -> bool) -> (Vec<String>, bool) {
    let mut left = Vec::new();
    let mut seen = 0;
    for rule in cleaned.iter_mut() {
      for path in std::mem::take(&mut rule.trashed) {
        match remove_measured_until(&path, stop) {
          Ok(Removed { freed, complete: true }) => rule.freed_bytes += freed,
          Ok(Removed { freed, complete: false }) => {
            rule.freed_bytes += freed;
            self.abandon();
            return (left, false);
          }
          Err(error) => {
            rule.error.get_or_insert_with(|| format!("cannot delete {} in the trash: {error}", path.display()));
            left.push(trash_left_path(&path, &error));
          }
        }
      }
      for path in std::mem::take(&mut rule.deferred) {
        match remove_counted(&path, stop, &mut seen) {
          Ok(Removed { freed, complete }) => {
            rule.freed_bytes += freed;
            if !complete {
              self.abandon();
              return (left, false);
            }
          }
          Err(error) => {
            rule.error.get_or_insert_with(|| format!("cannot delete {}: {error}", path.display()));
          }
        }
      }
    }
    let adopted = std::mem::take(&mut *self.adopted.lock().unwrap_or_else(PoisonError::into_inner));
    for path in adopted {
      if let Err(error) = remove_measured(&path) {
        warn!(path = %path.display(), error = %error, "cannot delete a path in the trash");
        left.push(trash_left_path(&path, &error));
      }
    }
    if left.is_empty() {
      let _ = fs_retry::remove_dir(&self.dir);
    }
    self.sweep();
    (left, true)
  }

  /// Leave what is in the trash of this run to the sweep of any run (`sweep`, `holds_gone_runs`), as if this run were
  /// gone: its directory is renamed `left-…`, an instant move. A delete that stopped once the volume had its minimum
  /// leaves the rest there, which the next get below the minimum deletes first, outside the lock of the shrink
  pub(crate) fn abandon(&self) {
    if let Some(name) = self.dir.file_name() {
      let left = self.root.join(format!("{LEFT_PREFIX}{}", name.to_string_lossy()));
      if let Err(error) = fs_retry::rename(&self.dir, &left)
        && error.kind() != std::io::ErrorKind::NotFound
      {
        warn!(trash = %self.dir.display(), error = %error, "cannot leave the rest of a trash to a later sweep");
      }
    }
  }

  /// Delete the trash directories of runs whose process is gone (killed by a timeout before it emptied its trash, or a
  /// get whose takeover left its trash for later)
  pub(crate) fn sweep(&self) {
    let Ok(entries) = std::fs::read_dir(&self.root) else { return };
    for entry in entries.flatten() {
      if entry.path() == self.dir || !of_gone_run(&entry.file_name().to_string_lossy()) {
        continue;
      }
      match remove_measured(&entry.path()) {
        Ok(freed) => debug!(trash = %entry.path().display(), freed, "deleted the trash of a run that is gone"),
        Err(error) => warn!(trash = %entry.path().display(), error = %error, "cannot delete the trash of a run that is gone"),
      }
    }
  }
}

/// Whether the trash under `root` holds a directory of a run whose process is gone, which `Trash::sweep` deletes: a
/// look at the names, no walk
pub(crate) fn holds_gone_runs(root: &Path) -> bool {
  std::fs::read_dir(root).is_ok_and(|entries| entries.flatten().any(|entry| of_gone_run(&entry.file_name().to_string_lossy())))
}

/// The start of the name of a trash directory its run left to the sweep of any run (`Trash::abandon`)
const LEFT_PREFIX: &str = "left-";

/// Whether the trash directory `name` (`<pid>-<start>-<uuid>`) is of a run whose process is gone, or one its run left
/// (`left-…`, `Trash::abandon`)
fn of_gone_run(name: &str) -> bool {
  if name.starts_with(LEFT_PREFIX) {
    return true;
  }
  let mut parts = name.splitn(3, '-');
  let (Some(pid), Some(started)) = (parts.next().and_then(|pid| pid.parse().ok()), parts.next().and_then(|started| started.parse().ok())) else {
    return false;
  };
  !is_running(pid, Some(started).filter(|started| *started != 0))
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
  if is_profile_dir(dir) {
    return vec![dir.to_path_buf()];
  }
  if depth == 0 {
    return Vec::new();
  }
  subdirs(dir).into_iter().flat_map(|sub| profile_dirs(&sub, depth - 1)).collect()
}

/// Whether cargo builds a profile into `dir`: it has `.cargo-lock`
fn is_profile_dir(dir: &Path) -> bool {
  dir.join(".cargo-lock").symlink_metadata().is_ok()
}

/// The directories in `dir`, not the symlinks to one, whose name is one of `names` byte for byte, as a `delete` matches
/// its `except` (`run_rule`): a lookup by `dir.join(name)` finds `debug` for `Debug` on a volume that ignores case or
/// the normalization of a name (APFS, NTFS), an entry that the delete removes
fn kept_dirs(dir: &Path, names: &[&str]) -> Vec<PathBuf> {
  subdirs(dir)
    .into_iter()
    .filter(|entry| entry.file_name().is_some_and(|name| names.iter().any(|kept| name == *kept)))
    .collect()
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
  /// The package of the unit (`-` as `_`); of a classic unit without a fingerprint dir, the crate of its files
  package: String,
  /// The kind and the name of its target as its fingerprint names them (`lib-bd_cli`, `test-bin-brd`,
  /// `run-build-script-build-script-build`), when it has one
  target: Option<String>,
  built: Option<SystemTime>,
  paths: Vec<PathBuf>,
  /// Where the files that rustc wrote for the unit are: the files of `deps/` of the classic layout, and the dirs that
  /// hold them (`build/<package>/<hash>/out/`, a classic `build/<package>-<hash>/`)
  outputs: Vec<Output>,
  /// The last run of rustc for the unit, as its fingerprint dir tells it
  compiled: Option<Compiled>,
  /// A dir of `incremental/`, whose sessions hold the objects that the next build of its crate reads
  incremental: Option<Session>,
  /// The last built of its group or built after the cutoff (`split_groups`): only such a unit keeps the incremental dir
  /// that it wrote (`split_units`)
  current: bool,
}

/// The last run of rustc for a unit, as its fingerprint dir tells it: cargo writes `invoked.timestamp` before it starts
/// rustc, and the other files of the dir once rustc ended
#[derive(Debug, Clone, Copy)]
struct Compiled {
  invoked: SystemTime,
  /// The newest mtime of a file of the fingerprint dir
  ended: SystemTime,
}

/// A dir of `incremental/<crate>-<id>/`: rustc names it by the crate and an id of its own, not by the hash of the unit
#[derive(Debug)]
struct Session {
  /// The crate of the name of the dir, which rustc compiles
  crate_name: String,
  /// When rustc started the newest session of the dir: the mtime of the newest `s-*.lock`, which rustc makes at the
  /// start and leaves in place
  started: Option<SystemTime>,
}

impl Unit {
  fn new(package: String) -> Self {
    Self { package, ..Self::default() }
  }

  fn add(&mut self, path: PathBuf) {
    let built = last_built(&path);
    self.built = Some(self.built.map_or(built.newest, |known| known.max(built.newest)));
    if self.target.is_none() {
      self.target = built.target;
    }
    if self.compiled.is_none() {
      self.compiled = built.compiled;
    }
    self.paths.push(path);
  }

  /// Whether a build wrote the unit since it was read: the newest `last_built` of its paths moved
  fn built_again(&self) -> bool {
    let built = self.paths.iter().map(|path| last_built(path).newest).max();
    built != self.built
  }

  /// Whether rustc started the newest session of the incremental dir `session` while it compiled this unit, a unit of
  /// its crate (`target_crate`): within the last run of rustc for the unit. Two units of one crate that cargo compiles
  /// at the same time (its library and its test harness) both hold the session of each other
  fn wrote(&self, session: &Session) -> bool {
    let (Some(compiled), Some(started)) = (self.compiled, session.started) else {
      return false;
    };
    compiled.invoked <= started && started <= compiled.ended
  }

  /// The group whose `keep` newest units stay: the package and the target, so each target of a package keeps its own
  /// newest units, as one package builds a dozen targets that are all current (its library, its check, its test
  /// harness, its binaries, its build script and the run of it)
  fn group(&self) -> String {
    match &self.target {
      Some(target) => format!("{} {target}", self.package),
      None => self.package.clone(),
    }
  }
}

/// Where the files of a unit that rustc wrote are (`Unit::outputs`)
#[derive(Debug)]
#[cfg_attr(not(unix), expect(dead_code, reason = "only Unix has the object pass, which needs the inode of a file"))]
enum Output {
  /// One file, with its inode as the read of its directory told it
  File(PathBuf, u64),
  /// Each file right in the dir
  Dir(PathBuf),
}

/// The units of a profile dir, by an id unique in it
type Units = HashMap<String, Unit>;

/// The inode of `entry` as the read of its directory tells it, no `stat`; 0 off Unix, where the object pass finds
/// nothing
#[cfg(unix)]
fn entry_inode(entry: &std::fs::DirEntry) -> u64 {
  use std::os::unix::fs::DirEntryExt;
  entry.ino()
}

#[cfg(not(unix))]
const fn entry_inode(_entry: &std::fs::DirEntry) -> u64 {
  0
}

/// The units of the classic layout of a profile dir: `<name>-<16 hex>` in `.fingerprint/` and `build/` (a build script
/// and its run, `out/`), whose name is the package, and in `deps/` (`lib<name>-<hash>.rlib`, `.rmeta`,
/// `<name>-<hash>.d`, the executable, its `.dSYM`, `.pdb` or `.exe`, the object files), whose name is the crate of the
/// target: a bin `brd` of the package `bd-cli` is `deps/brd-<hash>` beside `.fingerprint/bd-cli-<hash>/`. So one hash is
/// one unit, whose files go together, of the package its fingerprint dir names (else of the crate of its files). The
/// uplifted artifacts at the top of the profile dir and `.cargo-lock` are never part of one. Only the dirs `walks`
/// names are read
fn classic_units(profile: &Path, walks: &impl Fn(&str) -> bool) -> Units {
  let mut units = Units::new();
  // the dirs named by the package first, so the unit gets the package, which its fingerprint dir names
  for dir in [".fingerprint", "build", "deps"].into_iter().filter(|dir| walks(dir)) {
    let Ok(entries) = std::fs::read_dir(profile.join(dir)) else { continue };
    for entry in entries.flatten() {
      // a symlink is never part of a unit: its target may be out of the slot
      let Ok(file_type) = entry.file_type() else { continue };
      if file_type.is_symlink() {
        continue;
      }
      let name = entry.file_name().to_string_lossy().into_owned();
      if let Some((name, hash)) = classic_unit_name(&name) {
        let unit = units.entry(hash).or_insert_with(|| Unit::new(name));
        match dir {
          "deps" if !file_type.is_dir() => unit.outputs.push(Output::File(entry.path(), entry_inode(&entry))),
          "build" if file_type.is_dir() => unit.outputs.push(Output::Dir(entry.path())),
          _ => {}
        }
        unit.add(entry.path());
      }
    }
  }
  units
}

/// The name and the hash of a file or a dir of the classic layout: the name before `-<16 hex digits>`, which the end
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
fn add_build_dir_units(units: &mut Units, build: &Path) {
  for package in subdirs(build) {
    let name = package.file_name().map_or_default(|name| name.to_string_lossy().into_owned());
    if classic_unit_name(&name).is_some() {
      continue;
    }
    for dir in subdirs(&package) {
      let id = format!("build/{name}/{}", dir.file_name().map_or_default(|hash| hash.to_string_lossy().into_owned()));
      let mut unit = Unit::new(name.replace('-', "_"));
      unit.outputs.push(Output::Dir(dir.join("out")));
      unit.add(dir);
      units.insert(id, unit);
    }
  }
}

/// The units of `incremental/<crate>-<id>/`, grouped by crate (a crate name may hold `-`); their id is not the hash of
/// the other files, so each is a unit of its own
fn add_incremental_units(units: &mut Units, incremental: &Path) {
  for dir in subdirs(incremental) {
    let name = dir.file_name().map_or_default(|name| name.to_string_lossy().into_owned());
    let crate_name = name.rfind('-').map_or(name.as_str(), |dash| &name[..dash]).replace('-', "_");
    let mut unit = Unit::new(format!("incremental {crate_name}"));
    unit.incremental = Some(Session {
      crate_name,
      started: session_started(&dir),
    });
    unit.add(dir);
    units.insert(format!("incremental/{name}"), unit);
  }
}

/// The mtime of the newest `s-*.lock` of the incremental dir `dir`: rustc makes the lock of a session when it starts it
fn session_started(dir: &Path) -> Option<SystemTime> {
  std::fs::read_dir(dir)
    .ok()?
    .flatten()
    .filter(|entry| {
      let name = entry.file_name();
      let name = name.to_string_lossy();
      name.starts_with("s-") && name.ends_with(".lock")
    })
    .filter_map(|entry| entry.metadata().and_then(|metadata| metadata.modified()).ok())
    .max()
}

/// The crate that rustc compiles for the target that a fingerprint names (the kinds of cargo, after `test-` of a test
/// build): `lib-bd_cli` and `test-lib-bd_cli` are `bd_cli`, `test-bin-test-server` is `test_server`,
/// `build-script-build-script-build` is `build_script_build`. `None` for the run of a build script and for a doc,
/// which run no rustc of an incremental dir
fn target_crate(target: &str) -> Option<String> {
  let target = target.strip_prefix("test-").unwrap_or(target);
  let name = ["lib-", "bin-", "example-", "integration-test-", "bench-", "build-script-"]
    .iter()
    .find_map(|kind| target.strip_prefix(kind))?;
  Some(name.replace('-', "_"))
}

/// The stale and the kept units of `units`: a unit stays when it is among the `keep` last built of its group
/// (`Unit::group`) or was built after `cutoff`, since a slot builds a dozen variants an hour and age alone keeps all.
/// An incremental dir also stays while a current unit (the last built of its group, or one built after `cutoff`) wrote
/// its newest session (`Unit::wrote`): its crate has a dozen current units (the library, its check, its test harness,
/// each of them for each set of features). Of the other dirs of a crate only the newest stays, whatever `keep` says. An
/// older kept unit keeps no dir: in a slot of accio each group held at most 8 units, so each old unit stayed and kept
/// the dir it wrote, 814 dirs and 22 GB in one profile dir. Its next build without the dir compiles its crate in full
/// once. The files of a unit go together
fn split_units(units: Units, keep: usize, cutoff: SystemTime) -> (Vec<Unit>, Vec<Unit>) {
  let (sessions, compiled): (Vec<Unit>, Vec<Unit>) = units.into_values().partition(|unit| unit.incremental.is_some());
  let (mut stale, mut kept) = split_groups(compiled, keep, cutoff);
  // the newest dir of each crate stays whatever wrote it: the next build of the crate reads it most likely
  let (stale_sessions, kept_sessions) = split_groups(sessions, 1, cutoff);
  // the current kept units by the crate of their target, which names the incremental dirs of the crate
  let mut writers: HashMap<String, Vec<&Unit>> = HashMap::new();
  for unit in kept.iter().filter(|unit| unit.current) {
    if let Some(name) = unit.target.as_deref().and_then(target_crate) {
      writers.entry(name).or_default().push(unit);
    }
  }
  let (written, unwritten): (Vec<Unit>, Vec<Unit>) = stale_sessions.into_iter().partition(|dir| {
    dir
      .incremental
      .as_ref()
      .is_some_and(|session| writers.get(&session.crate_name).is_some_and(|units| units.iter().any(|unit| unit.wrote(session))))
  });
  stale.extend(unwritten);
  kept.extend(kept_sessions);
  kept.extend(written);
  (stale, kept)
}

/// The stale and the kept units of `units` by the `keep` last built of each group and `cutoff` alone; a kept unit is
/// `Unit::current` when it is the last built of its group or built after `cutoff`
fn split_groups(units: Vec<Unit>, keep: usize, cutoff: SystemTime) -> (Vec<Unit>, Vec<Unit>) {
  let mut groups: HashMap<String, Vec<Unit>> = HashMap::new();
  for unit in units {
    groups.entry(unit.group()).or_default().push(unit);
  }
  let (mut stale, mut kept) = (Vec::new(), Vec::new());
  for mut units in groups.into_values() {
    units.sort_by_key(|unit| std::cmp::Reverse(unit.built));
    for (index, mut unit) in units.into_iter().enumerate() {
      let old = unit.built.is_some_and(|built| built < cutoff);
      if index >= keep && old {
        stale.push(unit);
      } else {
        unit.current = index == 0 || !old;
        kept.push(unit);
      }
    }
  }
  (stale, kept)
}

/// When a file or a dir of a unit was last built, and the target its fingerprint names (`fingerprint_target`): a
/// rebuild rewrites its fingerprint files (`build/`) or adds a session (`incremental/`), not always the directory
/// itself, so the newest mtime of the directory and of the direct children of its `fingerprint` dir (else of itself)
/// counts; of a file its own mtime
fn last_built(unit: &Path) -> Built {
  let modified = |path: &Path| path.symlink_metadata().and_then(|metadata| metadata.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
  let fingerprint = unit.join("fingerprint");
  // the fingerprint of the unit: `fingerprint/` of the build-dir layout, a dir of `.fingerprint/` of the classic one
  let (children, names_target) = if fingerprint.is_dir() {
    (fingerprint, true)
  } else {
    (unit.to_path_buf(), unit.parent().and_then(Path::file_name).is_some_and(|parent| parent == ".fingerprint"))
  };
  let mut built = Built {
    newest: modified(unit),
    target: None,
    compiled: None,
  };
  let (mut invoked, mut ended) = (None, None::<SystemTime>);
  if let Ok(entries) = std::fs::read_dir(&children) {
    for entry in entries.flatten() {
      let entry_modified = modified(&entry.path());
      built.newest = built.newest.max(entry_modified);
      let name = entry.file_name().to_string_lossy().into_owned();
      if !names_target {
        continue;
      }
      if fingerprint_target(&name) && built.target.as_ref().is_none_or(|known| name < *known) {
        built.target = Some(name.clone());
      }
      if name == "invoked.timestamp" {
        invoked = Some(entry_modified);
      }
      if entry.file_type().is_ok_and(|file_type| !file_type.is_dir()) {
        ended = Some(ended.map_or(entry_modified, |known| known.max(entry_modified)));
      }
    }
  }
  if let (Some(invoked), Some(ended)) = (invoked, ended) {
    built.compiled = Some(Compiled { invoked, ended });
  }
  built
}

/// What `last_built` reads of a file or a dir of a unit
struct Built {
  newest: SystemTime,
  /// The target that its fingerprint names (`fingerprint_target`)
  target: Option<String>,
  /// The last run of rustc that its fingerprint tells
  compiled: Option<Compiled>,
}

/// Whether a file of a fingerprint dir names the kind and the name of the target of its unit: `lib-bd_cli`,
/// `test-bin-brd`, `build-script-build-script-build`; not its `.json`, `dep-` and `output-` files or
/// `invoked.timestamp`
fn fingerprint_target(name: &str) -> bool {
  name.contains('-') && !name.contains('.') && !name.starts_with("dep-") && !name.starts_with("output-")
}

/// An object of rustc in a kept unit that no build reads (`stale_objects`)
#[derive(Debug)]
struct StaleObject {
  path: PathBuf,
  inode: u64,
  links: u64,
  bytes: u64,
}

/// The name of an object of an incremental compile, `<crate>-<hash>.<cgu>.<invocation>.rcgu.o`, gives the rustc
/// invocation that wrote it; `None` for another file, and for an object of a compile without incremental
/// (`<crate>-<hash>.<crate>.<hex>-cgu.<n>.rcgu.o`), whose name tells no invocation
#[cfg(unix)]
fn incremental_invocation(name: &str) -> Option<&str> {
  let stem = name.strip_suffix(".rcgu.o")?;
  if stem.contains("-cgu.") {
    return None;
  }
  let (rest, invocation) = stem.rsplit_once('.')?;
  (rest.contains('.') && !invocation.is_empty()).then_some(invocation)
}

/// The stale objects of rustc in the `kept` units, older than `cutoff` and older than the newest output of their unit
/// (`final_output`). Only the objects of an incremental compile go:
///
/// - first the sessions of every dir of `incremental`, as it is now, the inode of each file (`held_inodes`). A session
///   links the objects that the next build of its crate reuses, under new names, so an object whose inode a session
///   holds stays. A dir that the walk found stale but did not move, as a build wrote it since, counts too;
/// - a unit that links only an rlib (`.rlib`, `.rmeta`, `.d` and objects): no reader opens its objects after the compile,
///   as the debug map of an executable names the members of the rlib. Each other object goes;
/// - another unit (an executable, a test harness, a build script, a dylib) names its own objects in its debug map: the
///   objects of each invocation that names an inode of the newest mtime of the unit stay, the others go.
///
/// An object of a compile without incremental stays, as its name tells no invocation. The pass runs without the locks
/// of cargo, so a build may run meanwhile. Two guards keep what it reads: `cutoff`, taken before the locks, keeps each
/// object that it compiles, and the sessions keep each object that it links again from one, which has the old mtime of
/// its inode. The newest output is no guard for such a link. A unit with no output yet keeps every object
#[cfg(unix)]
fn stale_objects(kept: &[Unit], incremental: &Path, cutoff: SystemTime) -> Vec<StaleObject> {
  use std::os::unix::fs::MetadataExt;
  let Some(held) = held_inodes(incremental) else {
    debug!(incremental = %incremental.display(), "the read of a session failed at each try, the objects stay");
    return Vec::new();
  };
  let mut stale = Vec::new();
  for unit in kept.iter().filter(|unit| unit.incremental.is_none()) {
    let files = output_files(unit);
    let objects: Vec<(&OutputFile, &str)> = files
      .iter()
      .filter_map(|file| incremental_invocation(&file.name).map(|invocation| (file, invocation)))
      .collect();
    if objects.is_empty() {
      continue;
    }
    let links_only_an_rlib = {
      let mut others = files.iter().filter(|file| !file.name.ends_with(".rcgu.o")).map(|file| file.name.as_str()).peekable();
      others.peek().is_some()
        && files.iter().any(|file| Path::new(&file.name).extension().is_some_and(|extension| extension == "rlib"))
        && others.all(|name| [".rlib", ".rmeta", ".d"].iter().any(|extension| name.ends_with(extension)))
    };
    let mut metadata: HashMap<u64, std::fs::Metadata> = HashMap::new();
    let mut stat = |file: &OutputFile| -> Option<std::fs::Metadata> {
      if let Some(known) = metadata.get(&file.inode) {
        return Some(known.clone());
      }
      let read = file.path.symlink_metadata().ok()?;
      metadata.insert(file.inode, read.clone());
      Some(read)
    };
    let newest_output = files
      .iter()
      .filter(|file| final_output(&file.name))
      .filter_map(|file| stat(file).and_then(|read| read.modified().ok()))
      .max();
    let Some(newest_output) = newest_output else { continue };
    // the inodes that the names of the newest invocations hold
    let mut kept_inodes = HashSet::new();
    if !links_only_an_rlib {
      let modified: Vec<(&OutputFile, &str, SystemTime)> = objects
        .iter()
        .filter_map(|(file, invocation)| stat(file).and_then(|read| read.modified().ok()).map(|modified| (*file, *invocation, modified)))
        .collect();
      let Some(newest) = modified.iter().map(|(_, _, modified)| *modified).max() else {
        continue;
      };
      let newest_invocations: HashSet<&str> = modified
        .iter()
        .filter(|(_, _, modified)| *modified == newest)
        .map(|(_, invocation, _)| *invocation)
        .collect();
      kept_inodes.extend(objects.iter().filter(|(_, invocation)| newest_invocations.contains(invocation)).map(|(file, _)| file.inode));
    }
    for (file, _) in objects {
      if held.contains(&file.inode) || kept_inodes.contains(&file.inode) {
        continue;
      }
      let Some(read) = stat(file) else { continue };
      if read.modified().is_ok_and(|modified| modified < cutoff && modified < newest_output) {
        stale.push(StaleObject {
          path: file.path.clone(),
          inode: file.inode,
          links: read.nlink(),
          bytes: allocated(&read),
        });
      }
    }
  }
  stale
}

/// Whether a file of a unit is an output that rustc writes once per compile, beside its objects: the rlib, the rmeta,
/// a dylib or a static library, the executable (no extension). Not an object, not the dep file, not a `.dSYM` or a file
/// that a build script generates
#[cfg(unix)]
fn final_output(name: &str) -> bool {
  Path::new(name)
    .extension()
    .is_none_or(|extension| ["rlib", "rmeta", "dylib", "so", "a"].iter().any(|known| extension == *known))
}

/// No inode off Unix: no object is known to be stale
#[cfg(not(unix))]
const fn stale_objects(_kept: &[Unit], _incremental: &Path, _cutoff: SystemTime) -> Vec<StaleObject> {
  Vec::new()
}

/// How many times `session_inodes` reads the sessions of a dir while one of them vanishes between the list and its read
#[cfg(unix)]
const SESSION_READS: usize = 4;

/// The inodes that the sessions of every dir of `incremental` hold; `None` when the read of a session of a dir failed
/// at each of `SESSION_READS` reads
#[cfg(unix)]
fn held_inodes(incremental: &Path) -> Option<HashSet<u64>> {
  let mut held = HashSet::new();
  for dir in subdirs(incremental) {
    held.extend(session_inodes(&dir, &read_session)?);
  }
  Some(held)
}

/// The inodes of the files of one session dir
#[cfg(unix)]
fn read_session(session: &Path) -> std::io::Result<Vec<u64>> {
  Ok(std::fs::read_dir(session)?.flatten().map(|entry| entry_inode(&entry)).collect())
}

/// The inodes of the files of the sessions (`s-*`, the working ones too) of the incremental dir `dir`, each read by
/// `read`. A running rustc renames its working session once it compiled, then deletes the old one, with the same inodes:
/// a session that vanished between the list and its read is read again under its new name, by a new list. `None` when
/// the read of one failed at each of `SESSION_READS` reads
#[cfg(unix)]
fn session_inodes(dir: &Path, read: &impl Fn(&Path) -> std::io::Result<Vec<u64>>) -> Option<Vec<u64>> {
  let mut inodes = Vec::new();
  for _ in 0..SESSION_READS {
    let mut vanished = false;
    for session in subdirs(dir) {
      if !session.file_name().is_some_and(|name| name.to_string_lossy().starts_with("s-")) {
        continue;
      }
      match read(&session) {
        Ok(read) => inodes.extend(read),
        Err(_) => vanished = true,
      }
    }
    if !vanished {
      return Some(inodes);
    }
  }
  None
}

/// A file that rustc wrote for a unit
#[cfg(unix)]
struct OutputFile {
  name: String,
  path: PathBuf,
  inode: u64,
}

/// The files of `Unit::outputs`
#[cfg(unix)]
fn output_files(unit: &Unit) -> Vec<OutputFile> {
  let mut files = Vec::new();
  for output in &unit.outputs {
    match output {
      Output::File(path, inode) => files.push(OutputFile {
        name: path.file_name().map_or_default(|name| name.to_string_lossy().into_owned()),
        path: path.clone(),
        inode: *inode,
      }),
      Output::Dir(dir) => {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        files.extend(entries.flatten().map(|entry| OutputFile {
          name: entry.file_name().to_string_lossy().into_owned(),
          path: entry.path(),
          inode: entry_inode(&entry),
        }));
      }
    }
  }
  files
}

/// What the deletes of `objects` free: the blocks of each inode whose every link is one of them, once; an inode that
/// another link keeps frees nothing (`unlink_frees`)
fn measure_objects(objects: &[StaleObject]) -> u64 {
  let mut names: HashMap<u64, (u64, u64, u64)> = HashMap::new();
  for object in objects {
    let entry = names.entry(object.inode).or_insert((0, object.links, object.bytes));
    entry.0 += 1;
  }
  names.values().filter(|(count, links, _)| count >= links).map(|(_, _, bytes)| bytes).sum()
}

/// The bytes on disk of `path` and of everything below it, symlinks counted as themselves
pub fn disk_size(path: &Path) -> u64 {
  fs_walk::tree_size(path)
}

/// The bytes on disk of the entry of `metadata`: its blocks, where the system tells them, else its length
#[cfg(unix)]
pub fn allocated(metadata: &std::fs::Metadata) -> u64 {
  use std::os::unix::fs::MetadataExt;
  metadata.blocks() * 512
}

#[cfg(not(unix))]
pub fn allocated(metadata: &std::fs::Metadata) -> u64 {
  metadata.len()
}

#[cfg(test)]
#[path = "lease_clean_test.rs"]
mod lease_clean_test;
