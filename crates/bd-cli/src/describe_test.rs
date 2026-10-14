use crate::fixture_test::{commit_all, git, repo_with_baseline, run_in, write};
use pretty_assertions::assert_eq;
use serde_json::json;
use test_utils::git_test_utils::TestRepo;

/// One branch, `auth`
fn repo_with_auth() -> TestRepo {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "auth.rs", "auth\n");
  commit_all(&repo, "(auth) Add login", 1_700_000_100);
  repo
}

#[test_log::test]
fn sets_prints_and_clears_a_description() {
  let repo = repo_with_auth();

  let (exit_code, output) = run_in(&repo, &["describe", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["description"], &output["changed"], &output["ref"]),
    (&json!(null), &json!(false), &json!("refs/heads/test-user/virtual/auth"))
  );

  let (exit_code, output) = run_in(&repo, &["describe", "auth", "--title", "Add login", "--body", "Adds the login form.\n\nWith tests."]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["description"], json!({ "title": "Add login", "body": "Adds the login form.\n\nWith tests." }));
  assert_eq!(output["changed"], json!(true));
  // git's own format, as `git branch --edit-description` writes it
  assert_eq!(
    git(&repo, &["config", "branch.test-user/virtual/auth.description"]),
    "Add login\n\nAdds the login form.\n\nWith tests."
  );

  // a new title keeps the body; status shows the title
  let (_, output) = run_in(&repo, &["describe", "auth", "--title", "Login form"]);
  assert_eq!(output["description"]["body"], json!("Adds the login form.\n\nWith tests."));
  let (_, status) = run_in(&repo, &["status"]);
  assert_eq!(status["branches"][0]["description"], json!({ "title": "Login form" }));

  let body = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(body.path(), "From a file\n").unwrap();
  let (_, output) = run_in(&repo, &["describe", "auth", "--body-file", body.path().to_str().unwrap()]);
  assert_eq!(output["description"], json!({ "title": "Login form", "body": "From a file" }));

  let (exit_code, output) = run_in(&repo, &["describe", "auth", "--clear"]);
  assert_eq!((exit_code, &output["description"], &output["changed"]), (0, &json!(null), &json!(true)));
  let (_, status) = run_in(&repo, &["status"]);
  assert_eq!(status["branches"][0].get("description"), None);
}

#[test_log::test]
fn describe_errors() {
  let repo = repo_with_auth();

  let (exit_code, output) = run_in(&repo, &["describe", "missing", "--title", "X"]);
  assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("unknown_branch")));
  // a body without a title
  let (exit_code, output) = run_in(&repo, &["describe", "auth", "--body", "Only a body"]);
  assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("invalid_arguments")));
  let (exit_code, output) = run_in(&repo, &["describe", "auth", "--title", "X", "--clear"]);
  assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("invalid_arguments")));
}
