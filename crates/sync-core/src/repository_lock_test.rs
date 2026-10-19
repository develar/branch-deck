//! Every rewrite of the checked out branch the app runs takes the repository lock in its core (`RewriteOptions`): it
//! waits for another holder, fails with `RepositoryLocked` after the timeout and changes nothing, and refuses another
//! HEAD than the expected one

use crate::add_issue_reference::{AddIssueReferenceParams, add_issue_reference_to_commits_core};
use crate::amend_to_branch::{AmendUncommittedToBranchParams, amend_uncommitted_to_branch_core};
use crate::autosquash::{AutosquashScope, SquashFixupsParams, squash_fixup_commits_core};
use crate::repository_lock::{LockHolder, RepositoryLocked, RewriteOptions, lock_repository, lock_repository_in};
use crate::reword_commit::{CommitRewrite, RewordCommitsRequest, reword_commits_core};
use crate::squash::{RestoreBranchHeadParams, SquashCommitsParams, restore_branch_head_core, squash_commits_core};
use crate::unapply_branch::{UnapplyBranchParams, unapply_branch_core};
use git_ops::model::CommitInfo;
use git_ops::rewrite_guard::HeadMoved;
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
            branch_name: "test-user/virtual/beta".to_string(),
            branch_prefix: "test-user".to_string(),
            original_commit_ids: vec![b1.clone()],
          },
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
        reword_commits_core(
          git,
          &RewordCommitsRequest {
            repository_path: repository_path.clone(),
            rewrites: vec![CommitRewrite {
              commit_id: a2.clone(),
              message: "(alpha) A2 reworded".to_string(),
            }],
          },
          options,
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
  let held = lock_repository(repo.git_executor(), &path(&repo), Duration::from_millis(100)).unwrap();
  let before = state(&repo);
  let options = RewriteOptions {
    lock_timeout: Duration::from_millis(100),
    ..RewriteOptions::default()
  };

  for (rewrite, error) in run_every_rewrite(&repo, &commits, &options) {
    let error = error.unwrap_or_else(|| panic!("{rewrite} ran while the lock was held"));
    // the holder is this test process, which names itself
    assert!(error.contains(&format!("(pid {}, for ", std::process::id())), "{rewrite}: {error}");
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
  let hold = |repository_path: String| {
    let held = lock_repository(repo.git_executor(), &repository_path, Duration::from_secs(1)).unwrap();
    let started = Instant::now();
    std::thread::spawn(move || {
      std::thread::sleep(Duration::from_millis(300));
      drop(held);
    });
    started
  };

  let started = hold(repository_path.clone());
  let params = SquashCommitsParams {
    repository_path: repository_path.clone(),
    commit_ids: vec![a1, a2],
    message: "(alpha) A1 and A2".to_string(),
  };
  let squashed = squash_commits_core(repo.git_executor(), &params, &RewriteOptions::default()).unwrap();
  assert!(started.elapsed() >= Duration::from_millis(300), "the squash did not wait");

  let started = hold(repository_path.clone());
  let undo = RestoreBranchHeadParams {
    repository_path,
    expected_head: squashed.new_head,
    previous_head: squashed.previous_head.clone(),
  };
  restore_branch_head_core(repo.git_executor(), &undo).unwrap();
  assert!(started.elapsed() >= Duration::from_millis(300), "the undo did not wait");
  assert_eq!(repo.head(), squashed.previous_head);
}

/// A caller that holds the lock passes it: the rewrite runs under it without a wait
#[test]
fn a_held_lock_is_not_taken_again() {
  let (repo, [a1, _, a2, _]) = repo();
  let held = lock_repository(repo.git_executor(), &path(&repo), Duration::from_millis(100)).unwrap();
  let options = RewriteOptions {
    lock_timeout: Duration::from_millis(100),
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
  let _held = lock_repository(repo.git_executor(), &path(&repo), Duration::from_millis(100)).unwrap();
  let options = RewriteOptions {
    lock_timeout: Duration::from_millis(100),
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

#[test]
fn a_lock_timeout_names_the_holder_and_its_release_deletes_the_record() {
  let dir = tempfile::tempdir().unwrap();
  let held = lock_repository_in(dir.path(), Duration::from_millis(100), &|| {}).unwrap();
  let record = dir.path().join("branch-deck.lock.holder");
  assert!(record.exists());

  let error = lock_repository_in(dir.path(), Duration::from_millis(100), &|| {}).unwrap_err();
  let locked = error.downcast_ref::<RepositoryLocked>().unwrap();
  let holder = locked.holder.as_ref().expect("the holder is named");
  assert_eq!(holder.pid, std::process::id());
  let program = std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().into_owned();
  assert!(holder.command.starts_with(&program), "{holder:?}");
  let message = error.to_string();
  assert!(message.starts_with(&format!("`{program}")), "{message}");
  assert!(message.contains(&format!("(pid {}, for ", std::process::id())), "{message}");
  assert!(message.contains(" s) holds "), "{message}");

  drop(held);
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
