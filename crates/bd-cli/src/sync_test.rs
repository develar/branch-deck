use crate::fixture_test::{commit_all, git, repo_with_baseline, run_in, write};
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::json;
use sync_core::repository_lock::{DEFAULT_LOCK_TIMEOUT, lock_repository};

#[test_log::test]
fn syncs_and_reports_each_branch() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "auth.rs", "auth\n");
  commit_all(&repo, "(auth) Add login", 1_700_000_100);
  write(&repo, "misc.txt", "misc\n");
  commit_all(&repo, "Tidy up", 1_700_000_200);

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
      "status": "created",
      "commit_count": 1,
      "tip": short_id(&tip),
      "remote": { "exists": false, "unpushed": 1, "behind": 0, "head": null },
      "push": "bd push auth",
    }])
  );

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["branches"][0]["status"], json!("unchanged"));
  assert_eq!(output["branches"][0]["tip"], json!(short_id(&tip)));
}

#[test_log::test]
fn a_conflict_exits_with_1() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  // `b` changes a file `a` creates: on the baseline alone it conflicts
  write(&repo, "shared.txt", "one\n");
  commit_all(&repo, "(a) Add shared", 1_700_000_100);
  write(&repo, "shared.txt", "two\n");
  let conflicting = commit_all(&repo, "(b) Change shared", 1_700_000_200);

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 1, "{output}");
  let b = output["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("b")).unwrap();
  assert_eq!(b["status"], json!("merge_conflict"));
  assert_eq!(b["tip"], json!(null));
  assert_eq!(b["conflict"]["commit"], json!(short_id(&conflicting)));
  assert_eq!(b["conflict"]["subject"], json!("(b) Change shared"));
  assert_eq!(b["conflict"]["files"], json!(["shared.txt"]));
  let a = output["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("a")).unwrap();
  assert_eq!(a["status"], json!("created"));
}

#[test_log::test]
fn waits_for_the_lock() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let _held = lock_repository(repo.git_executor(), repo.path().to_str().unwrap(), DEFAULT_LOCK_TIMEOUT).unwrap();
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!((exit_code, &output["error"]["code"]), (3, &json!("locked")));
}

#[test_log::test]
fn waits_while_a_rebase_is_in_progress() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "auth.rs", "auth\n");
  commit_all(&repo, "(auth) Add login", 1_700_000_100);
  std::fs::create_dir_all(repo.path().join(".git/rebase-merge")).unwrap();
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!((exit_code, &output["error"]["code"]), (4, &json!("operation_in_progress")), "{output}");
  assert_eq!(output["error"]["message"], json!("Not synced: a rebase is in progress"));
}

#[test_log::test]
fn reports_the_commits_the_baseline_already_has() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "auth.rs", "auth\n");
  let login = commit_all(&repo, "(auth) Add login", 1_700_000_100);
  write(&repo, "ui.rs", "ui\n");
  commit_all(&repo, "(ui) Add button", 1_700_000_200);
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
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "auth.rs", "auth\n");
  commit_all(&repo, "(auth) Add login", 1_700_000_100);
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
  assert_eq!(output["branches"][0]["status"], json!("updated"));
  assert_ne!(git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]), tip);
  assert_eq!(git(&repo, &["show", "refs/heads/test-user/virtual/auth:auth.rs"]), "auth\nmoved hunk");
}

/// `--no-cache` trusts no mapping note: a copy with another tree, behind a note that looks right, is copied again
#[test_log::test]
fn no_cache_verifies_each_note() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "auth.rs", "auth\n");
  commit_all(&repo, "(auth) Add login", 1_700_000_100);
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
  assert_eq!(output["branches"][0]["status"], json!("unchanged"), "{output}");
  let (exit_code, output) = run_in(&repo, &["--no-cache", "sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["branches"][0]["status"], json!("updated"));
  assert_eq!(git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]), copy);

  // an unchanged branch stays unchanged with --no-cache
  let (_, output) = run_in(&repo, &["--no-cache", "sync"]);
  assert_eq!(output["branches"][0]["status"], json!("unchanged"));
}
