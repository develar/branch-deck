use crate::lease::{Lease, LeaseState};
use crate::lease_notes::{
  NOTES_LINK, all_notes, close_notes, expire_notes, expire_pool_notes, expires_at, note_files, notes_dir, now_seconds, open_notes, read_notes, record_path, seam,
};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A slot, the common dir of its repository and the root of the notes, each a directory of `dir`
struct Pool {
  _dir: tempfile::TempDir,
  slot: PathBuf,
  git_common_dir: PathBuf,
  root: PathBuf,
}

impl Pool {
  fn new() -> Self {
    let dir = tempfile::tempdir().unwrap();
    let (slot, git_common_dir) = (dir.path().join("1/repo"), dir.path().join("repo/.git"));
    std::fs::create_dir_all(&slot).unwrap();
    std::fs::create_dir_all(&git_common_dir).unwrap();
    let root = git_common_dir.join("branch-deck/notes");
    Self {
      _dir: dir,
      slot,
      git_common_dir,
      root,
    }
  }

  fn lease(holder: &str) -> Lease {
    Lease::marker("lease", holder, LeaseState::Ready)
  }

  fn open(&self, lease: &Lease) -> PathBuf {
    open_notes(&self.root, &self.slot, &self.git_common_dir, lease).unwrap().dir
  }

  fn notes_local(&self) -> PathBuf {
    self.slot.join(NOTES_LINK)
  }
}

fn write(path: &Path, text: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, text).unwrap();
}

/// The get links `notes.local` to the notes of its lease, outside the worktree; the return removes the link and the
/// notes stay, with the return in their record; the next get of the slot sees an empty `notes.local` of its own
#[test]
fn the_notes_of_a_lease_outlive_its_return_and_the_next_lease_starts_empty() {
  let pool = Pool::new();
  let first = Pool::lease("first");
  let dir = pool.open(&first);
  assert_eq!(dir, notes_dir(&pool.root, &first.id));
  assert!(pool.notes_local().symlink_metadata().unwrap().file_type().is_symlink());
  write(&pool.notes_local().join("logs/gate.log"), "green\n");
  assert_eq!(std::fs::read_to_string(dir.join("logs/gate.log")).unwrap(), "green\n");

  let closed = close_notes(&pool.root, &pool.slot, &first.id).expect("the notes of the lease");
  assert!(pool.notes_local().symlink_metadata().is_err(), "the return removes the link");
  assert!(closed.record.returned_at.is_some(), "{closed:?}");
  assert_eq!(std::fs::read_to_string(dir.join("logs/gate.log")).unwrap(), "green\n");

  let second = Pool::lease("second");
  let next = pool.open(&second);
  assert!(std::fs::read_dir(pool.notes_local()).unwrap().next().is_none(), "the next holder starts empty");
  assert_ne!(next, dir);
  let holders: Vec<String> = all_notes(&pool.root).into_iter().map(|notes| notes.record.holder).collect();
  assert_eq!(holders.len(), 2, "{holders:?}");
  assert!(holders.contains(&"first".to_string()) && holders.contains(&"second".to_string()), "{holders:?}");
}

/// The inode of the file at `path` on Unix: a rename keeps it, a copy across volumes makes a new one
#[cfg_attr(unix, expect(clippy::unnecessary_wraps, reason = "none on the other systems"))]
fn file_id(path: &Path) -> Option<u64> {
  #[cfg(unix)]
  return Some(std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(path).unwrap()));
  #[cfg(not(unix))]
  {
    let _ = path;
    None
  }
}

/// That the file `moved` was copied across the volumes, a new file, or else renamed, the file of `before`
fn assert_moved(before: Option<u64>, moved: &Path, across_volumes: bool) {
  if let (Some(before), Some(after)) = (before, file_id(moved)) {
    assert_eq!(
      before != after,
      across_volumes,
      "across volumes {across_volumes}: {} was copied: {}",
      moved.display(),
      before != after
    );
  }
}

/// The entries of the root of the notes without a record (the records themselves left out)
fn unrecorded(pool: &Pool) -> Vec<String> {
  std::fs::read_dir(&pool.root)
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .filter(|name| Path::new(name).extension().is_none_or(|extension| extension != "json") && !pool.root.join(format!("{name}.json")).exists())
    .collect()
}

/// Where the system makes no link, `notes.local` is a directory of the worktree, and the return moves its entries into
/// the notes of the lease, also where the slot and the notes are on two volumes, which no rename crosses
#[test]
fn without_a_link_the_return_moves_the_notes_out_of_the_worktree() {
  for across_volumes in [false, true] {
    let pool = Pool::new();
    seam::NO_LINKS.set(true);
    seam::ACROSS_VOLUMES.set(across_volumes);
    let lease = Pool::lease("windows");
    let dir = pool.open(&lease);
    assert!(pool.notes_local().symlink_metadata().unwrap().is_dir());
    assert!(!read_notes(&pool.root, &lease.id).unwrap().record.linked);
    write(&pool.notes_local().join("probe/a.txt"), "a\n");
    write(&pool.notes_local().join("b.log"), "b\n");
    let before = file_id(&pool.notes_local().join("probe/a.txt"));

    close_notes(&pool.root, &pool.slot, &lease.id).expect("the notes of the lease");
    assert_moved(before, &dir.join("probe/a.txt"), across_volumes);
    assert!(
      pool.notes_local().symlink_metadata().is_err(),
      "across volumes {across_volumes}: the directory is moved out"
    );
    assert_eq!(std::fs::read_to_string(dir.join("probe/a.txt")).unwrap(), "a\n");
    assert_eq!(std::fs::read_to_string(dir.join("b.log")).unwrap(), "b\n");
    assert_eq!(unrecorded(&pool), Vec::<String>::new(), "across volumes {across_volumes}: no copy is left");
  }
}

/// A `notes.local` that an earlier lease left in the slot (a brd before the notes, a return that could not move it) goes
/// into notes of its own, returned: the next holder never reads it as its own, also across two volumes
#[test]
fn what_an_earlier_lease_left_in_notes_local_is_set_aside() {
  for across_volumes in [false, true] {
    let pool = Pool::new();
    seam::ACROSS_VOLUMES.set(across_volumes);
    write(&pool.notes_local().join("logs/stress.log"), "an earlier run\n");
    let before = file_id(&pool.notes_local().join("logs/stress.log"));
    let lease = Pool::lease("next");
    pool.open(&lease);
    assert!(
      std::fs::read_dir(pool.notes_local()).unwrap().next().is_none(),
      "across volumes {across_volumes}: the next holder starts empty"
    );
    let left = all_notes(&pool.root)
      .into_iter()
      .find(|notes| notes.record.lease_id.starts_with("left-"))
      .expect("the notes set aside");
    assert!(left.record.returned_at.is_some(), "{left:?}");
    assert_eq!(std::fs::read_to_string(left.dir.join("logs/stress.log")).unwrap(), "an earlier run\n");
    assert_moved(before, &left.dir.join("logs/stress.log"), across_volumes);
    assert_eq!(unrecorded(&pool), Vec::<String>::new(), "across volumes {across_volumes}: no copy is left");
  }
}

/// A get whose notes fail after their directory is made (here the record can't be written) leaves no notes: no
/// directory without a record, no link in the slot
#[test]
fn a_get_whose_notes_fail_leaves_none() {
  let pool = Pool::new();
  let lease = Pool::lease("failing");
  std::fs::create_dir_all(record_path(&pool.root, &lease.id)).unwrap();

  open_notes(&pool.root, &pool.slot, &pool.git_common_dir, &lease).unwrap_err();

  assert!(notes_dir(&pool.root, &lease.id).symlink_metadata().is_err(), "the directory of the notes is removed");
  assert!(pool.notes_local().symlink_metadata().is_err(), "the link is removed");
}

/// A directory of the notes without a record (a get that stopped before it wrote one) expires `keep` after its
/// modification time, never one of a held lease nor with `keep` 0, and a plan deletes nothing
#[test]
fn a_directory_without_a_record_expires_by_its_age() {
  let pool = Pool::new();
  let (orphan, held) = (pool.root.join("orphan"), pool.root.join("held"));
  write(&orphan.join("partial.log"), "partial\n");
  std::fs::create_dir_all(&held).unwrap();
  let holding: HashSet<String> = ["held".to_string()].into();
  let keep = Duration::from_hours(24);
  let later = now_seconds() + keep.as_secs() + 60;

  assert!(expire_notes(&pool.root, &holding, keep, now_seconds(), false).is_empty(), "not yet");
  assert!(expire_notes(&pool.root, &holding, Duration::ZERO, later, false).is_empty(), "keep 0");
  let planned = expire_notes(&pool.root, &holding, keep, later, true);
  assert_eq!(
    planned.iter().map(|notes| (notes.lease_id.as_str(), notes.holder.as_deref())).collect::<Vec<_>>(),
    [("orphan", None)]
  );
  assert!(orphan.is_dir(), "a plan deletes nothing");

  let expired = expire_notes(&pool.root, &holding, keep, later, false);
  assert_eq!(expired.len(), 1, "{expired:?}");
  assert!(expired[0].bytes > 0 && expired[0].error.is_none(), "{expired:?}");
  assert!(!orphan.exists() && held.is_dir());
}

/// The keeper deletes nothing where the state of the pool can't be read: without it no lease is known to be held
#[test]
fn no_notes_expire_where_the_state_of_the_pool_cannot_be_read() {
  let pool = Pool::new();
  let lease = Pool::lease("held long");
  let dir = pool.open(&lease);
  let mut record = read_notes(&pool.root, &lease.id).unwrap().record;
  record.leased_at = 1;
  std::fs::write(record_path(&pool.root, &lease.id), serde_json::to_vec(&record).unwrap()).unwrap();
  std::fs::write(pool.git_common_dir.join("branch-deck/leases.json"), "not the state").unwrap();

  assert_eq!(expire_pool_notes(&GitCommandExecutor::new(), &pool.git_common_dir), Vec::new());

  assert!(dir.is_dir() && read_notes(&pool.root, &lease.id).is_some(), "the notes stay");
  std::fs::remove_file(pool.git_common_dir.join("branch-deck/leases.json")).unwrap();
  assert_eq!(
    expire_pool_notes(&GitCommandExecutor::new(), &pool.git_common_dir).len(),
    1,
    "without the state file no lease is held"
  );
}

/// The get keeps the link out of git: `/notes.local` in `info/exclude`, once, after what the file had
#[test]
fn the_notes_are_excluded_once() {
  let pool = Pool::new();
  write(&pool.git_common_dir.join("info/exclude"), "*.swp");
  pool.open(&Pool::lease("one"));
  pool.open(&Pool::lease("two"));
  let exclude = std::fs::read_to_string(pool.git_common_dir.join("info/exclude")).unwrap();
  assert!(exclude.starts_with("*.swp\n"), "{exclude}");
  assert_eq!(exclude.lines().filter(|line| *line == "/notes.local").count(), 1, "{exclude}");
}

/// The notes of a returned lease expire `keep` after the return, never those of a held lease, never with `keep` 0; a
/// plan deletes nothing
#[test]
fn the_notes_expire_after_the_return_and_never_while_the_lease_is_held() {
  let pool = Pool::new();
  let (returned, held) = (Pool::lease("returned"), Pool::lease("held"));
  let returned_dir = pool.open(&returned);
  write(&returned_dir.join("old.log"), "old\n");
  let held_dir = pool.open(&held);
  close_notes(&pool.root, &pool.slot, &returned.id);
  let returned_at = read_notes(&pool.root, &returned.id).unwrap().record.returned_at.unwrap();
  let holding: HashSet<String> = [held.id.clone()].into();
  let keep = Duration::from_hours(14 * 24);
  assert_eq!(expires_at(&read_notes(&pool.root, &held.id).unwrap().record, &holding, keep), None);
  assert_eq!(expires_at(&read_notes(&pool.root, &returned.id).unwrap().record, &holding, Duration::ZERO), None);

  assert!(expire_notes(&pool.root, &holding, keep, returned_at + keep.as_secs() - 1, false).is_empty(), "not yet");
  let planned = expire_notes(&pool.root, &holding, keep, returned_at + keep.as_secs(), true);
  assert_eq!(planned.iter().map(|notes| notes.lease_id.as_str()).collect::<Vec<_>>(), [returned.id.as_str()]);
  assert!(returned_dir.is_dir(), "a plan deletes nothing");

  let expired = expire_notes(&pool.root, &holding, keep, returned_at + keep.as_secs(), false);
  assert_eq!(expired.len(), 1, "{expired:?}");
  assert!(expired[0].bytes > 0 && expired[0].error.is_none(), "{expired:?}");
  assert!(!returned_dir.exists() && read_notes(&pool.root, &returned.id).is_none());
  assert!(held_dir.is_dir() && read_notes(&pool.root, &held.id).is_some(), "the held lease keeps its notes");
}

/// The files of the notes, by path, without following a link
#[test]
fn the_files_of_the_notes_are_listed_by_path() {
  let pool = Pool::new();
  let dir = pool.open(&Pool::lease("lister"));
  write(&dir.join("b/c.txt"), "cc\n");
  write(&dir.join("a.log"), "a\n");
  let (files, more) = note_files(&dir);
  let listed: Vec<(&str, u64)> = files.iter().map(|file| (file.path.as_str(), file.bytes)).collect();
  assert_eq!(listed, [("a.log", 2), ("b/c.txt", 3)]);
  assert!(!more);
}
