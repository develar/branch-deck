use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{ACP_HOST, PREFIX, assert_error, commit_all, commit_dangling_reference, environment, git, next_steps, run_in, run_json, write};
use bd_cli::remote_fixture_test::{
  AUTH, LANDED_SUBJECTS, MASTER_RULES, STACK, Space, dismissing_rules, evaluated_review, fake_github, forking_stack, land_auth_by_content, list_pull_requests, minutes_ago,
  pull_request_json, push_from_elsewhere_to, rebuild_on_a_newer_baseline, reviewed_on_github, robot, safe_push, search_answer, stack_robot,
};
use bd_core::context::Environment;
use github::fake_github::{FakeAnswer, FakeGitHub};
use github::token::TokenSources;
use patronus::fake_patronus::robot_json;
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
  let (_, schema) = run_json(&environment(space.repo.path()), &["schema", "--all"]);
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
  assert_eq!(space.fake.starts.lock().unwrap().len(), 0);
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
  assert!(message.ends_with("--without-approvals starts anyway, only when the user says so"), "{message}");
  // a pull request can't be opened yet, and the config is the user's: no step
  assert_eq!(next_steps(&output["error"]), []);
  assert_eq!(output["error"]["review"]["groups"][0]["reviewers"][0]["username"], json!("alice"));
  assert_eq!((space.fake.starts.lock().unwrap().len(), space.remote_tip(AUTH)), (0, String::new()));

  let (exit_code, output) = space.run(&["auth", "--create-pull-request"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  // --check reports it instead of failing
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["refusals"][0]["code"], json!("approvals_missing"));
  assert_eq!(next_steps(&output["refusals"][0]), []);

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
fn a_profile_that_skips_the_review_starts_without_the_approvals_through_the_api() {
  let space = Space::new(&["(auth) Add auth"]);
  space.fake.review_required.store(true, Ordering::SeqCst);
  // the last robot of the branch ran with Large Scale Change, whose options say it skips the code-owner review
  space.add_robot("older", "FAILURE", "auth", "0000");
  space.fake.details.lock().unwrap().get_mut("older").unwrap()["options"]["patronus.review.approval.check.skip"] = json!("true");

  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["profile"]["id"].clone(), output.get("refusals")), (json!("lsc"), None));
  assert_eq!(
    (output["review"]["readiness"].clone(), output["review"]["skipped_by"].clone()),
    (json!("missing"), json!("profile"))
  );

  // the default checks don't skip it: the flag does
  let (exit_code, output) = space.run(&["auth", "--profile", "none", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output["refusals"][0]["code"].clone(), output["review"].get("skipped_by")),
    (json!("approvals_missing"), None)
  );
  let (exit_code, output) = space.run(&["auth", "--profile", "none", "--without-approvals", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output.get("refusals"), output["review"]["skipped_by"].clone()), (None, json!("without_approvals")));

  let (exit_code, output) = space.run(&["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["started_via"], json!("api"));
  assert_eq!(space.last_start()["mergeProfileId"], json!("lsc"));
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
  assert_eq!(next_steps(&output["error"]), [("bd robot auth".to_string(), false)]);
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
  assert!(
    output["error"]["message"]
      .as_str()
      .unwrap()
      .contains("Once the user approved, --cancel-running cancels it first"),
    "{output}"
  );
  assert_eq!(
    next_steps(&output["error"]),
    [("bd robot auth".to_string(), false), ("bd safe-push auth --cancel-running".to_string(), true)]
  );
  assert_eq!(space.remote_tip(AUTH), "");
  // a Dry Run replaces no Safe Push: its push into the branch is refused, --cancel-running or not
  for args in [&["auth", "--patronus-dry-run"][..], &["auth", "--patronus-dry-run", "--cancel-running"]] {
    let (exit_code, output) = space.run(args);
    assert_error(&output, exit_code, (4, "robot_running"));
    assert!(output["error"]["message"].as_str().unwrap().starts_with("A Safe Push of auth is running"), "{output}");
  }
  assert_eq!((space.fake.canceled.lock().unwrap().len(), space.remote_tip(AUTH)), (0, String::new()));

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
  rebuild_on_a_newer_baseline(&space.repo);

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
  assert!(message.contains("the start would run beside it. Once the user approved, --cancel-running"), "{output}");
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
  let (_, schema) = run_json(&space.environment(), &["schema", "--all"]);
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
    next_steps(&output["error"]),
    [
      (format!("bd push auth --force --expect-remote {}", git_ops::revs::short_id(&theirs)), true),
      ("bd safe-push auth --force".to_string(), true)
    ]
  );
  assert_eq!(
    (space.fake.starts.lock().unwrap().len(), space.remote_tip(AUTH)),
    (0, theirs),
    "nothing was pushed or started"
  );

  // once the user approved the drop (D12), the start pushes the commit of someone else away and starts
  let (exit_code, output) = space.run(&["auth", "--check", "--force"]);
  assert_eq!((exit_code, output.get("refusals")), (0, None), "{output}");
  let (exit_code, output) = space.run(&["auth", "--force"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output["started_via"].clone(), space.remote_tip(AUTH)),
    (json!("api"), space.local_tip(AUTH)),
    "pushed over their commit"
  );
}

/// A review Patronus can't answer (its pre-check fails) is never "approved" nor a failure of the command: the Safe Push
/// is refused as `review_unknown` (D14), the `review` saying `unavailable`, until the user says to start anyway
/// (`--allow-unknown-review`), and the robot checks the approvals when it starts. A Dry Run asks no review
#[test_log::test]
fn a_review_patronus_cannot_answer_refuses_the_safe_push_unless_allowed() {
  let space = Space::new(&["(auth) Add auth"]);
  *space.fake.review_hook.lock().unwrap() = Some(Arc::new(|_| Some((500, json!({"message": "down"})))));
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (refusal_codes(&output), output["review"]["readiness"].clone()),
    (vec![json!("review_unknown")], json!("unavailable"))
  );
  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (4, "review_unknown"));
  assert!(output["error"]["message"].as_str().unwrap().contains("--allow-unknown-review starts anyway"), "{output}");
  assert_eq!(
    (space.fake.starts.lock().unwrap().len(), space.remote_tip(AUTH)),
    (0, String::new()),
    "nothing was pushed or started"
  );

  let (exit_code, output) = space.run(&["auth", "--patronus-dry-run", "--check"]);
  assert_eq!((exit_code, output.get("refusals"), output["review"].clone()), (0, None, Value::Null), "{output}");
  let (exit_code, output) = space.run(&["auth", "--allow-unknown-review"]);
  assert_eq!((exit_code, output["started_via"].clone()), (0, json!("api")), "{output}");
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
  let (_, schema) = run_json(&space.environment(), &["schema", "--all"]);
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
  assert_eq!(space.fake.starts.lock().unwrap().len(), 1, "one robot");

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

/// auth with its open pull request #7 on GitHub, not approved: its start is a `/patronus` comment there
fn open_pull_request_on_github() -> (Space, Arc<FakeGitHub>, Environment) {
  let space = Space::new(&["(auth) Add auth"]);
  let (github, url) = fake_github(&space, &[pull_request_json(&[])]);
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
  let (space, github, environment) = open_pull_request_on_github();

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
  assert_eq!(space.fake.starts.lock().unwrap().len(), 0);
  // branches reviewed on GitHub: no Space merge request is looked up
  assert_eq!(space.fake.merge_request_calls.load(Ordering::SeqCst), 0);
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/issues/7/comments")[0].json(),
    json!({"body": "/patronus merge --keep-source-branch"})
  );
}

/// The robot a comment started is never an earlier one of the same pull request: a Safe Push and a Dry Run of #7
/// canceled a minute ago, newer than the robot of the comment, are listed before and after the comment. The Safe Push
/// the comment started is the robot, never the canceled one of its mode; a Dry Run whose robot Patronus lists not yet
/// has none, never the canceled Dry Run
#[test_log::test]
fn the_robot_of_a_comment_is_never_an_earlier_robot_of_the_pull_request() {
  let (space, github, environment) = open_pull_request_on_github();
  let mut robots = space.fake.robots.lock().unwrap().clone().unwrap();
  for (id, mode) in [("canceled-safe-push", "REBASE_AUTOSQUASH"), ("canceled-dry-run", "DRY_RUN")] {
    let mut canceled = robot_json(id, "CANCELED", "refs/heads/test-user/virtual/other", &minutes_ago(1));
    canceled["pushMode"] = json!(mode);
    canceled["githubPrUrl"] = json!("https://github.com/acme/monorepo/pull/7");
    robots.push(canceled);
  }
  space.set_robots(robots);

  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output["started_via"].clone(), output["robot"]["id"].clone()),
    (json!("pull_request"), json!("from-comment"))
  );

  let (exit_code, output) = safe_push(&environment, &["auth", "--patronus-dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output["started_via"].clone(), output["pull_request"]["reply"].clone(), output["robot"].clone()),
    (json!("pull_request"), json!("accepted"), Value::Null)
  );
  assert_eq!(
    github.requests_to("POST", "/repos/acme/monorepo/issues/7/comments")[1].json(),
    json!({"body": "/patronus dry-run"})
  );
}

#[test_log::test]
fn the_approvals_count_in_the_pull_request_it_opens_and_the_safe_push_starts_from_it() {
  let (space, github, environment) = needs_review_on_github();

  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "approvals_missing"));
  assert!(output["error"]["message"].as_str().unwrap().contains("bd safe-push auth --create-pull-request"), "{output}");
  // it pushes the branch: the user's to approve
  assert_eq!(next_steps(&output["error"]), [("bd safe-push auth --create-pull-request".to_string(), true)]);

  let output = create_pull_request(&environment);
  assert_eq!(
    output["pull_request"],
    json!({"number": 7, "url": "https://github.com/acme/monorepo/pull/7", "created": true, "comment_url": null, "reply": null, "assignees": ["me"]})
  );
  assert_eq!((output["robot"].clone(), space.fake.starts.lock().unwrap().len()), (Value::Null, 0));
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
  assert_eq!(next_steps(&output["error"]), [], "the approvals come in the open pull request");

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
  assert_eq!(space.fake.starts.lock().unwrap().len(), 0);

  *space.fake.pull_request_review.lock().unwrap() = Some(evaluated_review(true, "APPROVED"));
  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["review"]["readiness"], json!("approved"));
  assert_eq!(output["review"]["approved_on_github"], json!(["ikorennoy"]));
  assert_eq!(output["started_via"], json!("pull_request"));
}

/// auth with its open pull request #7, approved by Ilya Korennoy, whose approval Patronus counts when `counted` (else
/// the changes need no approval); the rules of master answer `rules`; never pushed, so a start pushes new commits
fn approved_pull_request_with_rules(counted: bool, rules: FakeAnswer) -> (Space, Arc<FakeGitHub>, Environment) {
  let space = Space::new(&["(auth) Add auth"]);
  if counted {
    space.fake.review_required.store(true, Ordering::SeqCst);
    *space.fake.pull_request_review.lock().unwrap() = Some(evaluated_review(true, "APPROVED"));
  }
  let (github, url) = fake_github(&space, &[pull_request_json(&["ikorennoy"])]);
  github.answer("GET", MASTER_RULES, rules);
  let environment = reviewed_on_github(&space, &url);
  (space, github, environment)
}

/// The comments the start posted on pull request #7
fn comments_on_seven(github: &FakeGitHub) -> usize {
  github.requests_to("POST", "/repos/acme/monorepo/issues/7/comments").len()
}

/// `bd safe-push <args>`, which starts from a comment on pull request #7
fn started_from_the_pull_request(environment: &Environment, args: &[&str]) -> Value {
  let (exit_code, output) = safe_push(environment, args);
  assert_eq!((exit_code, &output["started_via"]), (0, &json!("pull_request")), "{output}");
  output
}

/// The approvals Patronus counts would be gone with the push: the robot would start without them and fail, so the
/// Safe Push is refused before anything is pushed, as the app's popover refuses it, with the way out
#[test_log::test]
fn a_push_that_dismisses_the_approvals_patronus_counts_refuses_the_safe_push() {
  let (space, github, environment) = approved_pull_request_with_rules(true, FakeAnswer::ok(dismissing_rules()));

  let (exit_code, output) = safe_push(&environment, &["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(refusal_codes(&output), vec![json!("push_dismisses_approvals")]);
  assert_eq!(output["rules"]["dismiss_stale_reviews_on_push"], json!(true));
  assert_eq!(output["review"]["readiness"], json!("approved"));

  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "push_dismisses_approvals"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("pull request #7"), "{message}");
  assert!(message.contains("`bd push auth`") && message.contains("`bd request-review auth`"), "{message}");
  assert_eq!(
    next_steps(&output["error"]),
    [("bd push auth".to_string(), true), ("bd request-review auth".to_string(), true)]
  );
  assert_eq!(space.remote_tip(AUTH), "", "nothing is pushed");
  assert_eq!(comments_on_seven(&github), 0, "nothing is started");
  // one GET of the rules of the target per run
  assert_eq!(github.requests_to("GET", MASTER_RULES).len(), 2);
}

/// A push that dismisses approvals Patronus doesn't count (the changes need none) takes nothing the robot needs: it
/// starts, and `rules` says what the push takes away
#[test_log::test]
fn a_push_that_dismisses_approvals_patronus_does_not_count_starts_and_says_so() {
  let (_space, github, environment) = approved_pull_request_with_rules(false, FakeAnswer::ok(dismissing_rules()));

  let output = started_from_the_pull_request(&environment, &["auth"]);
  assert_eq!(output["rules"]["dismiss_stale_reviews_on_push"], json!(true));
  let message = output["rules"]["message"].as_str().unwrap();
  assert!(message.contains("The push dismisses the approvals of pull request #7"), "{message}");
  assert_eq!(comments_on_seven(&github), 1);
}

/// Rules GitHub doesn't answer are never "keeps the approvals": the start is refused unless the user said to start
/// anyway, and then `rules` says what is not known
#[test_log::test]
fn rules_github_does_not_answer_refuse_the_safe_push_unless_allowed() {
  let (space, github, environment) = approved_pull_request_with_rules(true, FakeAnswer::status(500, json!({"message": "Server Error"})));

  let (exit_code, output) = safe_push(&environment, &["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(refusal_codes(&output), vec![json!("rules_unknown")]);
  assert_eq!(output["rules"]["dismiss_stale_reviews_on_push"], Value::Null);

  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "rules_unknown"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("--allow-unknown-rules starts anyway only when the user says so"), "{message}");
  assert_eq!(next_steps(&output["error"]), [], "a suppression is never a step");
  assert_eq!(space.remote_tip(AUTH), "", "nothing is pushed");
  assert_eq!(comments_on_seven(&github), 0, "nothing is started");

  let output = started_from_the_pull_request(&environment, &["auth", "--allow-unknown-rules"]);
  assert_eq!(output["rules"]["dismiss_stale_reviews_on_push"], Value::Null);
  let message = output["rules"]["message"].as_str().unwrap();
  assert!(message.starts_with("Cannot check whether the push keeps the approvals of pull request #7"), "{message}");
}

/// Rules that keep the approvals start; the rules are asked only for a push of new commits into an approved pull request
/// the robot starts from, never for a pull request nobody approved, a dry run, `--create-pull-request` or a start
/// without a push: with rules GitHub can't answer, each of those starts without a read
#[test_log::test]
fn the_rules_are_read_only_for_a_push_into_an_approved_pull_request() {
  let (_space, github, environment) = approved_pull_request_with_rules(true, FakeAnswer::ok(json!([])));
  let output = started_from_the_pull_request(&environment, &["auth"]);
  assert_eq!(output["rules"]["dismiss_stale_reviews_on_push"], json!(false));
  assert_eq!(github.requests_to("GET", MASTER_RULES).len(), 1);

  let space = Space::new(&["(auth) Add auth"]);
  let (github, url) = fake_github(&space, &[pull_request_json(&[])]);
  let environment = reviewed_on_github(&space, &url);
  let output = started_from_the_pull_request(&environment, &["auth"]);
  assert_eq!(output.get("rules"), None);
  assert!(github.requests_to("GET", MASTER_RULES).is_empty());

  let not_read = |output: &Value, github: &FakeGitHub| {
    assert_eq!(output.get("rules"), None, "{output}");
    assert!(github.requests_to("GET", MASTER_RULES).is_empty(), "the rules were read");
  };
  let unknown_rules = || approved_pull_request_with_rules(true, FakeAnswer::status(500, json!({"message": "Server Error"})));
  // a dry run lands nothing: the approvals don't matter to it
  let (_space, github, environment) = unknown_rules();
  let (exit_code, output) = safe_push(&environment, &["auth", "--patronus-dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  not_read(&output, &github);
  // --create-pull-request: the pull request gets its approvals after this push
  let (_space, github, environment) = unknown_rules();
  not_read(&create_pull_request(&environment), &github);
  // a start without a push dismisses nothing
  let (_space, github, environment) = unknown_rules();
  let (exit_code, output) = run_json(&environment, &["--prefix", PREFIX, "push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let output = started_from_the_pull_request(&environment, &["auth"]);
  assert_eq!(output["push_needed"], json!(false));
  not_read(&output, &github);
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
  // the search does not list it yet (a retry after a timeout, the app at the same time): the REST list of the head does
  github.answer("POST", "/graphql", search_answer(&[]));
  github.answer(
    "GET",
    "/repos/acme/monorepo/pulls",
    FakeAnswer::ok(json!([{"number": 7, "html_url": "https://github.com/acme/monorepo/pull/7", "title": "Add auth",
      "head": {"ref": AUTH, "sha": "1111"}, "base": {"ref": "master"}}])),
  );
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
      "fake-agent removes CONTROL_URL_ENV in {removed}, and {ACP_HOST}:7 still uses it: {fixed} changes that use in another branch, so the build fails. To fix it, move {fixed} into fake-agent: it changes only {ACP_HOST}; --allow-dangling starts anyway, only when the user says it is a false alarm",
      removed = &removed_by[..8],
      fixed = &fixed_by[..8],
    ))
  );
  assert_eq!(space.remote_tip("test-user/virtual/fake-agent"), "", "nothing was pushed");
  // the fix moves a commit into another branch, which the message names: no command proven to run
  assert_eq!(next_steps(&output["error"]), []);

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

/// The Safe Push failure of 2026-10-21 is refused before anything is pushed: `flow` imports a function that `util`, a
/// branch outside what lands, adds; git merges it, the build fails. Stacked on `util`, `flow` lands it too: no refusal
#[test_log::test]
fn a_name_another_branch_adds_refuses_the_safe_push_unless_allowed() {
  let space = Space::new(&[]);
  git(&space.repo, &["config", "branchdeck.stacks", "true"]);
  write(&space.repo, "src/a/Util.kt", "package a\n\nfun controlUrl(): String = \"\"\n");
  let declares = commit_all(&space.repo, "(util) Add controlUrl", 1_700_000_100);
  write(&space.repo, "src/b/Use.kt", "package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n");
  let uses = commit_all(&space.repo, "(flow) Start with controlUrl", 1_700_000_200);

  let (exit_code, output) = space.run(&["flow"]);
  assert_error(&output, exit_code, (4, "missing_declaration"));
  assert_eq!(
    output["error"]["message"],
    json!(format!(
      "Nothing was pushed or started: {used} (Start with controlUrl) of flow uses controlUrl (src/b/Use.kt:3), which {declared} (Add controlUrl) of util adds; git merges it, but the build fails. `bd restack flow util` lands util with it, after the user approves, or move one of the two; --allow-missing starts anyway, only when the user says it is a false alarm",
      used = &uses[..8],
      declared = &declares[..8],
    ))
  );
  assert_eq!(next_steps(&output["error"]), [("bd restack flow util".to_string(), true)]);
  assert_eq!(space.remote_tip("test-user/virtual/flow"), "", "nothing was pushed");

  let (_, output) = space.run(&["flow", "--check"]);
  assert!(refusal_codes(&output).contains(&json!("missing_declaration")), "{output}");
  let (_, output) = space.run(&["flow", "--check", "--allow-missing"]);
  assert!(!refusal_codes(&output).contains(&json!("missing_declaration")), "{output}");

  for args in [&["restack", "flow", "util"][..], &["sync"]] {
    let (exit_code, output) = run_in(&space.repo, args);
    assert_eq!(exit_code, 0, "{output}");
  }
  let (_, output) = space.run(&["flow", "--check"]);
  assert!(!refusal_codes(&output).contains(&json!("missing_declaration")), "{output}");
  assert_eq!(output["lands"], json!(["util", "flow"]), "{output}");
}

/// `session` uses a name its sibling `login` adds: the robot of the whole stack lands both, so it refuses nothing; the
/// Safe Push of `session` alone lands `auth` and `session` without it, and a restack of `auth` on `login` would close a
/// cycle, so the fix is the move of the commit
#[test_log::test]
fn a_name_a_sibling_adds_refuses_the_branch_alone_but_not_the_whole_stack() {
  let space = Space::new(&["(auth) Add auth"]);
  git(&space.repo, &["config", "branchdeck.stacks", "true"]);
  write(&space.repo, "src/a/Util.kt", "package a\n\nfun controlUrl(): String = \"\"\n");
  let declares = commit_all(&space.repo, "(auth/login) Add controlUrl", 1_700_000_100);
  write(&space.repo, "src/b/Use.kt", "package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n");
  commit_all(&space.repo, "(auth/session) Start with controlUrl", 1_700_000_200);

  let (_, output) = space.run(&["session", "--whole-stack", "--check"]);
  assert_eq!(output["lands"], json!(["auth", "login", "session"]), "{output}");
  assert!(!refusal_codes(&output).contains(&json!("missing_declaration")), "{output}");

  let (_, output) = space.run(&["session", "--check"]);
  let refusal = output["refusals"]
    .as_array()
    .and_then(|refusals| refusals.iter().find(|refusal| refusal["code"] == json!("missing_declaration")))
    .unwrap_or_else(|| panic!("{output}"));
  let message = refusal["message"].as_str().unwrap();
  assert!(
    message.contains(&format!("Move {} into session; --allow-missing starts anyway, only when", &declares[..8])),
    "{output}"
  );
}

#[test_log::test]
fn push_refuses_a_branch_whose_safe_push_runs_and_restarts_it_when_told() {
  let space = login_on_auth();
  let push = |args: &[&str]| bd_push(&space.environment(), args);

  space.set_robots(vec![robot("running", "RUNNING", "auth", "0000")]);
  let (exit_code, output) = push(&["auth"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  // the push would move the branch the robot took its commit from
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.contains(&format!("A Safe Push of auth is running on {AUTH}: a push moves the branch it took its commit from")),
    "{output}"
  );
  assert!(message.contains("once the user approved, --restart-running cancels it, pushes and restarts it"), "{output}");
  assert_eq!(
    next_steps(&output["error"]),
    [("bd robot auth".to_string(), false), ("bd push auth --restart-running".to_string(), true)]
  );
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
  assert_eq!(space.fake.restarts.lock().unwrap().len(), 1);
  assert_eq!(output["restarted"].as_array().map(Vec::len), Some(1), "{output}");
  assert_eq!(space.remote_tip(AUTH), space.local_tip(AUTH));

  // a canceling or a dry-run robot holds nothing
  space.set_robots(vec![robot("canceling", "CANCELING", "login", "0000")]);
  let (exit_code, output) = push(&["login"]);
  assert_eq!(exit_code, 0, "{output}");
}

/// The refusals of `bd safe-push --check` by their codes
fn refusal_codes(output: &Value) -> Vec<Value> {
  output["refusals"]
    .as_array()
    .map_or_default(|refusals| refusals.iter().map(|refusal| refusal["code"].clone()).collect())
}

/// A new branch `env`, made after the robot started, restacked below `below` and synced
fn restack_env_below(space: &Space, below: &str) {
  write(&space.repo, "env.txt", "env\n");
  commit_all(&space.repo, "(env) Add env", 1_700_000_900);
  for args in [&["restack", below, "env"][..], &["sync"]] {
    let (exit_code, output) = run_in(&space.repo, args);
    assert_eq!(exit_code, 0, "{output}");
  }
}

/// The id of the commit of the checked-out branch with `subject`
fn commit_of(space: &Space, subject: &str) -> String {
  git(&space.repo, &["log", "-1", "--format=%H", "--fixed-strings", &format!("--grep={subject}")])
}

/// A restack after the start of a Safe Push placed a new root below its branch: the robot's commit has none of its
/// commits, so its push and its Safe Push go on; the robot's own branch stays held, the refusal names the branch the push
/// would move
#[test_log::test]
fn a_push_of_a_parent_restacked_below_a_running_safe_push_goes_on() {
  let space = Space::new(&["(auth) Add auth"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  let (exit_code, output) = run_in(&space.repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  space.set_robots(vec![robot("auth-robot", "RUNNING", "auth", &space.local_tip(AUTH))]);
  restack_env_below(&space, "auth");
  let env = "test-user/virtual/env";

  let (exit_code, output) = space.run(&["env", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert!(!refusal_codes(&output).contains(&json!("robot_running")), "{output}");
  let (exit_code, output) = bd_push(&space.environment(), &["env", "--alone"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(space.remote_tip(env), space.local_tip(env));

  let (exit_code, output) = bd_push(&space.environment(), &["auth"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert!(
    output["error"]["message"]
      .as_str()
      .unwrap()
      .contains(&format!("running on {AUTH}: a push moves the branch")),
    "{output}"
  );
  assert_eq!(space.remote_tip(AUTH), "");
}

/// A parent whose commits a running Safe Push of a branch built on it lands stays held, also with `--alone` and after a
/// restack placed a new root below it; the refusal names the commits. The new root goes on
#[test_log::test]
fn a_push_of_a_parent_whose_commits_a_running_safe_push_lands_is_refused() {
  let space = login_on_auth();
  let (exit_code, output) = run_in(&space.repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  space.set_robots(vec![robot("login-robot", "RUNNING", "login", &space.local_tip("test-user/virtual/login"))]);
  restack_env_below(&space, "auth");

  let auth = commit_of(&space, "Add auth");
  for args in [&["auth"][..], &["auth", "--alone"]] {
    let (exit_code, output) = bd_push(&space.environment(), args);
    assert_error(&output, exit_code, (4, "robot_running"));
    let message = output["error"]["message"].as_str().unwrap();
    assert!(
      message.contains(&format!(
        "A Safe Push of login is running: it lands 1 commit of auth with it ({}), a push won't land",
        &auth[..8]
      )),
      "{output}"
    );
  }
  assert_eq!(space.remote_tip(AUTH), "");
  let (exit_code, output) = space.run(&["auth", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  assert!(refusal_codes(&output).contains(&json!("robot_running")), "{output}");

  let (exit_code, output) = bd_push(&space.environment(), &["env", "--alone"]);
  assert_eq!(exit_code, 0, "{output}");
}

/// A Safe Push of a temporary ref (the IDE, the CLI) at the pushed tip of a branch holds only what its commit lands: a
/// root a restack placed below after it started goes on
#[test_log::test]
fn a_robot_of_a_temporary_ref_holds_only_what_its_commit_lands() {
  let space = login_on_auth();
  let login = "test-user/virtual/login";
  let (exit_code, output) = bd_push(&space.environment(), &["login"]);
  assert_eq!(exit_code, 0, "{output}");
  let mut temporary = robot_json("ide-robot", "RUNNING", "refs/heads/tmp/patronus-1", &minutes_ago(60));
  temporary["sourceRevision"] = json!(space.remote_tip(login));
  space.set_robots(vec![temporary]);
  restack_env_below(&space, "auth");
  assert_ne!(space.local_tip(login), space.remote_tip(login), "login was rebuilt on env");

  let (exit_code, output) = bd_push(&space.environment(), &["env", "--alone"]);
  assert_eq!(exit_code, 0, "{output}");
  let (exit_code, output) = bd_push(&space.environment(), &["auth", "--alone"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert!(output["error"]["message"].as_str().unwrap().contains("it lands 1 commit of auth with it"), "{output}");
}

/// The Safe Push of the whole stack runs on its stack branch: once the branches are no longer what it took (rewritten
/// after the start), their push goes on, and Push with Children leaves the stack branch as it is: a push into it fails
/// the robot at the end
#[test_log::test]
fn push_with_children_leaves_the_stack_branch_of_a_running_whole_stack_safe_push() {
  let space = forking_stack();
  let (exit_code, output) = space.run(&["session", "--whole-stack"]);
  assert_eq!(exit_code, 0, "{output}");
  let stack_tip = space.local_tip(STACK);
  space.set_robots(vec![stack_robot("whole", "RUNNING", &stack_tip)]);
  // every branch again with other changes: the robot's commit has none of their commits
  git(&space.repo, &["reset", "--hard", "origin/master"]);
  for (index, subject) in ["(auth) Add the auth module", "(auth/login) Add the login form", "(auth/session) Add the session store"]
    .iter()
    .enumerate()
  {
    write(&space.repo, &format!("f{index}.txt"), format!("{subject} v2"));
    commit_all(&space.repo, subject, 1_700_000_000 + i64::try_from(index).unwrap() * 100);
  }

  let (exit_code, output) = bd_push(&space.environment(), &["auth", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("auth"), &json!("login"), &json!("session")]);
  let warning = output["stack_warnings"][0].as_str().unwrap();
  assert!(
    warning.starts_with(&format!(
      "{STACK} was not pushed: a Safe Push of the whole auth stack is running on it: a push moves the branch"
    )),
    "{output}"
  );
  assert_eq!(space.remote_tip(STACK), stack_tip);
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
  let (_, schema) = run_json(&environment, &["schema", "--all"]);
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
  let (_, schema) = run_json(&environment, &["schema", "--all"]);
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

/// One open pull request per branch: `--create-pull-request` takes the one the branch has into a virtual branch and
/// retargets it to the target; one into another base counts the approvals for no Safe Push into the target, so it
/// refuses with `pull_request_other_base` (D3b), the retarget its only next step
#[test_log::test]
fn create_pull_request_retargets_the_pull_request_of_a_virtual_branch_and_refuses_one_into_another_base() {
  let (_space, github, environment) = needs_review_on_github();
  list_pull_requests(&github, &[pull_request_of("auth", 7, "release-252", &[])]);
  let (exit_code, output) = safe_push(&environment, &["auth", "--create-pull-request"]);
  assert_error(&output, exit_code, (4, "pull_request_other_base"));
  assert_eq!(next_steps(&output["error"]), [("gh pr edit 7 --repo acme/monorepo --base master".to_string(), true)]);
  assert!(github.requests_to("POST", "/repos/acme/monorepo/pulls").is_empty());

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

/// D3b: Patronus merges a pull request into its base, so the approvals of one into another base (a release branch)
/// count for no Safe Push into the target: a Safe Push that counts them refuses `pull_request_other_base` with the
/// retarget as its next step (the message names `--without-approvals`), never retargeting it silently; a Dry Run counts none and
/// starts, and so does a Safe Push without approvals, both through the API
#[test_log::test]
fn a_pull_request_into_another_base_refuses_only_a_safe_push_whose_approvals_count() {
  let (space, github, environment) = needs_review_on_github();
  list_pull_requests(&github, &[pull_request_of("auth", 7, "release-252", &[])]);
  let (exit_code, output) = safe_push(&environment, &["auth"]);
  assert_error(&output, exit_code, (4, "pull_request_other_base"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("pull request #7 of auth goes into release-252"), "{message}");
  assert_eq!(next_steps(&output["error"]), [("gh pr edit 7 --repo acme/monorepo --base master".to_string(), true)]);
  assert!(github.requests_to("PATCH", "/repos/acme/monorepo/pulls/7").is_empty());
  assert_eq!(space.fake.starts.lock().unwrap().len(), 0);

  for args in [&["auth", "--patronus-dry-run"][..], &["auth", "--without-approvals"]] {
    let (exit_code, output) = safe_push(&environment, args);
    assert_eq!(exit_code, 0, "{args:?}: {output}");
    assert_eq!(output["started_via"], json!("api"), "{args:?}");
  }
  assert!(github.requests_to("PATCH", "/repos/acme/monorepo/pulls/7").is_empty());
}

/// `--whole-stack` of any branch of a stack that forks lands every branch of it in one robot, from the stack branch of
/// its root, the only branch pushed. Both ways the robots wait for each other: the robot of the whole stack lands every
/// branch of it, the robot of a branch a part of it
#[test_log::test]
fn the_whole_stack_lands_in_one_robot_from_its_stack_branch() {
  let space = forking_stack();
  let (exit_code, output) = space.run(&["login", "--whole-stack", "--check"]);
  assert_eq!(exit_code, 0, "{output}");
  let (_, schema) = run_json(&space.environment(), &["schema", "--all"]);
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
  assert_eq!(space.fake.starts.lock().unwrap().len(), 1, "one robot");

  // the robot of the whole stack lands every branch of it: a start or a push of a branch waits for it, and a start of
  // the whole stack on the same commits too
  space.set_robots(vec![stack_robot("whole", "RUNNING", &stack_tip)]);
  let (exit_code, output) = space.run(&["login"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.starts_with("A Safe Push of the whole auth stack is running: it lands 1 commit of login with it ("),
    "{output}"
  );
  let (exit_code, output) = space.run(&["auth", "--whole-stack"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  assert_eq!(output["error"]["message"], json!("A Safe Push of the last commit is running"));
  let (exit_code, output) = run_json(&space.environment(), &["--prefix", PREFIX, "push", "session"]);
  assert_error(&output, exit_code, (4, "robot_running"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.contains("A Safe Push of the whole auth stack is running: it lands 1 commit of session with it ("),
    "{output}"
  );

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
  // the branch of the commit that conflicts lands with what it is built on: its check reads only
  let piece = [("bd safe-push session --check".to_string(), false)];
  assert_eq!(next_steps(&output["refusals"][0]), piece);
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
  assert_eq!(next_steps(&output["error"]), piece);
  assert_eq!((space.remote_tip(STACK), space.fake.starts.lock().unwrap().len()), (String::new(), 0));

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
  assert_eq!(next_steps(&output["error"]), [("bd sync".to_string(), false)]);
  assert_eq!(space.fake.starts.lock().unwrap().len(), 0);
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
  assert_eq!((space.fake.starts.lock().unwrap().len(), space.remote_tip(AUTH)), (0, String::new()));
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
