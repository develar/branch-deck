use crate::agent_help_test::writable_state;
use crate::fixture_test::assert_covered;
use crate::fixture_test::{PREFIX, assert_error, commit_all, environment, git, run_json, write};
use crate::safe_push_test::{AUTH, STACK, Space, forking_stack, robot, stack_robot};
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::fake_executable::fake_executable;

impl Space {
  /// `bd robot` with `args`
  fn robot(&self, args: &[&str]) -> (i32, Value) {
    let mut all_args = vec!["--prefix", PREFIX, "robot"];
    all_args.extend_from_slice(args);
    run_json(&self.environment(), &all_args)
  }

  /// `bd robot` with `args`, which succeeds with an output of its schema
  fn robot_ok(&self, args: &[&str]) -> Value {
    let (exit_code, output) = self.robot(args);
    assert_eq!(exit_code, 0, "{output}");
    assert_schema(self, &output);
    output
  }

  fn sync(&self) {
    let (exit_code, output) = run_json(&self.environment(), &["--prefix", PREFIX, "sync"]);
    assert_eq!(exit_code, 0, "{output}");
  }

  /// A robot of the list and, as `GET /robots/{id}` answers for it, with the Large Scale Change profile, rebased onto
  /// `bbb222` and built as `ccc111`
  fn add_robot(&self, id: &str, status: &str, name: &str, revision: &str) {
    self.add_listed_robot(id, robot(id, status, name, revision));
  }

  /// `add_robot` of the robot `listed` as the list has it
  fn add_listed_robot(&self, id: &str, listed: Value) {
    let status = listed["status"].as_str().unwrap_or_default().to_string();
    let mut details = listed.clone();
    details["options"] = json!({"patronus.merge.profile.id": "lsc", "patronus.merge.profile.name": "Large Scale Change"});
    details["testReferenceRevision"] = json!("ccc111");
    details["targetRevision"] = json!("bbb222");
    if status != "RUNNING" {
      details["finishDateTime"] = json!("2026-10-14T10:30:00Z");
    }
    self.fake.details.lock().unwrap().insert(id.to_string(), details);
    let mut robots = self.fake.robots.lock().unwrap().take().unwrap_or_default();
    robots.push(listed);
    *self.fake.robots.lock().unwrap() = Some(robots);
  }
}

/// The output matches the schema of `bd robot`
fn assert_schema(space: &Space, output: &Value) {
  let (_, schema) = run_json(&environment(space.repo.path()), &["schema"]);
  assert_covered(&schema["outputs"]["robot"], &schema["outputs"]["robot"], output, "robot");
}

/// `(commit, state)` of each commit of the map
fn commit_states(output: &Value) -> Vec<(Value, Value)> {
  output["commits"]
    .as_array()
    .unwrap_or_else(|| panic!("no commits in {output}"))
    .iter()
    .map(|commit| (commit["commit"].clone(), commit["state"].clone()))
    .collect()
}

#[test_log::test]
fn a_failed_robot_says_why_and_which_local_commits_it_tested() {
  let space = Space::new(&["(auth) Add auth", "(auth) Add login"]);
  space.sync();
  let tested = space.local_tip(AUTH);
  space.add_robot("finished", "FAILURE", "auth", &tested);
  // a fix since the robot ran, synced
  write(&space.repo, "fix.txt", "fix");
  commit_all(&space.repo, "(auth) Fix login", 1_700_000_900);
  space.sync();
  let before = writable_state(&space.repo);

  let output = space.robot_ok(&["auth"]);
  assert_eq!(writable_state(&space.repo), before, "bd robot is read only");
  assert_eq!(
    output["robot"],
    json!({
      "id": "finished", "name": "IJPL-1 fix the cache", "state": "failure", "mode": "safe_push", "commit": short_id(&tested),
      "url": output["robot"]["url"], "running": false, "profile": {"id": "lsc", "name": "Large Scale Change"}, "target": "master",
      "started_at": 1_791_970_200, "finished_at": 1_791_973_800,
    })
  );
  let failure = &output["failure"];
  assert_eq!(failure["kind"], json!("checks"));
  assert_eq!((failure["test_revision"].clone(), failure["target_revision"].clone()), (json!("ccc111"), json!("bbb222")));
  // a finished robot: only the failed checks, by their last attempt
  assert_eq!(
    failure["checks"],
    json!([{"name": "Fast Tests", "build_configuration_id": "a", "attempt": 1, "attempt_limit": 1, "build_id": "100",
      "build_url": "https://ci.example/build/100", "failed_tests": 3, "failed_builds": 2, "failed_to_start": 0}])
  );
  assert_eq!(
    failure["problems"],
    json!([{"title": "Changes require review approval", "subsystem": "OWNERSHIP_REVIEW_APPROVAL_CHECK"}])
  );
  assert_eq!(failure.get("builds"), None, "builds only with --full");

  // by the ids of the checked-out branch, newest first: the fix is not tested
  let head: Vec<String> = git(&space.repo, &["rev-list", "--max-count=3", "HEAD"]).lines().map(|id| id[..8].to_string()).collect();
  assert_eq!(
    commit_states(&output),
    [(json!(head[0]), json!("added")), (json!(head[1]), json!("tested")), (json!(head[2]), json!("tested"))]
  );
  assert_eq!(output["commits"][0]["subject"], json!("(auth) Fix login"));
  assert_eq!(output["head_branch"], json!("master"));
  assert_eq!((output.get("source_missing"), output.get("virtual_behind_head")), (None, None));

  // a commit the virtual branch doesn't have yet is not in the map; --full-ids prints full ids
  write(&space.repo, "later.txt", "later");
  let later = commit_all(&space.repo, "(auth) Later", 1_700_001_000);
  let (exit_code, output) = space.robot(&["auth", "--full-ids"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["virtual_behind_head"], json!(true));
  let commits = commit_states(&output);
  assert!(commits.iter().all(|(commit, _)| *commit != json!(later)), "{output}");
  assert_eq!(commits[0].0.as_str().unwrap().len(), 40, "{output}");
}

#[test_log::test]
fn full_adds_the_failed_builds_their_tests_and_the_log_tails_of_the_teamcity_cli() {
  let space = Space::new(&["(auth) Add auth"]);
  space.sync();
  space.add_robot("finished", "FAILURE", "auth", &space.local_tip(AUTH));
  let teamcity = fake_executable(
    space.home.path(),
    "teamcity",
    r#"case "$1 $2" in
  "auth status") echo '[{"server": "https://ci.example", "status": "authenticated"}]' ;;
  "run log") echo "Compilation failed: Foo.kt:12" ;;
  "run tests") echo '{"testOccurrence": [{"name": "com.intellij.FooTest.testBar", "firstFailed": {"build": {"id": 290}}}]}' ;;
esac"#,
  );
  let environment = bd_core::context::Environment {
    teamcity: Some(teamcity),
    ..space.environment()
  };

  let (exit_code, output) = run_json(&environment, &["--prefix", PREFIX, "robot", "auth", "--full"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_schema(&space, &output);
  let builds = output["failure"]["builds"].as_array().unwrap_or_else(|| panic!("no builds in {output}"));
  // the composite aggregator is left out, the compilation comes first, the ignored problem and test are left out
  assert_eq!(
    builds.iter().map(|build| build["name"].clone()).collect::<Vec<_>>(),
    [json!("Compile"), json!("Unit Tests")]
  );
  assert_eq!(
    builds[0]["problems"],
    json!([{"kind": "TC_COMPILATION_ERROR", "details": "Compilation error: Foo.kt:12: unresolved reference: bar"}])
  );
  assert!(builds[0]["log_tail"].as_str().unwrap().contains("Compilation failed: Foo.kt:12"), "{output}");
  assert_eq!(builds[1].get("log_tail"), None, "a build that failed only on tests has no log tail");
  let tests = builds[1]["failed_tests"].as_array().unwrap();
  assert_eq!(
    tests
      .iter()
      .map(|test| (test["name"].clone(), test.get("first_failed_before").cloned()))
      .collect::<Vec<_>>(),
    [(json!("com.intellij.FooTest.testBar"), Some(json!(true))), (json!("com.intellij.FooTest.testBaz"), None)]
  );
  assert!(tests[0]["message"].as_str().unwrap().starts_with("expected: <true> but was: <false>"), "{output}");
  assert_eq!(output["failure"]["omitted"], json!({"builds": 0, "tests": 0, "messages": 0, "truncated": false}));
}

#[test_log::test]
fn a_running_robot_whose_required_check_failed_is_failing() {
  let space = Space::new(&["(auth) Add auth"]);
  space.sync();
  // the fake answers `robot_is_failing` for the robot `running`
  space.add_robot("running", "RUNNING", "auth", &space.local_tip(AUTH));

  let output = space.robot_ok(&["auth"]);
  assert_eq!((output["robot"]["state"].clone(), output["robot"]["running"].clone()), (json!("running"), json!(true)));
  assert_eq!(output["robot"].get("finished_at"), None);
  // the running check whose attempt has a failed build is listed, running
  let checks: Vec<(Value, Option<Value>)> = output["failure"]["checks"]
    .as_array()
    .unwrap()
    .iter()
    .map(|check| (check["name"].clone(), check.get("running").cloned()))
    .collect();
  assert_eq!(checks, [(json!("Fast Tests"), None), (json!("Unit Tests"), Some(json!(true)))]);
  assert_eq!(commit_states(&output), [(output["commits"][0]["commit"].clone(), json!("tested"))]);
}

#[test_log::test]
fn a_successful_robot_has_no_failure_and_a_branch_without_one_has_no_robot() {
  let space = Space::new(&["(auth) Add auth"]);
  space.sync();
  space.set_robots(Vec::new());
  let output = space.robot_ok(&["auth"]);
  assert_eq!(output["robot"], Value::Null);
  assert_eq!((output.get("failure"), output.get("commits")), (None, None));

  space.add_robot("done", "SUCCESS", "auth", &space.local_tip(AUTH));
  let output = space.robot_ok(&["auth"]);
  assert_eq!(
    (output["robot"]["id"].clone(), output["robot"]["state"].clone(), output["robot"]["running"].clone()),
    (json!("done"), json!("success"), json!(false))
  );
  assert_eq!((output.get("failure"), output.get("commits"), output.get("head_branch")), (None, None, None));

  let (exit_code, output) = space.robot(&["nope"]);
  assert_error(&output, exit_code, (2, "unknown_branch"));
}

#[test_log::test]
fn robot_takes_a_robot_by_id_of_the_branch_only() {
  let space = Space::new(&["(auth) Add auth", "(login) Add login"]);
  space.sync();
  space.add_robot("finished-old", "FAILURE", "auth", &space.local_tip(AUTH));
  space.add_robot("running-new", "RUNNING", "auth", &space.local_tip(AUTH));
  space.add_robot("finished-login", "FAILURE", "login", &space.local_tip("test-user/virtual/login"));

  // a running robot wins
  let (_, output) = space.robot(&["auth"]);
  assert_eq!(output["robot"]["id"], json!("running-new"), "{output}");

  let (exit_code, output) = space.robot(&["auth", "--robot", "finished-old"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["robot"]["id"].clone(), output["failure"]["kind"].clone()), (json!("finished-old"), json!("checks")));

  let (exit_code, output) = space.robot(&["auth", "--robot", "finished-login"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(output["error"]["message"].as_str().unwrap().contains("bd robot login --robot finished-login"), "{output}");
}

#[test_log::test]
fn robot_without_a_token_or_space_asks_nothing() {
  let space = Space::new(&["(auth) Add auth"]);
  let environment = bd_core::context::Environment {
    home: Some(space.repo.path().join("no-home")),
    ..space.environment()
  };
  let (exit_code, output) = run_json(&environment, &["--prefix", PREFIX, "robot", "auth"]);
  assert_error(&output, exit_code, (4, "not_logged_in"));

  git(&space.repo, &["remote", "set-url", "origin", space.repo.path().to_str().unwrap()]);
  let (exit_code, output) = space.robot(&["auth"]);
  assert_error(&output, exit_code, (4, "not_on_space"));
  assert_eq!(space.fake.list_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test_log::test]
fn a_newer_robot_of_a_stacked_branch_tells_the_parent_with_the_commits_of_the_whole_stack() {
  let space = Space::new(&["(auth) Add auth", "(auth/login) Add form"]);
  git(&space.repo, &["config", "branchdeck.stacks", "true"]);
  space.sync();
  space.add_robot("finished-own", "FAILURE", "auth", &space.local_tip(AUTH));
  // the Safe Push of the child built the parent too, an hour later
  space.add_robot("finished-child", "FAILURE", "login", &space.local_tip("test-user/virtual/login"));
  for robot in space.fake.robots.lock().unwrap().iter_mut().flatten().filter(|robot| robot["id"] == "finished-child") {
    robot["startDateTime"] = json!("2026-10-14T10:30:00Z");
  }
  space.fake.details.lock().unwrap().get_mut("finished-child").unwrap()["startDateTime"] = json!("2026-10-14T10:30:00Z");

  let output = space.robot_ok(&["auth"]);
  assert_eq!((output["robot"]["id"].clone(), output["robot_branch"].clone()), (json!("finished-child"), json!("login")));
  assert_eq!(commit_branches(&output), [(json!("login"), json!("tested")), (json!("auth"), json!("tested"))]);

  // the child's own robot is its own
  let output = space.robot_ok(&["login"]);
  assert_eq!((output["robot"]["id"].clone(), output.get("robot_branch")), (json!("finished-child"), None));
  // the parent's robot by id, as the child's is
  let output = space.robot_ok(&["auth", "--robot", "finished-own"]);
  assert_eq!(output.get("robot_branch"), None);
}

/// The robot of the whole stack runs on the stack branch of its root: `--whole-stack` of any branch of the stack finds
/// it, and a newer one wins for a branch of the stack, as a newer robot of a branch stacked on it does, named by
/// `stack_root`; the stack branch has no mapping notes, so its failure maps no commits
#[test_log::test]
fn the_robot_of_the_whole_stack_is_found_from_any_branch_of_it() {
  let space = forking_stack();
  space.sync();
  space.add_robot("own", "FAILURE", "login", &space.local_tip("test-user/virtual/login"));
  let mut whole = stack_robot("whole", "FAILURE", &space.local_tip(STACK));
  // an hour after the branch's own
  whole["startDateTime"] = json!("2026-10-14T10:30:00Z");
  space.add_listed_robot("whole", whole);

  let output = space.robot_ok(&["login"]);
  assert_eq!(
    (output["robot"]["id"].clone(), output["stack_root"].clone(), output.get("robot_branch")),
    (json!("whole"), json!("auth"), None)
  );
  // the robot built the commits of every branch of the stack, each named with its branch
  assert_eq!(
    (output["failure"]["kind"].clone(), commit_branches(&output)),
    (
      json!("checks"),
      vec![(json!("session"), json!("tested")), (json!("login"), json!("tested")), (json!("auth"), json!("tested"))]
    )
  );
  let output = space.robot_ok(&["session", "--whole-stack"]);
  assert_eq!((output["robot"]["id"].clone(), output["stack_root"].clone()), (json!("whole"), json!("auth")));
  // the branch's own robot by id, and the whole stack's for a branch of it
  let output = space.robot_ok(&["login", "--robot", "own"]);
  assert_eq!((output["robot"]["id"].clone(), output.get("stack_root")), (json!("own"), None));
  let output = space.robot_ok(&["session", "--robot", "whole"]);
  assert_eq!(output["stack_root"], json!("auth"));
  // with --whole-stack, only the whole stack's
  let (exit_code, output) = space.robot(&["session", "--whole-stack", "--robot", "own"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(output["error"]["message"].as_str().unwrap().contains("bd robot login --robot own"), "{output}");
}

/// The branch and the state of each commit of a robot's output, newest first
fn commit_branches(output: &Value) -> Vec<(Value, Value)> {
  output["commits"]
    .as_array()
    .unwrap()
    .iter()
    .map(|commit| (commit["branch"].clone(), commit["state"].clone()))
    .collect()
}
