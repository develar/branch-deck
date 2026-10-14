use crate::fixture_test::{commit_all, git, repo_with_baseline, run_in, write};
use git_ops::rewrite_plan::short_id;
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
      "remote": { "exists": false, "unpushed": 1, "behind": 0 },
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
