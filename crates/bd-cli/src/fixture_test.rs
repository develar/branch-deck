//! Repositories and helpers shared by the tests (named `_test` so the worktree safety scan treats it as test code)

use crate::context::Environment;
use crate::run_with;
use serde_json::Value;
use std::ffi::OsString;
use std::fmt::Write;
use std::path::Path;
use test_utils::git_test_utils::TestRepo;

pub const PREFIX: &str = "test-user";

/// No app settings, git from `PATH`, started in `cwd`
pub fn environment(cwd: &Path) -> Environment {
  Environment {
    cwd: cwd.to_path_buf(),
    settings_file: None,
    git: None,
    lock_timeout: std::time::Duration::from_millis(200),
    home: None,
    executable: None,
    appimage: None,
    path: None,
    // nothing to answer: a test of `bd safe-push` starts its own Patronus
    patronus_url: "http://127.0.0.1:9".to_string(),
    space_token: None,
    // nothing to answer and no token: a test of a GitHub pull request starts its own GitHub
    github_api_url: "http://127.0.0.1:9".to_string(),
    github_tokens: github::token::TokenSources::default(),
  }
}

/// Run `bd` and parse its single JSON line
pub fn run_json(environment: &Environment, args: &[&str]) -> (i32, Value) {
  let mut out = Vec::new();
  let exit_code = run_with(args.iter().map(OsString::from), environment, &mut out);
  let text = String::from_utf8(out).expect("UTF-8 output");
  assert_eq!(text.lines().count(), 1, "one line of JSON, got: {text}");
  let value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("invalid JSON ({e}): {text}"));
  (exit_code, value)
}

/// The output is the error `expected.1` with the exit code `expected.0`
pub fn assert_error(output: &Value, exit_code: i32, expected: (i32, &str)) {
  assert_eq!((exit_code, &output["error"]["code"]), (expected.0, &serde_json::json!(expected.1)), "{output}");
}

/// `fields` after the `schema`, `repository` and `prefix` every output of `bd` in `repo` starts with
pub fn with_header(repo: &TestRepo, fields: Value) -> Value {
  let mut output = serde_json::json!({ "schema": 1, "repository": repository(repo), "prefix": PREFIX });
  let Value::Object(fields) = fields else { panic!("fields must be an object") };
  output.as_object_mut().unwrap().extend(fields);
  output
}

/// The repository path as `bd` prints it: the top level that git names, `C:/Users/...` on Windows
pub fn repository(repo: &TestRepo) -> String {
  git(repo, &["rev-parse", "--show-toplevel"])
}

/// `path` resolved like `std::fs::canonicalize`, without the `\\?\` prefix that it gives on Windows
pub fn canonical(path: &Path) -> std::path::PathBuf {
  std::path::PathBuf::from(sync_core::resolve_copies::display_path(&std::fs::canonicalize(path).unwrap()))
}

/// The `undo_token` of `output` is `<head>..<HEAD now>`, and `bd undo` with it moves HEAD back to `head`
pub fn assert_undo_restores(repo: &TestRepo, output: &Value, head: &str) {
  let token = output["undo_token"].as_str().unwrap_or_else(|| panic!("no undo token: {output}"));
  assert_eq!(token, format!("{head}..{}", repo.head()));
  let (exit_code, undo) = run_in(repo, &["undo", token]);
  assert_eq!(exit_code, 0, "{undo}");
  assert_eq!(repo.head(), head);
}

/// `(name, status)` of every branch of the `sync` part of a rewrite output
pub fn sync_statuses(output: &Value) -> Vec<(&Value, &Value)> {
  output["sync"]["branches"]
    .as_array()
    .unwrap_or_else(|| panic!("no sync: {output}"))
    .iter()
    .map(|branch| (&branch["name"], &branch["status"]))
    .collect()
}

/// Run `bd` in `repo` with `--prefix test-user`
pub fn run_in(repo: &TestRepo, args: &[&str]) -> (i32, Value) {
  let mut all_args = vec!["--prefix", PREFIX];
  all_args.extend_from_slice(args);
  run_json(&environment(repo.path()), &all_args)
}

/// A repository whose `origin/master` is its first commit, so every later commit is ahead of the baseline
pub fn repo_with_baseline(files: &[(&str, &str)]) -> TestRepo {
  let repo = TestRepo::new();
  let initial = repo.create_commit_with_files("Initial commit", files);
  git(&repo, &["remote", "add", "origin", "."]);
  git(&repo, &["update-ref", "refs/remotes/origin/master", &initial]);
  repo
}

pub fn git(repo: &TestRepo, args: &[&str]) -> String {
  repo
    .git_executor()
    .execute_command(args, repo.path().to_str().unwrap())
    .unwrap_or_else(|e| panic!("git {args:?} failed: {e}"))
}

/// Commit everything in the working tree, with a fixed author and committer time
pub fn commit_all(repo: &TestRepo, message: &str, timestamp: i64) -> String {
  git(repo, &["add", "-A"]);
  let date = format!("{timestamp} +0000");
  repo
    .git_executor()
    .execute_command_with_env(
      &["commit", "-m", message],
      repo.path().to_str().unwrap(),
      &[("GIT_AUTHOR_DATE", &date), ("GIT_COMMITTER_DATE", &date)],
    )
    .unwrap_or_else(|e| panic!("commit {message:?} failed: {e}"));
  repo.head()
}

/// One commit per subject, each adding its own file, 100 s apart; returns their ids
pub fn commit_subjects(repo: &TestRepo, subjects: &[&str]) -> Vec<String> {
  let mut ids = Vec::new();
  for (index, subject) in subjects.iter().enumerate() {
    write(repo, &format!("f{index}.txt"), *subject);
    ids.push(commit_all(repo, subject, 1_700_000_000 + i64::try_from(index).unwrap() * 100));
  }
  ids
}

pub fn write(repo: &TestRepo, path: &str, content: impl AsRef<[u8]>) {
  let path = repo.path().join(path);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, content).unwrap();
}

/// Lines `line 1` ..= `line n`
pub fn numbered_lines(n: usize) -> String {
  (1..=n).fold(String::new(), |mut text, i| {
    let _ = writeln!(text, "line {i}");
    text
  })
}

/// The paths of `commit_dangling_reference`
pub const ACP_AGENT: &str = "plugins/air/src/acp/AcpScenarioAgent.java";
pub const ACP_PROTOCOL: &str = "plugins/air/src/acp/AcpControlProtocol.java";
pub const ACP_HOST: &str = "plugins/air/tests/flow/AirFlowLaneHost.kt";

/// The Kotlin user of the constant, qualified by `class_name`; the use is on line 7
pub fn acp_host(class_name: &str) -> String {
  format!(
    "package flow\n\nimport acp.{class_name}\n\nclass AirFlowLaneHost {{\n  fun environment(url: String): Map<String, String> {{\n    return mapOf({class_name}.CONTROL_URL_ENV to url)\n  }}\n}}\n"
  )
}

/// The Safe Push failure of 2026-10-16 in `repo` (from `repo_with_baseline`): the baseline gets a Java class with a
/// constant and a Kotlin file that uses it; `(fake-agent)` moves the constant into a new class, and `(lane-document)`
/// changes the use. Returns `(the commit that moves it, the commit that changes the use)`
pub fn commit_dangling_reference(repo: &TestRepo) -> (String, String) {
  write(
    repo,
    ACP_AGENT,
    "package acp;\n\npublic final class AcpScenarioAgent {\n  public static final String CONTROL_URL_ENV = \"AIR_CONTROL_URL\";\n}\n",
  );
  write(repo, ACP_HOST, acp_host("AcpScenarioAgent"));
  let base = commit_all(repo, "Add the scenario agent", 1_700_000_050);
  git(repo, &["update-ref", "refs/remotes/origin/master", &base]);
  write(repo, ACP_AGENT, "package acp;\n\npublic final class AcpScenarioAgent {\n}\n");
  write(
    repo,
    ACP_PROTOCOL,
    "package acp;\n\npublic final class AcpControlProtocol {\n  public static final String CONTROL_URL_ENV = \"AIR_CONTROL_URL\";\n}\n",
  );
  let removed_by = commit_all(repo, "(fake-agent) IJAI-3880 Move the control constants", 1_700_000_100);
  write(repo, ACP_HOST, acp_host("AcpControlProtocol"));
  let fixed_by = commit_all(repo, "(lane-document) IJAI-3878 Use the control protocol", 1_700_000_200);
  (removed_by, fixed_by)
}
