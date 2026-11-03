use bd_cli::fixture_test::{assert_covered, assert_error, environment, git, repo_with_subjects, run_in, run_json};
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_utils::git_test_utils::TestRepo;

/// `(docs) Add f0`, `(other) Add f1`, `(docs) Add f2` ahead of `origin/master`, each adding its file `f<index>.txt`
fn repo() -> (TestRepo, Vec<String>) {
  repo_with_subjects(&[("README.md", "readme\n")], &["(docs) Add f0", "(other) Add f1", "(docs) Add f2"])
}

fn subjects(repo: &TestRepo) -> String {
  git(repo, &["log", "--format=%s", "origin/master..HEAD"])
}

#[test_log::test]
fn drop_takes_the_commit_and_its_file_out_of_head_and_the_undo_brings_them_back() {
  let (repo, commits) = repo();
  let head = repo.head();

  let (exit_code, output) = run_in(&repo, &["drop", short_id(&commits[0]), "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["dry_run"], &output["new_head"], output.get("undo_token")),
    (&json!(true), &json!(short_id(&head)), None),
    "{output}"
  );
  assert_eq!(
    output["dropped"],
    json!([{ "commit": short_id(&commits[0]), "subject": "(docs) Add f0", "branch": "docs" }])
  );
  assert_eq!((&output["files_changed"], &output["rebuilt_branches"]), (&json!(true), &json!(["docs", "other"])));
  assert_eq!(repo.head(), head);

  let (exit_code, output) = run_in(&repo, &["--full-ids", "drop", short_id(&commits[0]), "--expect-head", short_id(&head)]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo), "(docs) Add f2\n(other) Add f1");
  assert!(!repo.path().join("f0.txt").exists());
  assert_eq!(git(&repo, &["status", "--porcelain"]), "");
  let new_head = repo.head();
  assert_eq!(output["undo_token"], json!(format!("drop:{head}..{new_head}")));
  assert_eq!(git(&repo, &["log", "-g", "-1", "--format=%gs", "HEAD"]), "branch-deck: drop");

  let token = output["undo_token"].as_str().unwrap();
  let (exit_code, undo) = run_in(&repo, &["--full-ids", "undo", token]);
  assert_eq!((exit_code, &undo["already_undone"]), (0, &json!(false)), "{undo}");
  assert_eq!((&undo["new_head"], &undo["previous_head"]), (&json!(head), &json!(new_head)));
  assert_eq!(repo.head(), head);
  assert!(repo.path().join("f0.txt").exists());
  let (exit_code, again) = run_in(&repo, &["undo", token]);
  assert_eq!((exit_code, &again["already_undone"]), (0, &json!(true)), "{again}");
}

/// A commit and its revert: HEAD's files stay, and the branch left without a commit is named
#[test_log::test]
fn drop_of_a_commit_and_its_revert_moves_no_file() {
  let (repo, commits) = repo();
  git(&repo, &["revert", "--no-edit", &commits[2]]);
  let revert = repo.head();
  let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);

  let (exit_code, output) = run_in(&repo, &["drop", short_id(&revert), short_id(&commits[2])]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(subjects(&repo), "(other) Add f1\n(docs) Add f0");
  assert_eq!(git(&repo, &["rev-parse", "HEAD^{tree}"]), tree);
  assert_eq!((&output["files_changed"], &output["rebuilt_branches"]), (&json!(false), &json!(["docs"])), "{output}");
  // the revert groups nowhere: its subject has no prefix
  assert_eq!(output["dropped"][1]["branch"], json!(null), "{output}");
}

#[test_log::test]
fn drop_refuses_what_is_not_ahead_a_dependent_later_commit_local_changes_and_another_head() {
  let (repo, commits) = repo();
  let head = repo.head();
  let (exit_code, output) = run_in(&repo, &["drop", "0000000"]);
  assert_error(&output, exit_code, (2, "not_ahead_of_baseline"));
  assert_eq!(output["error"]["commits"], json!(["0000000"]));

  let (exit_code, output) = run_in(&repo, &["drop", short_id(&commits[0]), "--expect-head", short_id(&commits[1])]);
  assert_error(&output, exit_code, (3, "head_moved"));

  std::fs::write(repo.path().join("f0.txt"), "in the way").unwrap();
  let (exit_code, output) = run_in(&repo, &["drop", short_id(&commits[0])]);
  assert_error(&output, exit_code, (4, "uncommitted_overlap"));
  assert_eq!(output["error"]["paths"], json!(["f0.txt"]));
  git(&repo, &["checkout", "-q", "--", "f0.txt"]);

  // a later commit edits the file of the dropped one
  std::fs::write(repo.path().join("f0.txt"), "edited").unwrap();
  git(&repo, &["commit", "-q", "-am", "(other) Edit f0"]);
  let edited = repo.head();
  let (exit_code, output) = run_in(&repo, &["drop", short_id(&commits[0])]);
  assert_error(&output, exit_code, (4, "drop_conflict"));
  assert_eq!(repo.head(), edited);
  assert_ne!(edited, head);
}

#[test_log::test]
fn drop_outputs_match_the_schema() {
  let (repo, commits) = repo();
  let (_, schema) = run_json(&environment(repo.path()), &["schema", "--all"]);
  let (_, output) = run_in(&repo, &["drop", short_id(&commits[2])]);
  assert_covered(&schema["outputs"]["drop"], &schema["outputs"]["drop"], &output, "drop");
  let (_, undo) = run_in(&repo, &["undo", output["undo_token"].as_str().unwrap()]);
  assert_covered(&schema["outputs"]["undo"], &schema["outputs"]["undo"], &undo, "undo of a drop");
}
