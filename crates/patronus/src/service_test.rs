use crate::client::{PatronusClient, RobotDto};
use crate::fake_patronus::start_fake;
use crate::service::{PatronusService, robot_profile, to_robot};
use crate::token::{Token, TokenLookup, TokenStamp};
use domain_types::patronus::{KnownRobotStatus, PatronusAvailability, PatronusPullRequestLink, PatronusPushMode, PatronusRobot, PatronusRobotProfile, PatronusRobotStatus};
use pretty_assertions::assert_eq;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

pub(crate) fn token(value: &str) -> TokenLookup {
  TokenLookup::Found(Token {
    value: value.to_string(),
    stamp: TokenStamp::Env,
  })
}

/// A service whose token the test can replace
pub(crate) fn service(client: PatronusClient, lookup: TokenLookup) -> (PatronusService, Arc<Mutex<TokenLookup>>) {
  let current = Arc::new(Mutex::new(lookup));
  let source = Arc::clone(&current);
  (PatronusService::new(client, move |_| source.lock().unwrap().clone()), current)
}

#[tokio::test]
async fn lists_the_robots_newest_first_and_names_the_virtual_branch() {
  let (fake, client) = start_fake().await;
  let base_url = client.base_url().to_string();
  let (service, _) = service(client, token("t"));

  let result = service.robots("ultimate", "me", None, false, false).await;
  assert_eq!(result.availability, PatronusAvailability::Available);
  assert_eq!(
    result.robots,
    vec![
      PatronusRobot {
        id: "newer".to_string(),
        name: "IJPL-1 fix the cache".to_string(),
        status: PatronusRobotStatus::Running,
        push_mode: PatronusPushMode::RebaseAutosquash,
        // a Safe Push from the CLI: a temporary ref, matched by its revision
        branch_name: None,
        stack_root: None,
        source_revision: Some("abc123".to_string()),
        target_branch: "master".to_string(),
        start_time: 1_791_970_200,
        finish_time: None,
        cancellation_reason: None,
        url: format!("{base_url}/robot/newer"),
        pull_request: None,
        // a list has no options: the profile is not known
        profile: None,
      },
      PatronusRobot {
        id: "older".to_string(),
        status: PatronusRobotStatus::Failure,
        branch_name: Some("IJPL-1".to_string()),
        start_time: 1_791_883_800,
        url: format!("{base_url}/robot/older"),
        ..result.robots[0].clone()
      },
    ]
  );
  let query = fake.last_query.lock().unwrap().clone();
  assert_eq!(query.get("owner").map(String::as_str), Some("me"));
  assert_eq!(query.get("repository").map(String::as_str), Some("ultimate"));
  assert!(query.contains_key("start"), "the window is explicit: {query:?}");
}

#[tokio::test]
async fn with_the_new_api_a_list_names_the_merge_profile_patronus_put_on_a_robot() {
  let (fake, client) = start_fake().await;
  fake.list_profiles.store(true, Ordering::SeqCst);
  let (service, _) = service(client, token("t"));
  let result = service.robots("ultimate", "me", None, false, true).await;
  let profile = |id: &str| result.robots.iter().find(|robot| robot.id == id).unwrap().profile.clone();
  assert_eq!(
    profile("older"),
    Some(PatronusRobotProfile {
      id: Some("lsc".to_string()),
      name: Some("Large Scale Change".to_string()),
      skips_review: Some(true),
    })
  );
  // null: the robot ran without a profile, or the flag of Patronus is off for the user; unknown either way, never "none"
  assert_eq!(profile("newer"), None);

  // without the setting the list is read as before, whatever Patronus put on it
  let (fake, client) = start_fake().await;
  fake.list_profiles.store(true, Ordering::SeqCst);
  let (other, _) = crate::service_test::service(client, token("t"));
  let result = other.robots("ultimate", "me", None, false, false).await;
  assert!(result.robots.iter().all(|robot| robot.profile.is_none()));
}

#[test]
fn a_robot_started_by_a_pull_request_comment_links_the_pull_request() {
  // as `GET /robots` answers for a `/patronus merge` comment: the branch is not a ref
  let json = r#"{"id": "r1", "name": "Fix", "repository": "monorepo", "sourceBranch": "me/virtual/IJPL-1", "sourceRevision": "abc",
    "targetBranch": "master", "startDateTime": "2026-10-15T00:07:08.118860Z", "finishDateTime": null, "status": "RUNNING",
    "type": "GITHUB_PR", "pushMode": "REBASE_AUTOSQUASH", "cancellationReason": null, "spaceReviewKey": null,
    "githubPrUrl": "https://github.com/acme/monorepo/pull/1031", "githubPrLabel": "acme/monorepo#1031"}"#;
  let robot: RobotDto = serde_json::from_str(json).unwrap();
  let robot = to_robot(&robot, "me", &PatronusClient::new("http://patronus".to_string(), "test"));
  assert_eq!(robot.branch_name.as_deref(), Some("IJPL-1"));
  assert_eq!(
    robot.pull_request,
    Some(PatronusPullRequestLink {
      number: 1031,
      url: "https://github.com/acme/monorepo/pull/1031".to_string(),
    })
  );
}

/// The Safe Push of the whole stack starts from the stack branch `{prefix}/stack/<root>`: the robot names the root, and
/// no branch, so no row takes it for its own
#[test]
fn a_robot_of_the_stack_branch_names_the_root_of_its_stack_and_no_branch() {
  let robot_on = |source: &str| -> PatronusRobot {
    let json = format!(
      r#"{{"id": "r1", "name": "Fix", "repository": "monorepo", "sourceBranch": "{source}", "sourceRevision": "abc",
      "targetBranch": "master", "startDateTime": "2026-10-15T00:07:08Z", "finishDateTime": null, "status": "RUNNING",
      "type": "WEB_UI", "pushMode": "REBASE_AUTOSQUASH", "cancellationReason": null}}"#
    );
    to_robot(&serde_json::from_str(&json).unwrap(), "me", &PatronusClient::new("http://patronus".to_string(), "test"))
  };
  let named = |robot: PatronusRobot| (robot.branch_name, robot.stack_root);
  assert_eq!(named(robot_on("refs/heads/me/stack/air")), (None, Some("air".to_string())));
  assert_eq!(named(robot_on("me/stack/air")), (None, Some("air".to_string())));
  assert_eq!(named(robot_on("refs/heads/me/virtual/air")), (Some("air".to_string()), None));
  // another user's stack branch, and a branch of another kind
  assert_eq!(named(robot_on("refs/heads/alice/stack/air")), (None, None));
  assert_eq!(named(robot_on("refs/heads/me/unapplied/air")), (None, None));
}

#[test]
fn the_options_of_a_robot_say_whether_its_profile_skips_the_review() {
  let robot_with = |options: &str| -> RobotDto {
    let json = format!(
      r#"{{"id": "r1", "name": "Fix", "repository": "monorepo", "sourceBranch": "refs/heads/me/virtual/IJPL-1", "sourceRevision": "abc",
      "targetBranch": "master", "startDateTime": "2026-10-15T00:07:08Z", "finishDateTime": null, "status": "RUNNING",
      "type": "WEB_UI", "pushMode": "REBASE_AUTOSQUASH", "cancellationReason": null, "options": {options}}}"#
    );
    serde_json::from_str(&json).unwrap()
  };
  let skips = |options: &str| robot_profile(&robot_with(options)).and_then(|profile| profile.skips_review);
  assert_eq!(skips(r#"{"patronus.merge.profile.id": "lsc", "patronus.review.approval.check.skip": "true"}"#), Some(true));
  assert_eq!(
    skips(r#"{"patronus.merge.profile.id": "optional", "patronus.review.approval.check.skip": "false"}"#),
    Some(false)
  );
  // a robot just started: Patronus has not written its options yet
  assert_eq!(skips("{}"), None);
}

#[tokio::test]
async fn reuses_a_fresh_full_list_but_not_an_incremental_one() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));

  service.robots("ultimate", "me", None, false, false).await;
  service.robots("ultimate", "other", None, true, false).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 1, "a second window of the repository reuses the list");

  service.robots("ultimate", "me", Some(1_791_970_000), true, false).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2);
  assert_eq!(fake.last_query.lock().unwrap().get("start").map(String::as_str), Some("2026-10-14T09:26:40Z"));
}

#[tokio::test]
async fn sends_nothing_without_a_token() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, TokenLookup::Missing);
  let result = service.robots("ultimate", "me", None, false, false).await;
  assert_eq!(result.availability, PatronusAvailability::NotLoggedIn);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_rejected_token_is_not_sent_in_the_background_until_it_changes() {
  let (fake, client) = start_fake().await;
  let (service, current) = service(client, token("dead"));
  fake.set_status(401);

  assert_eq!(service.robots("ultimate", "me", Some(1), true, false).await.availability, PatronusAvailability::NotLoggedIn);
  assert_eq!(service.robots("ultimate", "me", Some(1), true, false).await.availability, PatronusAvailability::NotLoggedIn);
  let known = [KnownRobotStatus {
    id: "newer".to_string(),
    status: PatronusRobotStatus::Running,
  }];
  assert_eq!(service.poll("me", &known).await.availability, PatronusAvailability::NotLoggedIn);
  assert_eq!(
    (fake.list_calls.load(Ordering::SeqCst), fake.update_calls.load(Ordering::SeqCst)),
    (1, 0),
    "every request with a dead token costs Patronus a Space call"
  );

  // the user asks: sent again
  service.robots("ultimate", "me", Some(1), false, false).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2);

  // the CLI wrote a new token
  fake.set_status(200);
  *current.lock().unwrap() = TokenLookup::Found(Token {
    value: "fresh".to_string(),
    stamp: TokenStamp::File(Some(std::time::SystemTime::now())),
  });
  assert_eq!(service.robots("ultimate", "me", Some(1), true, false).await.availability, PatronusAvailability::Available);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn background_requests_back_off_after_a_server_failure() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  fake.set_status(503);

  let failed = service.robots("ultimate", "me", Some(1), true, false).await;
  assert!(
    matches!(failed.availability, PatronusAvailability::Error { ref message } if message == "no such repository"),
    "{failed:?}"
  );
  // a GET is asked once more after a transient failure
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2);

  let skipped = service.robots("ultimate", "me", Some(1), true, false).await;
  assert_eq!(skipped.availability, failed.availability);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2, "no request while backing off");

  // the user asks anyway
  fake.set_status(200);
  assert_eq!(service.robots("ultimate", "me", Some(1), false, false).await.availability, PatronusAvailability::Available);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 3);
  // and the backoff is over
  service.robots("ultimate", "me", Some(1), true, false).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn a_client_error_is_not_asked_again() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  fake.set_status(400);
  service.robots("ultimate", "me", Some(1), true, false).await;
  service.robots("ultimate", "me", Some(1), true, false).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2, "no retry and no backoff for a 4xx");
}

#[tokio::test]
async fn polls_only_running_robots_with_their_known_status() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));

  let empty = service.poll("me", &[]).await;
  assert_eq!((empty.availability, empty.robots.len()), (PatronusAvailability::Available, 0));
  assert_eq!(fake.update_calls.load(Ordering::SeqCst), 0, "nothing to poll, nothing sent");

  let known = [
    KnownRobotStatus {
      id: "newer".to_string(),
      status: PatronusRobotStatus::Running,
    },
    KnownRobotStatus {
      id: "unknown".to_string(),
      status: PatronusRobotStatus::Unknown,
    },
  ];
  let changed = service.poll("me", &known).await;
  assert_eq!(
    changed.robots.iter().map(|robot| (robot.id.as_str(), robot.status)).collect::<Vec<_>>(),
    [("newer", PatronusRobotStatus::Success)]
  );
  // the poll answers a robot with its options: its merge profile comes at no cost, the list never has it
  assert_eq!(
    changed.robots[0].profile,
    Some(PatronusRobotProfile {
      id: Some("lsc".to_string()),
      name: Some("Large Scale Change".to_string()),
      skips_review: Some(true),
    })
  );
  assert_eq!(
    *fake.last_updates.lock().unwrap(),
    serde_json::json!({"robots": [{"id": "newer", "lastKnownStatus": "RUNNING"}]})
  );
}
