use crate::actions::summarize_progress;
use crate::client::{BuildProgressDto, TeamCityCheckAttemptDto, TeamCityCheckDto};
use crate::fake_patronus::start_fake;
use crate::service_test::{service, token};
use crate::token::TokenLookup;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use sync_types::patronus::{PatronusChecks, PatronusProblem, PatronusPushMode, PatronusRobotProgress, PatronusRobotStatus};

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
  assert_eq!(
    details.problems,
    vec![PatronusProblem {
      title: "Changes require review approval".to_string(),
      subsystem: Some("OWNERSHIP_REVIEW_APPROVAL_CHECK".to_string()),
    }]
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

#[test]
fn a_robot_ends_with_its_slowest_check_and_runs_over_when_every_check_did() {
  let check = |name: &str, status: &str, build_id: &str| TeamCityCheckDto {
    build_configuration_name: name.to_string(),
    status: status.to_string(),
    attempts: vec![TeamCityCheckAttemptDto {
      number: 1,
      build_id: build_id.to_string(),
      status: if status == "RUNNING" { "RUNNING" } else { "SUCCESS" }.to_string(),
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
async fn start_options_leave_out_the_emergency_profiles_and_default_to_the_profile_of_the_last_robot() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let options = service.start_options("ij", "ultimate", "master", Some("finished")).await.unwrap();
  assert!(options.supported);
  assert_eq!(options.target_branch, "master");
  assert_eq!(options.profiles.iter().map(|profile| profile.id.as_str()).collect::<Vec<_>>(), ["default", "lsc"]);
  assert_eq!(options.profiles[1].allowed_push_modes, [PatronusPushMode::RebaseAutosquash, PatronusPushMode::DryRun]);
  assert_eq!(options.default_profile_id.as_deref(), Some("lsc"));

  let without_robot = service.start_options("ij", "ultimate", "master", None).await.unwrap();
  assert_eq!(without_robot.default_profile_id, None);
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
