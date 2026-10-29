use bd_cli::fixture_test::{commit_all, commit_subjects, git, repo_with_baseline, run_in, write};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test_log::test]
fn marks_a_virtual_branch_reviewed_and_again_after_a_new_commit() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commit_subjects(&repo, &["(auth) Add login"]);
  assert_eq!(run_in(&repo, &["sync"]).0, 0);
  let virtual_tip = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]);
  let base = git(&repo, &["rev-parse", "origin/master"]);

  let (exit_code, output) = run_in(&repo, &["--full-ids", "mark-reviewed", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["branch"], &output["ref"], &output["base"], &output["tip"]),
    (
      &json!("test-user/virtual/auth"),
      &json!("refs/heads/test-user/virtual/auth"),
      &json!(base),
      &json!(virtual_tip)
    )
  );
  assert_eq!((&output["previous"], &output["changed"], &output["pruned"]), (&json!(null), &json!(true), &json!(0)));
  let refs = git(&repo, &["for-each-ref", "--format=%(objectname)", "refs/branch-deck/reviewed/"]);
  assert_eq!(refs.lines().collect::<Vec<_>>(), [base.as_str(), virtual_tip.as_str()]);

  // the same tip again changes nothing
  let (exit_code, output) = run_in(&repo, &["--full-ids", "mark-reviewed", "auth"]);
  assert_eq!((exit_code, &output["changed"]), (0, &json!(false)), "{output}");

  // a new commit on the branch: the mark moves to the new tip and names the one it replaced
  write(&repo, "auth.rs", "auth 2\n");
  commit_all(&repo, "(auth) Fix login", 1_700_000_200);
  run_in(&repo, &["sync"]);
  let new_tip = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]);
  let (exit_code, output) = run_in(&repo, &["--full-ids", "mark-reviewed", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["tip"], &output["previous"], &output["changed"]),
    (&json!(new_tip), &json!({ "base": base, "tip": virtual_tip }), &json!(true))
  );
}

#[test_log::test]
fn a_stacked_branch_is_marked_on_its_parent() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  commit_subjects(&repo, &["(auth) Add login", "(auth/ui) Add form"]);
  assert_eq!(run_in(&repo, &["sync"]).0, 0);
  let parent_tip = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/auth"]);
  let tip = git(&repo, &["rev-parse", "refs/heads/test-user/virtual/ui"]);

  let (exit_code, output) = run_in(&repo, &["--full-ids", "mark-reviewed", "ui", "--base", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["base"], &output["tip"]), (&json!(parent_tip), &json!(tip)));
}

/// A `--base` that names no commit, or one with no history in common with the branch, is the caller's error and writes
/// no ref
#[test_log::test]
fn a_base_that_is_no_commit_or_shares_no_history_is_refused() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commit_subjects(&repo, &["(auth) Add login"]);
  assert_eq!(run_in(&repo, &["sync"]).0, 0);
  let orphan = git(&repo, &["commit-tree", "4b825dc642cb6eb9a060e54bf8d69288fbee4904", "-m", "Unrelated root"]);
  for base in ["nope", "--all", orphan.as_str()] {
    let (exit_code, output) = run_in(&repo, &["mark-reviewed", "auth", &format!("--base={base}")]);
    assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("invalid_arguments")), "{base}: {output}");
  }
  assert_eq!(git(&repo, &["for-each-ref", "refs/branch-deck/reviewed/"]), "");
}

#[test_log::test]
fn an_unknown_branch_is_refused() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let (exit_code, output) = run_in(&repo, &["mark-reviewed", "missing"]);
  assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("unknown_branch")), "{output}");
  assert_eq!(git(&repo, &["for-each-ref", "refs/branch-deck/reviewed/"]), "");
}
