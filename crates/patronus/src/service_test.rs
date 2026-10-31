use crate::client::{PatronusClient, RobotDto};
use crate::fake_patronus::{robot_json, start_fake};
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

/// A list that timed out is not asked again, as it already waited all the time a list has, and still backs the background
/// requests off (the clock of the test runs ahead while the answer is held)
#[tokio::test(start_paused = true)]
async fn a_list_that_timed_out_is_not_asked_again_and_backs_off() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let held = fake.hold("GET", "/app/rest/v1/robots");
  let failed = service.robots("ultimate", "me", Some(1), true, false).await;
  assert!(matches!(failed.availability, PatronusAvailability::Error { .. }), "{failed:?}");
  assert_eq!(held.arrivals(), 1);
  service.robots("ultimate", "me", Some(1), true, false).await;
  assert_eq!(held.arrivals(), 1, "no request while backing off");
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

/// The robots of the check of the app's push (`robots_for_checks`) are asked now, never answered from the kept list of
/// the page: a robot the IDE or `bd` started since the page's list is seen, and holds. The list a check fetched answers
/// the page's next list
#[tokio::test]
async fn the_checks_ask_patronus_every_time_and_see_a_robot_started_elsewhere() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let ids = |robots: &[PatronusRobot]| robots.iter().map(|robot| robot.id.clone()).collect::<Vec<_>>();

  // the page loads: no robot
  *fake.robots.lock().unwrap() = Some(Vec::new());
  assert_eq!(service.robots("ultimate", "me", None, false, false).await.robots, Vec::new());
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 1);

  // `bd safe-push` starts a robot, which no poll of the page knows yet
  let started = robot_json("started", "RUNNING", "refs/heads/me/virtual/IJPL-1", "2026-10-14T09:30:00Z");
  *fake.robots.lock().unwrap() = Some(vec![started]);
  assert_eq!(ids(&service.robots_for_checks("ultimate", "me").await), ["started"]);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2, "the check asked, within the reuse window of the page's list");

  // the page's list right after is the one the check fetched
  assert_eq!(ids(&service.robots("ultimate", "other", None, true, false).await.robots), ["started"]);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2);
}

/// A poll that sees a robot change drops the kept full list: the next list of the page asks Patronus instead of
/// showing the robot as it was, and a full list in flight across the poll is not kept (the generation, as after a
/// cancel: `a_full_list_in_flight_when_the_robots_changed_is_not_kept`)
#[tokio::test]
async fn a_poll_that_sees_a_change_drops_the_kept_full_list() {
  let (fake, client) = start_fake().await;
  fake.state_only.store(true, Ordering::SeqCst);
  let (service, _) = service(client, token("t"));
  let status_of = |robots: &[PatronusRobot], id: &str| robots.iter().find(|robot| robot.id == id).unwrap().status;
  let newer = |status: &str| robot_json("newer", status, "refs/heads/me/virtual/IJPL-1", "2026-10-14T09:30:00Z");

  *fake.robots.lock().unwrap() = Some(vec![newer("RUNNING")]);
  let listed = service.robots("ultimate", "me", None, false, false).await.robots;
  assert_eq!(status_of(&listed, "newer"), PatronusRobotStatus::Running);
  // the robot finishes; the page's poll sees it
  *fake.robots.lock().unwrap() = Some(vec![newer("SUCCESS")]);
  let known = [KnownRobotStatus {
    id: "newer".to_string(),
    status: PatronusRobotStatus::Running,
  }];
  assert_eq!(status_of(&service.poll("me", &known).await.robots, "newer"), PatronusRobotStatus::Success);

  let listed = service.robots("ultimate", "me", None, false, false).await.robots;
  assert_eq!(status_of(&listed, "newer"), PatronusRobotStatus::Success);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2, "the list of before the poll is not reused");

  // a poll that sees nothing change keeps the list
  let known = [KnownRobotStatus {
    id: "newer".to_string(),
    status: PatronusRobotStatus::Success,
  }];
  assert_eq!(service.poll("me", &known).await.robots, Vec::new());
  service.robots("ultimate", "me", None, false, false).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2);
}

/// The robots of a check are a background request: while the requests back off after a failure of Patronus, the check
/// sends nothing and holds nothing, so a Patronus that is down does not hold the push up
#[tokio::test]
async fn the_checks_send_nothing_while_the_requests_back_off() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  fake.set_status(503);
  service.robots("ultimate", "me", Some(1), true, false).await;
  let sent = fake.list_calls.load(Ordering::SeqCst);

  fake.set_status(200);
  assert_eq!(service.robots_for_checks("ultimate", "me").await, Vec::new());
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), sent, "no request while backing off");
}

/// A full list in flight when the robots changed (a cancel in another window) is not kept: the page's next list would
/// show the canceled robot running
#[tokio::test]
async fn a_full_list_in_flight_when_the_robots_changed_is_not_kept() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  let service = Arc::new(service);
  // the robots change while Patronus answers the list
  let changing = Arc::clone(&service);
  *fake.robots_hook.lock().unwrap() = Some(Arc::new(move |_| changing.forget_full_lists()));
  service.robots("ultimate", "me", None, false, false).await;
  *fake.robots_hook.lock().unwrap() = None;

  service.robots("ultimate", "me", None, false, false).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2, "the list of before the change is not reused");
  service.robots("ultimate", "me", None, false, false).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2, "the list of after it is");
}
