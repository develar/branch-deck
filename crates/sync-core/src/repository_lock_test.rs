//! Every rewrite of the checked out branch the app runs takes the repository lock in its core (`RewriteOptions`, the
//! `RegroupOptions` of the reword): it waits for another process that holds it (here the test itself, as another
//! process: `lock_repository_as_another_process`; the operations of one process wait in its line, `history_rewrite`'s
//! `repository_lock_test`), fails with `RepositoryLocked` after the timeout and changes nothing, and refuses another
//! HEAD than the expected one

use crate::add_issue_reference::{AddIssueReferenceParams, add_issue_reference_to_commits_core};
use crate::amend_to_branch::{AmendUncommittedToBranchParams, amend_uncommitted_to_branch_core};
use crate::unapply_branch::{UnapplyBranchParams, unapply_branch_core};
use domain_types::commit::CommitInfo;
use domain_types::rewrite::{AutosquashScope, CommitReword, RewordText};
use git_ops::rewrite_guard::HeadMoved;
use history_rewrite::autosquash::{SquashFixupsParams, squash_fixup_commits_core};
use history_rewrite::regroup::RegroupOptions;
use history_rewrite::repository_lock::{
  LockHolder, LockWait, LockWaitCanceled, LockWaiter, RepositoryLocked, RewriteOptions, lock_repository_as_another_process, lock_repository_in, lock_repository_while_held,
};
use history_rewrite::reword::{RewordRequest, reword_request_core};
use history_rewrite::squash::{RestoreBranchHeadParams, SquashCommitsParams, restore_branch_head_core, squash_commits_core};
use pretty_assertions::assert_eq;
use std::time::{Duration, Instant};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// Commits of `alpha` and `beta`, a fixup of the first one, the virtual branch of `beta`, and an uncommitted change of
/// `a1.txt`; returns `[a1, b1, a2, fixup]`
fn repo() -> (TestRepo, [String; 4]) {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "readme\n");
  let a1 = repo.create_commit("(alpha) A1", "a1.txt", "a1\n");
  let b1 = repo.create_commit("(beta) B1", "b1.txt", "b1\n");
  repo.create_branch("test-user/virtual/beta").unwrap();
  let a2 = repo.create_commit("(alpha) A2", "a2.txt", "a2\n");
  let fixup = repo.create_commit("fixup! (alpha) A1", "a1.txt", "a1 fixed\n");
  std::fs::write(repo.path().join("a2.txt"), "a2 amended\n").unwrap();
  (repo, [a1, b1, a2, fixup])
}

fn path(repo: &TestRepo) -> String {
  repo.path().to_str().unwrap().to_string()
}

/// The command line of the other process that holds the lock in these tests: an agent's `bd pick --follow`
const AGENT_PICK: &str = "bd pick --follow";

/// The repository lock of `repo`, held as another process holds it (the operations of this process wait for each other
/// in its line instead)
fn hold(repo: &TestRepo) -> history_rewrite::repository_lock::RepositoryLock {
  lock_repository_as_another_process(&repo.path().join(".git"), AGENT_PICK).unwrap()
}

/// The error of each rewrite of the app, run with `options`; `None` for one that succeeded
fn run_every_rewrite(repo: &TestRepo, [a1, b1, a2, fixup]: &[String; 4], options: &RewriteOptions) -> Vec<(&'static str, Option<String>)> {
  let git = repo.git_executor();
  let repository_path = path(repo);
  let error = |result: anyhow::Result<()>| result.err().map(|e| format!("{e:#}"));
  vec![
    (
      "squash",
      error(
        squash_commits_core(
          git,
          &SquashCommitsParams {
            repository_path: repository_path.clone(),
            commit_ids: vec![a1.clone(), a2.clone()],
            message: "(alpha) A1 and A2".to_string(),
          },
          options,
        )
        .map(drop),
      ),
    ),
    (
      "squash fixups",
      error(
        squash_fixup_commits_core(
          git,
          &SquashFixupsParams {
            repository_path: repository_path.clone(),
            expected_head: fixup.clone(),
            scope: AutosquashScope::All,
          },
          options,
        )
        .map(drop),
      ),
    ),
    (
      "unapply",
      error(
        unapply_branch_core(
          git,
          UnapplyBranchParams {
            repository_path: repository_path.clone(),
            branch_name: "beta".to_string(),
            original_commit_ids: Some(vec![b1.clone()]),
          },
          "test-user",
          options,
        )
        .map(drop),
      ),
    ),
    (
      "amend",
      amend_uncommitted_to_branch_core(
        git,
        AmendUncommittedToBranchParams {
          repository_path: repository_path.clone(),
          branch_name: "alpha".to_string(),
          original_commit_id: a2.clone(),
          files: vec!["a2.txt".to_string()],
        },
        options,
      )
      .err(),
    ),
    (
      "reword",
      error(
        reword_request_core(
          git,
          &RewordRequest {
            repository_path: repository_path.clone(),
            rewords: vec![CommitReword {
              commit: a2.clone(),
              text: RewordText::Message("(alpha) A2 reworded".to_string()),
            }],
          },
          // the reword is a regroup, whose options carry the same lock and HEAD
          &RegroupOptions {
            expect_head: options.expect_head.clone(),
            lock_wait: options.lock_wait,
            held_lock: options.held_lock,
            ..RegroupOptions::default()
          },
        )
        .map(drop),
      ),
    ),
    (
      "add issue reference",
      add_issue_reference_to_commits_core(
        git,
        AddIssueReferenceParams {
          repository_path,
          branch_name: "alpha".to_string(),
          commits: vec![CommitInfo {
            hash: a2.clone(),
            message: "(alpha) A2".to_string(),
          }],
          issue_reference: "ABC-1".to_string(),
        },
        options,
      )
      .err(),
    ),
  ]
}

/// The refs and the status: what a refused rewrite must leave as it was
fn state(repo: &TestRepo) -> (String, String) {
  (repo.git(&["for-each-ref", "--format=%(refname) %(objectname)"]), repo.git(&["status", "--porcelain"]))
}

#[test]
fn every_rewrite_fails_with_locked_while_another_holds_the_lock() {
  let (repo, commits) = repo();
  let held = hold(&repo);
  let before = state(&repo);
  let options = RewriteOptions {
    lock_wait: LockWait::Bounded(Duration::from_millis(100)),
    ..RewriteOptions::default()
  };

  for (rewrite, error) in run_every_rewrite(&repo, &commits, &options) {
    let error = error.unwrap_or_else(|| panic!("{rewrite} ran while the lock was held"));
    // the holder its record names: the command of the other process, and the pid of this test process
    assert!(error.starts_with(&format!("`{AGENT_PICK}` (pid {}, for ", std::process::id())), "{rewrite}: {error}");
    assert!(error.ends_with("; retry when it is done"), "{rewrite}: {error}");
  }
  assert_eq!(state(&repo), before);
  drop(held);
}

/// The app's rewrites run once the holder lets go, the undo of a squash too
#[test]
fn a_rewrite_waits_for_the_holder() {
  let (repo, [a1, _, a2, _]) = repo();
  let repository_path = path(&repo);
  // the other process lets go after a time: the bounded wait polls the lock on the system clock and tells no one
  // through `RewriteOptions` that it waits (only `LockWait::lock_with` takes an `on_wait`), so no event could time the
  // release
  let hold_for_a_while = || {
    let held = hold(&repo);
    let started = Instant::now();
    std::thread::spawn(move || {
      std::thread::sleep(Duration::from_millis(300));
      drop(held);
    });
    started
  };

  let started = hold_for_a_while();
  let params = SquashCommitsParams {
    repository_path: repository_path.clone(),
    commit_ids: vec![a1, a2],
    message: "(alpha) A1 and A2".to_string(),
  };
  let squashed = squash_commits_core(repo.git_executor(), &params, &RewriteOptions::default()).unwrap();
  assert!(started.elapsed() >= Duration::from_millis(300), "the squash did not wait");

  let started = hold_for_a_while();
  let undo = RestoreBranchHeadParams {
    repository_path,
    expected_head: squashed.new_head,
    previous_head: squashed.previous_head.clone(),
  };
  restore_branch_head_core(repo.git_executor(), &undo, LockWait::default()).unwrap();
  assert!(started.elapsed() >= Duration::from_millis(300), "the undo did not wait");
  assert_eq!(repo.head(), squashed.previous_head);
}

/// A caller that holds the lock passes it: the rewrite runs under it without a wait
#[test]
fn a_held_lock_is_not_taken_again() {
  let (repo, [a1, _, a2, _]) = repo();
  let held = hold(&repo);
  let options = RewriteOptions {
    lock_wait: LockWait::Bounded(Duration::from_millis(100)),
    held_lock: Some(&held),
    ..RewriteOptions::default()
  };
  let params = SquashCommitsParams {
    repository_path: path(&repo),
    commit_ids: vec![a1, a2],
    message: "(alpha) A1 and A2".to_string(),
  };

  let squashed = squash_commits_core(repo.git_executor(), &params, &options).unwrap();

  assert_eq!(repo.head(), squashed.new_head);
}

/// Another HEAD than `expect_head` refuses every rewrite with `HeadMoved` and changes nothing; an abbreviated id of HEAD
/// is HEAD
#[test]
fn every_rewrite_refuses_another_head_than_the_expected_one() {
  let (repo, commits) = repo();
  let before = state(&repo);
  let expected = &commits[2][..12];
  let options = RewriteOptions {
    expect_head: Some(expected.to_string()),
    ..RewriteOptions::default()
  };
  let moved = HeadMoved {
    branch: "master".to_string(),
    expected: expected.to_string(),
    actual: commits[3].clone(),
  };

  for (rewrite, error) in run_every_rewrite(&repo, &commits, &options) {
    assert_eq!(error.as_deref(), Some(moved.to_string().as_str()), "{rewrite}");
  }
  assert_eq!(state(&repo), before);

  let options = RewriteOptions {
    expect_head: Some(commits[3][..12].to_string()),
    ..RewriteOptions::default()
  };
  let squashed = squash_fixup_commits_core(
    repo.git_executor(),
    &SquashFixupsParams {
      repository_path: path(&repo),
      expected_head: commits[3].clone(),
      scope: AutosquashScope::All,
    },
    &options,
  )
  .unwrap();
  assert_eq!(squashed.squashed_count, 1);
}

/// `RepositoryLocked` stays an error of its own type through a core that returns `anyhow`, so `bd` names it `locked`
#[test]
fn the_lock_error_keeps_its_type() {
  let (repo, [_, _, _, fixup]) = repo();
  let _held = hold(&repo);
  let options = RewriteOptions {
    lock_wait: LockWait::Bounded(Duration::from_millis(100)),
    ..RewriteOptions::default()
  };
  let params = SquashFixupsParams {
    repository_path: path(&repo),
    expected_head: fixup,
    scope: AutosquashScope::All,
  };

  let error = squash_fixup_commits_core(repo.git_executor(), &params, &options).unwrap_err();

  assert!(error.downcast_ref::<RepositoryLocked>().is_some(), "{error:#}");
}

/// A holder writes its record once it has the lock (the command line of this process through its line) and deletes it
/// when it releases it; a bounded wait for another process refuses with the holder that record names
#[test]
fn a_lock_timeout_names_the_holder_and_its_release_deletes_the_record() {
  let dir = tempfile::tempdir().unwrap();
  let record = dir.path().join("branch-deck.lock.holder");
  let held = lock_repository_in(dir.path(), Duration::from_millis(100), &|| {}).unwrap();
  let written: LockHolder = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
  assert_eq!(written.pid, std::process::id());
  let program = std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().into_owned();
  assert!(written.command.starts_with(&program), "{written:?}");
  drop(held);
  assert!(!record.exists(), "the release deletes the record");

  let other = lock_repository_as_another_process(dir.path(), "bd check --fix").unwrap();
  let error = lock_repository_in(dir.path(), Duration::from_millis(100), &|| {}).unwrap_err();
  let locked = error.downcast_ref::<RepositoryLocked>().unwrap();
  let holder = locked.holder.as_ref().expect("the holder is named");
  assert_eq!((holder.command.as_str(), holder.pid), ("bd check --fix", std::process::id()));
  let message = error.to_string();
  assert!(message.starts_with(&format!("`bd check --fix` (pid {}, for ", std::process::id())), "{message}");
  assert!(message.contains(" s) holds "), "{message}");

  drop(other);
  assert!(!record.exists(), "the release deletes the record");
  drop(lock_repository_in(dir.path(), Duration::from_millis(100), &|| {}).unwrap());
}

#[test]
fn the_record_of_a_gone_holder_names_nobody() {
  let dir = tempfile::tempdir().unwrap();
  // a holder killed before it deleted its record, and a holder that writes none (Branch Deck 0.21)
  let gone = LockHolder {
    command: "bd sync".to_string(),
    pid: std::process::id(),
    process_started: Some(1),
    since: 0,
    subject: None,
  };
  std::fs::write(dir.path().join("branch-deck.lock.holder"), serde_json::to_vec(&gone).unwrap()).unwrap();
  let file = std::fs::OpenOptions::new()
    .create(true)
    .truncate(false)
    .write(true)
    .open(dir.path().join("branch-deck.lock"))
    .unwrap();
  file.try_lock().unwrap();

  let error = lock_repository_in(dir.path(), Duration::from_millis(100), &|| {}).unwrap_err();

  assert_eq!(error.downcast_ref::<RepositoryLocked>().unwrap().holder, None);
  assert!(error.to_string().starts_with("Another Branch Deck operation holds"), "{error}");
}

/// The app's wait for the lock (`lock_repository_while_held`) has no bound while the record names a live holder, which
/// it tells (`on_holder`), and ends with the lock once the holder releases it; a canceled wait ends with
/// `LockWaitCanceled`, and a lock no record names a live holder of refuses after the bound, as `lock_repository_in` does
#[test]
fn the_wait_for_a_live_holder_names_it_ends_when_canceled_and_refuses_an_unnamed_one() {
  let dir = tempfile::tempdir().unwrap();
  let held = lock_repository_as_another_process(dir.path(), AGENT_PICK).unwrap();
  // a bound of zero for a holder no record names: the wait for the named one doesn't count it
  let error = lock_repository_while_held(dir.path(), Duration::ZERO, &mut |_| {}, &|| true).unwrap_err();
  assert!(error.downcast_ref::<LockWaitCanceled>().is_some(), "{error:#}");

  let (told, holders) = std::sync::mpsc::channel();
  std::thread::scope(|scope| {
    // a bound of 500 ms for a holder no record names, which the wait passes while the named one holds; not zero: the
    // release deletes the record a moment before it frees the lock
    let wait = scope.spawn(|| lock_repository_while_held(dir.path(), Duration::from_millis(500), &mut |holder| told.send(holder.clone()).unwrap(), &|| false));
    let holder = holders.recv_timeout(Duration::from_mins(1)).unwrap();
    assert_eq!(holder.pid, std::process::id());
    let first = Instant::now();
    holders.recv_timeout(Duration::from_mins(1)).unwrap();
    assert!(first.elapsed() >= Duration::from_millis(500), "told again after {:?}", first.elapsed());
    drop(held);
    drop(wait.join().unwrap().expect("the lock once its holder released it"));
  });

  let held = lock_repository_as_another_process(dir.path(), AGENT_PICK).unwrap();
  std::fs::remove_file(dir.path().join("branch-deck.lock.holder")).unwrap();
  let error = lock_repository_while_held(dir.path(), Duration::from_millis(100), &mut |_| panic!("no holder to name"), &|| false).unwrap_err();
  assert_eq!(error.downcast_ref::<RepositoryLocked>().unwrap().holder, None);
  drop(held);
}

/// The app's waiter of a test: the holders it heard, a Stop Waiting that `stop` sets, and a short bound for a holder no
/// record names
#[derive(Default)]
struct TestWaiter {
  told: std::sync::Mutex<Vec<Option<u32>>>,
  stop: std::sync::atomic::AtomicBool,
}

impl LockWaiter for TestWaiter {
  fn holder(&self, holder: Option<&LockHolder>) {
    self.told.lock().unwrap().push(holder.map(|holder| holder.pid));
  }

  fn canceled(&self) -> bool {
    self.stop.load(std::sync::atomic::Ordering::Relaxed)
  }

  fn unknown_timeout(&self) -> Duration {
    Duration::from_millis(100)
  }
}

impl TestWaiter {
  /// Waits until the wait told a holder
  fn wait_told(&self) {
    let started = Instant::now();
    while self.told.lock().unwrap().is_empty() {
      assert!(started.elapsed() < Duration::from_mins(1), "the wait told no holder");
      std::thread::sleep(Duration::from_millis(10));
    }
  }
}

/// The app's rewrites (`LockWait::WhileHeld`) wait, without a bound, while the record of the holder names a live process
/// (an agent's `bd pick`: here the test itself as another process, past the bound for an unnamed holder), tell whom
/// they wait for, and run once the holder released the lock; the undo of a squash too
#[test]
fn a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after() {
  let (repo, [a1, _, a2, _]) = repo();
  let git = repo.git_executor();
  let waiter = TestWaiter::default();
  let options = RewriteOptions {
    lock_wait: LockWait::WhileHeld(&waiter),
    ..RewriteOptions::default()
  };
  let params = SquashCommitsParams {
    repository_path: path(&repo),
    commit_ids: vec![a1, a2],
    message: "(alpha) A1 and A2".to_string(),
  };
  let held = hold(&repo);
  let before = repo.head();
  let squashed = std::thread::scope(|scope| {
    let squash = scope.spawn(|| squash_commits_core(git, &params, &options));
    waiter.wait_told();
    // past the bound for a holder no record names: the named one keeps it waiting, nothing is written meanwhile. The
    // bound runs on the system clock of the wait, which takes no other, so the test lets it pass
    std::thread::sleep(Duration::from_millis(300));
    assert!(!squash.is_finished());
    assert_eq!(repo.head(), before);
    drop(held);
    squash.join().unwrap().unwrap()
  });
  assert_eq!(repo.head(), squashed.new_head);
  let told = waiter.told.lock().unwrap().clone();
  assert_eq!(told.first(), Some(&Some(std::process::id())));
  assert_eq!(told.last(), Some(&None), "the end of the wait is told: {told:?}");

  let undo = RestoreBranchHeadParams {
    repository_path: path(&repo),
    expected_head: squashed.new_head,
    previous_head: squashed.previous_head.clone(),
  };
  restore_branch_head_core(git, &undo, LockWait::WhileHeld(&waiter)).unwrap();
  assert_eq!(repo.head(), squashed.previous_head);
}

/// The error of each rewrite of the app that waits with `waiter` while the lock is held, which none may run past
fn refusals_while_waiting(repo: &TestRepo, commits: &[String; 4], waiter: &TestWaiter) -> Vec<(&'static str, String)> {
  let options = RewriteOptions::waiting(LockWait::WhileHeld(waiter));
  run_every_rewrite(repo, commits, &options)
    .into_iter()
    .map(|(rewrite, error)| (rewrite, error.unwrap_or_else(|| panic!("{rewrite} ran while the lock was held"))))
    .collect()
}

/// Stop Waiting ends the wait of every rewrite of the app with `LockWaitCanceled` and nothing changed; a lock no record
/// names a live holder of refuses them after the bound, with `RepositoryLocked`
#[test]
fn a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder() {
  let (repo, commits) = repo();
  let held = hold(&repo);
  let before = state(&repo);
  let stopped = TestWaiter::default();
  stopped.stop.store(true, std::sync::atomic::Ordering::Relaxed);
  for (rewrite, error) in refusals_while_waiting(&repo, &commits, &stopped) {
    assert!(error.contains("was canceled"), "{rewrite}: {error}");
  }
  assert_eq!(state(&repo), before);

  // a holder that writes no record: the bound of the waiter, then the refusal
  std::fs::remove_file(repo.path().join(".git/branch-deck.lock.holder")).unwrap();
  for (rewrite, error) in refusals_while_waiting(&repo, &commits, &TestWaiter::default()) {
    assert!(error.starts_with("Another Branch Deck operation holds"), "{rewrite}: {error}");
  }
  assert_eq!(state(&repo), before);
  drop(held);
}
