//! The local changes of an idle lease that a get or `brd lease reclaim` preserves before it takes the slot
//! (openspec/specs/cli/lease/spec.md, "Get: The quiet local changes of an idle lease are preserved in its notes";
//! "Lease" in docs/design/git-operations.md). An idle lease whose only work is local changes, none of them touched for
//! `branchdeck.leaseIdleChangesAfter` (12 h by default, `0` never), gives its slot back once its changes are patches in
//! its notes, which their tree proves. Until the commit point of the caller (a compare-and-swap under the pool lock that
//! marks the slot dirty) the slot is only read; the discard comes after it (`lease::finish_return`). The record of the
//! notes names the patches before that commit point, so neither a killed process nor the next get loses them.
//!
//! The changes stay where a patch can't carry them or the session may still need them as they are: an unmerged entry, a
//! gitlink or a nested repository, a path with a filter (Git LFS), an entry of the index that git does not check, a
//! lease without notes, patches past `branchdeck.leasePreserveMax` (`64M` by default)

use crate::lease::{DirtyCheck, FileStat, IdleCheck, Lease, SlotMark, slot_servers};
use crate::lease_clean::parse_age;
use crate::lease_notes::{LeaseNotes, NotesRoots, notes_keep, now_seconds, write_preserved};
use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::{LeaseConfig, PreserveRefusal, SlotChange, preserve_refusal, preserve_slot_changes, slot_change_entries};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};
use sync_utils::paths::display_path;
use sync_utils::process_scan::ProcessScan;
use sync_utils::size::parse_size;
use tracing::warn;

/// How long the local changes of an idle lease stay untouched before they are preserved, without
/// `branchdeck.leaseIdleChangesAfter`: a session that died in the evening keeps its slot over the night
pub const DEFAULT_IDLE_CHANGES_AFTER: Duration = Duration::from_hours(12);

/// The most bytes of the patches of a preserve, without `branchdeck.leasePreserveMax`
pub const DEFAULT_PRESERVE_MAX: u64 = 64 << 20;

/// The directory of the patches in the notes of the lease, of brd alone: a preserve that keeps the lease removes it
/// whole, and a session that writes into its notes picks no such name
pub const PRESERVED_DIR: &str = ".branch-deck-preserved";

/// When and how far a get and a reclaim preserve the local changes of an idle lease
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreserveConfig {
  /// How long every changed path stays untouched first; `None` (`0`): never. `0s` preserves them at once
  pub after: Option<Duration>,
  /// The most bytes of the patches
  pub max: u64,
  /// How long the notes of the lease stay after the preserve (`branchdeck.leaseNotesKeep`); zero: for ever
  pub keep: Duration,
}

impl PreserveConfig {
  /// From `branchdeck.leaseIdleChangesAfter` and `branchdeck.leasePreserveMax` of `config`; a value that can't be read
  /// keeps the default, with a warning
  pub fn of(config: &LeaseConfig) -> Self {
    let after = match config.idle_changes_after.as_deref() {
      None => Some(DEFAULT_IDLE_CHANGES_AFTER),
      Some("0") => None,
      Some(text) => Some(parse_age(text).unwrap_or_else(|error| {
        warn!(error = %format!("{error:#}"), "branchdeck.leaseIdleChangesAfter keeps the default");
        DEFAULT_IDLE_CHANGES_AFTER
      })),
    };
    let max = config.preserve_max.as_deref().map_or(DEFAULT_PRESERVE_MAX, |text| {
      parse_size(text).unwrap_or_else(|reason| {
        warn!(error = %reason, "branchdeck.leasePreserveMax keeps the default");
        DEFAULT_PRESERVE_MAX
      })
    });
    let (keep, error) = notes_keep(config);
    if let Some(error) = error {
      warn!(error = %error, "the notes of the pool keep the default");
    }
    Self { after, max, keep }
  }

  /// The keys as a check records them (`IdleCheck::preserve_config`): a check under other keys holds no more
  pub fn fingerprint(&self) -> String {
    let after = self.after.map_or_else(|| "never".to_string(), |after| format!("{}s", after.as_secs()));
    format!("after={after},max={}", self.max)
  }

  /// When changes last touched at `changed_at` (Unix seconds) are quiet; `None`: never
  pub fn quiet_at(&self, changed_at: u64) -> Option<u64> {
    self.after.map(|after| changed_at.saturating_add(after.as_secs()))
  }
}

/// The local changes of an idle lease as patches in its notes (`preserve_slot_changes`), which the record of its notes,
/// its slot record (`Reclaimed`) and the answers name
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preserved {
  /// The directory of the patches: `changes.patch`, and `staged.patch` where the index held another version
  pub dir: String,
  /// The commit both patches apply to; empty for a dry run
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub base: String,
  /// How many local changes the patches carry
  pub paths: usize,
  /// The bytes of the patches; 0 for a dry run
  #[serde(default, skip_serializing_if = "is_zero")]
  pub bytes: u64,
  /// `staged.patch` is there
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub staged: bool,
  /// When the notes expire by `branchdeck.leaseNotesKeep` as it was at the preserve, Unix seconds; `None`: never
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub expires_at: Option<u64>,
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature of `skip_serializing_if`")]
const fn is_zero(count: &u64) -> bool {
  *count == 0
}

/// What a check found of the local changes, the only work of an idle lease (`IdleCheck::of_changes`)
pub(crate) enum ChangesVerdict {
  /// They are quiet at this time (Unix seconds); `None`: never, the preserve is off
  Quiet(Option<u64>),
  /// They stay for `reason`, refused by `path` where one refuses
  Kept { reason: String, path: Option<String> },
}

/// What `preserve_changes` came to
pub(crate) enum Preserve {
  /// The patches are proven and in the record of the notes: the caller commits (the slot dirty by `dirty` until its
  /// discard) and discards the changes
  Ready { preserved: Preserved, dirty: DirtyCheck },
  /// What a dry run would preserve
  Planned(Preserved),
  /// The changes stay: the check says why (`IdleCheck::not_preserved`) or when they are quiet (`IdleCheck::quiet_at`)
  Kept(IdleCheck),
}

/// Whether a get or a reclaim tries to preserve the local changes that `check` found in an idle lease at `now`: its
/// only work, none of them keeps it, and their quiet time passed by the paths the check listed
pub(crate) fn preserves(check: &IdleCheck, now: u64) -> bool {
  check.commits.is_empty() && check.operation.is_none() && !check.paths.is_empty() && check.not_preserved.is_none() && check.quiet_at.is_some_and(|at| now >= at)
}

/// Preserve the local changes of the idle lease `old` of the slot at `path`, which `check` found (`preserves`): one
/// `status` that lists each untracked file, the stat of each path, then by `config` their quiet time again and what
/// keeps them (`preserve_refusal`, the notes of the lease in `roots`, the bytes of the changed files past the bound),
/// the patches (`preserve_slot_changes`) and their check (`proven`). Without `apply` it stops before the patches: what
/// would be preserved. No outcome but `Ready` leaves a patch
pub(crate) fn preserve_changes(
  git_executor: &GitCommandExecutor,
  path: &str,
  (old, check): (&Lease, &IdleCheck),
  config: PreserveConfig,
  roots: &NotesRoots,
  apply: bool,
) -> Result<Preserve> {
  let slot = Path::new(path);
  let changes = slot_change_entries(git_executor, path)?;
  if changes.is_empty() {
    bail!("The local changes of {path} went away");
  }
  // first the stat, then the hashes: an edit during the patches moves a stat, and the check after them sees it
  let stats: Vec<FileStat> = changes.iter().map(|change| FileStat::of(&slot.join(&change.path))).collect();
  let paths: Vec<String> = changes.iter().map(|change| change.path.clone()).collect();
  let kept = |verdict: ChangesVerdict| Preserve::Kept(IdleCheck::of_changes(slot, (check.mark.clone(), check.home_tip.clone()), paths.clone(), &config, verdict));
  let refused = |refusal: &PreserveRefusal| {
    kept(ChangesVerdict::Kept {
      reason: refusal.to_string(),
      path: refusal.path().map(str::to_string),
    })
  };
  let quiet_at = config.quiet_at(latest_change(slot, &paths));
  if quiet_at.is_none_or(|at| now_seconds() < at) {
    return Ok(kept(ChangesVerdict::Quiet(quiet_at)));
  }
  let Some(notes) = roots.find(&old.id) else {
    return Ok(kept(ChangesVerdict::Kept {
      reason: "the lease has no notes".to_string(),
      path: None,
    }));
  };
  if let Some(refusal) = preserve_refusal(git_executor, path, &changes)? {
    return Ok(refused(&refusal));
  }
  // `add --all` writes an object of each changed file into the store of the repository: past the bound, none
  if stats.iter().map(|stat| stat.len).sum::<u64>() > config.max {
    let largest = changes.iter().zip(&stats).max_by_key(|(_, stat)| stat.len).map(|(change, _)| change.path.clone());
    return Ok(refused(&PreserveRefusal::TooLarge { max: config.max, largest }));
  }
  let dir = notes.dir.join(PRESERVED_DIR);
  let mut preserved = Preserved {
    dir: display_path(&dir),
    base: String::new(),
    paths: paths.len(),
    bytes: 0,
    staged: false,
    expires_at: (!config.keep.is_zero()).then(|| now_seconds().saturating_add(config.keep.as_secs())),
  };
  if !apply {
    return Ok(Preserve::Planned(preserved));
  }
  let written = match preserve_slot_changes(git_executor, path, &dir, config.max) {
    Ok(Ok(patches)) => {
      preserved.base = patches.base;
      preserved.bytes = patches.bytes;
      preserved.staged = patches.staged;
      proven(git_executor, (slot, path), (&changes, &stats), check, (&notes, preserved))
    }
    Ok(Err(refusal)) => Ok(refused(&refusal)),
    Err(error) => Err(error),
  };
  // patches that no record names are no preserved work: `brd lease notes` would show them as such
  if !matches!(written, Ok(Preserve::Ready { .. })) {
    remove_patches(&dir);
  }
  written
}

/// After the patches of `changes` (with their `stats` before them) in the slot at `path`: one more `status` that must
/// find each path with its stat and the slot as `check` found it, a scan that must find no process but the Bazel servers
/// in the slot, then `preserved` in the record of `notes`
fn proven(
  git_executor: &GitCommandExecutor,
  (slot, path): (&Path, &str),
  (changes, stats): (&[SlotChange], &[FileStat]),
  check: &IdleCheck,
  (notes, preserved): (&LeaseNotes, Preserved),
) -> Result<Preserve> {
  let again = slot_change_entries(git_executor, path)?;
  let unchanged = again == changes && again.iter().zip(stats).all(|(change, stat)| FileStat::of(&slot.join(&change.path)) == *stat) && SlotMark::of(slot) == check.mark;
  if !unchanged {
    bail!("The local changes of {path} moved while they were preserved");
  }
  // a session that came back meanwhile keeps its lease: the status would call it busy, and no busy lease is returned
  let scan = ProcessScan::scan();
  let (_, others) = slot_servers(&scan, slot);
  if let Some(process) = others.first() {
    bail!("A process of another session works in {path} ({}, pid {})", process.name, process.pid);
  }
  write_preserved(notes, Some(&preserved))?;
  Ok(Preserve::Ready {
    dirty: DirtyCheck::of(slot, changes.iter().map(|change| change.path.clone()).collect()),
    preserved,
  })
}

/// The patches in `dir`, best effort
fn remove_patches(dir: &Path) {
  if dir.symlink_metadata().is_ok()
    && let Err(error) = sync_utils::fs_retry::remove_dir_all(dir)
  {
    warn!(dir = %dir.display(), error = %error, "cannot remove the patches of a preserve that did not take the slot");
  }
}

/// The preserve of the lease `lease_id` did not take its slot at the commit point (the slot moved): the record of its
/// notes names no patches, and they go. Best effort
pub(crate) fn withdraw_preserved(roots: &NotesRoots, lease_id: &str) {
  let Some(notes) = roots.find(lease_id) else { return };
  if let Some(preserved) = &notes.record.preserved {
    remove_patches(Path::new(&preserved.dir));
  }
  if let Err(error) = write_preserved(&notes, None) {
    warn!(lease = %lease_id, error = %error, "cannot take the preserve out of the record of the notes");
  }
}

/// The latest change of the changed `paths` of `slot` (relative, `/`-separated), Unix seconds: the later of the mtime
/// and the ctime (Unix) of each path itself, a link not followed, and of a deleted one of the nearest directory above it
/// that exists. The ctime counts the files that `cp -p`, `unzip` or `tar x` wrote now with an old mtime
pub(crate) fn latest_change(slot: &Path, paths: &[String]) -> u64 {
  paths
    .iter()
    .filter_map(|path| {
      let mut place = slot.join(path);
      loop {
        if let Ok(metadata) = place.symlink_metadata() {
          let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok().map(|since| since.as_secs());
          return modified.max(changed_at(&metadata));
        }
        if place.as_path() == slot || !place.pop() {
          return None;
        }
      }
    })
    .max()
    .unwrap_or(0)
}

/// The ctime of `metadata`, Unix seconds
#[cfg(unix)]
fn changed_at(metadata: &std::fs::Metadata) -> Option<u64> {
  use std::os::unix::fs::MetadataExt;
  u64::try_from(metadata.ctime()).ok()
}

/// No ctime where the system has none
#[cfg(not(unix))]
const fn changed_at(_metadata: &std::fs::Metadata) -> Option<u64> {
  None
}

#[cfg(test)]
#[path = "idle_changes_test.rs"]
mod idle_changes_test;
