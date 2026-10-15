use crate::fixture_test::{PREFIX, commit_all, environment, git, repo_with_baseline, run_in, write};
use crate::run_with;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::ffi::OsString;
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

/// Windows PowerShell 5.1 writes UTF-16 with a byte order mark (`Out-File`) or UTF-8 with one (`Set-Content -Encoding
/// UTF8`), and reads stdout with a legacy code page: bd reads both and prints only ASCII
#[test_log::test]
fn a_body_from_windows_powershell_is_read_and_printed_as_ascii() {
  let repo = repo_with_auth();
  let body = tempfile::NamedTempFile::new().unwrap();
  let utf16: Vec<u8> = [0xFF, 0xFE].into_iter().chain("Größe 🦀\r\n".encode_utf16().flat_map(u16::to_le_bytes)).collect();
  std::fs::write(body.path(), utf16).unwrap();
  let args = ["--prefix", PREFIX, "describe", "auth", "--title", "Login", "--body-file", body.path().to_str().unwrap()];
  let mut out = Vec::new();
  let exit_code = run_with(args.iter().map(OsString::from), &environment(repo.path()), &mut out);
  assert_eq!(exit_code, 0);
  assert!(out.is_ascii(), "{}", String::from_utf8_lossy(&out));
  let text = String::from_utf8(out).unwrap();
  assert!(text.contains(r#""body":"Gr\u00f6\u00dfe \ud83e\udd80""#), "{text}");
  assert_eq!(git(&repo, &["config", "branch.test-user/virtual/auth.description"]), "Login\n\nGröße 🦀");

  std::fs::write(body.path(), "\u{feff}Ünïcode\n").unwrap();
  let (_, output) = run_in(&repo, &["describe", "auth", "--body-file", body.path().to_str().unwrap()]);
  assert_eq!(output["description"], json!({ "title": "Login", "body": "Ünïcode" }));
}
