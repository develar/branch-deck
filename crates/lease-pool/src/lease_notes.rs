//! The notes of a lease (`brd lease notes`, openspec/specs/cli/lease/spec.md, "Notes"): the logs, probes and patches of
//! the session that holds a lease, kept outside its worktree so that they outlive the return. Each lease has a
//! directory of its own in the pool directory of the repository, `<pool dir>/notes/<lease id>/`, and a record beside it
//! (`<lease id>.json`: its holder, its slot, its branch, when it was got and returned). In the worktree, `notes.local`
//! links to the directory, so an agent writes there as it always did; the get adds `/notes.local` to `info/exclude` of
//! the repository where that file lacks the line, so no `git add` takes the link and no status of the pool counts it.
//! The return removes the link, so the next holder of the slot starts with an empty `notes.local`, and the directory
//! stays.
//!
//! In the pool directory, beside the slots: that directory is where the sessions that lead agents may read already, and
//! a link of `notes.local` is matched by its target too, so notes in `$GIT_COMMON_DIR` (of a main checkout,
//! `<repository>/.git`) asked the user for every read of a lead. `notes` is no number, so no slot of the pool
//! (`<pool dir>/<n>/<name>`), no clone leftover, no trim and no disk report takes it for one, and a trim deletes the
//! number directory of a slot, never the pool directory; only the expired notes go (`expire_notes`).
//!
//! The root of a new lease follows from its slot, `<pool dir>/<n>/<name>` → `<pool dir>/notes` (`slot_notes_root`; a
//! worktree that `brd lease adopt` kept elsewhere: `notes` of the pool directory of the get), so no file names it, and
//! a reader looks in every place the notes may be (`NotesRoots`): the root of each pool directory that a slot of the
//! state lives in and of the pool directory of the config where the reader knows it (`brd lease notes`, the trim, the
//! status, the keeper of a host), then `$GIT_COMMON_DIR/branch-deck/notes`, where a brd before kept them. Nothing moves
//! the notes of that old place: they are read and expire there, and the read of it goes once the last of them expired.
//! The return closes the notes where the link of the slot points.
//!
//! Where the system makes no link (Windows without the right to make one), `notes.local` is a directory of the worktree,
//! whose entries the return moves into the directory of the lease. A `notes.local` that an earlier lease left in the
//! slot (a return that could not move it, a brd before the notes) is moved into notes of its own by the next get
//! (`left-<uuid>`), never shown to the next holder. The slot and the notes may be on two volumes (a slot that `brd
//! lease adopt` kept where it was on another disk), so each move copies where no rename crosses them
//! (`fs_retry::move_path`).
//!
//! The notes of a returned lease expire `branchdeck.leaseNotesKeep` after its return (14 days by default, `0` keeps
//! them): `brd lease trim` and the keeper of a host delete them, in every place, never those of a lease the pool holds,
//! and nothing where the state of the pool can't be read. A directory without a record (a get that stopped before it
//! wrote one) expires by its modification time.

use crate::lease::{Lease, LeaseStore, Pool, canonical_path, pool_dir};
use crate::lease_clean::parse_age;
use crate::trim::main_worktree;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::{LEASE_NOTES_DIR, LeaseConfig, NOTES_LINK, lease_config, legacy_notes_root};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::fs_retry;
use sync_utils::paths::display_path;
use tracing::warn;

/// How long the notes of a returned lease stay without `branchdeck.leaseNotesKeep`
pub const DEFAULT_NOTES_KEEP: Duration = Duration::from_hours(14 * 24);

/// The line of `info/exclude` that keeps the notes out of git
const EXCLUDE_LINE: &str = "/notes.local";

/// The record of the notes of one lease, `<lease id>.json` beside their directory
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotesRecord {
  pub lease_id: String,
  pub holder: String,
  /// The slot the lease held, its top level
  pub slot: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  /// Unix seconds
  pub leased_at: u64,
  /// Unix seconds of the return; `None` while the lease is held, or for a lease that left the pool without a return
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub returned_at: Option<u64>,
  /// `notes.local` of the slot is a link to the directory; false: a directory of the worktree, which the return moves
  pub linked: bool,
  /// `$GIT_COMMON_DIR` of the repository of the lease, canonical: a root that two repositories share
  /// (`branchdeck.leasePool`) holds the notes of both, and each expires only its own. `None` in a record of a brd
  /// before it, whose repository its slot tells (`expire_place`)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
}

/// The notes of one lease
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseNotes {
  pub dir: PathBuf,
  pub record: NotesRecord,
}

impl LeaseNotes {
  /// The bytes on disk of the directory
  pub fn bytes(&self) -> u64 {
    fs_walk::tree_size(&self.dir)
  }
}

/// The file that marks a root of notes as one Branch Deck made (`open_notes`): only there a directory without a record
/// expires (`expire_place`), and a root of a slot counts by it wherever its pool directory is
pub const NOTES_MARKER: &str = ".branch-deck-notes";

/// The root of the notes of the leases of the slot `slot`, `<pool dir>/<n>/<name>`: `<pool dir>/notes`, beside the
/// number directories, where it is a root of Branch Deck (`is_pool_root`); `None` for a worktree elsewhere (`brd lease
/// adopt` without `--move` keeps `~/work/2026/repo-wt`, whose `~/work/notes` is the user's)
pub fn slot_notes_root(slot: &Path, pool_dir: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
  let number = slot.parent()?;
  number.file_name()?.to_str()?.parse::<u32>().ok()?;
  let root = number.parent()?.join(LEASE_NOTES_DIR);
  is_pool_root(&root, pool_dir, home).then_some(root)
}

/// Whether `root`, `notes` of a directory, is a root of notes of Branch Deck: it has the marker of one (`NOTES_MARKER`),
/// or its directory is `pool_dir` (the pool directory of the config) or a directory of `~/.branch-deck/pool` of `home`
fn is_pool_root(root: &Path, pool_dir: Option<&Path>, home: Option<&Path>) -> bool {
  let Some(dir) = root.parent() else { return false };
  let same = |a: &Path, b: &Path| canonical_path(a) == canonical_path(b);
  root.join(NOTES_MARKER).is_file()
    || pool_dir.is_some_and(|pool_dir| same(dir, pool_dir))
    || home.zip(dir.parent()).is_some_and(|(home, pools)| same(pools, &home.join(".branch-deck").join("pool")))
}

/// The root of the notes of a new lease in `slot`: `slot_notes_root`, else `notes` of `pool_dir`, the pool directory of
/// the get
pub(crate) fn new_notes_root(slot: &Path, pool_dir: &Path, home: Option<&Path>) -> PathBuf {
  slot_notes_root(slot, Some(pool_dir), home).unwrap_or_else(|| pool_dir.join(LEASE_NOTES_DIR))
}

/// The pool directory of the config of the pool of `git_common_dir` (`pool_dir`), a relative `branchdeck.leasePool`
/// read from the main worktree, as from any checkout of the repository the one directory
pub fn config_pool_dir(config: &LeaseConfig, git_common_dir: &Path, home: Option<&Path>) -> Result<PathBuf> {
  let repository = main_worktree(git_common_dir).unwrap_or_else(|| git_common_dir.to_path_buf());
  pool_dir(config, &display_path(&repository), git_common_dir, home)
}

/// Every place the notes of a pool may be in, each read in its order: the root of each pool directory that a slot of
/// the state lives in (`slot_notes_root`) and of the pool directory of the config where the reader knows it, then
/// `$GIT_COMMON_DIR/branch-deck/notes` (`legacy_notes_root`), where a brd before kept them, read and expired, never
/// written. That last place goes once the last of the notes there expired
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesRoots {
  places: Vec<PathBuf>,
  /// The repository whose notes these are, canonical: the expiry leaves the records of another one alone; `None`: any
  git_common_dir: Option<String>,
}

impl NotesRoots {
  /// The places of the pool of `git_common_dir` whose state is `store`, with `notes` of `pool_dir` (the pool directory
  /// of the config) and the pool directories of `home` (`is_pool_root`)
  pub fn of(git_common_dir: &Path, store: &LeaseStore, pool_dir: Option<&Path>, home: Option<&Path>) -> Self {
    let candidates = store
      .slots
      .iter()
      .filter_map(|slot| slot_notes_root(Path::new(&slot.path), pool_dir, home))
      .chain(pool_dir.map(|dir| dir.join(LEASE_NOTES_DIR)))
      .chain(std::iter::once(legacy_notes_root(git_common_dir)));
    let mut places: Vec<PathBuf> = Vec::new();
    for place in candidates {
      let place = PathBuf::from(canonical_path(&place));
      if !places.contains(&place) {
        places.push(place);
      }
    }
    Self {
      places,
      git_common_dir: Some(canonical_path(git_common_dir)),
    }
  }

  /// One place alone, of any repository: notes of a test
  pub fn at(root: &Path) -> Self {
    Self {
      places: vec![root.to_path_buf()],
      git_common_dir: None,
    }
  }

  /// Each place, in its order
  pub fn places(&self) -> impl Iterator<Item = &Path> {
    self.places.iter().map(PathBuf::as_path)
  }

  /// The notes of `lease_id`, in the first place that has its record
  pub fn find(&self, lease_id: &str) -> Option<LeaseNotes> {
    self.places().find_map(|place| read_notes(place, lease_id))
  }

  /// Every note of the repository in every place (`of_repository`: a shared root holds the notes of others too), each
  /// lease once (the first place wins), the lease got last first
  pub fn all(&self) -> Vec<LeaseNotes> {
    let mut seen = HashSet::new();
    let mut notes: Vec<LeaseNotes> = self
      .places()
      .flat_map(all_notes)
      .filter(|notes| of_repository(&notes.record, self.git_common_dir.as_deref()) && seen.insert(notes.record.lease_id.clone()))
      .collect();
    sort_notes(&mut notes);
    notes
  }

  /// The bytes on disk of every place
  pub fn bytes(&self) -> u64 {
    self.places().map(fs_walk::tree_size).sum()
  }
}

/// The directory of the notes of `lease_id`
pub fn notes_dir(root: &Path, lease_id: &str) -> PathBuf {
  root.join(lease_id)
}

pub(crate) fn record_path(root: &Path, lease_id: &str) -> PathBuf {
  root.join(format!("{lease_id}.json"))
}

/// Unix seconds now, the time of a record and of an expiry
pub fn now_seconds() -> u64 {
  SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// The notes of `lease_id`, when it has a record
pub fn read_notes(root: &Path, lease_id: &str) -> Option<LeaseNotes> {
  let text = std::fs::read(record_path(root, lease_id)).ok()?;
  let record: NotesRecord = serde_json::from_slice(&text).ok()?;
  Some(LeaseNotes {
    dir: notes_dir(root, &record.lease_id),
    record,
  })
}

/// Every note of `root` that has a record, the lease got last first
pub fn all_notes(root: &Path) -> Vec<LeaseNotes> {
  let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
  let mut notes: Vec<LeaseNotes> = entries
    .flatten()
    .filter_map(|entry| {
      let name = entry.file_name().to_string_lossy().into_owned();
      read_notes(root, name.strip_suffix(".json")?)
    })
    .collect();
  sort_notes(&mut notes);
  notes
}

fn sort_notes(notes: &mut [LeaseNotes]) {
  notes.sort_by(|a, b| b.record.leased_at.cmp(&a.record.leased_at).then_with(|| a.record.lease_id.cmp(&b.record.lease_id)));
}

fn write_record(root: &Path, record: &NotesRecord) -> std::io::Result<()> {
  let text = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
  write_atomically(&record_path(root, &record.lease_id), &text, AtomicWrite::default())
}

/// The notes of the get of `lease` in its slot `slot`: their directory and record in `root`, `notes.local` of the slot
/// replaced by a link to the directory (by a directory where the system makes no link), and `/notes.local` in
/// `info/exclude` of `git_common_dir`. What an earlier lease left at `notes.local` goes into notes of its own first. Notes
/// that fail leave none: the link or the empty directory made in the slot and the directory of the lease are removed,
/// and the get goes on without notes
pub(crate) fn open_notes(root: &Path, slot: &Path, git_common_dir: &Path, lease: &Lease) -> std::io::Result<LeaseNotes> {
  let dir = notes_dir(root, &lease.id);
  let link = slot.join(NOTES_LINK);
  let opened = make_notes(root, slot, git_common_dir, lease, &dir, &link);
  if opened.is_err() {
    undo_notes(&dir, &link);
  }
  opened
}

fn make_notes(root: &Path, slot: &Path, git_common_dir: &Path, lease: &Lease, dir: &Path, link: &Path) -> std::io::Result<LeaseNotes> {
  std::fs::create_dir_all(dir)?;
  mark_root(root)?;
  if let Err(error) = exclude_notes(git_common_dir) {
    warn!(error = %error, "cannot add {EXCLUDE_LINE} to info/exclude of the repository");
  }
  set_aside(root, slot, git_common_dir, link)?;
  let linked = match make_link(dir, link) {
    Ok(()) => true,
    Err(error) => {
      warn!(slot = %slot.display(), error = %error, "cannot link notes.local to the notes of the lease: a directory of the worktree, which the return moves");
      std::fs::create_dir_all(link)?;
      false
    }
  };
  let record = NotesRecord {
    lease_id: lease.id.clone(),
    holder: lease.holder.clone(),
    slot: slot.to_string_lossy().into_owned(),
    branch: lease.branch.clone(),
    leased_at: lease.leased_at,
    returned_at: None,
    linked,
    repository: Some(canonical_path(git_common_dir)),
  };
  write_record(root, &record)?;
  Ok(LeaseNotes { dir: dir.to_path_buf(), record })
}

/// `NOTES_MARKER` in `root`, once: the root is one Branch Deck made
fn mark_root(root: &Path) -> std::io::Result<()> {
  let marker = root.join(NOTES_MARKER);
  if marker.is_file() {
    return Ok(());
  }
  write_atomically(&marker, b"the notes of the leases of Branch Deck (brd lease notes)\n", AtomicWrite::default())
}

/// What a failed `make_notes` made: a link at `link` (never followed), an empty directory there, the directory `dir` if
/// empty; a directory with files (what an earlier lease left that could not be set aside) stays
fn undo_notes(dir: &Path, link: &Path) {
  let undone = match link.symlink_metadata() {
    Ok(metadata) if metadata.file_type().is_symlink() => fs_retry::remove_entry(link, &metadata.file_type()),
    Ok(metadata) if metadata.is_dir() && std::fs::read_dir(link).is_ok_and(|mut entries| entries.next().is_none()) => fs_retry::remove_dir(link),
    _ => Ok(()),
  };
  if let Err(error) = undone {
    warn!(link = %link.display(), error = %error, "cannot remove notes.local of a get without notes");
  }
  if std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none())
    && let Err(error) = fs_retry::remove_dir(dir)
  {
    warn!(dir = %dir.display(), error = %error, "cannot remove the notes of a get without notes");
  }
}

/// At the return of `lease_id` from `slot`: `notes.local` of the slot removed (a link), or its entries moved into the
/// directory of the lease (a directory), and the return in the record, beside the directory the link names, else in
/// the first place of `roots` that has it. No record
/// (a host slot, a lease of a brd before the notes): nothing. Never fails the return: what can't be moved stays, and
/// the next get sets it aside
pub(crate) fn close_notes(roots: &NotesRoots, slot: &Path, lease_id: &str) -> Option<LeaseNotes> {
  let link = slot.join(NOTES_LINK);
  // where the link of the slot points, else the first place that has them
  let linked = std::fs::read_link(&link)
    .ok()
    .filter(|target| target.file_name().is_some_and(|name| name == lease_id))
    .and_then(|target| read_notes(target.parent()?, lease_id));
  let mut notes = linked.or_else(|| roots.find(lease_id))?;
  let root = notes.dir.parent()?.to_path_buf();
  match link.symlink_metadata() {
    Ok(metadata) if metadata.file_type().is_symlink() => {
      if let Err(error) = fs_retry::remove_entry(&link, &metadata.file_type()) {
        warn!(slot = %slot.display(), error = %error, "cannot remove the link notes.local of the returned lease");
      }
    }
    Ok(metadata) if metadata.is_dir() => {
      if let Err(error) = move_entries(&link, &notes.dir).and_then(|()| fs_retry::remove_dir(&link)) {
        warn!(slot = %slot.display(), error = %error, "cannot move notes.local of the returned lease into its notes");
      }
    }
    _ => {}
  }
  notes.record.returned_at = Some(now_seconds());
  if let Err(error) = write_record(&root, &notes.record) {
    warn!(lease = %lease_id, error = %error, "cannot record the return in the notes of the lease");
  }
  Some(notes)
}

/// Move what is at `link` (a link of an earlier lease, a directory that a return could not move or a brd before the
/// notes left) out of the slot: a link goes, anything else into notes of its own, `left-<uuid>`, returned now. The
/// record comes first, so the directory is never one without a record, whose age is its old modification time
fn set_aside(root: &Path, slot: &Path, git_common_dir: &Path, link: &Path) -> std::io::Result<()> {
  let Ok(metadata) = link.symlink_metadata() else { return Ok(()) };
  if metadata.file_type().is_symlink() {
    return fs_retry::remove_entry(link, &metadata.file_type());
  }
  let lease_id = format!("left-{}", uuid::Uuid::new_v4());
  let dir = notes_dir(root, &lease_id);
  let at = now_seconds();
  write_record(
    root,
    &NotesRecord {
      lease_id: lease_id.clone(),
      holder: "an earlier lease of the slot".to_string(),
      slot: slot.to_string_lossy().into_owned(),
      branch: None,
      leased_at: at,
      returned_at: Some(at),
      linked: false,
      repository: Some(canonical_path(git_common_dir)),
    },
  )?;
  move_path(link, &dir).inspect_err(|_| {
    // a move that failed after the copy landed (the removal of the source) keeps the copy and its record
    if dir.symlink_metadata().is_err()
      && let Err(error) = fs_retry::remove_file(record_path(root, &lease_id))
    {
      warn!(lease = %lease_id, error = %error, "cannot remove the record of notes that were not set aside");
    }
  })
}

/// Move each entry of `from` into `to` (`move_path`: the slot and the state of the pool may be on two volumes)
fn move_entries(from: &Path, to: &Path) -> std::io::Result<()> {
  std::fs::create_dir_all(to)?;
  for entry in std::fs::read_dir(from)? {
    let entry = entry?;
    move_path(&entry.path(), &to.join(entry.file_name()))?;
  }
  Ok(())
}

/// `fs_retry::move_path`; the seam of the tests moves as across two volumes
fn move_path(from: &Path, to: &Path) -> std::io::Result<()> {
  #[cfg(test)]
  if seam::ACROSS_VOLUMES.get() {
    return fs_retry::move_across_volumes(from, to);
  }
  fs_retry::move_path(from, to)
}

/// `link` to the directory `target`; the seam of the tests makes none, as a system without the right to does
fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
  #[cfg(test)]
  if seam::NO_LINKS.get() {
    return Err(std::io::Error::other("the seam of the tests makes no link"));
  }
  #[cfg(unix)]
  return std::os::unix::fs::symlink(target, link);
  #[cfg(windows)]
  return std::os::windows::fs::symlink_dir(target, link);
  #[cfg(not(any(unix, windows)))]
  Err(std::io::Error::other("no links on this platform"))
}

/// `/notes.local` in `info/exclude` of `git_common_dir`, once: the file is shared by every worktree of the repository
fn exclude_notes(git_common_dir: &Path) -> std::io::Result<()> {
  let path = git_common_dir.join("info").join("exclude");
  let text = std::fs::read_to_string(&path).unwrap_or_default();
  if text.lines().any(|line| line.trim() == EXCLUDE_LINE) {
    return Ok(());
  }
  std::fs::create_dir_all(git_common_dir.join("info"))?;
  let separator = if text.is_empty() || text.ends_with('\n') { "" } else { "\n" };
  let added = format!("{text}{separator}# the notes of a lease of the pool of Branch Deck (brd lease notes)\n{EXCLUDE_LINE}\n");
  write_atomically(&path, added.as_bytes(), AtomicWrite::default())
}

/// A note `expire_notes` deleted, or with a plan would delete
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiredNotes {
  pub lease_id: String,
  /// `None`: a directory without a record
  pub holder: Option<String>,
  /// Unix seconds of the return, of the get of a lease that left the pool without one, or the modification time of a
  /// directory without a record
  pub returned_at: u64,
  /// The bytes on disk of its directory
  pub bytes: u64,
  /// Why it could not be deleted
  pub error: Option<String>,
}

/// When the notes of a lease expire: `keep` after its return; `None` while `held` names it, and for `keep` 0
pub fn expires_at(record: &NotesRecord, held: &HashSet<String>, keep: Duration) -> Option<u64> {
  if keep.is_zero() || held.contains(&record.lease_id) {
    return None;
  }
  Some(record.returned_at.unwrap_or(record.leased_at).saturating_add(keep.as_secs()))
}

/// How long the notes of a returned lease stay: `branchdeck.leaseNotesKeep` of `config` (`0` for ever), else
/// `DEFAULT_NOTES_KEEP`; a value that is no duration is the default, with why
pub fn notes_keep(config: &LeaseConfig) -> (Duration, Option<String>) {
  match config.notes_keep.as_deref() {
    None => (DEFAULT_NOTES_KEEP, None),
    Some("0") => (Duration::ZERO, None),
    Some(text) => parse_age(text).map_or_else(|error| (DEFAULT_NOTES_KEEP, Some(format!("branchdeck.leaseNotesKeep: {error:#}"))), |keep| (keep, None)),
  }
}

/// The leases of `store` (the state of a pool, read by the caller: no state, no expiry), by id, with the idle ones a get
/// takes over: their notes never expire
pub fn held_leases(store: &LeaseStore) -> HashSet<String> {
  store
    .slots
    .iter()
    .filter_map(|slot| slot.lease.as_ref())
    .flat_map(|lease| std::iter::once(lease.id.clone()).chain(lease.taking_over.as_ref().map(|old| old.id.clone())))
    .collect()
}

/// The notes of every place of `roots` that expired by `now` (`expire_place`)
pub fn expire_notes(roots: &NotesRoots, held: &HashSet<String>, keep: Duration, now: u64, plan: bool) -> Vec<ExpiredNotes> {
  roots
    .places()
    .flat_map(|place| expire_place(place, roots.git_common_dir.as_deref(), held, keep, now, plan))
    .collect()
}

/// Whether the notes of `record` are of the repository `git_common_dir` (canonical; `None`: any): its `repository`, else
/// the repository of its slot while that is a linked worktree; a record that tells neither is this one's
fn of_repository(record: &NotesRecord, git_common_dir: Option<&str>) -> bool {
  let Some(git_common_dir) = git_common_dir else { return true };
  match &record.repository {
    Some(repository) => repository == git_common_dir,
    None => git_ops::lease_slot::linked_worktree(Path::new(&record.slot)).is_none_or(|worktree| display_path(&worktree.git_common_dir) == git_common_dir),
  }
}

/// The notes of `root` that expired by `now` (`expires_at`), deleted unless `plan`: their directories and records; then
/// the directories without a record (a get that stopped before it wrote one, the copy of a move across volumes that
/// stopped), `keep` after their modification time, never one named by `held` nor with `keep` 0, and only in a root
/// Branch Deck made (`NOTES_MARKER`, the old place in the common dir): a directory of a user's `notes` stays. Only what
/// is in `root`, and only the notes of the repository `git_common_dir` (`of_repository`): a root that two repositories
/// share holds the held leases of the other, which this state does not name
fn expire_place(root: &Path, git_common_dir: Option<&str>, held: &HashSet<String>, keep: Duration, now: u64, plan: bool) -> Vec<ExpiredNotes> {
  let made_by_branch_deck =
    root.join(NOTES_MARKER).is_file() || git_common_dir.is_some_and(|git_common_dir| canonical_path(root) == canonical_path(&legacy_notes_root(Path::new(git_common_dir))));
  let recorded = all_notes(root)
    .into_iter()
    .filter(|notes| of_repository(&notes.record, git_common_dir) && expires_at(&notes.record, held, keep).is_some_and(|at| at <= now))
    .map(|notes| {
      let bytes = fs_walk::tree_size(&notes.dir);
      let error = (!plan).then(|| remove_notes(root, &notes)).and_then(Result::err).map(|error| error.to_string());
      ExpiredNotes {
        lease_id: notes.record.lease_id.clone(),
        holder: Some(notes.record.holder.clone()),
        returned_at: notes.record.returned_at.unwrap_or(notes.record.leased_at),
        bytes,
        error,
      }
    });
  let unrecorded = unrecorded_dirs(root)
    .into_iter()
    .filter(|_| made_by_branch_deck)
    .filter(|(name, _, modified_at)| !keep.is_zero() && !held.contains(name) && modified_at.saturating_add(keep.as_secs()) <= now)
    .map(|(name, dir, modified_at)| {
      let bytes = fs_walk::tree_size(&dir);
      let error = (!plan).then(|| fs_retry::remove_dir_all(&dir)).and_then(Result::err).map(|error| error.to_string());
      ExpiredNotes {
        lease_id: name,
        holder: None,
        returned_at: modified_at,
        bytes,
        error,
      }
    });
  let mut expired: Vec<ExpiredNotes> = recorded.collect();
  expired.extend(unrecorded);
  expired
}

/// The directories of `root` without a record (`<name>.json`): their names, paths and modification times, Unix seconds
fn unrecorded_dirs(root: &Path) -> Vec<(String, PathBuf, u64)> {
  let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
  let mut dirs: Vec<(String, PathBuf, u64)> = entries
    .flatten()
    .filter_map(|entry| {
      let metadata = entry.path().symlink_metadata().ok().filter(std::fs::Metadata::is_dir)?;
      let name = entry.file_name().to_string_lossy().into_owned();
      if record_path(root, &name).symlink_metadata().is_ok() {
        return None;
      }
      let modified_at = metadata.modified().ok()?.duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
      Some((name, entry.path(), modified_at))
    })
    .collect();
  dirs.sort();
  dirs
}

/// `expire_notes` of the pool of `git_common_dir` now, deleting: with its `branchdeck.leaseNotesKeep` (one `git config`)
/// and the leases it holds, for the keeper of a host, in every place (`NotesRoots`; `home` tells the pool directory of
/// the config). A config that can't be read keeps the default
pub fn expire_pool_notes(git_executor: &GitCommandExecutor, git_common_dir: &Path, home: Option<&Path>) -> Vec<ExpiredNotes> {
  let git_common_dir = PathBuf::from(canonical_path(git_common_dir));
  let config = lease_config(git_executor, &display_path(&git_common_dir)).ok();
  let keep = config.as_ref().map_or(DEFAULT_NOTES_KEEP, |config| {
    let (keep, error) = notes_keep(config);
    if let Some(error) = error {
      warn!(error = %error, "the notes of the pool keep the default");
    }
    keep
  });
  let pool_dir = config.as_ref().and_then(|config| config_pool_dir(config, &git_common_dir, home).ok());
  let pool = Pool::new(&git_common_dir);
  match pool.read() {
    Ok(store) => expire_notes(
      &NotesRoots::of(&git_common_dir, &store, pool_dir.as_deref(), home),
      &held_leases(&store),
      keep,
      now_seconds(),
      false,
    ),
    Err(error) => {
      warn!(pool = %git_common_dir.display(), error = %format!("{error:#}"), "the state of the pool can't be read: no notes expire");
      Vec::new()
    }
  }
}

/// The directory of `notes`, then its record: a record without a directory is no note any more
fn remove_notes(root: &Path, notes: &LeaseNotes) -> std::io::Result<()> {
  if notes.dir.symlink_metadata().is_ok() {
    fs_retry::remove_dir_all(&notes.dir)?;
  }
  fs_retry::remove_file(record_path(root, &notes.record.lease_id))
}

/// A file of the notes of a lease
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteFile {
  /// From the directory of the notes, `/`-separated
  pub path: String,
  pub bytes: u64,
}

/// The most files `note_files` lists
pub const NOTE_FILES_LIMIT: usize = 1000;

/// The files of the notes in `dir`, by path, the first `NOTE_FILES_LIMIT`, links not followed; with whether there are
/// more
pub fn note_files(dir: &Path) -> (Vec<NoteFile>, bool) {
  let mut files = Vec::new();
  let mut more = false;
  let mut pending = vec![(dir.to_path_buf(), String::new())];
  while let Some((current, prefix)) = pending.pop() {
    let Ok(entries) = std::fs::read_dir(&current) else { continue };
    for entry in entries.flatten() {
      let name = entry.file_name().to_string_lossy().into_owned();
      let path = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
      let Ok(metadata) = entry.path().symlink_metadata() else { continue };
      if metadata.is_dir() {
        pending.push((entry.path(), path));
      } else if files.len() < NOTE_FILES_LIMIT {
        files.push(NoteFile { path, bytes: metadata.len() });
      } else {
        more = true;
      }
    }
  }
  files.sort_by(|a, b| a.path.cmp(&b.path));
  (files, more)
}

/// The seams of the tests: a system that makes no link, a slot on another volume than the notes
#[cfg(test)]
pub(crate) mod seam {
  thread_local! {
    pub(crate) static NO_LINKS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(crate) static ACROSS_VOLUMES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
  }
}

#[cfg(test)]
#[path = "lease_notes_test.rs"]
mod lease_notes_test;
