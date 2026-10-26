use crate::remote_fixtures::{HistoryRobotFixture, fixture_history_robot, fixture_progress, fixture_robot};
use crate::session_routes_test::{subscribe, subscription};
use crate::test_support::{Named, create_repository, create_repository_from, execute_request, post, post_as, push_intent, start_test_server, sync, view_when};
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

/// The robots of the fixture come through the real service from the fake Patronus of the repository into the session's
/// facts, read by the scheduler of the facts: an empty `sourceRevision` is the tip of the robot's branch when Patronus
/// answers, and the fake counts the requests that reached it
#[tokio::test]
async fn the_robots_of_the_fixture_come_through_the_real_service() {
  let (addr, client, id, repository_path, tip) = repository_with_branch("feature", "HEAD~1").await;
  let session = json!({"repositoryPath": repository_path});
  post(&client, addr, "/invoke/subscribe_view", session.clone()).await;
  let refresh = json!({"repositoryPath": repository_path, "family": "robots", "reason": "manual"});
  let robots = || async {
    let snapshot = post(&client, addr, "/invoke/get_snapshot", session.clone()).await;
    snapshot["facts"]["robots"].clone()
  };

  post(&client, addr, "/invoke/refresh_facts", refresh.clone()).await;
  assert_eq!(robots().await["head"]["value"]["availability"], json!({"status": "notPatronus"}));

  post(&client, addr, &format!("/repositories/{id}/patronus"), running_on("feature")).await;
  post(&client, addr, "/invoke/refresh_facts", refresh).await;
  let listed = robots().await;
  assert_eq!(listed["head"]["value"]["availability"], json!({"status": "available"}));
  assert_eq!(listed["robots"]["r1"]["branchName"], "feature");
  assert_eq!(listed["robots"]["r1"]["sourceRevision"], tip.as_str());

  let log: serde_json::Value = client.get(format!("http://{addr}/repositories/{id}/patronus")).send().await.unwrap().json().await.unwrap();
  assert_eq!(log["listCalls"], 2);
  assert_eq!(log["requests"]["list"], 1);
  assert_eq!(log["robots"]["r1"]["sourceRevision"], tip.as_str());
}

/// The details of a robot a window shows come through the real service from the fake Patronus as a fact of the
/// session, read when the window demands them (`robotDetails`), counted by the fake. Settings → Patronus, the new API,
/// is read in Rust from the settings of the repository, never sent by the page: the details of a running robot ask
/// Patronus for its folded progress (`robots/progress`) only once the setting is on
#[tokio::test]
async fn the_details_a_window_demands_ask_the_folded_progress_only_with_the_new_api_of_the_settings() {
  let (addr, client, id, repository_path, _) = repository_with_branch("feature", "HEAD~1").await;
  post(&client, addr, &format!("/repositories/{id}/patronus"), running_on("feature")).await;
  let mut messages = subscribe(&client, addr, &id, &repository_path, 41).await;
  let subscribed = subscription(&mut messages).await;
  sync(&client, addr, &repository_path).await;
  let patronus = || async {
    let log: serde_json::Value = client.get(format!("http://{addr}/repositories/{id}/patronus")).send().await.unwrap().json().await.unwrap();
    (log["detailCalls"].clone(), log["requests"]["progress"].clone())
  };
  view_when(&client, addr, &repository_path, |view| view["facts"]["robots"]["robots"]["r1"].is_object()).await;
  assert_eq!(patronus().await, (json!(0), json!([])), "no window shows the details");
  let show = |robots: serde_json::Value| {
    let demand = json!({
      "focused": true, "online": true, "visibleBranches": [], "visibleStacks": [], "needed": [],
      "robotDetails": robots, "watchedRobots": [],
    });
    post_as(
      &client,
      addr,
      Named::Id(&id),
      "/invoke/set_demand",
      json!({ "subscription": subscribed["id"], "demand": demand }),
    )
  };

  show(json!(["r1"])).await;
  let view = view_when(&client, addr, &repository_path, |view| view["facts"]["robotDetails"]["r1"]["state"]["kind"] == "fresh").await;
  assert_eq!(view["facts"]["robotDetails"]["r1"]["value"]["robot"]["id"], "r1");
  assert_eq!(patronus().await, (json!(1), json!([])));
  show(json!([])).await;
  view_when(&client, addr, &repository_path, |view| view["facts"]["robotDetails"].get("r1").is_none()).await;

  let saved = client
    .post(format!("http://{addr}/store/{id}/appSettings"))
    .json(&json!({ "patronusNewApi": true }))
    .send()
    .await
    .unwrap();
  assert_eq!(saved.status(), 200);
  show(json!(["r1"])).await;
  view_when(&client, addr, &repository_path, |view| view["facts"]["robotDetails"]["r1"]["state"]["kind"] == "fresh").await;
  assert_eq!(patronus().await, (json!(2), json!(["r1"])));
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

/// The plan of `intent` in the session of the repository as the popover of the page asks it: a window subscribed, the
/// robots and the profiles read, then `prepare` again until `settled` holds of the plan (the landing needs the session
/// computes after its sync come a little later)
async fn plan_when(
  client: &reqwest::Client,
  addr: std::net::SocketAddr,
  repository_path: &str,
  intent: &serde_json::Value,
  settled: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
  post(client, addr, "/invoke/subscribe_view", json!({"repositoryPath": repository_path})).await;
  for family in ["robots", "profiles"] {
    let refresh = json!({"repositoryPath": repository_path, "family": family, "reason": "manual"});
    post(client, addr, "/invoke/refresh_facts", refresh).await;
  }
  let mut plan = json!(null);
  for _ in 0..2000 {
    plan = post(client, addr, "/invoke/prepare", json!({"repositoryPath": repository_path, "intent": intent})).await;
    if plan.is_object() && settled(&plan) {
      return plan;
    }
    tokio::task::yield_now().await;
  }
  panic!("the plan never settled: {plan}");
}

/// `execute` of `intent` on `plan`, every confirmation of it accepted, as the popover starts it: the outcome, or the
/// error the command answered
async fn execute_plan(client: &reqwest::Client, addr: std::net::SocketAddr, repository_path: &str, intent: &serde_json::Value, plan: &serde_json::Value) -> serde_json::Value {
  let execute = json!({"repositoryPath": repository_path, "intent": intent, "fingerprint": plan["fingerprint"], "accepted": plan["confirmations"]});
  post(client, addr, "/invoke/execute", execute).await
}

/// The intent of a start of the branch `name` in `mode` with `profile` (`{"kind": "default"}`, `{"kind": "id", …}`)
fn land(name: &str, mode: &str, profile: &serde_json::Value) -> serde_json::Value {
  json!({"kind": "land", "subject": {"kind": "branch", "name": name}, "mode": mode, "profile": profile})
}

/// A Dry Run of `test-branch` started through `execute`, as the popover starts it: the outcome tells each effect done
async fn start_dry_run_of_test_branch(client: &reqwest::Client, addr: std::net::SocketAddr, repository_path: &str) -> serde_json::Value {
  let intent = land("test-branch", "dryRun", &json!({"kind": "default"}));
  let plan = plan_when(client, addr, repository_path, &intent, |plan| plan["waits"] == json!([])).await;
  let outcome = execute_plan(client, addr, repository_path, &intent, &plan).await;
  let statuses: Vec<&serde_json::Value> = outcome["effects"]
    .as_array()
    .unwrap_or_else(|| panic!("{outcome}"))
    .iter()
    .map(|record| &record["status"]["kind"])
    .collect();
  assert!(statuses.iter().all(|status| *status == "done"), "{outcome}");
  outcome
}

/// A start its plan refuses is refused by `execute` in Rust before Patronus or GitHub is asked, in the words of the
/// refusal, the page's plan or not: a parked branch, then a branch whose Safe Push runs on its last commit (the robot
/// started after the plan the page showed); its pull request gets no `/patronus` comment, the fake logs no start
#[tokio::test]
async fn a_start_its_plan_refuses_is_refused_before_patronus_or_github_is_asked() {
  let (addr, client, id, repository_path) = synced_on_space(&[("test-branch", "IJPL-1")]).await;
  let github = json!({"availability": {"status": "available", "owner": "acme", "name": "monorepo"}, "pullRequests": {}, "stackPullRequests": {}});
  post(&client, addr, &format!("/repositories/{id}/pull-requests"), github).await;
  let intent = land("test-branch", "dryRun", &json!({"kind": "default"}));
  let refused = |answer: serde_json::Value| {
    assert_eq!(answer["status"], "error", "{answer}");
    (
      answer["error"]["code"].as_str().unwrap().to_string(),
      answer["error"]["message"].as_str().unwrap().to_string(),
    )
  };

  let parked = plan_when(&client, addr, &repository_path, &intent, |plan| plan["blockers"] != json!([])).await;
  let (code, message) = refused(execute_plan(&client, addr, &repository_path, &intent, &parked).await);
  assert_eq!(code, "parked");
  assert!(message.ends_with("Nothing was pushed or started: test-branch is parked: IJPL-1"), "{message}");

  post(
    &client,
    addr,
    "/invoke/unpark_branches",
    json!({"repositoryPath": repository_path, "branchNames": ["test-branch"]}),
  )
  .await;
  sync(&client, addr, &repository_path).await;
  let free = plan_when(&client, addr, &repository_path, &intent, |plan| plan["blockers"] == json!([]) && plan["waits"] == json!([])).await;
  post(&client, addr, &format!("/repositories/{id}/patronus"), running_on("test-branch")).await;
  let refresh = json!({"repositoryPath": repository_path, "family": "robots", "reason": "manual"});
  post(&client, addr, "/invoke/refresh_facts", refresh).await;
  let (code, message) = refused(execute_plan(&client, addr, &repository_path, &intent, &free).await);
  assert_eq!(code, "robot_running");
  assert!(message.ends_with("Nothing was pushed or started: A Safe Push of the last commit is running"), "{message}");

  let patronus: serde_json::Value = client.get(format!("http://{addr}/repositories/{id}/patronus")).send().await.unwrap().json().await.unwrap();
  assert_eq!(patronus["starts"], json!([]));
  let github: serde_json::Value = client
    .get(format!("http://{addr}/repositories/{id}/pull-requests"))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
  let writes: Vec<&serde_json::Value> = github["requests"]
    .as_array()
    .unwrap()
    .iter()
    .filter(|request| request["method"] == "POST" && request["path"] != "/graphql")
    .collect();
  assert_eq!(writes, Vec::<&serde_json::Value>::new(), "no comment reached GitHub");
}

/// The merge profiles of the fixture, written as `robot-start-supported` answers them, read back through the real
/// service as the fixture; a start, a cancel and a restart (`prepare` and `execute` of their intents) run the engine and
/// the real service against the fake, which lists the robots they started, answers their status, and logs each request
/// (`GET …/patronus`)
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

  // the session of a window reads them when a start asks
  let mut window = subscribe(&client, addr, &id, &repository_path, 1).await;
  subscription(&mut window).await;
  let refresh = json!({"repositoryPath": repository_path, "family": "profiles", "reason": "afterWrite"});
  assert!(post(&client, addr, "/invoke/refresh_facts", refresh).await.is_object());
  let view = post(&client, addr, "/invoke/get_snapshot", json!({"repositoryPath": repository_path})).await;
  assert_eq!(view["facts"]["profiles"]["state"]["kind"], "fresh");
  let options: PatronusStartOptions = serde_json::from_value(view["facts"]["profiles"]["value"].clone()).unwrap();
  assert_eq!(
    (options.target_branch.as_str(), options.profiles, options.emergency_profiles),
    ("master", vec![lsc], vec![emergency])
  );

  let intent = land("test-branch", "dryRun", &json!({"kind": "id", "id": "lsc"}));
  let plan = plan_when(&client, addr, &repository_path, &intent, |plan| plan["waits"] == json!([])).await;
  let outcome = execute_plan(&client, addr, &repository_path, &intent, &plan).await;
  let started: PatronusRobot = serde_json::from_value(outcome["result"]["robot"].clone()).unwrap_or_else(|error| panic!("{error}: {outcome}"));
  assert_eq!(
    (started.id.as_str(), started.status, started.branch_name.as_deref(), started.source_revision.as_deref()),
    ("started-1", PatronusRobotStatus::Running, Some("test-branch"), Some(tip.as_str()))
  );
  // the cancel and the restart run through the engine too, over the robots of the session: the running robot is
  // canceled (its status read first: it could have finished), the canceling one restarted
  let cancel = json!({"kind": "cancelRobot", "robotId": "started-1"});
  let plan = plan_when(&client, addr, &repository_path, &cancel, |_| true).await;
  let outcome = execute_plan(&client, addr, &repository_path, &cancel, &plan).await;
  let canceled: PatronusRobot = serde_json::from_value(outcome["result"]["robot"].clone()).unwrap_or_else(|error| panic!("{error}: {outcome}"));
  assert_eq!((canceled.status, &outcome["result"]["alreadyFinished"]), (PatronusRobotStatus::Canceling, &json!(false)));
  let restart = json!({"kind": "restartRobot", "robotId": "started-1"});
  let plan = plan_when(&client, addr, &repository_path, &restart, |_| true).await;
  assert_eq!(
    plan["effects"],
    json!([{"kind": "restartRobot", "robotId": "started-1"}]),
    "the remote has the branch: a plain restart"
  );
  let outcome = execute_plan(&client, addr, &repository_path, &restart, &plan).await;
  let restarted: PatronusRobot = serde_json::from_value(outcome["result"]["robot"].clone()).unwrap_or_else(|error| panic!("{error}: {outcome}"));
  assert_eq!((restarted.id.as_str(), restarted.branch_name.as_deref()), ("restarted-1", Some("test-branch")));

  let log: serde_json::Value = client.get(format!("http://{addr}/repositories/{id}/patronus")).send().await.unwrap().json().await.unwrap();
  assert_eq!(
    (log["starts"].clone(), log["restarts"].clone(), log["cancels"].clone(), log["statusReads"].clone()),
    (
      json!([{"type": "ROBOT_WEB_UI", "repository": "repository", "sourceBranch": "refs/heads/user-name/virtual/test-branch", "targetBranch": "master",
        "mode": "DRY_RUN", "sourceBranchDeletePolicy": "NEVER", "mergeProfileId": "lsc"}]),
      json!(["started-1"]),
      json!(["started-1"]),
      json!(["started-1"]),
    )
  );
}

/// A robot that finished before its cancel: the cancel answers it as it ended (`alreadyFinished`), Patronus is asked no
/// cancel, and a start that replaces it goes on
#[tokio::test]
async fn a_cancel_of_a_robot_that_finished_is_answered_as_it_ended_and_a_start_goes_on() {
  let (addr, client, id, repository_path) = synced_on_space(&[]).await;
  let fixture = format!("/repositories/{id}/patronus");
  let mut failed = running_on("test-branch");
  failed["robots"][0]["status"] = json!("failure");
  failed["robots"][0]["finishTime"] = json!(1_704_142_800);
  post(&client, addr, &fixture, failed).await;

  let cancel = json!({"kind": "cancelRobot", "robotId": "r1"});
  let plan = plan_when(&client, addr, &repository_path, &cancel, |_| true).await;
  let outcome = execute_plan(&client, addr, &repository_path, &cancel, &plan).await;
  let finished: PatronusRobot = serde_json::from_value(outcome["result"]["robot"].clone()).unwrap_or_else(|error| panic!("{error}: {outcome}"));
  assert_eq!(
    (finished.id.as_str(), finished.status, &outcome["result"]["alreadyFinished"]),
    ("r1", PatronusRobotStatus::Failure, &json!(true))
  );
  assert_eq!(outcome["effects"][0]["status"]["kind"], "done", "{outcome}");
  start_dry_run_of_test_branch(&client, addr, &repository_path).await;
  let log: serde_json::Value = client.get(format!("http://{addr}{fixture}")).send().await.unwrap().json().await.unwrap();
  assert_eq!((log["cancels"].clone(), log["starts"].as_array().map(Vec::len)), (json!([]), Some(1)));
}

/// A Dry Run started through `execute` (the engine, `operations::engine::execute_land`) asks Patronus no list of its own and runs
/// no trial of its own (C2): the list of the session, asked once, serves its plan, its witness and its start, the landing
/// needs of the session its gate; the push reaches the remote and the fake logs one start of the branch. The outcome
/// tells each effect done
#[tokio::test]
async fn a_start_through_execute_asks_no_list_of_its_own() {
  let (addr, client, id, repository_path) = synced_on_space(&[]).await;
  // the plans of the view come with its Patronus view, over the robots of the session, the landing needs of the session
  // its gate: the plan waits for nothing then
  start_dry_run_of_test_branch(&client, addr, &repository_path).await;

  let log: serde_json::Value = client.get(format!("http://{addr}/repositories/{id}/patronus")).send().await.unwrap().json().await.unwrap();
  assert_eq!(log["requests"]["list"], 1, "the list of the session only");
  assert_eq!(log["starts"].as_array().map(Vec::len), Some(1));
}

/// A push of a branch whose own Safe Push runs is refused in Rust, by the run of the push intent on the robots the fake
/// lists now (the session knew of none: the robot started after its list), before git pushes anything
#[tokio::test]
async fn a_push_into_a_branch_whose_safe_push_runs_is_refused_before_git_pushes() {
  let (addr, client, id, repository_path) = synced_on_space(&[]).await;
  post(&client, addr, &format!("/repositories/{id}/patronus"), running_on("test-branch")).await;

  // the row offers the push: the session lists no robot of it yet
  let execute = execute_request(&client, addr, &repository_path, &push_intent("test-branch", None)).await;
  let answer = post(&client, addr, "/invoke/execute", execute).await;
  assert_eq!(
    (answer["status"].clone(), answer["error"]["code"].clone()),
    (json!("error"), json!("robot_running")),
    "{answer}"
  );
  let refusal = answer["error"]["message"].as_str().unwrap();
  assert!(
    refusal.contains(
      "Nothing was pushed: A Safe Push of test-branch is running: a push moves the branch it took its commit from, so it won't land and fails the Safe Push at the end. Wait until it merges, or restart it on the current branch ("
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

/// A start whose build needs a branch outside what it lands is refused by `execute` with every refusal of the landing
/// gate and its fix (`ErrorDetails::Landing`, which the toast offers): `lane-run` of the `landing_needs` template uses a
/// name only `stills-codec` adds, while no robot landed `stills-codec`. Start Anyway is the same start with the override
/// (`allow`), whose plan has nothing left to refuse. A merged Safe Push of `stills-codec` the fake lists counts its
/// commits as landed (`landedBy` of the view: the target itself never gets the commit here), so the plan of the start
/// refuses nothing either: the robots are the session's, never the page's
#[tokio::test]
async fn a_start_whose_build_needs_a_branch_outside_it_is_refused_with_the_fix() {
  let (addr, client, fixture, repository_path) = synced_landing_needs(json!({"availability": {"status": "available"}, "robots": []})).await;
  let intent = land("lane-run", "dryRun", &json!({"kind": "default"}));
  let plan = plan_when(&client, addr, &repository_path, &intent, |plan| plan["blockers"] != json!([])).await;
  let refused = execute_plan(&client, addr, &repository_path, &intent, &plan).await;
  assert_eq!(refused["error"]["code"], "missing_declaration", "{refused}");
  let refusals = &refused["error"]["details"]["refusals"];
  assert_eq!(
    (refused["error"]["details"]["type"].as_str(), refusals.as_array().map(Vec::len)),
    (Some("landing"), Some(1)),
    "{refused}"
  );
  assert_eq!((&refusals[0]["kind"], &refusals[0]["identifier"]), (&json!("missingDeclaration"), &json!("Stills")));
  assert_eq!(refusals[0]["fix"], json!({"kind": "restack", "branch": "lane-run", "onto": "stills-codec"}));

  let mut anyway = intent.clone();
  anyway["allow"] = json!({"allowMissing": true});
  let allowed = plan_when(&client, addr, &repository_path, &anyway, |plan| plan["waits"] == json!([])).await;
  assert_eq!(allowed["blockers"], json!([]), "{allowed}");

  // the Safe Push of stills-codec merged: the view counts its commits as landed (the target is unchanged), so lane-run's
  // start has what it needs
  post(&client, addr, &fixture, stills_codec_merged()).await;
  let landed = plan_when(&client, addr, &repository_path, &intent, |plan| plan["blockers"] == json!([]) && plan["waits"] == json!([])).await;
  assert_eq!(landed["lands"], json!(["lane-run"]), "{landed}");
}

/// A server with a repository of the `landing_needs` template, its Patronus fixture `patronus` set, synced: the server,
/// its client, the fixture path of the repository and its path
async fn synced_landing_needs(patronus: serde_json::Value) -> (std::net::SocketAddr, reqwest::Client, String, String) {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, repository_path) = create_repository_from(&client, addr, "landing_needs").await;
  let fixture = format!("/repositories/{id}/patronus");
  post(&client, addr, &fixture, patronus).await;
  sync(&client, addr, &repository_path).await;
  (addr, client, fixture, repository_path)
}

/// The Patronus fixture of the merged Safe Push of `stills-codec` of the `landing_needs` template
fn stills_codec_merged() -> serde_json::Value {
  json!({"availability": {"status": "available"}, "robots": [{
    "id": "landed", "name": "Safe Push of stills-codec", "status": "success", "pushMode": "rebaseAutosquash", "branchName": "stills-codec",
    "sourceRevision": "", "targetBranch": "master", "startTime": 1_704_139_200, "finishTime": 1_704_142_800, "cancellationReason": null,
    "url": "ignored",
  }]})
}

/// The push that restarts the Safe Push of its branch (`restartRunning`, the push of the toast of a remote that moved
/// under a restart): the running robot is canceled first, the branch pushed, and the robot restarted on the pushed
/// commit, in the order of its plan; the outcome names the new robot
#[tokio::test]
async fn a_push_that_restarts_the_safe_push_of_its_branch_cancels_pushes_and_restarts_it() {
  let (addr, client, id, repository_path) = synced_on_space(&[]).await;
  post(&client, addr, &format!("/repositories/{id}/patronus"), running_on("test-branch")).await;
  let intent = json!({"kind": "push", "subject": {"kind": "branch", "name": "test-branch"}, "scope": {"kind": "alone"},
    "expectRemote": null, "force": false, "restartRunning": true});
  // the session lists the robot and derives what it holds: the plan cancels it before the push and restarts it after
  let plan = plan_when(&client, addr, &repository_path, &intent, |plan| {
    plan["effects"].as_array().is_some_and(|effects| effects.len() == 3)
  })
  .await;
  assert_eq!(
    plan["effects"],
    json!([
      {"kind": "cancelRobot", "robotId": "r1"},
      {"kind": "push", "branches": ["test-branch"], "force": false},
      {"kind": "restartRobot", "robotId": "r1"},
    ]),
    "{plan}"
  );
  let outcome = execute_plan(&client, addr, &repository_path, &intent, &plan).await;
  let statuses: Vec<serde_json::Value> = outcome["effects"].as_array().unwrap().iter().map(|record| record["status"]["kind"].clone()).collect();
  assert_eq!(statuses, [json!("done"), json!("done"), json!("done")], "{outcome}");
  assert_eq!(outcome["result"]["restarted"]["id"], "restarted-1", "{outcome}");
  let log: serde_json::Value = client.get(format!("http://{addr}/repositories/{id}/patronus")).send().await.unwrap().json().await.unwrap();
  assert_eq!((log["cancels"].clone(), log["restarts"].clone()), (json!(["r1"]), json!(["r1"])));
}
