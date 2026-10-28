//! The line of this process before the repository lock: its operations wait there for each other, in the order they
//! came and as long as the ones before them take, so none refuses another (`RepositoryLocked`, the bound of `bd` and the
//! bound of a holder no record names are for another process); a stopped wait leaves its place, a try waits for nobody

use crate::repository_lock::{
  DEFAULT_LOCK_TIMEOUT, LockWait, LockWaitCanceled, RepositoryLocked, RewriteOptions, line_key, lines, lock_repository_as_another_process, lock_repository_in,
  lock_repository_while_held, try_lock_repository_in,
};
use crate::squash::{SquashCommitsParams, squash_commits_core};
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::thread::ScopedJoinHandle;
use std::time::{Duration, Instant};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The command line the record of another process names: an agent's push
const AGENT_PUSH: &str = "bd push auth";

/// How many operations of this process wait in the line of the lock file `path`
fn waiting(path: &Path) -> usize {
  lines().get(&line_key(path)).map_or(0, |line| line.state().waiting.len())
}

/// Until `count` operations wait in the line of `path`; `run`, the last that came, must not end first
fn wait_until_waiting<T>(path: &Path, count: usize, run: &ScopedJoinHandle<'_, T>) {
  let started = Instant::now();
  while waiting(path) < count {
    assert!(!run.is_finished(), "it ended without a wait in the line");
    assert!(started.elapsed() < Duration::from_mins(1), "{} wait in the line, not {count}", waiting(path));
    std::thread::yield_now();
  }
}

/// A rewrite of the app behind a short push of the same app (its freeze under the lock): it waits in the line, past
/// any bound, and runs once the push let go; never `locked`, though its bound for another process is zero
#[test]
fn a_rewrite_behind_a_short_push_of_its_own_process_waits_in_its_line_and_is_never_locked() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "README.md", "readme");
  let a = repo.create_commit("Add a", "a.txt", "a");
  let b = repo.create_commit("Add b", "b.txt", "b");
  let common_dir = repo.path().join(".git");
  let lock = common_dir.join("branch-deck.lock");
  let params = SquashCommitsParams {
    repository_path: repo.path().to_str().unwrap().to_string(),
    commit_ids: vec![a, b],
    message: "Add a and b".to_string(),
  };
  // a file lock of this process would refuse it at once
  let options = RewriteOptions::waiting(LockWait::Bounded(Duration::ZERO));
  let push = lock_repository_in(&common_dir, DEFAULT_LOCK_TIMEOUT, &|| {}).unwrap();
  let before = repo.head();

  let squashed = std::thread::scope(|scope| {
    let squash = scope.spawn(|| squash_commits_core(repo.git_executor(), &params, &options));
    wait_until_waiting(&lock, 1, &squash);
    assert_eq!(repo.head(), before, "nothing is written while it waits");
    drop(push);
    squash.join().unwrap()
  });

  let squashed = squashed.unwrap_or_else(|error| panic!("the rewrite was refused: {error:#}"));
  assert_eq!(repo.head(), squashed.new_head);
  assert!(!lines().contains_key(&line_key(&lock)), "the line goes once nobody is in it");
}

/// The turn goes in the order the operations came; one whose wait is stopped in the line leaves it (`LockWaitCanceled`)
/// and the next takes its place; no bound counts in the line, not even a zero one
#[test]
fn the_line_gives_the_turn_in_the_order_they_came_and_a_stopped_wait_leaves_it() {
  let dir = tempfile::tempdir().unwrap();
  let lock = dir.path().join("branch-deck.lock");
  let held = lock_repository_in(dir.path(), Duration::ZERO, &|| {}).unwrap();
  let order = Mutex::new(Vec::new());
  let stop = AtomicBool::new(false);
  let take = |name: &'static str| {
    let lock = lock_repository_in(dir.path(), Duration::ZERO, &|| {}).unwrap();
    order.lock().unwrap().push(name);
    drop(lock);
  };

  std::thread::scope(|scope| {
    let first = scope.spawn(|| take("first"));
    wait_until_waiting(&lock, 1, &first);
    let stopped = scope.spawn(|| lock_repository_while_held(dir.path(), Duration::ZERO, &mut |_| {}, &|| stop.load(Ordering::SeqCst)));
    wait_until_waiting(&lock, 2, &stopped);
    let second = scope.spawn(|| take("second"));
    wait_until_waiting(&lock, 3, &second);

    stop.store(true, Ordering::SeqCst);
    let error = stopped.join().unwrap().unwrap_err();
    assert!(error.downcast_ref::<LockWaitCanceled>().is_some(), "{error:#}");
    assert_eq!(waiting(&lock), 2);
    drop(held);
    first.join().unwrap();
    second.join().unwrap();
  });

  assert_eq!(*order.lock().unwrap(), ["first", "second"]);
}

/// A try takes the lock only when nobody holds it, this process or another one, and never waits
#[test]
fn a_try_takes_the_lock_only_while_nobody_holds_it() {
  let dir = tempfile::tempdir().unwrap();
  let held = try_lock_repository_in(dir.path()).unwrap().expect("a free lock");
  assert!(try_lock_repository_in(dir.path()).unwrap().is_none(), "this process holds it");
  drop(held);
  let other = lock_repository_as_another_process(dir.path(), AGENT_PUSH).unwrap();
  assert!(try_lock_repository_in(dir.path()).unwrap().is_none(), "another process holds it");
  drop(other);
  assert!(try_lock_repository_in(dir.path()).unwrap().is_some());
}

/// While another process holds the lock, every wait of this process names it: the one whose turn it is at the file
/// lock, the ones behind it in the line; once it let go, each takes the lock in turn
#[test]
fn every_wait_of_this_process_names_the_other_process_that_holds_the_lock() {
  let dir = tempfile::tempdir().unwrap();
  let lock = dir.path().join("branch-deck.lock");
  let other = lock_repository_as_another_process(dir.path(), AGENT_PUSH).unwrap();
  let (told, heard) = channel();
  let common_dir = dir.path();
  let wait = |name: &'static str| {
    let told = told.clone();
    move || {
      lock_repository_while_held(common_dir, DEFAULT_LOCK_TIMEOUT, &mut |holder| told.send((name, holder.command.clone())).unwrap(), &|| {
        false
      })
      .map(drop)
    }
  };
  let heard_from = |name: &str| {
    while heard.recv_timeout(Duration::from_mins(1)).unwrap().0 != name {}
  };

  std::thread::scope(|scope| {
    let first = scope.spawn(wait("first"));
    heard_from("first");
    let second = scope.spawn(wait("second"));
    wait_until_waiting(&lock, 1, &second);
    heard_from("second");
    assert!(!first.is_finished());
    drop(other);
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
  });

  drop(told);
  assert!(heard.iter().all(|(_, command)| command == AGENT_PUSH));
}

/// A bounded wait (`bd`) refuses after its bound only another process, which it names; a lock taken as another process
/// waits for nobody of this process and refuses at once while it is held
#[test]
fn only_another_process_refuses_a_bounded_wait() {
  let dir = tempfile::tempdir().unwrap();
  let other = lock_repository_as_another_process(dir.path(), AGENT_PUSH).unwrap();
  let error = lock_repository_in(dir.path(), Duration::from_millis(100), &|| {}).unwrap_err();
  let holder = error
    .downcast_ref::<RepositoryLocked>()
    .and_then(|locked| locked.holder.clone())
    .expect("the holder is named");
  assert_eq!((holder.command.as_str(), holder.pid), (AGENT_PUSH, std::process::id()));
  assert!(error.to_string().starts_with("`bd push auth` (pid "), "{error}");

  let error = lock_repository_as_another_process(dir.path(), "bd sync").unwrap_err();
  assert!(error.downcast_ref::<RepositoryLocked>().is_some(), "{error:#}");
  drop(other);
  let held = lock_repository_in(dir.path(), Duration::ZERO, &|| {}).unwrap();
  assert!(lock_repository_as_another_process(dir.path(), "bd sync").is_err(), "this process holds it");
  drop(held);
}

/// A rewrite computed before the lock (`Prepared`), as a test sees what the lock does with it
struct Computed {
  witness: crate::repository_lock::Witness,
  /// Which computation it is: 1 before the lock, 2 the one under it
  round: u32,
  discarded: std::rc::Rc<std::cell::Cell<u32>>,
}

impl crate::repository_lock::Prepared for Computed {
  fn witness(&self) -> crate::repository_lock::Witness {
    self.witness.clone()
  }

  fn discard(&self, _git_executor: &git_executor::git_command_executor::GitCommandExecutor, _repository_path: &str) {
    self.discarded.set(self.discarded.get() + 1);
  }
}

/// `lock_prepared` writes what was computed before the lock while its witness holds; a witness that missed discards it
/// and computes once more under the lock; a lock not taken discards it and computes nothing more; a caller that holds
/// the lock computed under it, so no witness is read
#[test]
fn a_rewrite_computed_before_the_lock_is_written_while_its_witness_holds_else_computed_once_more_under_it() {
  use crate::repository_lock::{Witness, lock_prepared};
  let repo = TestRepo::new();
  let head = repo.create_commit("One", "one.txt", "one");
  let branch = repo.git(&["symbolic-ref", "--short", "HEAD"]);
  let discarded = std::rc::Rc::new(std::cell::Cell::new(0));
  let computed = |round, head: &str| Computed {
    witness: Witness::of_head(head, &branch),
    round,
    discarded: std::rc::Rc::clone(&discarded),
  };
  let (git, path) = (repo.git_executor(), repo.path_str());
  let lock = (LockWait::default(), None);

  let (written, held) = lock_prepared(lock, git, path, computed(1, &head), || anyhow::Ok(computed(2, &head))).unwrap();
  assert_eq!((written.round, held.is_some(), discarded.get()), (1, true, 0));
  drop(held);

  let (written, held) = lock_prepared(lock, git, path, computed(1, "0000000000000000000000000000000000000000"), || anyhow::Ok(computed(2, &head))).unwrap();
  assert_eq!((written.round, held.is_some(), discarded.get()), (2, true, 1), "the witness missed");
  drop(held);

  let other = lock_repository_as_another_process(&repo.path().join(".git"), AGENT_PUSH).unwrap();
  let bounded = (LockWait::Bounded(Duration::ZERO), None);
  let error = lock_prepared(bounded, git, path, computed(1, &head), || anyhow::Ok(computed(2, &head))).err().unwrap();
  assert!(error.downcast_ref::<RepositoryLocked>().is_some(), "{error:#}");
  assert_eq!(discarded.get(), 2, "a lock not taken discards what was computed");

  let (written, held) = lock_prepared((LockWait::default(), Some(&other)), git, path, computed(1, "moved"), || anyhow::Ok(computed(2, &head))).unwrap();
  assert_eq!((written.round, held.is_none(), discarded.get()), (1, true, 2), "computed under the held lock");
}
