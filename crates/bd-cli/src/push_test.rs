use crate::fixture_test::{commit_all, git, repo_with_baseline, run_in, write};
use crate::push_command::{PUSHED_NAMESPACE, parse_porcelain};
use crate::schema_test::assert_covered;
use git_ops::rewrite_plan::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::HashMap;
use tempfile::TempDir;
use test_utils::git_test_utils::{TestRepo, disable_background_maintenance};

const AUTH: &str = "test-user/virtual/auth";

/// A repository whose `origin` is a local bare repository, with the commits of `subjects`
fn repo_with_remote(subjects: &[&str]) -> (TestRepo, TempDir) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let remote = tempfile::tempdir().unwrap();
  git(&repo, &["init", "--bare", "-q", remote.path().to_str().unwrap()]);
  disable_background_maintenance(remote.path());
  git(&repo, &["remote", "set-url", "origin", remote.path().to_str().unwrap()]);
  for (index, subject) in subjects.iter().enumerate() {
    write(&repo, &format!("f{index}.txt"), *subject);
    commit_all(&repo, subject, 1_700_000_000 + i64::try_from(index).unwrap() * 100);
  }
  (repo, remote)
}

fn remote_tip(remote: &TempDir, branch: &str) -> String {
  let output = std::process::Command::new("git")
    .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
    .current_dir(remote.path())
    .output()
    .unwrap();
  String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn branch<'a>(output: &'a Value, name: &str) -> &'a Value {
  output["branches"]
    .as_array()
    .unwrap()
    .iter()
    .find(|branch| branch["name"] == json!(name))
    .unwrap_or_else(|| panic!("no {name} in {output}"))
}

fn assert_error(output: &Value, exit_code: i32, expected: (i32, &str)) {
  assert_eq!((exit_code, &output["error"]["code"]), (expected.0, &json!(expected.1)), "{output}");
}

#[test_log::test]
fn pushes_with_a_lease_and_prints_the_pull_request() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  run_in(&repo, &["describe", "auth", "--title", "Add login", "--body", "The login form."]);

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!(remote_tip(&remote, AUTH), tip);
  assert_eq!(git(&repo, &["rev-parse", &format!("{PUSHED_NAMESPACE}{AUTH}")]), tip, "the push is recorded");
  assert_eq!(output["remote"], json!("origin"));
  assert_eq!(
    branch(&output, "auth"),
    &json!({
      "name": "auth",
      "remote_branch": AUTH,
      "commit": short_id(&tip),
      "status": "new",
      "lease": null,
      "lease_source": "absent",
      "pull_request": {
        "base": "master",
        "head": AUTH,
        "title": "Add login",
        "body": "The login form.",
        "command": format!("gh pr create --base master --head {AUTH} --title 'Add login' --body-file -"),
      },
    })
  );

  let (_, schema) = run_in(&repo, &["schema"]);
  assert_covered(&schema["outputs"]["push"], &schema["outputs"]["push"], &output, "push");

  // nothing new: the lease is the recorded push
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&branch(&output, "auth")["status"], &branch(&output, "auth")["lease_source"]),
    (&json!("up_to_date"), &json!("recorded"))
  );
}

#[test_log::test]
fn parents_go_first_and_a_child_needs_its_parent_on_the_remote() {
  let (repo, remote) = repo_with_remote(&["(auth) Add auth", "(auth/login) Add login"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();

  let (exit_code, output) = run_in(&repo, &["push", "login"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  assert!(output["error"]["message"].as_str().unwrap().contains("parent auth is not on the remote"), "{output}");
  assert_eq!(remote_tip(&remote, "test-user/virtual/login"), "");

  let (exit_code, output) = run_in(&repo, &["push", "login", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("auth"), &json!("login")]);
  assert_eq!(branch(&output, "login")["pull_request"]["base"], json!(AUTH));
  assert_eq!(
    branch(&output, "login")["pull_request"]["command"],
    json!(format!("gh pr create --base {AUTH} --head test-user/virtual/login --fill"))
  );
}

#[test_log::test]
fn all_needs_yes_and_unsynced_branches_are_refused() {
  let (repo, _remote) = repo_with_remote(&["(a) Add shared", "(b) Unrelated"]);
  let (exit_code, output) = run_in(&repo, &["push", "--all"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["push"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["push", "--all", "--yes"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["branches"].as_array().unwrap().len(), 2);

  // c changes a file only a has: it conflicts on the baseline and is not pushed
  write(&repo, "f0.txt", "changed\n");
  commit_all(&repo, "(c) Change shared", 1_700_000_900);
  let (exit_code, output) = run_in(&repo, &["push", "c"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  assert!(output["error"]["message"].as_str().unwrap().contains("c is not synced"), "{output}");
}

#[test_log::test]
fn a_moved_remote_branch_rejects_the_lease_until_fetched_and_forced() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let pushed = git(&repo, &["rev-parse", &format!("{PUSHED_NAMESPACE}{AUTH}")]);

  // someone else adds a commit to the remote branch from another clone; the remote-tracking ref doesn't know
  let elsewhere = tempfile::tempdir().unwrap();
  let run = |args: &[&str]| {
    let status = std::process::Command::new("git").args(args).current_dir(elsewhere.path()).output().unwrap();
    assert!(status.status.success(), "{args:?}: {}", String::from_utf8_lossy(&status.stderr));
  };
  run(&["clone", "-q", "--branch", AUTH, remote.path().to_str().unwrap(), "."]);
  disable_background_maintenance(elsewhere.path());
  run(&[
    "-c",
    "user.name=Other",
    "-c",
    "user.email=other@example.com",
    "commit",
    "-q",
    "--allow-empty",
    "-m",
    "Someone else",
  ]);
  run(&["push", "-q", "origin", AUTH]);
  let other = remote_tip(&remote, AUTH);
  write(&repo, "more.txt", "more\n");
  commit_all(&repo, "(auth) Add logout", 1_700_000_500);

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_error(&output, exit_code, (3, "lease_rejected"));
  assert_eq!(output["error"]["retryable"], json!(true));
  assert_eq!(remote_tip(&remote, AUTH), other, "the remote is unchanged");
  assert_eq!(git(&repo, &["rev-parse", &format!("{PUSHED_NAMESPACE}{AUTH}")]), pushed, "nothing is recorded");

  // fetched, the remote branch is ahead: a push needs --force, leased on what was fetched
  git(&repo, &["fetch", "-q", "origin"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--force"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&branch(&output, "auth")["status"], &branch(&output, "auth")["lease_source"]),
    (&json!("forced"), &json!("tracking"))
  );
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

#[test]
fn parses_porcelain_lines() {
  let output = "To /tmp/remote\n*\trefs/heads/a:refs/heads/a\t[new branch]\n+\trefs/heads/b:refs/heads/b\t1111...2222 (forced update)\n!\trefs/heads/c:refs/heads/c\t[rejected] (stale info)\nDone";
  assert_eq!(
    parse_porcelain(output),
    HashMap::from([("refs/heads/a".to_string(), '*'), ("refs/heads/b".to_string(), '+'), ("refs/heads/c".to_string(), '!')])
  );
}
