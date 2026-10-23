use bd_cli::fixture_test::{assert_covered, assert_error, run_json, writable_state};
use bd_cli::remote_fixture_test::{Space, minutes_ago, robot};
use bd_core::context::Environment;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

/// A finished robot started `minutes` ago that ran an hour, of the user or of another owner
fn finished(id: &str, status: &str, minutes: u32, owner: &str) -> Value {
  let mut listed = robot(id, status, id, "abc");
  listed["startDateTime"] = json!(minutes_ago(minutes));
  listed["finishDateTime"] = json!(minutes_ago(minutes - 60));
  listed["owner"] = json!({"type": "USER", "id": owner, "name": "Someone"});
  listed
}

fn robot_stats(environment: &Environment, args: &[&str]) -> (i32, Value) {
  let args: Vec<&str> = std::iter::once("robot-stats").chain(args.iter().copied()).collect();
  run_json(environment, &args)
}

#[test_log::test]
fn the_statistics_count_the_users_robots_against_everyone_elses_and_write_nothing() {
  let space = Space::new(&["(auth) Add auth"]);
  space.set_robots(vec![
    finished("merged", "SUCCESS", 600, "1"),
    finished("failed", "FAILURE", 900, "1"),
    finished("canceled", "CANCELED", 1200, "1"),
    finished("theirs", "SUCCESS", 600, "2"),
  ]);
  let before = writable_state(&space.repo);

  let (exit_code, output) = robot_stats(&space.environment(), &[]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(writable_state(&space.repo), before, "bd robot-stats is read only");
  assert_eq!((output["stale"].clone(), output.get("warning")), (json!(false), None));
  assert_eq!((output["window"]["target"].clone(), output["window"]["period"].clone()), (json!("master"), json!("30d")));
  assert_eq!(
    (output["mine"]["merged"].clone(), output["mine"]["failed"].clone(), output["mine"]["canceled"].clone()),
    (json!(1), json!(1), json!(1))
  );
  assert_eq!(output["mine"]["robots_per_merge"], json!(3.0));
  // one other person: too few for a median person
  assert_eq!(output["everyone"], Value::Null);
  // the failed robot's checks and problems were read: the built-in checks fail Fast Tests
  assert_eq!(output["failures"]["kinds"], json!([{"kind": "checks", "count": 1}]));
  let check = &output["failures"]["checks"][0];
  assert_eq!((check["name"].clone(), check["count"].clone()), (json!("Fast Tests"), json!(1)));
  // the newest robot it failed opens in the web UI
  assert!(check["robot_url"].as_str().is_some_and(|url| url.ends_with("/robot/failed")), "{check}");
  assert_eq!(output["failures"]["checks"].as_array().map(Vec::len), Some(1));
  assert_eq!(output["start_hours"].as_array().unwrap().len(), 24);
  let (_, schema) = run_json(&space.environment(), &["schema"]);
  assert_covered(&schema["outputs"]["robot_stats"], &schema["outputs"]["robot_stats"], &output, "robot_stats");

  // everyone's robots into the target, week by week: the UTC weeks of 30 days are 5 or 6, by the weekday
  let queries = space.fake.list_queries.lock().unwrap().clone();
  assert!((5..=6).contains(&queries.len()), "{}", queries.len());
  assert!(queries.iter().all(|query| !query.contains_key("owner") && query.contains_key("end")));

  // a second run asks Patronus nothing: the weeks and the runs are kept
  let lists = space.fake.list_calls.load(Ordering::SeqCst);
  let checks = space.fake.checks_calls.lock().unwrap().len();
  let (_, again) = robot_stats(&space.environment(), &[]);
  assert_eq!(again["mine"], output["mine"]);
  assert_eq!(
    (space.fake.list_calls.load(Ordering::SeqCst), space.fake.checks_calls.lock().unwrap().len()),
    (lists, checks)
  );

  // --refresh lists the weeks that may still change
  robot_stats(&space.environment(), &["--refresh"]);
  assert!(space.fake.list_calls.load(Ordering::SeqCst) > lists);
}

#[test_log::test]
fn the_statistics_say_why_there_are_none() {
  let space = Space::new(&["(auth) Add auth"]);
  // no robot, nothing kept, Patronus fails
  space.fake.set_status(503);
  let (exit_code, output) = robot_stats(&space.environment(), &[]);
  assert_error(&output, exit_code, (5, "patronus_error"));

  // no token
  let environment = Environment {
    home: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
    ..space.environment()
  };
  let (exit_code, output) = robot_stats(&environment, &[]);
  assert_error(&output, exit_code, (4, "not_logged_in"));
}

#[test_log::test]
fn a_failure_after_a_read_prints_the_kept_numbers_with_a_warning() {
  let space = Space::new(&["(auth) Add auth"]);
  space.set_robots(vec![finished("merged", "SUCCESS", 600, "1")]);
  robot_stats(&space.environment(), &[]);
  space.fake.set_status(503);

  let (exit_code, output) = robot_stats(&space.environment(), &["--refresh"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["stale"], json!(true));
  assert!(output["warning"].as_str().is_some_and(|warning| !warning.is_empty()), "{output}");
  assert_eq!(output["mine"]["merged"], json!(1));
}

#[test_log::test]
fn the_period_is_the_days_that_count() {
  let space = Space::new(&["(auth) Add auth"]);
  space.set_robots(vec![finished("recent", "SUCCESS", 600, "1"), finished("older", "SUCCESS", 20 * 24 * 60, "1")]);

  let (exit_code, output) = robot_stats(&space.environment(), &["--period", "14d"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((output["window"]["period"].clone(), output["mine"]["merged"].clone()), (json!("14d"), json!(1)));
  // 14 days are 3 UTC weeks, whatever the weekday
  assert_eq!(space.fake.list_queries.lock().unwrap().len(), 3);

  let (_, output) = robot_stats(&space.environment(), &["--period", "10w"]);
  assert_eq!((output["window"]["period"].clone(), output["mine"]["merged"].clone()), (json!("10w"), json!(2)));
  assert_eq!(space.fake.list_queries.lock().unwrap().len(), 11, "the 3 weeks kept are not listed again");

  let (exit_code, output) = robot_stats(&space.environment(), &["--period", "3d"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
}
