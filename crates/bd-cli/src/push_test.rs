use crate::fixture_test::{PREFIX, commit_all, git, repo_with_baseline, run_in, write};
use crate::schema_test::assert_covered;
use git_ops::rewrite_plan::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sync_core::push::{PushBranchParams, PushOutcome, push_branch};
use sync_core::push_lease::PUSHED_NAMESPACE;
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

/// `bd push auth` has nothing to push, and its lease is the remote head its sync saw: the pushed commit
fn assert_pushes_up_to_date_on_the_seen_head(repo: &TestRepo) {
  let (exit_code, output) = run_in(repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let auth = branch(&output, "auth");
  let tip = git(repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!(
    (&auth["status"], &auth["lease"], &auth["lease_source"]),
    (&json!("up_to_date"), &json!(short_id(&tip)), &json!("expected"))
  );
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

  // nothing new: the lease is the remote head the sync saw
  assert_pushes_up_to_date_on_the_seen_head(&repo);
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

/// The file and the content of the commit `push_from_elsewhere` makes for `subject`
fn their_change(subject: &str) -> (String, String) {
  (format!("{}.txt", subject.to_lowercase().replace(' ', "-")), format!("{subject}\n"))
}

/// Someone else commits `subject` (`their_change`) onto the remote branch `AUTH` from another clone; returns the commit
fn push_from_elsewhere(remote: &TempDir, subject: &str) -> String {
  let elsewhere = tempfile::tempdir().unwrap();
  let run = |args: &[&str]| {
    let status = std::process::Command::new("git").args(args).current_dir(elsewhere.path()).output().unwrap();
    assert!(status.status.success(), "{args:?}: {}", String::from_utf8_lossy(&status.stderr));
  };
  run(&["clone", "-q", "--branch", AUTH, remote.path().to_str().unwrap(), "."]);
  disable_background_maintenance(elsewhere.path());
  let (file, content) = their_change(subject);
  std::fs::write(elsewhere.path().join(&file), content).unwrap();
  run(&["add", &file]);
  run(&["-c", "user.name=Other", "-c", "user.email=other@example.com", "commit", "-q", "-m", subject]);
  run(&["push", "-q", "origin", AUTH]);
  remote_tip(remote, AUTH)
}

/// Someone else's fetched commit that the branch has by content (the user brought it in): the remote lacks nothing,
/// and a plain push goes through on the first try, although Branch Deck's last push is older
#[test_log::test]
fn a_plain_push_replaces_a_remote_commit_the_branch_has_by_content() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let theirs = push_from_elsewhere(&remote, "Their fix");
  git(&repo, &["fetch", "-q", "origin"]);
  let (file, content) = their_change("Their fix");
  write(&repo, &file, &content);
  commit_all(&repo, "(auth) Their fix", 1_700_000_500);

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let auth = branch(&output, "auth");
  assert_eq!((&auth["lease"], &auth["lease_source"]), (&json!(short_id(&theirs)), &json!("expected")));
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// A remote the sync doesn't compare with: the lease is the record, else the remote-tracking ref, else absent
#[test_log::test]
fn another_remote_leases_on_the_record() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  git(&repo, &["remote", "add", "mirror", remote.path().to_str().unwrap()]);
  // the baseline may be detected on either remote
  git(&repo, &["update-ref", "refs/remotes/mirror/master", "refs/remotes/origin/master"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(branch(&output, "auth")["lease_source"], json!("absent"));

  write(&repo, "more.txt", "more\n");
  commit_all(&repo, "(auth) Add logout", 1_700_000_500);
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(branch(&output, "auth")["lease_source"], json!("recorded"));
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// The subjects of the `dropped_commits` of a remote status in an output
fn dropped_subjects(remote: &Value) -> Vec<&str> {
  remote["dropped_commits"]
    .as_array()
    .unwrap()
    .iter()
    .map(|commit| commit["subject"].as_str().unwrap())
    .collect()
}

#[test_log::test]
fn a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let pushed = git(&repo, &["rev-parse", &format!("{PUSHED_NAMESPACE}{AUTH}")]);

  // someone else adds a commit to the remote branch; the remote-tracking ref doesn't know
  let other = push_from_elsewhere(&remote, "Someone else");
  write(&repo, "more.txt", "more\n");
  commit_all(&repo, "(auth) Add logout", 1_700_000_500);

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_error(&output, exit_code, (3, "lease_rejected"));
  assert_eq!(output["error"]["retryable"], json!(true));
  assert_eq!(remote_tip(&remote, AUTH), other, "the remote is unchanged");
  assert_eq!(git(&repo, &["rev-parse", &format!("{PUSHED_NAMESPACE}{AUTH}")]), pushed, "nothing is recorded");
  // bd fetched the branch and says what the remote has now
  assert_eq!(git(&repo, &["rev-parse", &format!("refs/remotes/origin/{AUTH}")]), other);
  let rejected = &output["error"]["branches"][0];
  assert_eq!(rejected["name"], json!("auth"));
  assert_eq!((&rejected["remote"]["head"], &rejected["remote"]["behind"]), (&json!(short_id(&other)), &json!(1)));
  assert_eq!(dropped_subjects(&rejected["remote"]), ["Someone else"]);
  assert_eq!(rejected["remote"]["dropped_commits"][0]["author"], json!("Other"));
  let (_, schema) = run_in(&repo, &["schema"]);
  assert_covered(&schema["outputs"]["error"], &schema["outputs"]["error"], &output, "error");

  // the remote branch has a commit the branch lacks: a push needs --force, leased on what the sync saw
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  assert_eq!(dropped_subjects(&output["error"]["branches"][0]["remote"]), ["Someone else"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--force"]);
  assert_eq!(exit_code, 0, "{output}");
  let auth = branch(&output, "auth");
  assert_eq!(
    (&auth["status"], &auth["lease"], &auth["lease_source"]),
    (&json!("forced"), &json!(short_id(&other)), &json!("expected"))
  );
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// --force replaces the remote head the sync compared with, not a commit pushed after the last fetch;
/// --expect-remote pins the lease to the head the user saw
#[test_log::test]
fn force_drops_only_what_the_sync_saw_and_expect_remote_pins_the_lease() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  push_from_elsewhere(&remote, "Seen fix");
  git(&repo, &["fetch", "-q", "origin"]);
  let newer = push_from_elsewhere(&remote, "Unseen fix");

  let (exit_code, output) = run_in(&repo, &["push", "auth", "--force"]);
  assert_error(&output, exit_code, (3, "lease_rejected"));
  assert_eq!(remote_tip(&remote, AUTH), newer, "the unseen commit is not dropped");
  let rejected = &output["error"]["branches"][0]["remote"];
  assert_eq!(dropped_subjects(rejected), ["Unseen fix", "Seen fix"]);
  let head = rejected["head"].as_str().unwrap().to_string();
  assert_eq!(head, short_id(&newer));

  let (exit_code, output) = run_in(&repo, &["push", "auth", "--force", "--expect-remote", "f00d"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["push", "auth", "other", "--expect-remote", &head]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  // the user saw the two dropped commits and approved
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--force", "--expect-remote", &head]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&branch(&output, "auth")["lease"], &branch(&output, "auth")["lease_source"]),
    (&json!(head), &json!("expected"))
  );
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// The app's push of a branch, as the push button runs it: leased on the remote head its status showed
fn app_push(repo: &TestRepo, branch: &str) {
  let (remote_tip, exit_code) = repo
    .git_executor()
    .execute_command_with_status(
      &["rev-parse", "--verify", "--quiet", &format!("refs/remotes/origin/{PREFIX}/virtual/{branch}")],
      repo.path().to_str().unwrap(),
    )
    .unwrap();
  let params = PushBranchParams {
    repository_path: repo.path().to_str().unwrap().to_string(),
    branch_prefix: PREFIX.to_string(),
    branch_name: branch.to_string(),
    my_email: None,
    baseline_branch: "origin/master".to_string(),
    parent_branch: None,
    expected_remote: (exit_code == 0).then_some(remote_tip),
  };
  assert!(matches!(push_branch(repo.git_executor(), &params, &mut |_| {}).unwrap(), PushOutcome::Pushed { .. }));
}

#[test_log::test]
fn the_app_and_bd_share_the_lease() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");

  // the app pushes, then bd: the app's push moved the remote head bd's sync sees
  app_push(&repo, "auth");
  let tip = git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!(git(&repo, &["rev-parse", &format!("{PUSHED_NAMESPACE}{AUTH}")]), tip);
  assert_pushes_up_to_date_on_the_seen_head(&repo);
  write(&repo, "more.txt", "more\n");
  commit_all(&repo, "(auth) Add logout", 1_700_000_500);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(branch(&output, "auth")["status"], json!("updated"));

  // bd pushed, then the app
  write(&repo, "even-more.txt", "more\n");
  commit_all(&repo, "(auth) Add reset", 1_700_000_600);
  run_in(&repo, &["sync"]);
  app_push(&repo, "auth");
  let tip = git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!(remote_tip(&remote, AUTH), tip);
  assert_eq!(git(&repo, &["rev-parse", &format!("{PUSHED_NAMESPACE}{AUTH}")]), tip);
  // bd again: the app's push moved the remote head, so this is not a stale lease
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(branch(&output, "auth")["status"], json!("up_to_date"));
}
