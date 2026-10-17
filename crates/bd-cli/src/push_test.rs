use crate::fixture_test::{PREFIX, assert_error, commit_all, commit_subjects, git, repo_with_baseline, run_in, write};
use crate::schema_test::assert_covered;
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sync_core::push::{PushBranchParams, PushOutcome, push_branch};
use tempfile::TempDir;
use test_utils::git_test_utils::{TestRepo, disable_background_maintenance, git_command};

const AUTH: &str = "test-user/virtual/auth";

/// A repository whose `origin` is a local bare repository, with the commits of `subjects`
pub(crate) fn repo_with_remote(subjects: &[&str]) -> (TestRepo, TempDir) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let remote = tempfile::tempdir().unwrap();
  git(&repo, &["init", "--bare", "-q", remote.path().to_str().unwrap()]);
  disable_background_maintenance(remote.path());
  git(&repo, &["remote", "set-url", "origin", remote.path().to_str().unwrap()]);
  commit_subjects(&repo, subjects);
  (repo, remote)
}

fn remote_tip(remote: &TempDir, branch: &str) -> String {
  let output = git_command()
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
  assert_pushed(&output, exit_code, "up_to_date", &git(repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// `bd push` pushed `auth` with `status`, leased on the remote head `lease` it saw
fn assert_pushed(output: &Value, exit_code: i32, status: &str, lease: &str) {
  assert_eq!(exit_code, 0, "{output}");
  let auth = branch(output, "auth");
  assert_eq!(
    (&auth["status"], &auth["lease"], &auth["lease_source"]),
    (&json!(status), &json!(short_id(lease)), &json!("expected"))
  );
}

#[test_log::test]
fn pushes_with_a_lease_and_prints_the_pull_request() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  run_in(&repo, &["describe", "auth", "--title", "Add login", "--body", "The login form."]);

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!(remote_tip(&remote, AUTH), tip);
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
        "command": format!("gh pr create --base master --head {AUTH} --assignee @me --title 'Add login' --body-file -"),
      },
    })
  );

  let (_, schema) = run_in(&repo, &["schema"]);
  assert_covered(&schema["outputs"]["push"], &schema["outputs"]["push"], &output, "push");

  // nothing new: the lease is the remote head the sync saw
  assert_pushes_up_to_date_on_the_seen_head(&repo);
}

/// A GitHub repository that `origin` doesn't push to (`branchdeck.githubRepository`, a mirror) is named with `--repo`:
/// `gh` finds it among no remote. The sync read the setting; the push URL of `origin` tells the mirror apart
#[test_log::test]
fn the_pull_request_command_names_a_github_mirror() {
  let (repo, _remote) = repo_with_remote(&["(auth) Add login"]);
  repo.set_config("branchdeck.githubRepository", "acme/widgets").unwrap();
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    branch(&output, "auth")["pull_request"]["command"],
    json!(format!("gh pr create --repo acme/widgets --base master --head {AUTH} --assignee @me --fill"))
  );
}

#[test_log::test]
fn parents_go_first_and_a_child_alone_needs_its_parent_on_the_remote() {
  let (repo, remote) = repo_with_remote(&["(auth) Add auth", "(auth/login) Add login"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();

  let (exit_code, output) = run_in(&repo, &["push", "login", "--alone"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  assert!(output["error"]["message"].as_str().unwrap().contains("parent auth is not on the remote"), "{output}");
  assert_eq!(remote_tip(&remote, "test-user/virtual/login"), "");
  let (exit_code, output) = run_in(&repo, &["push", "--alone"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  // the branch it is built on goes along, first
  let (exit_code, output) = run_in(&repo, &["push", "login"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("auth"), &json!("login")]);
  assert_eq!(branch(&output, "login")["pull_request"]["base"], json!(AUTH));
  assert_eq!(
    branch(&output, "login")["pull_request"]["command"],
    json!(format!("gh pr create --base {AUTH} --head test-user/virtual/login --assignee @me --fill"))
  );
}

#[test_log::test]
fn a_parent_goes_along_only_with_commits_for_the_remote_and_brings_no_children() {
  let (repo, _remote) = repo_with_remote(&["(auth) Add auth", "(auth/session) Add session", "(auth/login) Add login"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");

  // auth is on the remote as it is: login goes alone; session, a child of auth, is not taken along by --with-children of login
  let (exit_code, output) = run_in(&repo, &["push", "login", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("login")]);
}

#[test_log::test]
fn with_children_pushes_the_branches_stacked_on_the_named_ones() {
  let (repo, remote) = repo_with_remote(&["(auth) Add auth", "(auth/login) Add login", "(login/oauth) Add oauth", "(docs) Add docs"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();

  let (exit_code, output) = run_in(&repo, &["push", "--with-children"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children", "--expect-remote", "HEAD"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  // the grandchild too, parents first; docs is not stacked on auth
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("auth"), &json!("login"), &json!("oauth")]);
  assert_eq!(remote_tip(&remote, "test-user/virtual/docs"), "");
  assert_eq!(branch(&output, "oauth")["pull_request"]["base"], json!("test-user/virtual/login"));

  // a new commit of login rebuilds oauth on it: the push replaces oauth's remote copy
  write(&repo, "form.txt", "form\n");
  commit_all(&repo, "(auth/login) Add the login form", 1_700_000_900);
  let (exit_code, output) = run_in(&repo, &["push", "login", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  let statuses: Vec<(&Value, &Value)> = output["branches"].as_array().unwrap().iter().map(|branch| (&branch["name"], &branch["status"])).collect();
  assert_eq!(statuses, [(&json!("login"), &json!("updated")), (&json!("oauth"), &json!("forced"))]);
}

#[test_log::test]
fn with_children_refuses_a_blocked_child() {
  let (repo, remote) = repo_with_remote(&["(docs) Add docs", "(auth) Add auth"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  // login changes a file only docs has: it conflicts on auth, and oauth on it is blocked
  write(&repo, "f0.txt", "changed\n");
  commit_all(&repo, "(auth/login) Change docs", 1_700_000_900);
  write(&repo, "oauth.txt", "oauth\n");
  commit_all(&repo, "(login/oauth) Add oauth", 1_700_001_000);

  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("login is not synced") && message.contains("oauth is not synced"), "{output}");
  assert_eq!(remote_tip(&remote, AUTH), "");
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
  push_from_elsewhere_to(remote, AUTH, subject)
}

/// `push_from_elsewhere` onto the remote branch `branch`
fn push_from_elsewhere_to(remote: &TempDir, branch: &str, subject: &str) -> String {
  let elsewhere = tempfile::tempdir().unwrap();
  let run = |args: &[&str]| {
    let status = git_command().args(args).current_dir(elsewhere.path()).output().unwrap();
    assert!(status.status.success(), "{args:?}: {}", String::from_utf8_lossy(&status.stderr));
  };
  run(&["clone", "-q", "--branch", branch, remote.path().to_str().unwrap(), "."]);
  disable_background_maintenance(elsewhere.path());
  let (file, content) = their_change(subject);
  std::fs::write(elsewhere.path().join(&file), content).unwrap();
  run(&["add", &file]);
  run(&["-c", "user.name=Other", "-c", "user.email=other@example.com", "commit", "-q", "-m", subject]);
  run(&["push", "-q", "origin", branch]);
  remote_tip(remote, branch)
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

  // someone else adds a commit to the remote branch; the remote-tracking ref doesn't know
  let other = push_from_elsewhere(&remote, "Someone else");
  write(&repo, "more.txt", "more\n");
  commit_all(&repo, "(auth) Add logout", 1_700_000_500);

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_error(&output, exit_code, (3, "lease_rejected"));
  assert_eq!(output["error"]["retryable"], json!(true));
  assert_eq!(remote_tip(&remote, AUTH), other, "the remote is unchanged");
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
  assert_pushed(&output, exit_code, "forced", &other);
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// A remote other than `origin`, which the sync compares with: bd compares with it the same way, so the lease, the
/// refusal of a branch behind it and the status of a rejected push are about that remote
#[test_log::test]
fn another_remote_is_compared_with_as_origin_is() {
  let (repo, origin) = repo_with_remote(&["(auth) Add login"]);
  let mirror = tempfile::tempdir().unwrap();
  git(&repo, &["init", "--bare", "-q", mirror.path().to_str().unwrap()]);
  disable_background_maintenance(mirror.path());
  git(&repo, &["remote", "add", "mirror", mirror.path().to_str().unwrap()]);
  // the baseline may be detected on either remote
  git(&repo, &["update-ref", "refs/remotes/mirror/master", "refs/remotes/origin/master"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "nowhere"]);
  assert_error(&output, exit_code, (4, "no_remote"));

  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!((&output["remote"], &branch(&output, "auth")["lease_source"]), (&json!("mirror"), &json!("absent")));
  assert_eq!((remote_tip(&mirror, AUTH), remote_tip(&origin, AUTH)), (tip.clone(), String::new()));
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror"]);
  assert_pushed(&output, exit_code, "up_to_date", &tip);

  // someone else pushes to the mirror: rejected, fetched and reported
  let other = push_from_elsewhere(&mirror, "Someone else");
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror"]);
  assert_error(&output, exit_code, (3, "lease_rejected"));
  let rejected = &output["error"]["branches"][0]["remote"];
  assert_eq!((&rejected["head"], &rejected["behind"]), (&json!(short_id(&other)), &json!(1)));
  assert_eq!(dropped_subjects(rejected), ["Someone else"]);

  // fetched now: the branch is behind the mirror, and replacing it needs the user's approval
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  assert_eq!(dropped_subjects(&output["error"]["branches"][0]["remote"]), ["Someone else"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror", "--force", "--expect-remote", &other]);
  assert_pushed(&output, exit_code, "forced", &other);
  assert_eq!(remote_tip(&mirror, AUTH), tip);
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
    tip: None,
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
  // bd again: the app's push moved the remote head, so this is not a stale lease
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(branch(&output, "auth")["status"], json!("up_to_date"));
}

/// A commit amended at the root of a stack: the root's remote branch has its older version (amended), the remote
/// branches built on it have it as the commit of an ancestor (restacked). Neither is someone's work a push drops: `bd
/// push` of the leaf pushes the stack without --force, also the restacked middle, which has no commit of its own to push
#[test_log::test]
fn amended_and_restacked_commits_push_without_force() {
  let subjects = ["(auth) Add auth", "(auth/login) Add login", "(login/oauth) Add oauth"];
  let (repo, remote) = repo_with_remote(&subjects);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let (exit_code, output) = run_in(&repo, &["push", "oauth"]);
  assert_eq!(exit_code, 0, "{output}");

  // the auth commit amended with its author time and subject; the others as they were
  git(&repo, &["reset", "--hard", "HEAD~3"]);
  write(&repo, "f0.txt", "(auth) Add auth, amended");
  commit_all(&repo, subjects[0], 1_700_000_000);
  for (index, subject) in subjects.iter().enumerate().skip(1) {
    write(&repo, &format!("f{index}.txt"), *subject);
    commit_all(&repo, subject, 1_700_000_000 + i64::try_from(index).unwrap() * 100);
  }

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  let auth = &branch(&output, "auth")["remote"];
  assert_eq!((&auth["unpushed"], &auth["amended"], &auth["behind"]), (&json!(1), &json!(1), &json!(0)));
  for name in ["login", "oauth"] {
    let remote = &branch(&output, name)["remote"];
    assert_eq!((&remote["unpushed"], &remote["restacked"], &remote["behind"]), (&json!(0), &json!(1), &json!(0)), "{name}");
  }
  assert!(
    auth.get("restacked").is_none() && branch(&output, "login")["remote"].get("amended").is_none(),
    "a zero count is left out: {output}"
  );
  let (_, schema) = run_in(&repo, &["schema"]);
  assert_covered(&schema["outputs"]["sync"], &schema["outputs"]["sync"], &output, "sync");
  // their next step: a push without --force, also for the restacked branches with no commit of their own
  for name in ["auth", "login", "oauth"] {
    assert_eq!(branch(&output, name)["push"], json!(format!("bd push {name}")));
  }

  let (exit_code, output) = run_in(&repo, &["push", "oauth"]);
  assert_eq!(exit_code, 0, "{output}");
  let statuses: Vec<(&Value, &Value)> = output["branches"].as_array().unwrap().iter().map(|branch| (&branch["name"], &branch["status"])).collect();
  assert_eq!(
    statuses,
    [(&json!("auth"), &json!("forced")), (&json!("login"), &json!("forced")), (&json!("oauth"), &json!("forced"))]
  );
  assert_eq!(
    remote_tip(&remote, "test-user/virtual/login"),
    git(&repo, &["rev-parse", "refs/heads/test-user/virtual/login"])
  );
}

/// `bd push` with the arguments of a `push` line of the sync output
fn run_push_line(repo: &TestRepo, line: &str) -> (i32, Value) {
  let args: Vec<&str> = line.strip_prefix("bd ").unwrap_or_else(|| panic!("{line}")).split(' ').collect();
  run_in(repo, &args)
}

/// The sync names the push each branch needs: `--force --expect-remote` for a branch whose push drops a commit someone
/// else pushed, `--force` alone when a parent it takes along has commits for the remote, and none when it's up to date
#[test_log::test]
fn sync_names_the_push_of_each_branch() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (_, output) = run_in(&repo, &["sync"]);
  assert_eq!(branch(&output, "auth")["push"], json!("bd push auth"));
  run_in(&repo, &["push", "auth"]);
  let (_, output) = run_in(&repo, &["sync"]);
  assert_eq!(branch(&output, "auth").get("push"), None, "up to date: {output}");

  let theirs = push_from_elsewhere(&remote, "Their fix");
  git(&repo, &["fetch", "-q", "origin"]);
  let (_, output) = run_in(&repo, &["sync"]);
  let line = format!("bd push auth --force --expect-remote {}", short_id(&theirs));
  assert_eq!(branch(&output, "auth")["push"], json!(line));
  let (exit_code, output) = run_push_line(&repo, &line);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

#[test_log::test]
fn a_child_behind_its_remote_with_a_parent_to_push_forces_without_expect_remote() {
  let (repo, remote) = repo_with_remote(&["(auth) Add auth", "(auth/login) Add login"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  run_in(&repo, &["push", "login"]);
  push_from_elsewhere_to(&remote, "test-user/virtual/login", "Their fix");
  git(&repo, &["fetch", "-q", "origin"]);
  write(&repo, "auth2.txt", "more auth\n");
  commit_all(&repo, "(auth) More auth", 1_700_000_900);

  let (_, output) = run_in(&repo, &["sync"]);
  assert_eq!(branch(&output, "auth")["push"], json!("bd push auth"));
  let line = branch(&output, "login")["push"].as_str().unwrap().to_string();
  assert_eq!(line, "bd push login --force");
  let (exit_code, output) = run_push_line(&repo, &line);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    remote_tip(&remote, "test-user/virtual/login"),
    git(&repo, &["rev-parse", "refs/heads/test-user/virtual/login"])
  );
}
