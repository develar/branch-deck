#[cfg(unix)]
use crate::fixture_test::{PREFIX, beside_a_held_sync};
use crate::fixture_test::{commit_all, git, hold_the_lock, repo_with_commits, repo_with_login, run_in, write};
#[cfg(unix)]
use crate::remote_fixture_test::{remote_tip, repo_with_remote};
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::json;
#[cfg(unix)]
use test_utils::git_test_utils::TestRepo;
use test_utils::history::HistoryCommit;

#[test_log::test]
fn syncs_and_reports_each_branch() {
  let (repo, _) = repo_with_commits(
    &[("README.md", "readme\n")],
    &[
      HistoryCommit::new("(auth) Add login", &[("auth.rs", "auth\n")]).at(1_700_000_100),
      HistoryCommit::new("Tidy up", &[("misc.txt", "misc\n")]).at(1_700_000_200),
    ],
  );

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]);
  assert_eq!(output["baseline"], json!("origin/master"));
  assert_eq!(output["unassigned_count"], json!(1));
  assert_eq!(
    output["branches"],
    json!([{
      "name": "auth",
      "ref": "refs/heads/test-user/virtual/auth",
      "state": "created",
      "commit_count": 1,
      "tip": short_id(&tip),
      "remote": { "exists": false, "unpushed": 1, "behind": 0, "head": null },
      "push": "bd push auth",
    }])
  );

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["branches"][0]["state"], json!("unchanged"));
  assert_eq!(output["branches"][0]["tip"], json!(short_id(&tip)));
}

#[test_log::test]
fn a_conflict_exits_with_1() {
  let (repo, [_, conflicting, blocked]) = repo_with_commits(
    &[("README.md", "readme\n")],
    &[
      // `b` changes a file `a` creates: on the baseline alone it conflicts
      HistoryCommit::new("(a) Add shared", &[("shared.txt", "one\n")]).at(1_700_000_100),
      HistoryCommit::new("(b) Change shared", &[("shared.txt", "two\n")]).at(1_700_000_200),
      // not synced after the conflict
      HistoryCommit::new("(b) Add other", &[("other.txt", "other\n")]).at(1_700_000_300),
    ],
  );

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 1, "{output}");
  let b = output["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("b")).unwrap();
  assert_eq!(b["state"], json!("conflict"));
  assert_eq!(b["tip"], json!(null));
  assert_eq!(b["conflict"]["commit"], json!(short_id(&conflicting)));
  assert_eq!(b["conflict"]["subject"], json!("(b) Change shared"));
  assert_eq!(b["conflict"]["files"], json!(["shared.txt"]));
  assert_eq!(b["blocked"], json!([short_id(&blocked)]));
  let a = output["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("a")).unwrap();
  assert_eq!(a["state"], json!("created"));
}

/// A sync moves its refs by compare-and-swap, as the app's does: it never waits for a rewrite or Update Project that
/// holds the repository lock
#[test_log::test]
fn syncs_while_another_process_holds_the_lock() {
  let repo = repo_with_login();
  let _held = hold_the_lock(&repo);
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!((exit_code, &output["branches"][0]["state"]), (0, &json!("created")), "{output}");
}

/// A repository with a commit of `auth` after one of no branch (so the branch's commit is merged onto the baseline), and
/// the id of the branch's commit
#[cfg(unix)]
fn repo_with_auth() -> (TestRepo, String) {
  let (repo, [_, login]) = repo_with_commits(
    &[("README.md", "readme\n")],
    &[
      HistoryCommit::new("Tidy up", &[("misc.txt", "misc\n")]).at(1_700_000_050),
      HistoryCommit::new("(auth) Add login", &[("auth.rs", "auth\n")]).at(1_700_000_100),
    ],
  );
  (repo, login)
}

/// An agent's `bd sync`, held in its merges, beside the user's regroup (the app's Move takes the same lock): the regroup
/// is done within the bounded wait of the lock (200 ms in the fixture), and the sync then ends
#[cfg(unix)]
#[test_log::test]
fn a_rewrite_beside_a_running_sync_does_not_wait_for_it() {
  let (repo, _) = repo_with_auth();
  beside_a_held_sync(&repo, || {
    let (exit_code, renamed) = run_in(&repo, &["rename", "auth", "login"]);
    assert_eq!(exit_code, 0, "the rewrite does not wait for the sync: {renamed}");
  });
  assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "(login) Add login");
}

/// The app's waiter of a rewrite (`LockWait::WhileHeld`) that stops at the first holder it is told of: a rewrite that
/// would wait for anyone fails with `LockWaitCanceled` instead of hanging
#[cfg(unix)]
#[derive(Default)]
struct NeverWaits(std::sync::atomic::AtomicBool);

#[cfg(unix)]
impl history_rewrite::repository_lock::LockWaiter for NeverWaits {
  fn holder(&self, holder: Option<&history_rewrite::repository_lock::LockHolder>) {
    if holder.is_some() {
      self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
  }

  fn canceled(&self) -> bool {
    self.0.load(std::sync::atomic::Ordering::SeqCst)
  }
}

/// An agent's `bd sync`, held in its merges, beside the app's squash (`squash_commits_core` with the app's wait for the
/// repository lock, as long as its holder lives): the sync holds no lock, so the squash never waits and is done while
/// the sync is still held; the sync then ends
#[cfg(unix)]
#[test_log::test]
fn the_apps_squash_beside_a_held_sync_never_waits_for_it() {
  use history_rewrite::repository_lock::{LockWait, RewriteOptions};
  use history_rewrite::squash::{SquashCommitsParams, squash_commits_core};

  let (repo, login) = repo_with_auth();
  write(&repo, "auth.rs", "auth 2\n");
  let fix = commit_all(&repo, "(auth) Fix login", 1_700_000_200);
  beside_a_held_sync(&repo, || {
    let waiter = NeverWaits::default();
    let params = SquashCommitsParams {
      repository_path: repo.path().to_str().unwrap().to_owned(),
      commit_ids: vec![login.clone(), fix.clone()],
      message: "(auth) Add login".to_owned(),
    };
    let squashed = squash_commits_core(repo.git_executor(), &params, &RewriteOptions::waiting(LockWait::WhileHeld(&waiter)));
    assert!(squashed.is_ok(), "the squash waits for no holder while the sync is held: {squashed:?}");
  });
  assert_eq!(git(&repo, &["log", "-2", "--format=%s"]), "(auth) Add login\nTidy up");
}

/// The virtual branch of auth in `repo`
#[cfg(unix)]
fn auth_tip(repo: &TestRepo) -> String {
  git(repo, &["rev-parse", &format!("refs/heads/{PREFIX}/virtual/auth")])
}

/// A failure beside a held sync fails the test at once: the sync goes on as the panic leaves `beside_a_held_sync`, and
/// ends with its build, so no test waits until nextest kills it for a sync nobody lets go
#[cfg(unix)]
#[test_log::test]
fn a_failure_beside_a_held_sync_lets_the_sync_go_and_end() {
  let (repo, _) = repo_with_auth();
  let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    beside_a_held_sync(&repo, || panic!("an assertion beside the held sync failed"));
  }));
  assert!(failed.is_err(), "the failure beside the sync is the test's");
  assert!(!auth_tip(&repo).is_empty(), "the sync went on and built auth");
}

/// Two agents' `bd sync` at once, the first held in its merges: the second builds auth and ends; the first then ends too,
/// on the same build, which its compare-and-swap leaves as the second wrote it
#[cfg(unix)]
#[test_log::test]
fn a_sync_beside_a_held_sync_ends_and_both_leave_one_build() {
  let (repo, _) = repo_with_auth();
  let mut built = String::new();
  beside_a_held_sync(&repo, || {
    let (exit_code, output) = run_in(&repo, &["sync"]);
    assert_eq!((exit_code, &output["branches"][0]["state"]), (0, &json!("created")), "{output}");
    built = auth_tip(&repo);
  });
  assert_eq!(auth_tip(&repo), built);
}

/// An agent's `bd push`, beside another agent's `bd sync` held in its merges: the sync holds no lock, so the push syncs,
/// pushes and ends while that sync is still held; the remote has the build both syncs leave
#[cfg(unix)]
#[test_log::test]
fn a_push_beside_a_held_sync_does_not_wait_for_it() {
  // a commit before it, so each sync merges the branch's commit onto the baseline
  let (repo, remote) = repo_with_remote(&["Tidy up", "(auth) Add login"]);
  beside_a_held_sync(&repo, || {
    let (exit_code, output) = run_in(&repo, &["push", "auth"]);
    assert_eq!(exit_code, 0, "the push does not wait for the sync: {output}");
  });
  assert_eq!(remote_tip(&remote, &format!("{PREFIX}/virtual/auth")), auth_tip(&repo));
}

#[test_log::test]
fn waits_while_a_rebase_is_in_progress() {
  let repo = repo_with_login();
  std::fs::create_dir_all(repo.path().join(".git/rebase-merge")).unwrap();
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!((exit_code, &output["error"]["code"]), (4, &json!("operation_in_progress")), "{output}");
  assert_eq!(output["error"]["message"], json!("Not synced: a rebase is in progress"));
}

#[test_log::test]
fn reports_the_commits_the_baseline_already_has() {
  let (repo, [login, _]) = repo_with_commits(
    &[("README.md", "readme\n")],
    &[
      HistoryCommit::new("(auth) Add login", &[("auth.rs", "auth\n")]).at(1_700_000_100),
      HistoryCommit::new("(ui) Add button", &[("ui.rs", "ui\n")]).at(1_700_000_200),
    ],
  );
  // a robot rebased the commit onto the baseline without its prefix, keeping the author time
  git(&repo, &["switch", "--quiet", "--create", "robot", "refs/remotes/origin/master"]);
  write(&repo, "auth.rs", "auth\n");
  let landed = commit_all(&repo, "Add login", 1_700_000_100);
  git(&repo, &["update-ref", "refs/remotes/origin/master", &landed]);
  git(&repo, &["switch", "--quiet", "master"]);

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    output["update"]["landing"],
    json!([{
      "branch": "auth",
      "commits": [{ "commit": short_id(&login), "subject": "Add login", "upstream": short_id(&landed) }],
      "remaining": 0,
    }])
  );
  assert_eq!(output["landed"], json!(null), "nothing landed with an update yet");
}

/// The note of the virtual branch commit, carried by `git notes copy` to a commit that a hunk was moved into: the sync
/// copies the commit again instead of keeping the copy without the hunk
#[test_log::test]
fn a_stale_note_rebuilds_the_branch() {
  let repo = repo_with_login();
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]);

  let old = repo.head();
  write(&repo, "auth.rs", "auth\nmoved hunk\n");
  git(&repo, &["add", "-A"]);
  git(&repo, &["commit", "--amend", "--no-edit"]);
  git(&repo, &["notes", "copy", &old, "HEAD"]);

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["branches"][0]["state"], json!("updated"));
  assert_ne!(git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]), tip);
  assert_eq!(git(&repo, &["show", "refs/heads/test-user/virtual/auth:auth.rs"]), "auth\nmoved hunk");
}

/// `--no-cache` trusts no mapping note: a copy with another tree, behind a note that looks right, is copied again
#[test_log::test]
fn no_cache_verifies_each_note() {
  let repo = repo_with_login();
  run_in(&repo, &["sync"]);
  let copy = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]);

  // a copy of the same message and author on the same base, with the tree of the baseline
  let fields = git(&repo, &["log", "-1", "--format=%an%x1f%ae%x1f%ad%x1f%T%x1f%P", "--date=raw", &format!("{copy}~1")]);
  let base: Vec<&str> = fields.split('\u{1f}').collect();
  let author = git(&repo, &["log", "-1", "--format=%an%x1f%ae%x1f%ad", "--date=raw", &copy]);
  let author: Vec<&str> = author.split('\u{1f}').collect();
  let env = [("GIT_AUTHOR_NAME", author[0]), ("GIT_AUTHOR_EMAIL", author[1]), ("GIT_AUTHOR_DATE", author[2])];
  let base_commit = git(&repo, &["rev-parse", &format!("{copy}~1")]);
  let wrong = repo
    .git_executor()
    .cmd(&["commit-tree", base[3], "-p", &base_commit, "-m", "Add login"], repo.path().to_str().unwrap())
    .env(&env)
    .text()
    .unwrap();
  git(&repo, &["update-ref", "refs/heads/test-user/virtual/auth", &wrong]);
  let note = git(&repo, &["notes", "show", "HEAD"]);
  git(&repo, &["notes", "add", "-f", "-m", &note.replace(&copy, &wrong), "HEAD"]);

  let (_, output) = run_in(&repo, &["sync"]);
  assert_eq!(output["branches"][0]["state"], json!("unchanged"), "{output}");
  let (exit_code, output) = run_in(&repo, &["--no-cache", "sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["branches"][0]["state"], json!("updated"));
  assert_eq!(git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]), copy);

  // an unchanged branch stays unchanged with --no-cache
  let (_, output) = run_in(&repo, &["--no-cache", "sync"]);
  assert_eq!(output["branches"][0]["state"], json!("unchanged"));
}

/// `bd sync` exits with 1 for every branch it could not build, not only a conflict
#[test]
fn a_branch_that_is_not_built_is_exit_1() {
  use bd_core::sync_command::SyncState;
  let built: Vec<bool> = [
    SyncState::Created,
    SyncState::Updated,
    SyncState::Unchanged,
    SyncState::Error,
    SyncState::Conflict,
    SyncState::Blocked,
  ]
  .into_iter()
  .map(SyncState::is_built)
  .collect();
  assert_eq!(built, [true, true, true, false, false, false]);
}

/// `b` changes the third argument of the call whose first one `a` changed: git stops, but the words resolve. The sync
/// builds `b` and says what it resolved; `bd status` and `bd commits` tell it from the mapping note; `bd check` agrees
#[test_log::test]
fn a_conflict_that_its_words_resolve_builds_and_says_so() {
  let (repo, [_, resolved]) = repo_with_commits(
    &[("Call.kt", "call(a, b, c)\nend\n")],
    &[
      HistoryCommit::new("(a) Change the first argument", &[("Call.kt", "call(A, b, c)\nend\n")]).at(1_700_000_100),
      HistoryCommit::new("(b) Change the third argument", &[("Call.kt", "call(A, b, C)\nend\n")]).at(1_700_000_200),
    ],
  );

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  let b = output["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("b")).unwrap();
  assert_eq!(b["state"], json!("created"));
  assert_eq!(
    b["resolved"],
    json!([{ "commit": short_id(&resolved), "conflicts": 1, "file_count": 1, "files": [{ "path": "Call.kt", "conflicts": 1 }] }])
  );
  assert_eq!(git(&repo, &["show", "refs/heads/test-user/virtual/b:Call.kt"]), "call(a, b, C)\nend");

  let (exit_code, status) = run_in(&repo, &["status"]);
  assert_eq!(exit_code, 0, "{status}");
  let b = status["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("b")).unwrap();
  assert_eq!(b["commits"][0]["resolved_by_words"], json!({ "conflicts": 1, "file_count": 1 }));
  let (_, commits) = run_in(&repo, &["commits", "--branch", "b"]);
  assert_eq!(commits["commits"][0]["resolved_by_words"], json!({ "conflicts": 1, "file_count": 1 }), "{commits}");

  let (exit_code, check) = run_in(&repo, &["check"]);
  assert_eq!(exit_code, 0, "{check}");
  let b = check["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("b")).unwrap();
  assert_eq!(b["result"], json!("ok"), "{check}");
}
