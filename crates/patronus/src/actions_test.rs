use crate::actions::{summarize_checks, summarize_progress};
use crate::client::{BuildProgressDto, TeamCityCheckAttemptDto, TeamCityCheckDto};
use crate::fake_patronus::start_fake;
use crate::service_test::{service, token};
use crate::teamcity_cli::TeamCityCli;
use crate::token::TokenLookup;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::Duration;
use sync_types::patronus::{PatronusChecks, PatronusProblem, PatronusPushMode, PatronusRobotProgress, PatronusRobotStatus};
use sync_types::robot_failure::{BuildProblem, FailedBuild, FailedCheck, FailedTest, OmittedFailures, RobotFailureKind, RobotFailureProblem};
use test_utils::fake_executable::fake_executable;

#[tokio::test]
async fn details_have_the_profile_the_checks_and_the_error_problems() {
  let (_, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let details = service.robot_details("me", "running").await.unwrap();
  assert_eq!(details.robot.branch_name.as_deref(), Some("IJPL-1"));
  assert_eq!((details.profile_id.as_deref(), details.profile_name.as_deref()), (Some("lsc"), Some("Large Scale Change")));
  assert_eq!(
    details.checks,
    Some(PatronusChecks {
      failed: 1,
      running: 3,
      passed: 2,
      canceled: 0,
      failed_names: vec!["Fast Tests".to_string()],
    })
  );
  // the warning that the robot will fail is not a problem to show, it marks the robot
  assert_eq!(
    details.problems,
    vec![PatronusProblem {
      title: "Changes require review approval".to_string(),
      subsystem: Some("OWNERSHIP_REVIEW_APPROVAL_CHECK".to_string()),
    }]
  );
  assert!(details.failing);
  assert!(!service.robot_details("me", "finished").await.unwrap().failing);
}

#[tokio::test]
async fn a_running_robot_whose_required_check_failed_is_failing() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let ids = ["newer", "running", "started"].map(str::to_string);
  assert_eq!(service.failing_robots(&ids).await.unwrap(), ["running"]);
  let mut asked = fake.problem_calls.lock().unwrap().clone();
  asked.sort();
  assert_eq!(asked, ["newer", "running", "started"]);
  // nothing to ask
  fake.problem_calls.lock().unwrap().clear();
  assert_eq!(service.failing_robots(&[]).await.unwrap(), Vec::<String>::new());
  assert!(fake.problem_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_problems_of_a_finished_robot_are_its_errors() {
  let (_, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let problems = service.robot_problems("running").await.unwrap();
  assert_eq!(
    problems.iter().map(|problem| problem.title.as_str()).collect::<Vec<_>>(),
    ["Changes require review approval"]
  );
}

#[tokio::test]
async fn details_of_a_running_robot_have_the_estimate_of_its_running_checks() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let details = service.robot_details("me", "running").await.unwrap();
  assert_eq!(
    details.progress,
    Some(PatronusRobotProgress {
      remaining_seconds: Some(540),
      check_name: Some("Unit Tests".to_string()),
      check_percent: Some(93),
      overtime_seconds: None,
      queued: 1,
      probably_hanging: false,
    })
  );
  // the current attempt of each running check, not the failed first one
  let mut builds = fake.build_progress_calls.lock().unwrap().clone();
  builds.sort();
  assert_eq!(builds, ["102", "201"]);
}

#[tokio::test]
async fn the_progress_of_a_build_is_asked_once_a_minute_and_never_for_a_finished_robot() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  service.robot_details("me", "finished").await.unwrap();
  assert!(fake.build_progress_calls.lock().unwrap().is_empty());
  service.robot_details("me", "running").await.unwrap();
  // the details asked again (another window): the builds are not
  service.caches.details.invalidate("running");
  let details = service.robot_details("me", "running").await.unwrap();
  assert_eq!(fake.build_progress_calls.lock().unwrap().len(), 2);
  assert_eq!(details.progress.and_then(|progress| progress.remaining_seconds), Some(540));
}

/// The failed checks a robot names: all of them up to one more than the limit, never "and 1 more"; from two more on,
/// the first three (`list_cut::kept`)
#[test]
fn the_failed_checks_of_a_robot_are_named_whole_when_only_one_is_past_the_limit() {
  let failed = |count: usize| -> Vec<TeamCityCheckDto> {
    (1..=count)
      .map(|index| TeamCityCheckDto {
        name: None,
        build_configuration_id: None,
        build_configuration_name: format!("Check {index}"),
        status: "FAILURE".to_string(),
        attempt_limit: None,
        attempts: Vec::new(),
      })
      .collect()
  };
  let names = |count: usize| summarize_checks(&failed(count)).map(|checks| (checks.failed, checks.failed_names));
  let listed = |names: &[&str]| names.iter().map(ToString::to_string).collect::<Vec<_>>();
  assert_eq!(names(3), Some((3, listed(&["Check 1", "Check 2", "Check 3"]))));
  assert_eq!(names(4), Some((4, listed(&["Check 1", "Check 2", "Check 3", "Check 4"]))));
  assert_eq!(names(5), Some((5, listed(&["Check 1", "Check 2", "Check 3"]))));
}

#[test]
fn a_robot_ends_with_its_slowest_check_and_runs_over_when_every_check_did() {
  let check = |name: &str, status: &str, build_id: &str| TeamCityCheckDto {
    name: None,
    build_configuration_id: None,
    build_configuration_name: name.to_string(),
    status: status.to_string(),
    attempt_limit: None,
    attempts: vec![TeamCityCheckAttemptDto {
      number: 1,
      build_id: build_id.to_string(),
      status: if status == "RUNNING" { "RUNNING" } else { "SUCCESS" }.to_string(),
      ..TeamCityCheckAttemptDto::default()
    }],
  };
  let build = |elapsed: i64, total: Option<i64>, percent: i64, hanging: bool| BuildProgressDto {
    queued: false,
    percentage_complete: percent,
    elapsed_running_seconds: elapsed,
    estimated_total_running_seconds: total,
    probably_hanging: Some(hanging),
  };
  let checks = [
    check("Compilation", "SUCCESS", "1"),
    check("Unit Tests", "RUNNING", "2"),
    check("Smoke", "RUNNING", "3"),
    check("Queued", "QUEUED", "4"),
  ];
  // Smoke ran 4 minutes over its estimate, Unit Tests 1 minute; one of them probably hangs
  let over = HashMap::from([("2".to_string(), build(660, Some(600), 100, false)), ("3".to_string(), build(840, Some(600), 100, true))]);
  assert_eq!(
    summarize_progress(&checks, &over),
    Some(PatronusRobotProgress {
      remaining_seconds: None,
      check_name: Some("Unit Tests".to_string()),
      check_percent: Some(100),
      overtime_seconds: Some(60),
      queued: 1,
      probably_hanging: true,
    })
  );
  // a build in the TeamCity queue counts as queued; without an estimate there is no time left to tell
  let queued = HashMap::from([
    (
      "2".to_string(),
      BuildProgressDto {
        queued: true,
        ..build(0, None, 0, false)
      },
    ),
    ("3".to_string(), build(100, None, 10, false)),
  ]);
  let progress = summarize_progress(&checks, &queued).unwrap();
  assert_eq!((progress.remaining_seconds, progress.check_name, progress.queued), (None, None, 2));
  // nothing runs yet
  assert_eq!(summarize_progress(&checks[..1], &HashMap::new()), None);
}

#[tokio::test]
async fn details_of_a_finished_robot_are_asked_once() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  service.robot_details("me", "finished").await.unwrap();
  service.robot_details("me", "finished").await.unwrap();
  assert_eq!(fake.detail_calls.load(Ordering::SeqCst), 1);
  // a running robot is cached for a minute too
  service.robot_details("me", "running").await.unwrap();
  service.robot_details("me", "running").await.unwrap();
  assert_eq!(fake.detail_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn start_options_keep_the_emergency_profiles_apart_and_default_to_the_profile_of_the_last_robot() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let options = service.start_options("ij", "ultimate", "master", Some("finished")).await.unwrap();
  assert!(options.supported);
  assert_eq!(options.target_branch, "master");
  assert_eq!(options.profiles.iter().map(|profile| profile.id.as_str()).collect::<Vec<_>>(), ["default", "lsc"]);
  assert_eq!(options.profiles[1].allowed_push_modes, [PatronusPushMode::RebaseAutosquash, PatronusPushMode::DryRun]);
  assert_eq!(options.default_profile_id.as_deref(), Some("lsc"));
  // the options of the last robot say that its profile skips the review; Patronus says it of no other profile
  assert_eq!(options.profiles.iter().map(|profile| profile.skips_review).collect::<Vec<_>>(), [None, Some(true)]);
  assert_eq!(
    options
      .emergency_profiles
      .iter()
      .map(|profile| (profile.id.as_str(), profile.skips_review))
      .collect::<Vec<_>>(),
    [("emergency", Some(true))]
  );

  let without_robot = service.start_options("ij", "ultimate", "master", None).await.unwrap();
  assert_eq!(without_robot.default_profile_id, None);
  assert_eq!(without_robot.profiles[1].skips_review, None);
  assert_eq!(fake.start_supported_calls.load(Ordering::SeqCst), 1, "the profiles are cached");
}

#[tokio::test]
async fn starts_a_robot_on_the_pushed_branch_and_keeps_the_branch() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let robot = service
    .start(
      "ultimate",
      "me",
      "refs/heads/me/virtual/IJPL-1",
      "master",
      PatronusPushMode::RebaseAutosquash,
      Some("lsc".to_string()),
      None,
    )
    .await
    .unwrap();
  assert_eq!(robot.id, "started");
  // Patronus has not written the profile into the new robot's options yet: the robot knows the one it was started with
  assert_eq!(robot.profile.and_then(|profile| profile.id).as_deref(), Some("lsc"));
  assert_eq!(
    *fake.last_start.lock().unwrap(),
    serde_json::json!({
      "type": "ROBOT_WEB_UI",
      "repository": "ultimate",
      "sourceBranch": "refs/heads/me/virtual/IJPL-1",
      "targetBranch": "master",
      "mode": "REBASE_AUTOSQUASH",
      "sourceBranchDeletePolicy": "NEVER",
      "mergeProfileId": "lsc",
    })
  );
}

#[tokio::test]
async fn a_failed_start_is_never_repeated() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  fake.set_status(503);
  let error = service
    .start("ultimate", "me", "refs/heads/me/virtual/IJPL-1", "master", PatronusPushMode::DryRun, None, None)
    .await
    .unwrap_err();
  assert_eq!(error, "no such repository");
  assert_eq!(fake.start_calls.load(Ordering::SeqCst), 1);

  // nor a 400 right after a push: Patronus fetches the pushed branch while it handles the start
  fake.set_status(400);
  service
    .start("ultimate", "me", "refs/heads/me/virtual/IJPL-1", "master", PatronusPushMode::DryRun, None, None)
    .await
    .unwrap_err();
  assert_eq!(fake.start_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn restart_answers_the_new_robot_and_cancel_the_canceling_one() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  assert_eq!(service.restart("me", "finished").await.unwrap().id, "restarted");
  assert_eq!(service.cancel("me", "running").await.unwrap().id, "running");
  assert_eq!(service.robot_status("running").await.unwrap(), PatronusRobotStatus::Canceling);
  assert_eq!((fake.restart_calls.load(Ordering::SeqCst), fake.cancel_calls.load(Ordering::SeqCst)), (1, 1));
}

#[tokio::test]
async fn an_action_without_a_token_tells_how_to_log_in() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, TokenLookup::Missing);
  assert_eq!(service.cancel("me", "running").await.unwrap_err(), "Run ./safePush.cmd once to log in to Patronus");
  assert_eq!(fake.cancel_calls.load(Ordering::SeqCst), 0);
}

fn failed_check(name: &str, build_id: &str, attempt: u32, limit: u32) -> FailedCheck {
  FailedCheck {
    name: name.to_string(),
    build_configuration_id: None,
    attempt,
    attempt_limit: Some(limit),
    build_id: build_id.to_string(),
    build_url: Some(format!("https://ci.example/build/{build_id}")),
    failed_tests: None,
    failed_builds: None,
    failed_to_start: None,
    running: false,
  }
}

/// One failed check whose attempt `attempt` has these counts
fn one_failed_check(attempt: &str, failed_tests: u32, failed_to_start: u32) -> serde_json::Value {
  serde_json::json!([
    {"id": "1", "name": "Fast Tests", "buildConfigurationId": "a", "buildConfigurationName": "Aggregator", "status": "FAILURE", "attemptLimit": 1, "attempts": [
      {"id": attempt, "number": 0, "buildId": "100", "buildUrl": "https://ci.example/build/100", "status": "FAILURE",
       "failedTestsNumber": failed_tests, "failedBuildsNumber": 1, "failedToStartBuildsNumber": failed_to_start},
    ]},
    {"id": "2", "buildConfigurationId": "b", "buildConfigurationName": "Compilation", "status": "SUCCESS", "attempts": [
      {"id": "ok", "number": 0, "buildId": "99", "status": "SUCCESS"},
    ]},
  ])
}

fn error_problem(title: &str, subsystem: &str, details: Option<&str>) -> serde_json::Value {
  serde_json::json!({"id": "p", "uniqueId": null, "title": title, "detailsMarkdown": details, "severity": "ERROR", "subsystem": subsystem})
}

#[tokio::test]
async fn the_failure_of_a_finished_robot_is_its_failed_checks_and_error_problems_without_a_request() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  service.robot_details("me", "finished").await.unwrap();
  let failure = service.robot_failure("me", "finished").await.unwrap();
  assert_eq!(fake.detail_calls.load(Ordering::SeqCst), 1, "the failure is read from the cached details");
  assert_eq!(failure.robot.branch_name.as_deref(), Some("IJPL-1"));
  assert!(!failure.running);
  assert_eq!((failure.test_revision.as_deref(), failure.target_revision.as_deref()), (Some("ccc111"), Some("bbb222")));
  // the running Unit Tests of a finished robot don't count, its first attempt was retried
  assert_eq!(
    failure.checks,
    [FailedCheck {
      build_configuration_id: Some("a".to_string()),
      failed_tests: Some(3),
      failed_builds: Some(2),
      failed_to_start: Some(0),
      ..failed_check("Fast Tests", "100", 1, 1)
    }]
  );
  // the approval is a problem too: the caller tells a missing review by its subsystem
  assert_eq!(
    failure.problems,
    [RobotFailureProblem {
      title: "Changes require review approval".to_string(),
      subsystem: Some("OWNERSHIP_REVIEW_APPROVAL_CHECK".to_string()),
      details: None,
    }]
  );
  assert_eq!(failure.kind, RobotFailureKind::Checks);
  assert_eq!((failure.builds, failure.omitted), (None, OmittedFailures::default()));
  assert!(fake.attempt_detail_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_running_robot_fails_also_by_the_running_checks_with_a_failed_build() {
  let (_, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let failure = service.robot_failure("me", "running").await.unwrap();
  assert!(failure.running);
  // Integration Tests runs without a failure yet
  assert_eq!(
    failure.checks.iter().map(|check| (check.name.as_str(), check.attempt, check.running)).collect::<Vec<_>>(),
    [("Fast Tests", 1, false), ("Unit Tests", 2, true)]
  );
  assert_eq!(failure.checks[1].build_url.as_deref(), Some("https://ci.example/build/102"));
}

#[tokio::test]
async fn the_full_failure_has_the_failed_builds_without_composite_and_ignored_ones_asked_once() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let failure = service.robot_failure_full("me", "finished", None).await.unwrap();
  let message = |test_id: &str| Some(format!("expected: <true> but was: <false>\n\tat com.intellij.Test{test_id}.run(Test.kt:1)"));
  let test = |name: &str, test_id: &str| FailedTest {
    name: name.to_string(),
    url: Some(format!("https://ci.example/test/{test_id}")),
    message: message(test_id),
    first_failed_before: None,
  };
  assert_eq!(
    failure.builds,
    Some(vec![
      FailedBuild {
        name: "Compile".to_string(),
        project: Some("IntelliJ / Build".to_string()),
        build_id: "300".to_string(),
        build_url: Some("https://ci.example/build/300".to_string()),
        failed_to_start: false,
        problems: vec![BuildProblem {
          kind: Some("TC_COMPILATION_ERROR".to_string()),
          details: "Compilation error: Foo.kt:12: unresolved reference: bar".to_string(),
        }],
        failed_tests: vec![],
        log_tail: None,
      },
      FailedBuild {
        name: "Unit Tests".to_string(),
        project: Some("IntelliJ / Tests".to_string()),
        build_id: "301".to_string(),
        build_url: Some("https://ci.example/build/301".to_string()),
        failed_to_start: false,
        problems: vec![BuildProblem {
          kind: Some("TC_FAILED_TESTS".to_string()),
          details: "2 tests failed".to_string(),
        }],
        failed_tests: vec![test("com.intellij.FooTest.testBar", "-7"), test("com.intellij.FooTest.testBaz", "-8")],
        log_tail: None,
      },
    ])
  );
  assert_eq!(failure.omitted, OmittedFailures::default());
  assert_eq!(*fake.attempt_detail_calls.lock().unwrap(), ["a0"]);
  assert_eq!(fake.test_run_calls.lock().unwrap().len(), 2);

  // a finished robot does not change: the builds are not asked again
  let again = service.robot_failure_full("me", "finished", None).await.unwrap();
  assert_eq!(again.builds, failure.builds);
  assert_eq!(fake.attempt_detail_calls.lock().unwrap().len(), 1);
  assert_eq!(fake.test_run_calls.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn a_build_in_the_chains_of_two_failed_attempts_is_listed_once() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let failure = service.robot_failure_full("me", "running", None).await.unwrap();
  let mut asked = fake.attempt_detail_calls.lock().unwrap().clone();
  asked.sort();
  assert_eq!(asked, ["a0", "a2"]);
  let builds: Vec<String> = failure.builds.unwrap().into_iter().map(|build| build.build_id).collect();
  assert_eq!(builds, ["300", "301"]);
  // a running robot is asked again
  service.caches.details.invalidate("running");
  service.robot_failure_full("me", "running", None).await.unwrap();
  assert_eq!(fake.attempt_detail_calls.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn the_full_failure_keeps_to_its_limits_and_tells_what_it_left_out() {
  let (fake, client) = start_fake().await;
  fake.checks.lock().unwrap().insert("finished-big".to_string(), one_failed_check("big", 50, 0));
  let builds: Vec<serde_json::Value> = (0..7)
    .map(|build| {
      let tests: Vec<serde_json::Value> = (0..10)
        .map(|test| serde_json::json!({"name": format!("Test{build}.test{test}"), "url": null, "isIgnoredFailure": false, "buildId": format!("{build}"), "testId": format!("{test}"), "testRunId": "1"}))
        .collect();
      // the last one failed not only on tests
      serde_json::json!({"buildId": format!("{build}"), "buildUrl": null, "buildConfigurationName": format!("Tests {build}"), "isComposite": false,
        "isFailedToStart": false, "isFailedTestsOnlyFailure": build != 6,
        "problems": [{"details": "x".repeat(5000), "type": "TC_FAILED_TESTS", "isIgnoredProblem": false, "problemId": "p"}], "failedTests": tests})
    })
    .collect();
  fake.attempt_builds.lock().unwrap().insert("big".to_string(), serde_json::Value::Array(builds));
  let (service, _) = service(client, token("t"));
  let failure = service.robot_failure_full("me", "finished-big", None).await.unwrap();
  let builds = failure.builds.unwrap();
  assert_eq!(builds.iter().map(|build| build.build_id.as_str()).collect::<Vec<_>>(), ["6", "0", "1", "2", "3"]);
  assert_eq!(builds[0].problems[0].details.len(), 4 * 1024);
  assert_eq!(builds.iter().map(|build| build.failed_tests.len()).sum::<usize>(), 40);
  assert_eq!(builds.iter().flat_map(|build| &build.failed_tests).filter(|test| test.message.is_some()).count(), 20);
  assert_eq!(
    failure.omitted,
    OmittedFailures {
      builds: 2,
      tests: 10,
      messages: 20,
      truncated: true,
    }
  );
  assert_eq!(fake.test_run_calls.lock().unwrap().len(), 20);
  let most = fake.max_test_runs_in_flight.load(Ordering::SeqCst);
  assert!((1..=8).contains(&most), "{most} test messages at once");
}

#[tokio::test]
async fn a_list_of_the_full_failure_one_longer_than_its_limit_is_given_whole() {
  let (fake, client) = start_fake().await;
  // builds of failed tests only, `tests[i]` tests in build i
  let failed_builds = |tests: &[usize]| -> serde_json::Value {
    let builds: Vec<serde_json::Value> = tests
      .iter()
      .enumerate()
      .map(|(build, &count)| {
        let tests: Vec<serde_json::Value> = (0..count)
          .map(|test| serde_json::json!({"name": format!("Test{build}.test{test}"), "url": null, "isIgnoredFailure": false, "buildId": format!("{build}"), "testId": format!("{test}"), "testRunId": "1"}))
          .collect();
        serde_json::json!({"buildId": format!("{build}"), "buildUrl": null, "buildConfigurationName": format!("Tests {build}"), "isComposite": false,
          "isFailedToStart": false, "isFailedTestsOnlyFailure": true, "problems": [], "failedTests": tests})
      })
      .collect();
    serde_json::Value::Array(builds)
  };
  // 6 builds of 21 tests: one more than the limits of the builds (5) and of the messages (20)
  fake.checks.lock().unwrap().insert("finished-six".to_string(), one_failed_check("six", 21, 0));
  fake.attempt_builds.lock().unwrap().insert("six".to_string(), failed_builds(&[4, 4, 4, 3, 3, 3]));
  // 41 tests: one more than the limit of the names (40)
  fake.checks.lock().unwrap().insert("finished-names".to_string(), one_failed_check("names", 41, 0));
  fake.attempt_builds.lock().unwrap().insert("names".to_string(), failed_builds(&[21, 20]));
  let (service, _) = service(client, token("t"));

  let six = service.robot_failure_full("me", "finished-six", None).await.unwrap();
  let builds = six.builds.unwrap();
  assert_eq!(builds.len(), 6);
  assert_eq!(builds.iter().flat_map(|build| &build.failed_tests).filter(|test| test.message.is_some()).count(), 21);
  assert_eq!(six.omitted, OmittedFailures::default());

  let names = service.robot_failure_full("me", "finished-names", None).await.unwrap();
  assert_eq!(names.builds.unwrap().iter().map(|build| build.failed_tests.len()).sum::<usize>(), 41);
  assert_eq!(names.omitted.tests, 0);
  // the messages leave out more than one: the first 20
  assert_eq!(names.omitted.messages, 21);
}

#[tokio::test]
async fn the_kind_of_a_failure_decides_the_first_step() {
  let (fake, client) = start_fake().await;
  let set = |robot: &str, checks: serde_json::Value, problems: Vec<serde_json::Value>| {
    fake.checks.lock().unwrap().insert(robot.to_string(), checks);
    fake.problems.lock().unwrap().insert(robot.to_string(), serde_json::Value::Array(problems));
  };
  let passed =
    serde_json::json!([{"id": "1", "buildConfigurationName": "Aggregator", "status": "SUCCESS", "attempts": [{"id": "ok", "number": 0, "buildId": "1", "status": "SUCCESS"}]}]);
  set("finished-infra", one_failed_check("x", 0, 2), vec![]);
  set("finished-tests", one_failed_check("x", 3, 2), vec![]);
  set(
    "finished-massive",
    one_failed_check("x", 900, 0),
    vec![error_problem("Too many tests failed", "MASSIVE_TEST_FAILURE_DETECTOR", None)],
  );
  let conflict = format!("CONFLICT (content): Merge conflict in Foo.kt\n{}", "y".repeat(5000));
  set(
    "finished-rebase",
    passed.clone(),
    vec![error_problem("Cannot rebase onto master", "VCS_INIT", Some(&conflict))],
  );
  set("finished-nothing", passed, vec![]);
  let (service, _) = service(client, token("t"));
  let kind = async |robot: &str| service.robot_failure("me", robot).await.unwrap().kind;
  // only builds that never started: the CI, a restart is the first step
  assert_eq!(kind("finished-infra").await, RobotFailureKind::Infrastructure);
  assert_eq!(kind("finished-tests").await, RobotFailureKind::Checks);
  assert_eq!(kind("finished-massive").await, RobotFailureKind::Massive);
  assert_eq!(kind("finished-nothing").await, RobotFailureKind::Unknown);
  let rebase = service.robot_failure("me", "finished-rebase").await.unwrap();
  assert_eq!(rebase.kind, RobotFailureKind::Patronus);
  let details = rebase.problems[0].details.as_deref().unwrap();
  assert!(details.starts_with("CONFLICT (content)") && details.len() == 4 * 1024);
  assert!(rebase.omitted.truncated);
}

#[tokio::test]
async fn the_teamcity_cli_adds_the_log_of_a_build_that_failed_not_only_on_tests_and_whether_a_test_failed_before() {
  let (fake, client) = start_fake().await;
  let dir = tempfile::tempdir().unwrap();
  let shown = dir.path().display();
  std::fs::write(
    dir.path().join("tests.json"),
    r#"{"testOccurrence": [{"name": "com.intellij.FooTest.testBar", "firstFailed": {"build": {"id": 290}}}, {"name": "com.intellij.FooTest.testBaz"}]}"#,
  )
  .unwrap();
  let binary = fake_executable(
    dir.path(),
    "teamcity",
    &format!(
      r#"echo "$*" >> '{shown}/calls'
case "$1 $2" in
  "auth status") echo '[{{"server": "https://ci.example/", "status": "authenticated"}}]' ;;
  "run log") echo "the log of $3" ;;
  "run tests") cat '{shown}/tests.json' ;;
esac"#
    ),
  );
  let teamcity = TeamCityCli::open(binary, Duration::from_secs(10)).await.unwrap();
  let (service, _) = service(client, token("t"));
  let builds = service.robot_failure_full("me", "finished", Some(&teamcity)).await.unwrap().builds.unwrap();
  assert_eq!(
    builds.iter().map(|build| (build.build_id.as_str(), build.log_tail.as_deref())).collect::<Vec<_>>(),
    [("300", Some("the log of 300")), ("301", None)]
  );
  assert_eq!(builds[1].failed_tests.iter().map(|test| test.first_failed_before).collect::<Vec<_>>(), [Some(true), None]);
  let mut calls: Vec<String> = std::fs::read_to_string(dir.path().join("calls")).unwrap().lines().map(str::to_string).collect();
  calls.sort();
  assert_eq!(
    calls,
    [
      "auth status --json --no-input --no-color",
      "run log 300 --tail 150 --raw --no-input --no-color",
      "run tests 301 --failed --json --limit 500 --no-input --no-color",
    ]
  );
  // the builds without the CLI are another answer
  let without = service.robot_failure_full("me", "finished", None).await.unwrap().builds.unwrap();
  assert_eq!(without[0].log_tail, None);
  assert_eq!(fake.attempt_detail_calls.lock().unwrap().len(), 2);
}
