use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{ACP_HOST, PREFIX, assert_error, commit_all, commit_dangling_reference, environment, git, run_in, run_json, write};
use bd_cli::remote_fixture_test::{
  AUTH, LANDED_SUBJECTS, STACK, Space, evaluated_review, fake_github, forking_stack, land_auth_by_content, list_pull_requests, pull_request_json, push_from_elsewhere_to,
  reviewed_on_github, robot, safe_push, search_answer, stack_robot,
};
use bd_core::context::Environment;
use github::fake_github::{FakeAnswer, FakeGitHub};
use github::token::TokenSources;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// `bd push` with `args`
fn bd_push(environment: &Environment, args: &[&str]) -> (i32, Value) {
  let mut all_args = vec!["--prefix", PREFIX, "push"];
  all_args.extend_from_slice(args);
  run_json(environment, &all_args)
}

#[test_log::test]
fn check_names_the_profiles_and_the_review_and_starts_nothing() {
  let space = Space::new(&["(auth) Add auth"]);
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let (_, schema) = run_json(&environment(space.repo.path()), &["schema"]);
  assert_covered(&schema["outputs"]["safe_push"], &schema["outputs"]["safe_push"], &output, "safe_push");
  assert_eq!(output["mode"], json!("safe_push"));
  assert_eq!(output["target"], json!("master"));
  // a root lands alone: no trial build of it alone
  assert_eq!((output["lands"].clone(), output.get("alone")), (json!(["auth"]), None));
  // the emergency profile is never offered; no robot of the branch yet, so no default
  assert_eq!(
    output["profiles"],
    json!([{"id": "default", "name": "Default"}, {"id": "lsc", "name": "Large Scale Change", "description": "All the tests"}])
  );
  assert_eq!(output["profile"], Value::Null);
  assert_eq!(
    output["review"],
    json!({"required": false, "skipped": false, "counts_in": "space_merge_request", "merge_request": null, "groups": [], "readiness": "not_required"})
  );
  assert_eq!((output["push_needed"].clone(), output["robot"].clone()), (json!(true), Value::Null));
  assert_eq!(space.fake.start_calls.load(Ordering::SeqCst), 0);
  assert_eq!(space.remote_tip(AUTH), "");
}

#[test_log::test]
fn missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway() {
  let space = Space::new(&["(auth) Add auth"]);
  space.fake.review_required.store(true, Ordering::SeqCst);

  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "approvals_missing"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("Needs review by Air Plugin Release Team (suggested: Alice)"), "{message}");
  // without a GitHub repository nothing can collect them
  assert!(message.contains("git config branchdeck.githubRepository"), "{message}");
  assert!(message.contains("bd safe-push auth --create-pull-request"), "{message}");
  assert_eq!(output["error"]["review"]["groups"][0]["reviewers"][0]["username"], json!("alice"));
  assert_eq!((space.fake.start_calls.load(Ordering::SeqCst), space.remote_tip(AUTH)), (0, String::new()));

  let (exit_code, output) = space.run(&["auth", "--create-pull-request"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  // --check reports it instead of failing
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["refusals"][0]["code"], json!("approvals_missing"));

  let (exit_code, output) = space.run(&["auth", "--without-approvals"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["started_via"], json!("api"));
  assert_eq!(output["robot"]["id"], json!("started"));
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH));
  let start = space.last_start();
  assert_eq!(start["sourceBranch"], json!(format!("refs/heads/{AUTH}")));
  assert_eq!(start["mode"], json!("REBASE_AUTOSQUASH"));
  assert_eq!(start.get("spaceReview"), None, "{start}");
}

#[test_log::test]
fn a_dry_run_takes_a_profile_by_name_and_checks_no_review() {
  let space = Space::new(&["(auth) Add auth"]);
  space.fake.review_required.store(true, Ordering::SeqCst);

  let (exit_code, output) = space.run(&["auth", "--patronus-dry-run", "--profile", "large scale change"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["mode"].clone(), output["review"].clone()), (json!("dry_run"), Value::Null));
  assert_eq!(output["profile"]["id"], json!("lsc"));
  let start = space.last_start();
  assert_eq!((start["mode"].clone(), start["mergeProfileId"].clone()), (json!("DRY_RUN"), json!("lsc")));
  assert_eq!(space.fake.check_calls.load(Ordering::SeqCst), 0);

  let (exit_code, output) = space.run(&["auth", "--patronus-dry-run", "--profile", "nightly"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(output["error"]["message"].as_str().unwrap().contains("lsc (Large Scale Change)"), "{output}");

  // an emergency profile skips all checks: only the app offers it, after a warning
  let (exit_code, output) = space.run(&["auth", "--patronus-dry-run", "--profile", "emergency"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
}

#[test_log::test]
fn the_profile_of_the_last_robot_is_the_default_and_none_drops_it() {
  let space = Space::new(&["(auth) Add auth"]);
  // its details name the Large Scale Change profile
  space.set_robots(vec![robot("older", "FAILURE", "auth", "0000")]);

  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["profile"]["id"], json!("lsc"));

  let (exit_code, output) = space.run(&["auth", "--profile", "none"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["profile"], Value::Null);
  assert_eq!(space.last_start()["mergeProfileId"], Value::Null);
}

/// A Safe Push of auth runs on `revision`: `bd safe-push auth` refuses to start another one on the same commits
fn assert_refused_for_a_robot_on(space: &Space, revision: &str) {
  space.set_robots(vec![robot("running", "RUNNING", "auth", revision)]);
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert_eq!(output["error"]["message"], json!("A Safe Push of the last commit is running"));
}

#[test_log::test]
fn a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told() {
  let space = Space::new(&["(auth) Add auth"]);
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = space.local_tip(AUTH);

  assert_refused_for_a_robot_on(&space, &tip);

  // a fix since: the push would fail the robot of the older commit
  space.set_robots(vec![robot("running", "RUNNING", "auth", "0000")]);
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert!(output["error"]["message"].as_str().unwrap().contains("--cancel-running"), "{output}");
  assert_eq!(space.remote_tip(AUTH), "");

  let (exit_code, output) = space.run(&["auth", "--cancel-running"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["canceled"]["id"], json!("running"));
  assert_eq!(*space.fake.canceled.lock().unwrap(), vec!["running".to_string()]);
  assert_eq!(output["robot"]["id"], json!("started"));
}

/// Pushes auth (`bd push`), then moves the baseline on and the user's branch onto it, so the next sync rebuilds auth on
/// it: the remote branch has its commits by content, under other hashes. Returns the pushed commit.
fn push_and_rebuild_on_a_newer_baseline(space: &Space, environment: &Environment) -> String {
  let (exit_code, output) = run_json(environment, &["--prefix", PREFIX, "push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let pushed = space.remote_tip(AUTH);
  let baseline = git(&space.repo, &["rev-parse", "refs/remotes/origin/master"]);
  let upstream = git(&space.repo, &["commit-tree", &format!("{baseline}^{{tree}}"), "-p", &baseline, "-m", "Upstream change"]);
  git(&space.repo, &["update-ref", "refs/remotes/origin/master", &upstream]);
  git(&space.repo, &["rebase", "-q", &upstream]);

  let (exit_code, output) = safe_push(environment, &["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["push_needed"], json!(false));
  assert_ne!(space.local_tip(AUTH), pushed, "the sync rebuilt auth on the newer baseline");
  pushed
}

/// `bd safe-push auth` after a failed robot on `pushed`: it starts a new one
fn start_auth_after_a_failed_robot(space: &Space, pushed: &str) -> Value {
  space.set_robots(vec![robot("older", "FAILURE", "auth", pushed)]);
  let (exit_code, output) = space.run(&["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["robot"]["id"], json!("started"));
  output
}

#[test_log::test]
fn a_branch_only_rebuilt_on_a_newer_baseline_starts_from_its_remote_branch_without_a_push() {
  let space = Space::new(&["(auth) Add auth"]);
  let pushed = push_and_rebuild_on_a_newer_baseline(&space, &space.environment());

  // a robot on the remote branch runs on the same commits: a start would repeat it
  assert_refused_for_a_robot_on(&space, &pushed);

  let output = start_auth_after_a_failed_robot(&space, &pushed);
  assert_eq!((output["push_needed"].clone(), output.get("pushed")), (json!(false), None));
  assert_eq!(space.remote_tip(AUTH), pushed, "nothing was pushed");
  assert_eq!(space.last_start()["sourceBranch"], json!(format!("refs/heads/{AUTH}")));

  // a Safe Push of an older commit runs: the start replaces it, as the form does, once told to cancel it
  space.set_robots(vec![robot("running", "RUNNING", "auth", "0000")]);
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("the start would run beside it. Pass --cancel-running"), "{output}");
  // a dry run runs beside it
  let (exit_code, output) = space.run(&["auth", "--patronus-dry-run", "--check"]);
  assert_eq!((exit_code, output.get("refusals")), (0, None), "{output}");
  let (exit_code, output) = space.run(&["auth", "--cancel-running"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["canceled"]["id"].clone(), output.get("pushed")), (json!("running"), None));
  assert_eq!(*space.fake.canceled.lock().unwrap(), vec!["running".to_string()]);
  assert_eq!(output["robot"]["id"], json!("started"));
  assert_eq!(space.remote_tip(AUTH), pushed, "nothing was pushed");
}

/// `bd safe-push auth --check` of a branch whose remote branch is not as it is: a push is due, nothing refuses the
/// start, and the output matches the schema; returns the remote state of auth in `remotes`
fn check_auth_needs_a_push(space: &Space) -> Value {
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!((exit_code, output.get("refusals")), (0, None), "{output}");
  assert_eq!(output["push_needed"], json!(true));
  assert_eq!(output["remotes"][0]["name"], json!("auth"));
  let (_, schema) = run_json(&space.environment(), &["schema"]);
  assert_covered(&schema["outputs"]["safe_push"], &schema["outputs"]["safe_push"], &output, "safe_push");
  output["remotes"][0]["remote"].clone()
}

/// `check_auth_needs_a_push` of a branch whose one commit is on the remote by content in its old message
fn check_auth_is_reworded(space: &Space) {
  let remote = check_auth_needs_a_push(space);
  assert_eq!((&remote["unpushed"], &remote["reworded"], &remote["behind"]), (&json!(0), &json!(1), &json!(0)));
}

/// A reword after the push keeps the changes, so the remote has the commit by content, but with the old message: a
/// robot on the remote branch would land it. The check says a push is due (`remote.reworded`), and the start pushes
/// before it starts, as it does after an amend
#[test_log::test]
fn a_branch_reworded_since_its_push_is_pushed_before_the_start() {
  let space = Space::new(&["(auth) Add auth"]);
  let pushed = push_and_rebuild_on_a_newer_baseline(&space, &space.environment());
  git(&space.repo, &["commit", "--amend", "-q", "-m", "(auth) Add the auth module"]);

  check_auth_is_reworded(&space);

  let output = start_auth_after_a_failed_robot(&space, &pushed);
  assert_eq!(output["pushed"][0]["name"], json!("auth"));
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH), "the reworded commit is on the remote");
}

/// The same through `bd reword --sync`: the check counts the reworded commit as a push (`remote.reworded`), and
/// `bd push --dry-run` would replace the remote commit
#[test_log::test]
fn a_branch_reworded_by_bd_reword_needs_a_push() {
  let space = Space::new(&["(auth) Add the auth module with a subject that is far longer than the rule allows"]);
  push_and_rebuild_on_a_newer_baseline(&space, &space.environment());
  let head = space.repo.head();
  let (exit_code, output) = run_json(
    &space.environment(),
    &[
      "--prefix",
      PREFIX,
      "reword",
      &head,
      "--subject",
      "(auth) Add the auth module",
      "--expect-head",
      &head,
      "--sync",
    ],
  );
  assert_eq!((exit_code, &output["rewritten"]), (0, &json!(1)), "{output}");

  check_auth_is_reworded(&space);

  let (exit_code, output) = bd_push(&space.environment(), &["auth", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["branches"][0]["state"], json!("forced"), "{output}");
}

/// A member of a stack that landed whole got a new commit: its remote branch has its commits as they were before the
/// landing, which the baseline has by identity (`remote.landed`). The check names them and refuses nothing, the start
/// pushes without --force; someone else's commit there, which the baseline lacks, refuses the check and the start with
/// the push that drops it
#[test_log::test]
fn check_counts_the_remote_commits_the_baseline_landed_and_refuses_a_drop() {
  let space = Space::new(&LANDED_SUBJECTS);
  let (exit_code, output) = bd_push(&space.environment(), &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  land_auth_by_content(&space.repo);

  let remote = check_auth_needs_a_push(&space);
  assert_eq!((&remote["unpushed"], &remote["landed"], &remote["behind"]), (&json!(1), &json!(1), &json!(0)));

  let theirs = push_from_elsewhere_to(&space.remote, AUTH, "Someone else");
  git(&space.repo, &["fetch", "-q", "origin"]);
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["refusals"][0]["code"], json!("push_refused"), "{output}");
  let message = output["refusals"][0]["message"].as_str().unwrap();
  assert!(
    message.contains(&format!("`bd push auth --force --expect-remote {}`", git_ops::revs::short_id(&theirs))),
    "{message}"
  );
  assert_eq!(
    (&output["remotes"][0]["remote"]["behind"], &output["remotes"][0]["remote"]["landed"]),
    (&json!(1), &json!(1))
  );

  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  let refused = &output["error"]["branches"][0]["remote"];
  assert_eq!(refused["dropped_commits"][0]["subject"], json!("Someone else"));
  assert_eq!(
    (space.fake.start_calls.load(Ordering::SeqCst), space.remote_tip(AUTH)),
    (0, theirs),
    "nothing was pushed or started"
  );
}

/// login stacked on auth, each with one commit of its own file
fn login_on_auth() -> Space {
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add login"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  space
}

/// `bd safe-push login --check`, which lands auth with login and refuses nothing; the output matches the schema
fn check_login_lands_auth(space: &Space) -> Value {
  let (exit_code, output) = space.run(&["login", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["lands"].clone(), output.get("refusals")), (json!(["auth", "login"]), None));
  let (_, schema) = run_json(&space.environment(), &["schema"]);
  assert_covered(&schema["outputs"]["safe_push"], &schema["outputs"]["safe_push"], &output, "safe_push");
  output
}

/// A parked branch starts no robot, and neither does a branch built on it, before anything is asked of Patronus
#[test_log::test]
fn a_parked_branch_and_a_branch_built_on_it_start_nothing() {
  let space = login_on_auth();
  let (exit_code, output) = run_in(&space.repo, &["park", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  for args in [&["auth", "--check"][..], &["login"], &["login", "--patronus-dry-run"]] {
    let (exit_code, output) = space.run(args);
    assert_error(&output, exit_code, (4, "parked"));
  }
  assert_eq!(space.remote_tip("test-user/virtual/auth"), "");
  run_in(&space.repo, &["unpark", "auth"]);
  check_login_lands_auth(&space);
}

/// The virtual branch of a stacked branch carries the commits of the branches it is built on: one robot lands them all.
/// No refusal and no flag; `--check` adds the trial build of the branch alone, the second way, which a start never runs
#[test_log::test]
fn a_stacked_branch_lands_its_parents_with_it() {
  let space = login_on_auth();
  let output = check_login_lands_auth(&space);
  // its commit changes a file of its own: it builds alone
  assert_eq!(
    output["alone"],
    json!({"result": "builds", "parent": "auth", "symbols": true,
      "message": "login builds alone on the baseline and uses no name its stack adds: `bd unstack login` makes it a root that lands alone (not checked by a compiler)"})
  );
  // the obsolete --land-stack is accepted and changes nothing
  let (exit_code, landed_stack) = space.run(&["login", "--land-stack", "--check"]);
  assert_eq!((exit_code, &landed_stack), (0, &output));

  let (exit_code, output) = space.run(&["login"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["lands"].clone(), output.get("alone")), (json!(["auth", "login"]), None));
  let pushed: Vec<&Value> = output["pushed"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(pushed, [&json!("auth"), &json!("login")]);
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH));
  assert_eq!(space.last_start()["sourceBranch"], json!("refs/heads/test-user/virtual/login"));
  assert_eq!(space.fake.start_calls.load(Ordering::SeqCst), 1, "one robot");

  // a dry run checks the same commits
  let (exit_code, output) = space.run(&["login", "--patronus-dry-run"]);
  assert_eq!((exit_code, output["lands"].clone()), (0, json!(["auth", "login"])), "{output}");
  assert_eq!(space.last_start()["mode"], json!("DRY_RUN"));
}

#[test_log::test]
fn a_stacked_branch_that_needs_a_commit_of_its_parent_to_land_alone_names_it() {
  let space = login_on_auth();
  // a second commit of login changes the file the commit of auth adds
  write(&space.repo, "f0.txt", "(auth) Add auth, for login");
  let uses = commit_all(&space.repo, "(auth/login) Use auth", 1_700_000_500)[..8].to_string();
  let auth = git(&space.repo, &["rev-parse", "HEAD~2"])[..8].to_string();

  let output = check_login_lands_auth(&space);
  assert_eq!(
    output["alone"],
    json!({"result": "needs", "parent": "auth",
      "conflict": {"commit": uses, "subject": "Use auth", "files": ["f0.txt"]},
      "needs": [{"commit": auth, "subject": "Add auth", "branch": "auth"}], "movable": true,
      "message": format!("login needs {auth} (auth): move it into the branch, then `bd unstack login`")})
  );
}

#[test_log::test]
fn without_a_token_or_space_nothing_runs() {
  let space = Space::new(&["(auth) Add auth"]);
  let environment = Environment {
    home: Some(Path::new(space.repo.path()).join("no-home")),
    ..space.environment()
  };
  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "not_logged_in"));
  assert!(output["error"]["message"].as_str().unwrap().contains("./safePush.cmd"), "{output}");

  git(&space.repo, &["remote", "set-url", "origin", space.remote.path().to_str().unwrap()]);
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "not_on_space"));
  assert_eq!(space.fake.list_calls.load(Ordering::SeqCst), 0);
}

/// auth needs the code-owner review, which counts in its GitHub pull request (`fake_github`, `reviewed_on_github`)
fn needs_review_on_github() -> (Space, Arc<FakeGitHub>, Environment) {
  let space = Space::new(&["(auth) Add auth"]);
  space.fake.review_required.store(true, Ordering::SeqCst);
  let (github, url) = fake_github(&space, &[]);
  let environment = reviewed_on_github(&space, &url);
  (space, github, environment)
}

/// `bd safe-push auth --create-pull-request`, which succeeds
fn create_pull_request(environment: &Environment) -> Value {
  let (exit_code, output) = safe_push(environment, &["auth", "--create-pull-request"]);
  assert_eq!(exit_code, 0, "{output}");
  output
}

#[test_log::test]
fn a_branch_with_an_open_pull_request_starts_from_a_comment() {
  let space = Space::new(&["(auth) Add auth"]);
  let (github, url) = fake_github(&space, &[pull_request_json(&[])]);
  let environment = reviewed_on_github(&space, &url);

  // a comment can't choose a profile
  let (exit_code, output) = safe_push(&environment, &["auth", "--profile", "lsc"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["started_via"], json!("pull_request"));
  assert_eq!(output["review"]["counts_in"], json!("pull_request"));
  assert_eq!(
    output["pull_request"],
    json!({"number": 7, "url": "https://github.com/acme/monorepo/pull/7", "comment_url": "https://github.com/acme/monorepo/pull/7#issuecomment-4402", "reply": "accepted"})
  );
  assert_eq!(output["robot"]["id"], json!("from-comment"));
  assert_eq!(space.fake.start_calls.load(Ordering::SeqCst), 0);
  // branches reviewed on GitHub: no Space merge request is looked up
  assert_eq!(space.fake.merge_request_calls.load(Ordering::SeqCst), 0);
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/issues/7/comments")[0].json(),
    json!({"body": "/patronus merge --keep-source-branch"})
  );
}

#[test_log::test]
fn the_approvals_count_in_the_pull_request_it_opens_and_the_safe_push_starts_from_it() {
  let (space, github, environment) = needs_review_on_github();

  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "approvals_missing"));
  assert!(output["error"]["message"].as_str().unwrap().contains("bd safe-push auth --create-pull-request"), "{output}");

  let output = create_pull_request(&environment);
  assert_eq!(
    output["pull_request"],
    json!({"number": 7, "url": "https://github.com/acme/monorepo/pull/7", "created": true, "comment_url": null, "reply": null, "assignees": ["me"]})
  );
  assert_eq!((output["robot"].clone(), space.fake.start_calls.load(Ordering::SeqCst)), (Value::Null, 0));
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH));
  // the text of its one commit: its subject
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/pulls")[0].json(),
    json!({"title": "Add auth", "body": "", "head": AUTH, "base": "master"})
  );

  // not approved yet: the error names the pull request
  list_pull_requests(&github, &[pull_request_json(&[])]);
  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "approvals_missing"));
  assert!(output["error"]["message"].as_str().unwrap().contains("pull request #7"), "{output}");

  // a Patronus that doesn't evaluate the pull request can't tell whether alice counts: the robot decides
  list_pull_requests(&github, &[pull_request_json(&["alice"])]);
  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["review"]["approved_on_github"], json!(["alice"]));
  assert_eq!(
    (output["review"]["readiness"].clone(), output["review"]["readiness_reason"].clone()),
    (json!("unknown"), json!("approvals_not_evaluated"))
  );
  assert_eq!(output["started_via"], json!("pull_request"));
}

/// The screenshot of 2026-10-17: a bot approved, the code owner is asked and hasn't. Patronus counts no approval, so the
/// start is refused, naming who it waits for and the approval it doesn't count; once the owner approves it starts
#[test_log::test]
fn patronus_judges_the_approvals_of_the_pull_request() {
  let space = Space::new(&["(auth) Add auth"]);
  space.fake.review_required.store(true, Ordering::SeqCst);
  *space.fake.pull_request_review.lock().unwrap() = Some(evaluated_review(false, "REVIEWING"));
  let mut waiting = pull_request_json(&["junie-jetbrains"]);
  waiting["reviewRequests"] = json!({"nodes": [{"requestedReviewer": {"__typename": "User", "login": "ikorennoy"}}]});
  let (_github, url) = fake_github(&space, &[waiting]);
  let environment = reviewed_on_github(&space, &url);

  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "approvals_missing"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("Needs review by IJPL Code Platform (requested: Ilya Korennoy)."), "{message}");
  assert!(
    message.contains("junie-jetbrains approved, but no eligible code owner of these files: Patronus doesn't count it."),
    "{message}"
  );
  let review = &output["error"]["review"];
  assert_eq!(
    (review["readiness"].clone(), review["readiness_reason"].clone()),
    (json!("missing"), json!("approvals_missing"))
  );
  assert_eq!(review["waiting_for"], json!([{"login": "ikorennoy", "full_name": "Ilya Korennoy"}]));
  assert_eq!(review["not_counted"], json!([{"login": "junie-jetbrains", "full_name": null}]));
  assert_eq!(review.get("approved_on_github"), None, "the bot's approval doesn't count");
  assert_eq!((review["groups"][0]["approved"].clone(), review["groups"][0]["needed"].clone()), (json!(false), json!(1)));
  assert_eq!(review["groups"][0]["reviewers"][0]["approval_state"], json!("reviewing"));
  assert_eq!(space.fake.checked_pull_requests.lock().unwrap()[0], json!({"repository": "acme/monorepo", "number": 7}));
  assert_eq!(space.fake.start_calls.load(Ordering::SeqCst), 0);

  *space.fake.pull_request_review.lock().unwrap() = Some(evaluated_review(true, "APPROVED"));
  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["review"]["readiness"], json!("approved"));
  assert_eq!(output["review"]["approved_on_github"], json!(["ikorennoy"]));
  assert_eq!(output["started_via"], json!("pull_request"));
}

#[test_log::test]
fn a_pull_request_without_a_github_token_is_not_logged_in() {
  let space = Space::new(&["(auth) Add auth"]);
  let (github, url) = fake_github(&space, &[]);
  let environment = Environment {
    github_tokens: TokenSources::default(),
    ..reviewed_on_github(&space, &url)
  };
  let (exit_code, output) = safe_push(&environment, &["auth", "--create-pull-request"]);
  assert_error(&output, exit_code, (4, "not_logged_in"));
  assert!(github.requests().is_empty());
}

#[test_log::test]
fn a_pull_request_that_already_exists_is_found_instead() {
  let space = Space::new(&["(auth) Add auth", "(auth) Test auth"]);
  let (github, url) = fake_github(&space, &[]);
  let environment = reviewed_on_github(&space, &url);
  github.answer(
    "POST",
    "/repos/acme/monorepo/pulls",
    FakeAnswer::status(
      422,
      json!({"message": "Validation Failed", "errors": [{"resource": "PullRequest", "code": "custom", "message": format!("A pull request already exists for acme:{AUTH}.")}]}),
    ),
  );
  // GitHub lists it once the search sees it, e.g. after the plan of this run
  github.answer_in_turn("POST", "/graphql", vec![search_answer(&[]), search_answer(&[pull_request_json(&[])])]);
  let output = create_pull_request(&environment);
  assert_eq!(
    output["pull_request"],
    json!({"number": 7, "url": "https://github.com/acme/monorepo/pull/7", "comment_url": null, "reply": null,
      "warnings": ["pull request #7 already existed: it was left as it is, without the assignee and the reviewers"]})
  );
  assert!(github.requests_to("POST", "/repos/acme/monorepo/issues/7/assignees").is_empty());
  // the text of its two commits: titled by the first, an item for each
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/pulls")[0].json(),
    json!({"title": "Add auth", "body": "1. **Add auth**\n\n2. **Test auth**", "head": AUTH, "base": "master"})
  );
}

#[test_log::test]
fn the_pull_request_of_a_branch_only_rebuilt_on_a_newer_baseline_is_opened_without_a_push() {
  let (space, github, environment) = needs_review_on_github();
  let pushed = push_and_rebuild_on_a_newer_baseline(&space, &environment);
  let output = create_pull_request(&environment);
  assert_eq!((output["pull_request"]["created"].clone(), output.get("pushed")), (json!(true), None));
  assert_eq!(space.remote_tip(AUTH), pushed, "nothing was pushed");
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/pulls")[0].json(),
    json!({"title": "Add auth", "body": "", "head": AUTH, "base": "master"})
  );
}

#[test_log::test]
fn the_pull_request_it_opens_asks_the_suggested_code_owner_by_the_github_login_patronus_names() {
  let (_space, github, environment) = needs_review_on_github();
  let (exit_code, output) = safe_push(&environment, &["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let login = output["review"]["groups"][0]["reviewers"][0]["github_login"].clone();
  assert_eq!(login, json!("alice-gh"));

  let (exit_code, output) = safe_push(&environment, &["auth", "--create-pull-request", "--reviewer", "alice-gh", "--no-assign"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["pull_request"]["reviewers"], json!(["alice-gh"]));
  assert_eq!(output["pull_request"].get("assignees"), None);
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/pulls/7/requested_reviewers")[0].json(),
    json!({"reviewers": ["alice-gh"]})
  );
  assert!(github.requests_to("POST", "/repos/acme/monorepo/issues/7/assignees").is_empty());
}

/// The Safe Push failure of 2026-10-16 is refused before anything is pushed: the build would fail on the use that
/// another branch changes
#[test_log::test]
fn a_dangling_reference_refuses_the_safe_push_unless_allowed() {
  let space = Space::new(&[]);
  let (removed_by, fixed_by) = commit_dangling_reference(&space.repo);

  let (exit_code, output) = space.run(&["fake-agent"]);
  assert_error(&output, exit_code, (4, "dangling_reference"));
  assert_eq!(
    output["error"]["message"],
    json!(format!(
      "fake-agent removes CONTROL_URL_ENV in {removed}, and {ACP_HOST}:7 still uses it: {fixed} changes that use in another branch, so the build fails. To fix it, move {fixed} into fake-agent: it changes only {ACP_HOST}; --allow-dangling starts anyway",
      removed = &removed_by[..8],
      fixed = &fixed_by[..8],
    ))
  );
  assert_eq!(space.remote_tip("test-user/virtual/fake-agent"), "", "nothing was pushed");

  let codes = |output: &Value| -> Vec<Value> {
    output["refusals"]
      .as_array()
      .map_or_default(|refusals| refusals.iter().map(|refusal| refusal["code"].clone()).collect())
  };
  let (_, output) = space.run(&["fake-agent", "--check"]);
  assert!(codes(&output).contains(&json!("dangling_reference")), "{output}");
  let (_, output) = space.run(&["fake-agent", "--check", "--allow-dangling"]);
  assert!(!codes(&output).contains(&json!("dangling_reference")), "{output}");
}

#[test_log::test]
fn push_refuses_a_branch_whose_safe_push_runs_and_restarts_it_when_told() {
  let space = login_on_auth();
  let push = |args: &[&str]| bd_push(&space.environment(), args);

  space.set_robots(vec![robot("running", "RUNNING", "auth", "0000")]);
  let (exit_code, output) = push(&["auth"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert!(output["error"]["message"].as_str().unwrap().contains("--restart-running"), "{output}");
  assert_eq!(space.remote_tip(AUTH), "");

  // login is built on auth: its push starts above it, auth stays as the robot took it, and login carries its commits
  let (exit_code, output) = push(&["login"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("login")]);
  assert_eq!(space.remote_tip(AUTH), "");

  let (exit_code, output) = push(&["auth", "--restart-running"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(*space.fake.canceled.lock().unwrap(), vec!["running".to_string()]);
  assert_eq!(space.fake.restart_calls.load(Ordering::SeqCst), 1);
  assert_eq!(output["restarted"].as_array().map(Vec::len), Some(1), "{output}");
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH));

  // a canceling or a dry-run robot holds nothing
  space.set_robots(vec![robot("canceling", "CANCELING", "login", "0000")]);
  let (exit_code, output) = push(&["login"]);
  assert_eq!(exit_code, 0, "{output}");
}

/// The open pull request #`number` of `name` into `base`, approved by `approvers`
fn pull_request_of(name: &str, number: u32, base: &str, approvers: &[&str]) -> Value {
  let mut pull_request = pull_request_json(approvers);
  pull_request["number"] = json!(number);
  pull_request["url"] = json!(format!("https://github.com/acme/monorepo/pull/{number}"));
  pull_request["headRefName"] = json!(format!("{PREFIX}/virtual/{name}"));
  pull_request["baseRefName"] = json!(base);
  pull_request
}

/// The `PATCH` bodies GitHub got, by pull request number
fn retargets_sent(github: &FakeGitHub) -> Vec<(String, Value)> {
  github
    .requests()
    .into_iter()
    .filter(|request| request.method == "PATCH")
    .map(|request| (request.path.clone(), request.json()))
    .collect()
}

/// Every pull request of a virtual branch goes into the baseline: the open one of a pushed branch whose base is a
/// virtual branch (its parent's) is retargeted after the push; one into master or a release branch is left as it is
#[test_log::test]
fn push_retargets_a_pull_request_into_a_virtual_branch_to_the_baseline() {
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add login", "(docs) Add docs"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  let pull_requests = [
    pull_request_of("auth", 7, "master", &[]),
    pull_request_of("login", 8, AUTH, &["alice"]),
    pull_request_of("docs", 9, "release-252", &[]),
  ];
  let (github, url) = fake_github(&space, &pull_requests);
  github.answer("PATCH", "/repos/acme/monorepo/pulls/8", FakeAnswer::ok(json!({"number": 8})));
  let environment = reviewed_on_github(&space, &url);
  let push = |args: &[&str]| bd_push(&environment, args);
  let retargeted = json!([{"branch": "login", "number": 8, "url": "https://github.com/acme/monorepo/pull/8", "old_base": AUTH, "new_base": "master", "approved_by": ["alice"]}]);

  // a dry run says it: GitHub is only asked
  let (exit_code, output) = push(&["login", "docs", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["retargeted"], retargeted);
  assert_eq!(retargets_sent(&github), []);

  let (exit_code, output) = push(&["login", "docs"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["retargeted"], retargeted);
  assert_eq!(retargets_sent(&github), [("/repos/acme/monorepo/pulls/8".to_string(), json!({"base": "master"}))]);

  // GitHub refuses it: the push is done, the warning says how to retarget it
  github.answer(
    "PATCH",
    "/repos/acme/monorepo/pulls/8",
    FakeAnswer::status(403, json!({"message": "Resource not accessible by integration"})),
  );
  let (exit_code, output) = push(&["login"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output.get("retargeted"), None);
  let warning = output["retarget_warnings"][0].as_str().unwrap();
  assert!(
    warning.starts_with("cannot retarget pull request #8 of login to master: ") && warning.ends_with("; `gh pr edit 8 --repo acme/monorepo --base master` retargets it"),
    "{output}"
  );
  let (_, schema) = run_json(&environment, &["schema"]);
  assert_covered(&schema["outputs"]["push"], &schema["outputs"]["push"], &output, "push");
}

/// `bd push <root> --with-children` takes the stack branch of the whole stack along while its pull request is open,
/// also before this clone has its remote branch, as the app's Push with Children does: the pull request follows the
/// branches. GitHub is searched once, under the lock, and the listing serves the retargets after the push too
#[test_log::test]
fn push_with_children_takes_the_stack_branch_along_while_its_pull_request_is_open() {
  let space = forking_stack();
  let (github, url) = fake_github(&space, &[]);
  let environment = reviewed_on_github(&space, &url);
  let push = |args: &[&str]| bd_push(&environment, args);
  let searches = || github.requests_to("POST", "/graphql").len();

  // no pull request of the whole stack: the branches go, its stack branch stays local
  let (exit_code, output) = push(&["auth", "--with-children"]);
  assert_eq!((exit_code, output.get("stacks"), output.get("stack_warnings")), (0, None, None), "{output}");
  assert_eq!(space.remote_tip(STACK), "");
  assert_eq!(searches(), 1);

  // its pull request is open: the stack branch goes along, a new remote branch
  let mut stack_pull_request = pull_request_json(&[]);
  stack_pull_request["number"] = json!(12);
  stack_pull_request["headRefName"] = json!(STACK);
  list_pull_requests(&github, &[stack_pull_request]);
  let (exit_code, output) = push(&["auth", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  let pushed = &output["stacks"][0];
  assert_eq!(
    (&pushed["name"], &pushed["remote_branch"], &pushed["state"], &pushed["lease_source"]),
    (&json!("auth"), &json!(STACK), &json!("new"), &json!("absent"))
  );
  assert_eq!(space.remote_tip(STACK), space.local_tip(STACK));
  assert_eq!(searches(), 2, "one search per push: the stack branch and the retargets");
}

/// The robot starts from the pull request of the branch, which goes into the target: one into a virtual branch is
/// retargeted before anything is pushed, as are those of the branches its push takes along
#[test_log::test]
fn the_pull_requests_of_the_branch_and_of_the_parents_it_pushes_go_into_the_target_before_the_start() {
  let space = login_on_auth();
  // auth's goes into the branch it was stacked on before, login's into auth's; the comment is answered on #7
  let (github, url) = fake_github(
    &space,
    &[pull_request_of("auth", 6, "test-user/virtual/core", &[]), pull_request_of("login", 7, AUTH, &["alice"])],
  );
  github.answer("PATCH", "/repos/acme/monorepo/pulls/6", FakeAnswer::ok(json!({"number": 6})));
  github.answer("PATCH", "/repos/acme/monorepo/pulls/7", FakeAnswer::ok(json!({"number": 7})));
  let environment = reviewed_on_github(&space, &url);
  let retargeted = json!([
    {"branch": "auth", "number": 6, "url": "https://github.com/acme/monorepo/pull/6", "old_base": "test-user/virtual/core", "new_base": "master"},
    {"branch": "login", "number": 7, "url": "https://github.com/acme/monorepo/pull/7", "old_base": AUTH, "new_base": "master", "approved_by": ["alice"]},
  ]);

  let (exit_code, output) = safe_push(&environment, &["login", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["retargeted"], &output["pull_request"]["number"]), (&retargeted, &json!(7)));
  assert_eq!(retargets_sent(&github), []);
  let (_, schema) = run_json(&environment, &["schema"]);
  assert_covered(&schema["outputs"]["safe_push"], &schema["outputs"]["safe_push"], &output, "safe_push");

  let (exit_code, output) = safe_push(&environment, &["login"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["retargeted"], &output["started_via"]), (&retargeted, &json!("pull_request")));
  assert_eq!(
    retargets_sent(&github),
    [
      ("/repos/acme/monorepo/pulls/6".to_string(), json!({"base": "master"})),
      ("/repos/acme/monorepo/pulls/7".to_string(), json!({"base": "master"})),
    ]
  );
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/issues/7/comments")[0].json(),
    json!({"body": "/patronus merge --keep-source-branch"})
  );

  // GitHub refuses the retarget of the pull request the robot starts from: nothing is pushed or started
  github.answer(
    "PATCH",
    "/repos/acme/monorepo/pulls/7",
    FakeAnswer::status(403, json!({"message": "Resource not accessible by integration"})),
  );
  write(&space.repo, "form.txt", "form\n");
  commit_all(&space.repo, "(auth/login) Add the form", 1_700_000_900);
  let pushed = space.remote_tip("test-user/virtual/login");
  let (exit_code, output) = safe_push(&environment, &["login"]);
  assert_error(&output, exit_code, (5, "network_error"));
  assert!(
    output["error"]["message"]
      .as_str()
      .unwrap()
      .starts_with(&format!("Nothing was pushed or started: cannot retarget pull request #7 of login from {AUTH} to master: ")),
    "{output}"
  );
  assert_eq!(space.remote_tip("test-user/virtual/login"), pushed);
}

/// One open pull request per branch: `--create-pull-request` takes the one the branch has, whatever its base, and
/// retargets one into a virtual branch to the target
#[test_log::test]
fn create_pull_request_takes_the_open_pull_request_of_the_branch_whatever_its_base() {
  let (_space, github, environment) = needs_review_on_github();
  list_pull_requests(&github, &[pull_request_of("auth", 7, "release-252", &[])]);
  let output = create_pull_request(&environment);
  let warning = output["pull_request"]["warnings"][0].as_str().unwrap();
  assert!(warning.starts_with("pull request #7 already existed and goes into release-252: "), "{output}");
  assert_eq!(output["pull_request"]["number"], json!(7));
  assert_eq!((output["pull_request"].get("created"), output.get("retargeted")), (None, None));

  list_pull_requests(&github, &[pull_request_of("auth", 7, "test-user/virtual/core", &[])]);
  github.answer("PATCH", "/repos/acme/monorepo/pulls/7", FakeAnswer::ok(json!({"number": 7})));
  let output = create_pull_request(&environment);
  assert_eq!(
    output["pull_request"],
    json!({"number": 7, "url": "https://github.com/acme/monorepo/pull/7", "comment_url": null, "reply": null})
  );
  assert_eq!(output["retargeted"][0]["old_base"], json!("test-user/virtual/core"));
  assert!(github.requests_to("POST", "/repos/acme/monorepo/pulls").is_empty());
}

/// `--whole-stack` of any branch of a stack that forks lands every branch of it in one robot, from the stack branch of
/// its root, the only branch pushed. Both ways the robots wait for each other: the robot of the whole stack lands every
/// branch of it, the robot of a branch a part of it
#[test_log::test]
fn the_whole_stack_lands_in_one_robot_from_its_stack_branch() {
  let space = forking_stack();
  let (exit_code, output) = space.run(&["login", "--whole-stack", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let (_, schema) = run_json(&space.environment(), &["schema"]);
  assert_covered(&schema["outputs"]["safe_push"], &schema["outputs"]["safe_push"], &output, "safe_push");
  assert_eq!(
    (output["lands"].clone(), output["whole_stack"].clone(), output.get("refusals"), output.get("alone")),
    (json!(["auth", "login", "session"]), json!({"root": "auth", "source": STACK, "state": "built"}), None, None)
  );
  assert_eq!(output["push_needed"], json!(true));

  let (exit_code, output) = space.run(&["session", "--whole-stack"]);
  assert_eq!(exit_code, 0, "{output}");
  // only the stack branch, leased: the branches of the stack stay as they are on the remote
  let pushed: Vec<(&Value, &Value, &Value)> = output["pushed"]
    .as_array()
    .unwrap()
    .iter()
    .map(|branch| (&branch["name"], &branch["remote_branch"], &branch["lease_source"]))
    .collect();
  assert_eq!(pushed, [(&json!("auth"), &json!(STACK), &json!("absent"))]);
  assert_eq!(output["pushed"][0]["pull_request"]["head"], json!(STACK));
  let stack_tip = space.local_tip(STACK);
  assert_eq!((space.remote_tip(STACK), space.remote_tip(AUTH)), (stack_tip.clone(), String::new()));
  assert_eq!(space.last_start()["sourceBranch"], json!(format!("refs/heads/{STACK}")));
  assert_eq!(space.fake.start_calls.load(Ordering::SeqCst), 1, "one robot");

  // the robot of the whole stack lands every branch of it: a start or a push of a branch waits for it, and a start of
  // the whole stack on the same commits too
  space.set_robots(vec![stack_robot("whole", "RUNNING", &stack_tip)]);
  let (exit_code, output) = space.run(&["login"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert_eq!(output["error"]["message"], json!("A Safe Push of the whole auth stack is running: it lands login with it"));
  let (exit_code, output) = space.run(&["auth", "--whole-stack"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert_eq!(output["error"]["message"], json!("A Safe Push of the last commit is running"));
  let (exit_code, output) = run_json(&space.environment(), &["--prefix", PREFIX, "push", "session"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("A Safe Push of the whole auth stack is running: it lands session with it"), "{output}");

  // the Safe Push of a branch lands a part of the stack: the whole stack waits for it; a dry run lands nothing
  space.set_robots(vec![robot("piece", "RUNNING", "login", "0000")]);
  let (exit_code, output) = space.run(&["auth", "--whole-stack"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert_eq!(output["error"]["message"], json!("A Safe Push of login is running: wait until it merges"));
  let (exit_code, output) = space.run(&["auth", "--whole-stack", "--patronus-dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  // on the remote as it is: nothing to push
  assert_eq!(
    (output["mode"].clone(), output["push_needed"].clone(), output.get("pushed")),
    (json!("dry_run"), json!(false), None)
  );
  assert_eq!(
    (space.last_start()["mode"].clone(), space.last_start()["sourceBranch"].clone()),
    (json!("DRY_RUN"), json!(format!("refs/heads/{STACK}")))
  );
}

/// A straight chain has no stack branch: its last branch carries the whole stack, and the robot starts from it
#[test_log::test]
fn the_whole_stack_of_a_straight_chain_lands_from_its_last_branch() {
  let space = login_on_auth();
  let (exit_code, output) = space.run(&["auth", "--whole-stack", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output["lands"].clone(), output["whole_stack"].clone()),
    (json!(["auth", "login"]), json!({"root": "auth", "source": "test-user/virtual/login"}))
  );
  let (exit_code, output) = space.run(&["auth", "--whole-stack"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(space.last_start()["sourceBranch"], json!("refs/heads/test-user/virtual/login"));
}

/// A commit of the stack branch that conflicts with the commits of the stack before it refuses the start with the
/// conflict (exit 1), nothing pushed or started: the stack lands in pieces. A branch of the stack that fails keeps the
/// stack branch from being built: `stack_not_built`
#[test_log::test]
fn a_whole_stack_that_conflicts_is_refused_with_the_conflict() {
  let space = forking_stack();
  // session changes a file that only an unassigned commit creates
  write(&space.repo, "config.txt", "a");
  commit_all(&space.repo, "Add config", 1_700_000_500);
  write(&space.repo, "config.txt", "b");
  let changed = commit_all(&space.repo, "(auth/session) Change config", 1_700_000_600)[..8].to_string();

  let (exit_code, output) = space.run(&["auth", "--whole-stack", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output["whole_stack"]["state"].clone(), output["refusals"][0]["code"].clone(), output["review"].clone()),
    (json!("conflict"), json!("stack_conflict"), Value::Null),
    "a stack branch that is not built has no commits to review"
  );
  let (exit_code, output) = space.run(&["login", "--whole-stack"]);
  assert_error(&output, exit_code, (4, "stack_conflict"));
  assert_eq!(
    (output["error"]["commits"].clone(), output["error"]["paths"].clone()),
    (json!([changed]), json!(["config.txt"]))
  );
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.starts_with(&format!(
      "Nothing was pushed or started: the whole auth stack doesn't build as one branch: {changed} (Change config, of session) conflicts in config.txt"
    )),
    "{output}"
  );
  assert!(message.contains("Land it in pieces"), "{output}");
  assert_eq!((space.remote_tip(STACK), space.fake.start_calls.load(Ordering::SeqCst)), (String::new(), 0));

  // auth's own commit changes the file its child creates: auth conflicts, the stack branch is not built
  let space = forking_stack();
  write(&space.repo, "config.txt", "a");
  commit_all(&space.repo, "(auth/login) Add config", 1_700_000_500);
  write(&space.repo, "config.txt", "b");
  commit_all(&space.repo, "(auth) Change config", 1_700_000_600);
  let (exit_code, output) = space.run(&["session", "--whole-stack"]);
  assert_error(&output, exit_code, (4, "stack_not_built"));
  assert_eq!(
    output["error"]["message"],
    json!(
      "Nothing was pushed or started: the stack branch of auth is not the build of its commits: auth conflicts, so the stack branch was not rebuilt. `bd sync` says what to fix"
    )
  );
  assert_eq!(space.fake.start_calls.load(Ordering::SeqCst), 0);
}

/// The approvals of the whole stack count in the pull request of its stack branch: `--create-pull-request` opens it
/// with the text of the whole stack, the readiness asks Patronus about it, and the start comments on it
#[test_log::test]
fn the_pull_request_of_the_whole_stack_is_its_stack_branchs_and_the_start_comments_on_it() {
  let space = forking_stack();
  git(
    &space.repo,
    &["config", "branch.test-user/virtual/auth.description", "IJPL-1 Add auth\n\nThe auth service."],
  );
  let (github, url) = fake_github(&space, &[]);
  let environment = reviewed_on_github(&space, &url);

  let (exit_code, output) = safe_push(&environment, &["login", "--whole-stack", "--create-pull-request", "--reviewer", "alice-gh"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["pull_request"]["created"].clone(), output["robot"].clone()), (json!(true), Value::Null));
  assert_eq!(space.remote_tip(STACK), space.local_tip(STACK));
  let created = github.requests_to("POST", "/repos/acme/monorepo/pulls")[0].json();
  assert_eq!(
    (&created["title"], &created["head"], &created["base"]),
    (&json!("IJPL-1 Add auth, with the 2 branches built on it"), &json!(STACK), &json!("master"))
  );
  assert_eq!(
    created["body"],
    json!(
      "The whole auth stack in one Safe Push into master: 3 branches · 3 commits.\n\n## auth: IJPL-1 Add auth\n\nThe auth service.\n\n## login\n\nBuilt on auth.\n\n- Add login\n\n## session\n\nBuilt on auth.\n\n- Add session"
    )
  );
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/pulls/7/requested_reviewers")[0].json(),
    json!({"reviewers": ["alice-gh"]})
  );

  // the open pull request of the stack branch is the stack's, not auth's: the start comments on it
  let mut stack_pull_request = pull_request_json(&["alice-gh"]);
  stack_pull_request["headRefName"] = json!(STACK);
  list_pull_requests(&github, &[stack_pull_request]);
  let (exit_code, output) = safe_push(&environment, &["auth", "--whole-stack"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["started_via"].clone(), output["pull_request"]["number"].clone()), (json!("pull_request"), json!(7)));
  assert_eq!(
    space.fake.checked_pull_requests.lock().unwrap().last(),
    Some(&json!({"repository": "acme/monorepo", "number": 7}))
  );
  let (exit_code, output) = safe_push(&environment, &["auth", "--check"]);
  assert_eq!((exit_code, output.get("pull_request")), (0, None), "{output}");
}

#[test_log::test]
fn a_check_frees_the_repository_lock_after_its_sync_and_a_start_holds_it() {
  let space = Space::new(&["(auth) Add auth"]);
  // while bd asks Patronus for the start options, another bd writer tries the lock
  let taken = Arc::new(std::sync::Mutex::new(Vec::new()));
  let git_common_dir = space.repo.path().join(".git");
  *space.fake.on_start_options.lock().unwrap() = Some(Box::new({
    let taken = Arc::clone(&taken);
    move || {
      let lock = history_rewrite::repository_lock::lock_repository_in(&git_common_dir, std::time::Duration::from_millis(200), &|| {});
      taken.lock().unwrap().push(lock.is_ok());
    }
  }));

  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let (exit_code, output) = space.run(&["auth"]);
  assert_eq!(exit_code, 0, "{output}");

  // the check writes nothing after its sync; the start pushes after the options, under the lock it synced with
  assert_eq!(*taken.lock().unwrap(), [true, false]);
}

/// The target moved after the last sync with a change of the lines a commit of the branch changes: Patronus could not
/// rebase it and would fail the robot at once. The check fetches the target, replays the branch onto it and refuses the
/// start (`target_conflict`, exit 4), nothing pushed or started; `--check` lists it in `refusals`
#[test_log::test]
fn a_target_that_moved_with_a_conflicting_change_after_the_last_sync_refuses_the_start() {
  let space = Space::new(&["(auth) Add auth"]);
  space.publish_target();
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["target_check"]["result"], json!("clean"), "{output}");
  let original = git(&space.repo, &["rev-parse", "HEAD"])[..8].to_string();
  let target = space.land_on_target("f0.txt", "theirs", "Change f0")[..8].to_string();

  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (
      output["refusals"][0]["code"].clone(),
      output["target_check"]["result"].clone(),
      output["target_check"]["fetched"].clone()
    ),
    (json!("target_conflict"), json!("conflict"), json!(true)),
    "{output}"
  );
  assert_eq!(output["target_check"]["target_tip"], json!(target));
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "target_conflict"));
  assert_eq!(
    (output["error"]["commits"].clone(), output["error"]["paths"].clone()),
    (json!([original]), json!(["f0.txt"]))
  );
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.starts_with(&format!(
      "Nothing was pushed or started: Patronus can't rebase auth onto origin/master: {original} (Add auth) conflicts in f0.txt, which origin/master changed after the base of auth: {target} (Alice: Change f0)"
    )),
    "{message}"
  );
  assert!(message.contains("`bd update`") && message.contains("`bd safe-push auth` again"), "{message}");
  assert_eq!((space.fake.start_calls.load(Ordering::SeqCst), space.remote_tip(AUTH)), (0, String::new()));
}

/// The target moved after the last sync with a change of other files: the replay applies, and the start goes on. A
/// fetch that fails doesn't stop the check: it replays onto the last fetched target and says so
#[test_log::test]
fn a_target_that_moved_without_a_conflict_starts() {
  let space = Space::new(&["(auth) Add auth"]);
  // no target on the remote: the fetch fails, the check uses the last fetched target
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output["target_check"]["result"].clone(), output["target_check"]["fetched"].clone()),
    (json!("clean"), json!(false)),
    "{output}"
  );
  let message = output["target_check"]["message"].as_str().unwrap();
  assert!(
    message.starts_with("The fetch of origin/master failed, so the check used the last fetched origin/master: "),
    "{message}"
  );

  space.publish_target();
  let target = space.land_on_target("other.txt", "theirs", "Add other")[..8].to_string();
  let (exit_code, output) = space.run(&["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["target_check"], json!({"result": "clean", "target_tip": target, "fetched": true}), "{output}");
  assert_eq!(output["robot"]["id"], json!("started"));
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH));
}
