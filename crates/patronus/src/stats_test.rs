use crate::fake_patronus::{robot_json, start_fake};
use crate::service_test::{service, token};
use crate::stats::{owner_key, to_history_robot};
use domain_types::patronus::PatronusRobotStatus;
use domain_types::robot_failure::RobotFailureKind;
use domain_types::robot_stats::{CheckRun, HistoryRobot, HistoryRobotKind, RobotRun};
use pretty_assertions::assert_eq;
use std::sync::atomic::Ordering;

const SALT: &[u8] = b"salt";

fn others_robot(id: &str) -> serde_json::Value {
  let mut robot = robot_json(id, "SUCCESS", "refs/heads/jane/virtual/secret", "2026-10-14T09:30:00Z");
  robot["owner"] = serde_json::json!({"type": "USER", "id": "2", "name": "Jane", "email": "jane@example.com"});
  robot["finishDateTime"] = serde_json::json!("2026-10-14T10:30:00Z");
  robot["sourceRobotId"] = serde_json::json!("before");
  robot["type"] = serde_json::json!("SPACE_SAFE_MERGE");
  robot
}

#[tokio::test]
async fn the_history_lists_everyone_into_the_target_and_keeps_no_name_and_no_branch_of_others() {
  let (fake, client) = start_fake().await;
  let mut cherry_pick = robot_json("pick", "SUCCESS", "refs/heads/cherry", "2026-10-14T09:30:00Z");
  cherry_pick["type"] = serde_json::json!("SPACE_REVIEW_CHERRY_PICK");
  let mut release = robot_json("release", "SUCCESS", "refs/heads/me/virtual/x", "2026-10-14T09:30:00Z");
  release["targetBranch"] = serde_json::json!("263");
  *fake.robots.lock().unwrap() = Some(vec![
    robot_json("mine", "RUNNING", "refs/heads/me/virtual/IJPL-1", "2026-10-14T09:30:00Z"),
    others_robot("theirs"),
    cherry_pick,
    release,
  ]);
  let (service, _) = service(client, token("t"));

  // 2026-10-13 and 2026-10-20
  let history = service.robot_history("ultimate", "master", 1_791_849_600, 1_792_454_400, SALT).await.unwrap();
  let query = fake.last_query.lock().unwrap().clone();
  assert_eq!(query.get("owner"), None);
  assert_eq!(query.get("targetBranch").map(String::as_str), Some("master"));
  assert_eq!(query.get("end").map(String::as_str), Some("2026-10-20T00:00:00Z"));

  let me = history.me.unwrap();
  let ids: Vec<&str> = history.robots.iter().map(|robot| robot.id.as_str()).collect();
  assert_eq!(ids, vec!["mine", "theirs", "pick"]);
  assert_eq!(history.robots[0].source_branch.as_deref(), Some("me/virtual/IJPL-1"));
  assert_eq!(history.robots[0].owner, me);
  assert_eq!(
    history.robots[1],
    HistoryRobot {
      id: "theirs".to_string(),
      owner: history.robots[1].owner,
      user: true,
      kind: HistoryRobotKind::SpaceSafeMerge,
      status: PatronusRobotStatus::Success,
      dry_run: false,
      emergency: false,
      start: 1_791_970_200,
      finish: Some(1_791_973_800),
      source_robot_id: Some("before".to_string()),
      // another person's branch is not kept
      source_branch: None,
    }
  );
  assert_ne!(history.robots[1].owner, me);
  assert_eq!(history.robots[2].kind, HistoryRobotKind::CherryPick);
}

#[test]
fn the_owner_key_depends_on_the_salt() {
  let dto: crate::client::RobotDto = serde_json::from_value(others_robot("x")).unwrap();
  let owner = dto.owner.clone().unwrap();
  assert_ne!(owner_key(SALT, &owner), owner_key(b"other", &owner));
  assert_eq!(to_history_robot(&dto, SALT, None).unwrap().owner, owner_key(SALT, &owner));
}

fn history_robot(id: &str, status: PatronusRobotStatus) -> HistoryRobot {
  HistoryRobot {
    id: id.to_string(),
    owner: 1,
    user: true,
    kind: HistoryRobotKind::WebUi,
    status,
    dry_run: false,
    emergency: false,
    start: 1,
    finish: Some(2),
    source_robot_id: None,
    source_branch: None,
  }
}

#[tokio::test]
async fn at_most_four_robots_are_read_at_once_and_a_rejected_token_stops_the_reads() {
  let (fake, client) = start_fake().await;
  fake.checks_overlap.set(4);
  let (service, _) = service(client, token("t"));
  let robots: Vec<HistoryRobot> = (0..10).map(|index| history_robot(&format!("r{index}"), PatronusRobotStatus::Canceled)).collect();
  let counted = std::sync::Mutex::new(0);
  let read = service.robot_runs(&robots, &|count| *counted.lock().unwrap() = count).await;
  assert_eq!((read.runs.len(), *counted.lock().unwrap()), (10, 10));
  assert_eq!(fake.max_checks_in_flight.load(Ordering::SeqCst), 4);

  fake.set_status(401);
  let read = service.robot_runs(&robots, &|_| {}).await;
  assert_eq!(read.failure, Some(domain_types::patronus::PatronusAvailability::NotLoggedIn));
  assert_eq!(read.runs, Vec::<(String, RobotRun)>::new());
}

#[tokio::test]
async fn a_failed_run_reads_the_checks_and_problems_a_merged_one_the_checks_and_changes() {
  let (fake, client) = start_fake().await;
  fake.checks.lock().unwrap().insert(
    "merged".to_string(),
    serde_json::json!([
      {"id": "1", "name": "UI Tests", "buildConfigurationName": "UI", "status": "SUCCESS", "attempts": [
        {"number": 0, "buildId": "1", "status": "FAILURE"}, {"number": 1, "buildId": "2", "status": "SUCCESS"}]},
      {"id": "2", "buildConfigurationName": "Optional", "status": "SKIPPED"},
    ]),
  );
  fake
    .changes
    .lock()
    .unwrap()
    .insert("merged".to_string(), serde_json::json!({"type": "DELIVERED", "commitsNumber": 4, "topCommits": []}));
  let (service, _) = service(client, token("t"));

  let read = service
    .robot_runs(
      &[history_robot("failed", PatronusRobotStatus::Failure), history_robot("merged", PatronusRobotStatus::Success)],
      &|_| {},
    )
    .await;
  assert_eq!(read.failure, None);
  let run = |id: &str| read.runs.iter().find(|(run_id, _)| run_id == id).unwrap().1.clone();
  let failed = run("failed");
  assert_eq!(failed.failure_kind, Some(RobotFailureKind::Checks));
  assert_eq!(failed.failed_checks, vec!["Fast Tests".to_string()]);
  assert_eq!(
    failed.checks,
    vec![
      CheckRun {
        name: "Fast Tests".to_string(),
        passed: false,
        attempts: 1
      },
      CheckRun {
        name: "Compilation".to_string(),
        passed: true,
        attempts: 1
      }
    ]
  );

  assert_eq!(
    run("merged"),
    RobotRun {
      failure_kind: None,
      failed_checks: Vec::new(),
      checks: vec![CheckRun {
        name: "UI Tests".to_string(),
        passed: true,
        attempts: 2
      }],
      commits: Some(4),
    }
  );
  assert_eq!(*fake.problem_calls.lock().unwrap(), vec!["failed".to_string()]);
  assert_eq!(*fake.changes_calls.lock().unwrap(), vec!["merged".to_string()]);
  let mut checked = fake.checks_calls.lock().unwrap().clone();
  checked.sort();
  assert_eq!(checked, vec!["failed".to_string(), "merged".to_string()]);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 0);
}
