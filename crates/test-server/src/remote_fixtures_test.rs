use crate::remote_fixtures::{HistoryRobotFixture, fixture_history_robot, fixture_progress, fixture_robot};
use crate::test_support::{create_repository, create_repository_from, post, start_test_server, sync};
use domain_types::patronus::{
  PatronusProfile, PatronusPullRequestLink, PatronusPushMode, PatronusRobot, PatronusRobotProfile, PatronusRobotProgress, PatronusRobotStatus, PatronusStartOptions,
};
use patronus::actions::progress_of;
use patronus::client::{PatronusClient, RobotDto, RobotProgressDto};
use patronus::service::to_robot;
use patronus::stats::{owner_key, to_history_robot};
use pretty_assertions::assert_eq;
use serde_json::json;
use test_utils::git_test_utils::git_command;

fn robot(id: &str, client: &PatronusClient) -> PatronusRobot {
  PatronusRobot {
    id: id.to_string(),
    name: format!("Safe Push {id}"),
    status: PatronusRobotStatus::Running,
    push_mode: PatronusPushMode::RebaseAutosquash,
    branch_name: Some("feature".to_string()),
    stack_root: None,
    source_revision: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
    target_branch: "master".to_string(),
    start_time: 1_704_139_200,
    finish_time: None,
    cancellation_reason: None,
    url: client.robot_url(id),
    pull_request: None,
    profile: None,
  }
}

/// Each fixture robot, written in the wire form of the fake, reads back through the real parser and `to_robot` as the
/// fixture: the URL is the one the client derives
#[test]
fn a_fixture_robot_reads_back_through_the_real_parser() {
  let client = PatronusClient::new("http://127.0.0.1:1", "test");
  let robots = [
    robot("running", &client),
    PatronusRobot {
      branch_name: None,
      stack_root: Some("auth".to_string()),
      status: PatronusRobotStatus::Failure,
      push_mode: PatronusPushMode::DryRun,
      finish_time: Some(1_704_142_800),
      pull_request: Some(PatronusPullRequestLink {
        number: 7,
        url: "https://github.com/owner/name/pull/7".to_string(),
      }),
      profile: Some(PatronusRobotProfile {
        id: Some("lsc".to_string()),
        name: Some("Large Scale Change".to_string()),
        skips_review: Some(true),
      }),
      ..robot("stack", &client)
    },
    PatronusRobot {
      status: PatronusRobotStatus::Canceled,
      cancellation_reason: Some("Canceled by the user".to_string()),
      source_revision: None,
      profile: Some(PatronusRobotProfile {
        id: None,
        name: None,
        skips_review: None,
      }),
      ..robot("canceled", &client)
    },
  ];
  for fixture in robots {
    let wire: RobotDto = serde_json::from_value(fixture_robot(&fixture, "user-name").unwrap()).unwrap();
    assert_eq!(to_robot(&wire, "user-name", &client), fixture);
  }
}

/// A robot of the history fixture reads back through the real parser of the statistics as the fixture: the owner, the
/// times, the restart, the dry run, and the branch of the user's own
#[test]
fn a_history_robot_reads_back_through_the_parser_of_the_statistics() {
  let fixture = HistoryRobotFixture {
    id: "h1".to_string(),
    owner: "1".to_string(),
    status: PatronusRobotStatus::Success,
    start_time: 1_704_000_000,
    finish_time: Some(1_704_003_600),
    branch: Some("auth".to_string()),
    dry_run: true,
    source_robot_id: Some("h0".to_string()),
  };
  let wire: RobotDto = serde_json::from_value(fixture_history_robot(&fixture, "user-name").unwrap()).unwrap();
  let me = owner_key(b"salt", wire.owner.as_ref().unwrap());
  let robot = to_history_robot(&wire, b"salt", Some(me)).unwrap();
  assert_eq!(
    (
      robot.status,
      robot.start,
      robot.finish,
      robot.dry_run,
      robot.source_robot_id.as_deref(),
      robot.source_branch.as_deref()
    ),
    (
      PatronusRobotStatus::Success,
      1_704_000_000,
      Some(1_704_003_600),
      true,
      Some("h0"),
      Some("user-name/virtual/auth")
    )
  );
  assert!(robot.user);
}

/// The progress of a fixture, written as `robots/progress` folds it, reads back through the real fold as the fixture
#[test]
fn a_fixture_progress_reads_back_through_the_real_fold() {
  let now = 1_704_139_200;
  let progresses = [
    PatronusRobotProgress {
      remaining_seconds: Some(540),
      check_name: Some("Unit Tests".to_string()),
      check_percent: Some(93),
      overtime_seconds: None,
      queued: 1,
      probably_hanging: false,
    },
    PatronusRobotProgress {
      remaining_seconds: None,
      check_name: Some("Compilation".to_string()),
      check_percent: Some(100),
      overtime_seconds: Some(120),
      queued: 0,
      probably_hanging: true,
    },
  ];
  for fixture in progresses {
    let wire: RobotProgressDto = serde_json::from_value(fixture_progress("robot", &fixture, now)).unwrap();
    assert_eq!(progress_of(&wire, now), Some(fixture));
  }
}

/// A server with a repository of the simple template whose virtual branch `name` points to `start`: the server, its
/// client, the repository's id and path, and the commit of the branch
async fn repository_with_branch(name: &str, start: &str) -> (std::net::SocketAddr, reqwest::Client, String, String, String) {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  let created = git_command()
    .args(["branch", &format!("user-name/virtual/{name}"), start])
    .current_dir(&repository_path)
    .output()
    .unwrap();
  assert!(created.status.success());
  let tip = git_command().args(["rev-parse", start]).current_dir(&repository_path).output().unwrap();
  let tip = String::from_utf8(tip.stdout).unwrap().trim().to_string();
  (addr, client, id, repository_path, tip)
}

/// The Patronus fixture of one robot `r1` running on `branch`, its commit the tip of the branch when Patronus answers
fn running_on(branch: &str) -> serde_json::Value {
  json!({"availability": {"status": "available"}, "robots": [{
    "id": "r1", "name": format!("Safe Push of {branch}"), "status": "running", "pushMode": "rebaseAutosquash", "branchName": branch,
    "sourceRevision": "", "targetBranch": "master", "startTime": 1_704_139_200, "finishTime": null, "cancellationReason": null,
    "url": "ignored",
  }]})
}

/// The robots of the fixture come through the real service from the fake Patronus of the repository: an empty
/// `sourceRevision` is the tip of the robot's branch when Patronus answers, and the fake counts the requests that
/// reached it
#[tokio::test]
async fn the_robots_of_the_fixture_come_through_the_real_service() {
  let (addr, client, id, repository_path, tip) = repository_with_branch("feature", "HEAD~1").await;

  let params = json!({"repositoryPath": repository_path, "branchPrefix": "user-name", "since": null, "background": false});
  let before = post(&client, addr, "/invoke/get_patronus_robots", params.clone()).await;
  assert_eq!(before["availability"], json!({"status": "notPatronus"}));

  post(&client, addr, &format!("/repositories/{id}/patronus"), running_on("feature")).await;
  let listed = post(&client, addr, "/invoke/get_patronus_robots", params).await;
  assert_eq!(listed["availability"], json!({"status": "available"}));
  assert_eq!(listed["robots"][0]["branchName"], "feature");
  assert_eq!(listed["robots"][0]["sourceRevision"], tip.as_str());

  let log: serde_json::Value = client.get(format!("http://{addr}/repositories/{id}/patronus")).send().await.unwrap().json().await.unwrap();
  assert_eq!(log["listCalls"], 2);
  assert_eq!(log["requests"]["list"], 1);
}

/// A server with a repository of the simple template on Space (a Patronus fixture without robots), its branches parked
/// as `parked` names them (branch, note), synced: the server, its client, the repository's id and path
async fn synced_on_space(parked: &[(&str, &str)]) -> (std::net::SocketAddr, reqwest::Client, String, String) {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository(&client, addr).await;
  post(
    &client,
    addr,
    &format!("/repositories/{id}/patronus"),
    json!({"availability": {"status": "available"}, "robots": []}),
  )
  .await;
  if !parked.is_empty() {
    let branches: Vec<serde_json::Value> = parked.iter().map(|(branch, note)| json!({"branchName": branch, "note": note})).collect();
    post(&client, addr, "/invoke/park_branches", json!({"repositoryPath": repository_path, "branches": branches})).await;
  }
  sync(&client, addr, &repository_path).await;
  (addr, client, id, repository_path)
}

/// A start the IPC asks for a parked branch is refused in Rust, by the checks of the app's command, before Patronus is
/// asked: the fake logs no start
#[tokio::test]
async fn a_start_of_a_parked_branch_is_refused_before_patronus_is_asked() {
  let (addr, client, id, repository_path) = synced_on_space(&[("test-branch", "IJPL-1")]).await;
  let start = json!({"repositoryPath": repository_path, "branchPrefix": "user-name", "baselineBranch": "origin/master", "branchName": "test-branch",
    "mode": "rebaseAutosquash", "profileId": null, "mergeRequest": null});
  let answer = post(&client, addr, "/invoke/start_patronus_robot", start).await;
  assert_eq!(answer["status"], "error");
  let refusal = answer["error"]["message"].as_str().unwrap();
  assert!(
    refusal.ends_with("Nothing was started: test-branch is parked: IJPL-1; it stays local until the user unparks it"),
    "{refusal}"
  );

  let log: serde_json::Value = client.get(format!("http://{addr}/repositories/{id}/patronus")).send().await.unwrap().json().await.unwrap();
  assert_eq!(log["starts"], json!([]));
}

/// The merge profiles of the fixture, written as `robot-start-supported` answers them, read back through the real
/// service as the fixture; a start, a restart and a cancel run the app's core and the real service against the fake,
/// which lists the robots they started, answers their status, and logs each request (`GET …/patronus`)
#[tokio::test]
async fn a_start_restart_and_cancel_run_the_real_service_against_the_fake() {
  let (addr, client, id, repository_path) = synced_on_space(&[]).await;
  let tip = git_command()
    .args(["rev-parse", "user-name/virtual/test-branch"])
    .current_dir(&repository_path)
    .output()
    .unwrap();
  let tip = String::from_utf8(tip.stdout).unwrap().trim().to_string();
  let lsc = PatronusProfile {
    id: "lsc".to_string(),
    name: "Large Scale Change".to_string(),
    description: Some("All the tests".to_string()),
    allowed_push_modes: vec![PatronusPushMode::RebaseAutosquash, PatronusPushMode::DryRun],
    skips_review: None,
  };
  let emergency = PatronusProfile {
    id: "emergency".to_string(),
    name: "Emergency".to_string(),
    description: None,
    allowed_push_modes: Vec::new(),
    skips_review: Some(true),
  };
  post(
    &client,
    addr,
    &format!("/repositories/{id}/patronus-profiles"),
    json!({"profiles": [lsc], "emergencyProfiles": [emergency]}),
  )
  .await;

  let options = post(
    &client,
    addr,
    "/invoke/get_patronus_start_options",
    json!({"repositoryPath": repository_path, "baselineBranch": "origin/master", "lastRobotId": null}),
  )
  .await;
  let options: PatronusStartOptions = serde_json::from_value(options).unwrap();
  assert_eq!(
    (options.target_branch.as_str(), options.profiles, options.emergency_profiles),
    ("master", vec![lsc], vec![emergency])
  );

  let start = json!({"repositoryPath": repository_path, "branchPrefix": "user-name", "baselineBranch": "origin/master", "branchName": "test-branch",
    "mode": "rebaseAutosquash", "profileId": "lsc", "mergeRequest": null});
  let started: PatronusRobot = serde_json::from_value(post(&client, addr, "/invoke/start_patronus_robot", start).await).unwrap();
  assert_eq!(
    (started.id.as_str(), started.status, started.branch_name.as_deref(), started.source_revision.as_deref()),
    ("started-1", PatronusRobotStatus::Running, Some("test-branch"), Some(tip.as_str()))
  );
  let robot = |robot_id: &str| json!({"repositoryPath": repository_path, "branchPrefix": "user-name", "robotId": robot_id});
  let restarted: PatronusRobot = serde_json::from_value(post(&client, addr, "/invoke/restart_patronus_robot", robot("started-1")).await).unwrap();
  assert_eq!((restarted.id.as_str(), restarted.branch_name.as_deref()), ("restarted-1", Some("test-branch")));
  let canceled: PatronusRobot = serde_json::from_value(post(&client, addr, "/invoke/cancel_patronus_robot", robot("started-1")).await).unwrap();
  assert_eq!(canceled.status, PatronusRobotStatus::Canceling);
  assert_eq!(post(&client, addr, "/invoke/get_patronus_robot_status", robot("restarted-1")).await, "running");

  let log: serde_json::Value = client.get(format!("http://{addr}/repositories/{id}/patronus")).send().await.unwrap().json().await.unwrap();
  assert_eq!(
    (log["starts"].clone(), log["restarts"].clone(), log["cancels"].clone(), log["statusReads"].clone()),
    (
      json!([{"type": "ROBOT_WEB_UI", "repository": "repository", "sourceBranch": "refs/heads/user-name/virtual/test-branch", "targetBranch": "master",
        "mode": "REBASE_AUTOSQUASH", "sourceBranchDeletePolicy": "NEVER", "mergeProfileId": "lsc"}]),
      json!(["started-1"]),
      json!(["started-1"]),
      json!(["restarted-1"]),
    )
  );
}

/// A push the IPC asks for a branch whose own Safe Push runs is refused in Rust, by the checks of the app's command on
/// the robots the fake lists (the page knew of none: the robot started after its list), before git pushes anything
#[tokio::test]
async fn a_push_into_a_branch_whose_safe_push_runs_is_refused_before_git_pushes() {
  let (addr, client, id, repository_path) = synced_on_space(&[]).await;
  post(&client, addr, &format!("/repositories/{id}/patronus"), running_on("test-branch")).await;

  let push = json!({"repositoryPath": repository_path, "branchPrefix": "user-name", "branchName": "test-branch", "myEmail": null,
    "baselineBranch": "origin/master", "parentBranch": null, "expectedRemote": null, "tip": null});
  let answer = post(&client, addr, "/invoke/push_branch", push).await;
  assert_eq!(answer["status"], "error");
  let refusal = answer["error"]["message"].as_str().unwrap();
  assert!(
    refusal.contains(
      "Nothing was pushed: A Safe Push of test-branch is running on user-name/virtual/test-branch: a push moves the branch it took its commit from, so it won't land and fails the Safe Push at the end. Wait until it merges, or restart it on the current branch ("
    ),
    "{refusal}"
  );
  let remote = git_command()
    .args(["for-each-ref", "--format=%(refname)", "refs/remotes/origin/user-name/"])
    .current_dir(&repository_path)
    .output()
    .unwrap();
  assert_eq!(String::from_utf8(remote.stdout).unwrap(), "", "nothing reached the remote");
  let log: serde_json::Value = client.get(format!("http://{addr}/repositories/{id}/patronus")).send().await.unwrap().json().await.unwrap();
  assert_eq!(log["requests"]["list"], 1, "one full list of the user's robots");
}

/// The landing check of the popover (`check_landing_gate`) counts as landed what the merged Safe Pushes of the robots
/// Rust reads landed, never the page's robots: `lane-run` of the `landing_needs` template uses a name only
/// `stills-codec` adds, so its start is refused with the restack while no robot landed `stills-codec`; a merged Safe
/// Push of `stills-codec` the fake lists (its commit in the target) leaves nothing to refuse, and nothing of the page
/// said so
#[tokio::test]
async fn the_landing_check_counts_what_the_robots_rust_reads_landed() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository_from(&client, addr, "landing_needs").await;
  let fixture = format!("/repositories/{id}/patronus");
  post(&client, addr, &fixture, json!({"availability": {"status": "available"}, "robots": []})).await;
  sync(&client, addr, &repository_path).await;

  let check = json!({"repositoryPath": repository_path, "branchPrefix": "user-name", "branchName": "lane-run"});
  let refused = post(&client, addr, "/invoke/check_landing_gate", check.clone()).await;
  assert_eq!(refused["refusals"].as_array().map(Vec::len), Some(1), "{refused}");
  assert_eq!(refused["refusals"][0]["kind"], "missingDeclaration");
  assert_eq!(refused["refusals"][0]["identifier"], "Stills");
  assert_eq!(refused["refusals"][0]["fix"], json!({"kind": "restack", "branch": "lane-run", "onto": "stills-codec"}));

  // the Safe Push of stills-codec merged: its commit is in the target, so lane-run's start has what it needs
  let merged = json!({"availability": {"status": "available"}, "robots": [{
    "id": "landed", "name": "Safe Push of stills-codec", "status": "success", "pushMode": "rebaseAutosquash", "branchName": "stills-codec",
    "sourceRevision": "", "targetBranch": "master", "startTime": 1_704_139_200, "finishTime": 1_704_142_800, "cancellationReason": null,
    "url": "ignored",
  }]});
  post(&client, addr, &fixture, merged).await;
  let landed = post(&client, addr, "/invoke/check_landing_gate", check).await;
  assert_eq!(landed, json!({"refusals": []}));
  let log: serde_json::Value = client.get(format!("http://{addr}{fixture}")).send().await.unwrap().json().await.unwrap();
  assert_eq!(log["requests"]["list"], 2, "each check asked the fake for the user's robots");
}
