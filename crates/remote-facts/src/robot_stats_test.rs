use crate::robot_stats::{LIST_TTL_SECONDS, MAX_RUNS_PER_READ, RobotStatsReads, RobotStatsSource, read_key, robot_stats_core};
use domain_types::calendar::Calendar;
use domain_types::patronus::PatronusAvailability;
use domain_types::robot_stats::{RobotStatsEvent, RobotStatsResult, StatsPeriod};
use object_cache::cache::{CacheConfig, ObjectCache};
use patronus::fake_patronus::{FakePatronus, robot_json, start_fake};
use patronus::service::PatronusService;
use patronus::token::{Token, TokenLookup, TokenStamp};
use pretty_assertions::assert_eq;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sync_utils::single_flight::Begin;
use sync_utils::timestamp::format_utc_timestamp;

/// Friday 2026-10-23 10:00 UTC
const NOW: u32 = 1_792_749_600;
const HOUR: u32 = 3_600;
const DAY: u32 = 86_400;
/// The weeks of 70 days from `NOW`, by UTC epoch weeks: 2026-08-14 is in the week of 2026-08-13
const WEEKS: usize = 11;

struct Setup {
  fake: Arc<FakePatronus>,
  service: PatronusService,
  cache: ObjectCache,
  _dir: tempfile::TempDir,
}

async fn setup(token: bool) -> Setup {
  let (fake, client) = start_fake().await;
  let service = PatronusService::new(client, move |_| {
    if token {
      TokenLookup::Found(Token {
        value: "t".to_string(),
        stamp: TokenStamp::Env,
      })
    } else {
      TokenLookup::Missing
    }
  });
  let dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(dir.path(), &CacheConfig::default());
  Setup { fake, service, cache, _dir: dir }
}

/// A finished robot, started `ago` before `NOW`; of the user (owner 1) unless `owner` names another
fn robot(id: &str, status: &str, ago: u32, owner: &str) -> serde_json::Value {
  let mut robot = robot_json(id, status, &format!("refs/heads/me/virtual/{id}"), &format_utc_timestamp(NOW - ago));
  robot["finishDateTime"] = serde_json::json!(format_utc_timestamp(NOW - ago + HOUR));
  robot["owner"] = serde_json::json!({"type": "USER", "id": owner, "name": "Someone"});
  robot
}

/// A read of 10 weeks
async fn read(setup: &Setup, now: u32, refresh: bool) -> (RobotStatsResult, Vec<RobotStatsEvent>) {
  read_with(setup, now, StatsPeriod::Weeks10, refresh, None).await
}

async fn read_period(setup: &Setup, now: u32, period: StatsPeriod, reads: Option<&RobotStatsReads>) -> (RobotStatsResult, Vec<RobotStatsEvent>) {
  read_with(setup, now, period, false, reads).await
}

fn source_of<'a>(setup: &'a Setup, reads: Option<&'a RobotStatsReads>) -> RobotStatsSource<'a> {
  RobotStatsSource {
    service: &setup.service,
    cache: &setup.cache,
    repository: "ultimate",
    target: "master",
    branch_prefix: "me",
    reads,
  }
}

async fn read_with(setup: &Setup, now: u32, period: StatsPeriod, refresh: bool, reads: Option<&RobotStatsReads>) -> (RobotStatsResult, Vec<RobotStatsEvent>) {
  let source = source_of(setup, reads);
  let events = Mutex::new(Vec::new());
  let result = robot_stats_core(&source, &Calendar::utc(), now, period, refresh, &|event| events.lock().unwrap().push(event)).await;
  (result, events.into_inner().unwrap())
}

fn lists(setup: &Setup) -> u32 {
  setup.fake.list_calls.load(Ordering::SeqCst)
}

#[tokio::test]
async fn the_weeks_are_listed_once_then_only_those_that_have_not_settled_after_the_ttl() {
  let setup = setup(true).await;
  *setup.fake.robots.lock().unwrap() = Some(vec![
    robot("merged", "SUCCESS", DAY, "1"),
    robot("failed", "FAILURE", 2 * DAY, "1"),
    robot("canceled", "CANCELED", 2 * DAY, "1"),
    robot("old", "SUCCESS", 30 * DAY, "1"),
    robot("theirs", "SUCCESS", DAY, "2"),
  ]);

  let (result, events) = read(&setup, NOW, false).await;
  assert_eq!(result.availability, PatronusAvailability::Available);
  assert!(!result.stale);
  let stats = result.stats.unwrap();
  assert_eq!((stats.mine.merged, stats.mine.failed, stats.mine.canceled), (2, 1, 1));
  assert_eq!(stats.coverage.runs_missing, 0);
  let queries = setup.fake.list_queries.lock().unwrap().clone();
  assert_eq!(queries.len(), WEEKS);
  assert!(
    queries
      .iter()
      .all(|query| query.get("owner").is_none() && query.get("targetBranch").map(String::as_str) == Some("master") && query.contains_key("end"))
  );
  // the runs of the user's merged and failed robots of the last 14 days, not of the canceled, old or others' ones
  let mut checked = setup.fake.checks_calls.lock().unwrap().clone();
  checked.sort();
  assert_eq!(checked, vec!["failed".to_string(), "merged".to_string()]);
  assert_eq!(*setup.fake.problem_calls.lock().unwrap(), vec!["failed".to_string()]);
  assert!(matches!(events.first(), Some(RobotStatsEvent::Progress { lists_total: 11, .. })));
  assert!(
    events.iter().any(|event| matches!(event, RobotStatsEvent::Partial(_))),
    "the numbers of the lists before the runs"
  );

  // within the TTL nothing is asked
  read(&setup, NOW + 60, false).await;
  assert_eq!(setup.fake.list_calls.load(Ordering::SeqCst), u32::try_from(WEEKS).unwrap());
  assert_eq!(setup.fake.checks_calls.lock().unwrap().len(), 2);

  // after it, the weeks that ended less than 2 days ago or have a running robot: this week, which began on Thursday
  // (UTC epoch weeks), and the one that ended then
  read(&setup, NOW + LIST_TTL_SECONDS + 60, false).await;
  assert_eq!(setup.fake.list_calls.load(Ordering::SeqCst), u32::try_from(WEEKS).unwrap() + 2);
  assert_eq!(setup.fake.checks_calls.lock().unwrap().len(), 2, "a run is read once");

  // Refresh lists it again within the TTL
  read(&setup, NOW + LIST_TTL_SECONDS + 120, true).await;
  assert_eq!(setup.fake.list_calls.load(Ordering::SeqCst), u32::try_from(WEEKS).unwrap() + 4);
}

#[tokio::test]
async fn a_failure_keeps_what_was_kept_and_says_it_is_stale() {
  let setup = setup(true).await;
  *setup.fake.robots.lock().unwrap() = Some(vec![robot("merged", "SUCCESS", DAY, "1")]);
  read(&setup, NOW, false).await;

  setup.fake.set_status(503);
  let (result, _) = read(&setup, NOW + LIST_TTL_SECONDS + 60, false).await;
  assert!(result.stale);
  assert!(matches!(result.availability, PatronusAvailability::Error { .. }));
  assert_eq!(result.stats.unwrap().mine.merged, 1);
}

#[tokio::test]
async fn nothing_is_asked_without_a_token_and_nothing_kept_has_no_numbers() {
  let setup = setup(false).await;
  let (result, _) = read(&setup, NOW, false).await;
  assert_eq!(result.availability, PatronusAvailability::NotLoggedIn);
  assert_eq!(result.stats, None);
  assert_eq!(setup.fake.list_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_read_asks_for_at_most_the_newest_runs_and_the_next_one_for_the_rest() {
  let setup = setup(true).await;
  let count = MAX_RUNS_PER_READ + 5;
  let robots = (0..count)
    .map(|index| robot(&format!("r{index:03}"), "SUCCESS", HOUR + u32::try_from(index).unwrap() * 60, "1"))
    .collect();
  *setup.fake.robots.lock().unwrap() = Some(robots);
  setup.fake.checks_overlap.set(4);

  let (result, _) = read(&setup, NOW, false).await;
  assert_eq!(result.stats.unwrap().coverage.runs_missing, 5);
  assert_eq!(setup.fake.checks_calls.lock().unwrap().len(), MAX_RUNS_PER_READ);
  assert!(!setup.fake.checks_calls.lock().unwrap().contains(&format!("r{:03}", count - 1)), "the oldest wait");
  assert_eq!(setup.fake.max_checks_in_flight.load(Ordering::SeqCst), 4);

  let (result, _) = read(&setup, NOW + 60, false).await;
  assert_eq!(result.stats.unwrap().coverage.runs_missing, 0);
  assert_eq!(setup.fake.checks_calls.lock().unwrap().len(), count);
}

#[tokio::test]
async fn a_longer_period_lists_only_the_weeks_not_kept_and_a_shorter_one_asks_nothing() {
  let setup = setup(true).await;
  *setup.fake.robots.lock().unwrap() = Some(vec![robot("merged", "SUCCESS", DAY, "1"), robot("older", "SUCCESS", 20 * DAY, "1")]);

  // 30 days from Friday reach back to the UTC epoch week of Thursday 2026-09-17: 6 weeks
  let (result, _) = read_period(&setup, NOW, StatsPeriod::Days30, None).await;
  let stats = result.stats.unwrap();
  assert_eq!((stats.window.period, stats.mine.merged), (StatsPeriod::Days30, 2));
  assert_eq!(lists(&setup), 6);

  // the 3 weeks of 14 days are kept: nothing is asked, and the older merge is out of the window
  let (result, _) = read_period(&setup, NOW + 60, StatsPeriod::Days14, None).await;
  let stats = result.stats.unwrap();
  assert_eq!((stats.window.period, stats.mine.merged), (StatsPeriod::Days14, 1));
  assert_eq!(lists(&setup), 6);

  // 6 months are 27 weeks: only the 21 not kept are listed
  read_period(&setup, NOW + 120, StatsPeriod::Months6, None).await;
  assert_eq!(lists(&setup), 27);
}

#[tokio::test]
async fn a_read_with_nothing_to_ask_does_not_wait_for_a_running_read() {
  let setup = setup(true).await;
  *setup.fake.robots.lock().unwrap() = Some(vec![robot("merged", "SUCCESS", DAY, "1")]);
  let reads = RobotStatsReads::default();
  read_period(&setup, NOW, StatsPeriod::Days30, Some(&reads)).await;

  // another read asks Patronus: one from the cache answers, one that must ask waits for it
  let Begin::Lead(running) = reads.begin(read_key(&source_of(&setup, None))) else {
    panic!("no read in flight");
  };
  let (kept, _) = tokio::time::timeout(Duration::from_secs(5), read_period(&setup, NOW + 60, StatsPeriod::Days14, Some(&reads)))
    .await
    .expect("a read from the cache does not wait");
  assert_eq!(kept.stats.unwrap().mine.merged, 1);
  let asking = read_with(&setup, NOW + 60, StatsPeriod::Days14, true, Some(&reads));
  tokio::pin!(asking);
  assert!(
    tokio::time::timeout(Duration::from_millis(200), &mut asking).await.is_err(),
    "a Refresh waits for the running read"
  );
  drop(running);
  asking.await;
  assert_eq!(lists(&setup), 6 + 2, "the Refresh lists the two weeks that have not settled");
}

#[tokio::test]
async fn two_reads_at_once_ask_once() {
  let setup = setup(true).await;
  *setup.fake.robots.lock().unwrap() = Some(vec![robot("merged", "SUCCESS", DAY, "1"), robot("theirs", "SUCCESS", DAY, "2")]);
  let reads = RobotStatsReads::default();

  let ((first, _), (second, _)) = tokio::join!(
    read_period(&setup, NOW, StatsPeriod::Days30, Some(&reads)),
    read_period(&setup, NOW, StatsPeriod::Days30, Some(&reads))
  );
  assert_eq!(lists(&setup), 6, "the second read waited and found the weeks kept");
  assert_eq!(setup.fake.checks_calls.lock().unwrap().len(), 1);
  assert_eq!(first.stats, second.stats);
}
