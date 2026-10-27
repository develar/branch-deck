use super::{Identity, IndexLock, IndexLocked, remove_if_same};
use crate::rewrite_guard::IndexUpdate;
use anyhow::{Result, bail};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn lock_path(repo: &TestRepo) -> PathBuf {
  repo.path().join(".git").join("index.lock")
}

fn lock_of(repo: &TestRepo) -> IndexLock<'_> {
  IndexLock::at(repo.git_executor(), repo.path_str(), &repo.path().join(".git").display().to_string()).with_owner_check(Duration::from_millis(100))
}

/// `update-index -q --refresh` of the user's index, counted once git answered: git refuses it while another process
/// holds the lock, as the refresh has stat data to write (`repo_with_a_commit` touched the file)
fn refresh(repo: &TestRepo, runs: &AtomicU32) -> Result<()> {
  let answer = repo.git_executor().execute_command_with_status(&["update-index", "-q", "--refresh"], repo.path_str());
  runs.fetch_add(1, Ordering::SeqCst);
  let (output, exit_code) = answer?;
  if exit_code != 0 {
    bail!("Failed to refresh the index: {output}");
  }
  Ok(())
}

/// A repository whose index has stale stat data of `a.txt`: its refresh writes the index, so it needs the lock
fn repo_with_a_commit() -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let file = std::fs::File::options().write(true).open(repo.path().join("a.txt")).unwrap();
  file.set_modified(SystemTime::now() + Duration::from_secs(60)).unwrap();
  repo
}

#[test]
fn a_step_blocked_by_the_lock_of_another_process_runs_again_once_the_lock_is_gone() {
  let repo = repo_with_a_commit();
  let path = lock_path(&repo);
  std::fs::write(&path, "").unwrap();
  let runs = Arc::new(AtomicU32::new(0));
  // the other process lets go of the lock once the first run was refused
  let remover = {
    let (path, runs) = (path, Arc::clone(&runs));
    std::thread::spawn(move || {
      while runs.load(Ordering::SeqCst) == 0 {
        std::thread::sleep(Duration::from_millis(10));
      }
      std::fs::remove_file(path).unwrap();
    })
  };
  // owners are asked every 2 s: the thread removes the lock long before a removal of an ownerless one
  let lock = IndexLock::at(repo.git_executor(), repo.path_str(), &repo.path().join(".git").display().to_string());
  lock.retry(Duration::from_secs(20), || refresh(&repo, &runs)).unwrap();
  remover.join().unwrap();
  assert_eq!(runs.load(Ordering::SeqCst), 2);
}

#[test]
fn a_step_that_fails_on_its_own_returns_its_error_after_one_run_although_a_younger_lock_exists() {
  let repo = repo_with_a_commit();
  let path = lock_path(&repo);
  let runs = Cell::new(0);
  let error = lock_of(&repo)
    // the step started a minute before the lock appeared: a peer took the lock well after it, and the step refused for a
    // reason of its own
    .with_clock(|| SystemTime::now() - Duration::from_mins(1))
    .retry(Duration::from_secs(5), || -> Result<()> {
      runs.set(runs.get() + 1);
      std::fs::write(&path, "").unwrap();
      bail!("own refusal")
    })
    .unwrap_err();
  assert_eq!((format!("{error:#}"), runs.get()), ("own refusal".to_string(), 1));
  assert!(error.downcast_ref::<IndexLocked>().is_none());
}

#[test]
fn a_lock_that_no_process_owns_is_removed_and_the_step_runs_again() {
  let repo = repo_with_a_commit();
  let path = lock_path(&repo);
  // what a git killed by SIGKILL leaves: an empty lock that nobody has open
  std::fs::write(&path, "").unwrap();
  let runs = AtomicU32::new(0);
  let result = lock_of(&repo).retry(Duration::from_secs(3), || refresh(&repo, &runs));
  if cfg!(windows) {
    // no owner check: the lock stays and the answer is `IndexLocked`, without a process to name
    let error = result.unwrap_err();
    let locked = error.downcast_ref::<IndexLocked>().unwrap_or_else(|| panic!("not locked: {error:#}"));
    assert_eq!(locked.holders, []);
    assert!(path.exists());
  } else {
    result.unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 2);
    assert!(!path.exists());
  }
}

#[cfg(unix)]
#[test]
fn a_lock_that_a_process_has_open_stays_and_the_answer_names_the_process() {
  let repo = repo_with_a_commit();
  let path = lock_path(&repo);
  // a git in the middle of its write: the file is open
  let mut holder = std::process::Command::new("sh")
    .args(["-c", "exec 3>\"$1\"; exec sleep 30", "sh"])
    .arg(&path)
    .spawn()
    .unwrap();
  while !path.exists() {
    std::thread::sleep(Duration::from_millis(10));
  }
  let runs = AtomicU32::new(0);
  let result = lock_of(&repo).retry(Duration::from_secs(1), || refresh(&repo, &runs));
  holder.kill().unwrap();
  holder.wait().unwrap();
  let error = result.unwrap_err();
  let locked = error.downcast_ref::<IndexLocked>().unwrap_or_else(|| panic!("not locked: {error:#}"));
  assert_eq!(locked.holders.iter().map(|holder| holder.pid).collect::<Vec<_>>(), [holder.id()]);
  assert!(locked.holders[0].command.contains("sleep 30"), "{locked:?}");
  assert!(format!("{error:#}").contains(&format!("({})", holder.id())), "{error:#}");
  assert_eq!(runs.load(Ordering::SeqCst), 1);
  assert!(path.exists(), "a held lock is never removed");
}

#[test]
fn a_lock_file_that_took_the_place_of_the_checked_one_is_not_removed() {
  let repo = repo_with_a_commit();
  let path = lock_path(&repo);
  std::fs::write(&path, "").unwrap();
  let checked = Identity::of(&path).unwrap();
  replace(&path);
  assert!(!remove_if_same(&path, &checked));
  assert!(path.exists());
  let now = Identity::of(&path).unwrap();
  assert!(remove_if_same(&path, &now));
  assert!(!path.exists());
}

/// Another file at `path`: another inode, and a later change (a minute on, so no clock has to tick for it)
fn replace(path: &Path) {
  let other = path.with_extension("other");
  std::fs::write(&other, "x").unwrap();
  let later = std::fs::metadata(path).unwrap().modified().unwrap() + Duration::from_mins(1);
  std::fs::File::options().write(true).open(&other).unwrap().set_modified(later).unwrap();
  std::fs::remove_file(path).unwrap();
  std::fs::rename(&other, path).unwrap();
}

#[test]
fn the_index_of_a_rewrite_follows_past_a_lock_that_no_process_owns() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let old_tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
  let rewritten = repo.create_commit("change", "b.txt", "b\n");
  let new_tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
  // the branch was rewritten to `new_tree`, and the index still holds the old tree: a drop's state before its update
  repo.git(&["read-tree", &old_tree]);
  let path = lock_path(&repo);
  std::fs::write(&path, "").unwrap();
  let update = IndexUpdate::TwoWay { old_tree, new_tree };
  let result = update.apply(repo.git_executor(), repo.path_str());
  if cfg!(windows) {
    // no owner check: the index can't follow, and the error says how to repair it
    assert!(format!("{:#}", result.unwrap_err()).contains("git reset -q"));
  } else {
    result.unwrap();
    assert!(!path.exists());
    assert_eq!(repo.git(&["diff", "--cached", "--name-only", &rewritten]), "");
  }
}
