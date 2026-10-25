use bd_cli::fixture_test::{assert_covered, assert_error, commit_subjects, environment, git, repo_with_baseline, run_in, run_json};
use pretty_assertions::assert_eq;
use serde_json::json;
use test_utils::git_test_utils::TestRepo;

/// `(docs) Add f0`, `(other) Add f1`, `(docs) Add f2` ahead of `origin/master`, synced by `bd sync`
fn synced_repo() -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let commits = commit_subjects(&repo, &["(docs) Add f0", "(other) Add f1", "(docs) Add f2"]);
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  (repo, commits)
}

#[test_log::test]
fn unapply_takes_the_branch_and_its_files_out_of_head_and_the_undo_brings_them_back() {
  let (repo, commits) = synced_repo();
  let head = repo.head();
  let tip = git(&repo, &["rev-parse", "test-user/virtual/docs"]);

  let (exit_code, output) = run_in(&repo, &["--full-ids", "unapply", "docs"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["branch"], &output["unapplied_branch"], &output["commits"], &output["previous_head"]),
    (&json!("docs"), &json!("test-user/unapplied/docs"), &json!([commits[0], commits[2]]), &json!(head))
  );
  let new_head = repo.head();
  assert_eq!(output["new_head"], json!(new_head));
  assert_eq!(
    output["undo_token"],
    json!(format!("unapply:{head}..{new_head}:{tip}:test-user/virtual/docs:test-user/unapplied/docs"))
  );
  assert!(!repo.path().join("f0.txt").exists());
  assert_eq!(git(&repo, &["status", "--porcelain"]), "");
  assert_eq!(git(&repo, &["rev-parse", "test-user/unapplied/docs"]), tip);

  let token = output["undo_token"].as_str().unwrap();
  let undo = run_in(&repo, &["--full-ids", "undo", token]).1;
  let undone = [&undo["new_head"], &undo["previous_head"], &undo["restored_branch"], &undo["already_undone"]];
  assert_eq!(undone, [&json!(head), &json!(new_head), &json!("test-user/virtual/docs"), &json!(false)], "{undo}");
  assert_eq!(repo.head(), head);
  assert!(repo.path().join("f0.txt").exists());
  assert_eq!(git(&repo, &["rev-parse", "test-user/virtual/docs"]), tip);
  let (exit_code, again) = run_in(&repo, &["undo", token]);
  assert_eq!((exit_code, &again["already_undone"]), (0, &json!(true)), "{again}");
}

#[test_log::test]
fn unapply_refuses_an_unknown_branch_one_not_built_and_local_changes_in_the_way() {
  let (repo, _) = synced_repo();
  let (exit_code, output) = run_in(&repo, &["unapply", "ghost"]);
  assert_error(&output, exit_code, (2, "unknown_branch"));

  std::fs::write(repo.path().join("f2.txt"), "in the way").unwrap();
  let (exit_code, output) = run_in(&repo, &["unapply", "docs"]);
  assert_error(&output, exit_code, (4, "uncommitted_overlap"));
  assert_eq!(output["error"]["paths"], json!(["f2.txt"]));
  std::fs::remove_file(repo.path().join("f2.txt")).unwrap();
  git(&repo, &["checkout", "-q", "--", "f2.txt"]);

  commit_subjects(&repo, &["(docs) Add f3"]);
  let (exit_code, output) = run_in(&repo, &["unapply", "docs"]);
  assert_error(&output, exit_code, (4, "branch_not_built"));

  // a later commit depends on a commit of the branch
  run_in(&repo, &["sync"]);
  std::fs::write(repo.path().join("f0.txt"), "edited").unwrap();
  git(&repo, &["commit", "-q", "-am", "(other) Edit f0"]);
  run_in(&repo, &["sync"]);
  let (exit_code, output) = run_in(&repo, &["unapply", "docs"]);
  assert_error(&output, exit_code, (4, "drop_conflict"));
}

#[test_log::test]
fn unapply_outputs_match_the_schema() {
  let (repo, _) = synced_repo();
  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
  let (_, output) = run_in(&repo, &["unapply", "docs"]);
  assert_covered(&schema["outputs"]["unapply"], &schema["outputs"]["unapply"], &output, "unapply");
  let (_, undo) = run_in(&repo, &["undo", output["undo_token"].as_str().unwrap()]);
  assert_covered(&schema["outputs"]["undo"], &schema["outputs"]["undo"], &undo, "undo of an unapply");
}

#[test_log::test]
fn status_lists_an_unapplied_branch_push_refuses_it_by_its_name_and_pushes_it_with_unapplied() {
  let (repo, _) = synced_repo();
  run_in(&repo, &["unapply", "docs"]);
  let tip = git(&repo, &["rev-parse", "--short=8", "test-user/unapplied/docs"]);

  let (_, status) = run_in(&repo, &["status"]);
  // the subject of the copy, without the grouping prefix
  assert_eq!(
    status["unapplied"],
    json!([{"name": "test-user/unapplied/docs", "branch": "docs", "tip": tip, "subject": "Add f2"}]),
    "{status}"
  );
  let (exit_code, output) = run_in(&repo, &["push", "docs"]);
  assert_error(&output, exit_code, (4, "unapplied"));
  let (exit_code, output) = run_in(&repo, &["safe-push", "docs", "--check"]);
  assert_error(&output, exit_code, (4, "unapplied"));

  let (exit_code, pushed) = run_in(&repo, &["push", "--unapplied", "docs"]);
  assert_eq!(exit_code, 0, "{pushed}");
  assert_eq!(
    (
      &pushed["branches"][0]["remote_branch"],
      &pushed["branches"][0]["state"],
      &pushed["branches"][0]["pull_request"]
    ),
    (&json!("test-user/unapplied/docs"), &json!("new"), &serde_json::Value::Null)
  );
  let (_, status) = run_in(&repo, &["status"]);
  assert_eq!(status["unapplied"][0]["remote"], json!({"head": tip, "up_to_date": true}), "{status}");
  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
  assert_covered(&schema["outputs"]["push"], &schema["outputs"]["push"], &pushed, "push --unapplied");
  assert_covered(&schema["outputs"]["status"], &schema["outputs"]["status"], &status, "status with an unapplied branch");
}

#[test_log::test]
fn reapply_puts_the_branch_back_with_its_prefix_and_the_undo_takes_it_out_again() {
  let (repo, _) = synced_repo();
  run_in(&repo, &["unapply", "docs"]);
  let head = repo.head();

  let (exit_code, dry_run) = run_in(&repo, &["reapply", "docs", "--dry-run"]);
  assert_eq!(
    (exit_code, &dry_run["dry_run"], &dry_run["undo_token"]),
    (0, &json!(true), &serde_json::Value::Null),
    "{dry_run}"
  );
  assert_eq!(repo.head(), head);

  let (exit_code, output) = run_in(&repo, &["--full-ids", "reapply", "docs"]);
  assert_eq!(exit_code, 0, "{output}");
  let subjects: Vec<&serde_json::Value> = output["reapplied"].as_array().unwrap().iter().map(|commit| &commit["subject"]).collect();
  assert_eq!(subjects, [&json!("(docs) Add f0"), &json!("(docs) Add f2")]);
  assert_eq!(
    (&output["branch"], &output["prefix_restored"], &output["previous_head"]),
    (&json!("docs"), &json!(2), &json!(head))
  );
  assert!(repo.path().join("f0.txt").exists());
  assert_eq!(git(&repo, &["branch", "--list", "test-user/unapplied/docs"]), "");

  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
  assert_covered(&schema["outputs"]["reapply"], &schema["outputs"]["reapply"], &output, "reapply");
  let (exit_code, undo) = run_in(&repo, &["undo", output["undo_token"].as_str().unwrap()]);
  assert_eq!((exit_code, &undo["restored_branch"]), (0, &json!("test-user/unapplied/docs")), "{undo}");
  assert_eq!(repo.head(), head);

  // HEAD writes a file of the branch in another way: the reapply conflicts and changes nothing
  std::fs::write(repo.path().join("f2.txt"), "theirs").unwrap();
  git(&repo, &["add", "f2.txt"]);
  git(&repo, &["commit", "-q", "-m", "(other) Own f2"]);
  let (exit_code, output) = run_in(&repo, &["reapply", "docs"]);
  assert_error(&output, exit_code, (4, "reapply_conflict"));
  assert_eq!(output["error"]["paths"], json!(["f2.txt"]));
  let (exit_code, output) = run_in(&repo, &["reapply", "ghost"]);
  assert_error(&output, exit_code, (2, "unknown_branch"));
}
