use crate::fake_patronus::start_fake;
use crate::service_test::{service, token};
use crate::token::TokenLookup;
use pretty_assertions::assert_eq;
use std::sync::atomic::Ordering;
use sync_types::patronus::{PatronusChecks, PatronusProblem, PatronusPushMode, PatronusRobotStatus};

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
      running: 1,
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
      false,
    )
    .await
    .unwrap();
  assert_eq!(robot.id, "started");
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
async fn a_start_right_after_the_push_is_asked_once_more_when_patronus_does_not_see_the_branch() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  fake.unseen_starts.store(1, Ordering::SeqCst);
  let robot = service
    .start("ultimate", "me", "refs/heads/me/virtual/IJPL-1", "master", PatronusPushMode::DryRun, None, true)
    .await
    .unwrap();
  assert_eq!((robot.id.as_str(), fake.start_calls.load(Ordering::SeqCst)), ("started", 2));

  // without a push right before, a 400 is the answer
  fake.unseen_starts.store(1, Ordering::SeqCst);
  let error = service
    .start("ultimate", "me", "refs/heads/me/virtual/IJPL-1", "master", PatronusPushMode::DryRun, None, false)
    .await
    .unwrap_err();
  assert_eq!(error, "Reference refs/heads/me/virtual/IJPL-1 does not exist");
  assert_eq!(fake.start_calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn a_failed_start_is_never_repeated() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  fake.set_status(503);
  let error = service
    .start("ultimate", "me", "refs/heads/me/virtual/IJPL-1", "master", PatronusPushMode::DryRun, None, true)
    .await
    .unwrap_err();
  assert_eq!(error, "no such repository");
  assert_eq!(fake.start_calls.load(Ordering::SeqCst), 1);
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
