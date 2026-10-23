//! The reads of the Safe Push statistics (`spec/repository/safe-push-statistics.spec.md`), the one core of the app
//! (`get_robot_stats`), `bd robot-stats` and the E2E server: everyone's robots into the target of the last 70 days,
//! listed by UTC weeks, and the runs of the user's finished robots of the last 14 days, all kept in the object cache
//! (`patronus_history`), then `robot_stats_aggregate`. No git and no other module of this crate: in stage s3 of the
//! rework it moves to `remote-facts` as it is.
//!
//! - A week is listed once it settled (it ended 2 days ago and each of its robots finished): never again. The other
//!   weeks are listed again when their list is older than `LIST_TTL_SECONDS`, or on Refresh.
//! - A run is read once: a finished robot does not change. At most `MAX_RUNS_PER_READ` per read, the newest first; the
//!   next read asks for the rest.
//! - A failure keeps what was kept: the numbers are of it (`stale`), and a week never listed is missing.

use crate::robot_stats_aggregate::{RUNS_DAYS, WINDOW_DAYS, aggregate_robot_stats, needs_run};
use domain_types::calendar::Calendar;
use domain_types::patronus::PatronusAvailability;
use domain_types::robot_stats::{HistoryRobot, RobotRun, RobotStatsEvent, RobotStatsInput, RobotStatsResult};
use object_cache::cache::ObjectCache;
use patronus::service::PatronusService;
use patronus::stats::HistoryList;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use tracing::{info, instrument};

const DAY: u32 = 86_400;
pub const WEEK: u32 = 7 * DAY;
/// A list of a week that has not settled holds this long
pub const LIST_TTL_SECONDS: u32 = 300;
/// A week settles this long after its end, once each of its robots finished
const SETTLE_SECONDS: u32 = 2 * DAY;
/// The runs one read asks for at most
pub const MAX_RUNS_PER_READ: usize = 120;
/// The version of what is kept, part of every key
const KEPT_VERSION: &str = "1";

/// Where the statistics read
pub struct RobotStatsSource<'a> {
  pub service: &'a PatronusService,
  pub cache: &'a ObjectCache,
  /// The Space repository, e.g. `ultimate`
  pub repository: &'a str,
  /// The branch the robots push into, e.g. `master`
  pub target: &'a str,
  pub branch_prefix: &'a str,
}

/// A week of everyone's robots as it was listed
#[derive(Clone, Debug, Serialize, Deserialize)]
struct KeptWeek {
  fetched_at: u32,
  settled: bool,
  me: Option<u64>,
  robots: Vec<HistoryRobot>,
}

/// The statistics of the robots into the target of `source` at `now`, by `calendar`; `refresh` lists the weeks that
/// have not settled again whatever their age. `on_event` gets the progress, and the numbers of the lists before the
/// runs are read.
#[instrument(skip_all, fields(repository = source.repository, target = source.target, refresh))]
pub async fn robot_stats_core(source: &RobotStatsSource<'_>, calendar: &Calendar, now: u32, refresh: bool, on_event: &(dyn Fn(RobotStatsEvent) + Sync)) -> RobotStatsResult {
  let salt = salt(source);
  let window_start = now.saturating_sub(WINDOW_DAYS * DAY);
  let weeks: Vec<u32> = (window_start / WEEK * WEEK..=now).step_by(WEEK as usize).collect();
  let keys: Vec<[u8; 32]> = weeks
    .iter()
    .map(|week| key(source, &["week", source.repository, source.target, &week.to_string()]))
    .collect();
  let mut kept: Vec<Option<KeptWeek>> = source.cache.get_patronus_history(&keys);
  let to_list: Vec<usize> = (0..weeks.len())
    .filter(|&index| {
      kept[index]
        .as_ref()
        .is_none_or(|week| !week.settled && (refresh || now.saturating_sub(week.fetched_at) >= LIST_TTL_SECONDS))
    })
    .collect();
  let lists_total = u32::try_from(to_list.len()).unwrap_or(u32::MAX);
  let progress = |lists_read: usize, runs_read: usize, runs_total: usize| {
    on_event(RobotStatsEvent::Progress {
      lists_read: u32::try_from(lists_read).unwrap_or(u32::MAX),
      lists_total,
      runs_read: u32::try_from(runs_read).unwrap_or(u32::MAX),
      runs_total: u32::try_from(runs_total).unwrap_or(u32::MAX),
    });
  };
  progress(0, 0, 0);

  let mut failure: Option<PatronusAvailability> = None;
  let mut listed = Vec::new();
  let mut lists_read = 0;
  // two weeks at once
  for pair in to_list.chunks(2) {
    if failure == Some(PatronusAvailability::NotLoggedIn) {
      break;
    }
    let list = |index: usize| {
      source
        .service
        .robot_history(source.repository, source.target, weeks[index], (weeks[index] + WEEK).min(now), &salt)
    };
    let answers = match pair {
      [first, second] => {
        let (a, b) = tokio::join!(list(*first), list(*second));
        vec![(*first, a), (*second, b)]
      }
      [first] => vec![(*first, list(*first).await)],
      _ => Vec::new(),
    };
    for (index, answer) in answers {
      match answer {
        Ok(HistoryList { me, robots }) => {
          let settled = weeks[index] + WEEK + SETTLE_SECONDS < now && robots.iter().all(|robot| robot.status.is_finished());
          let week = KeptWeek {
            fetched_at: now,
            settled,
            me,
            robots,
          };
          listed.push((keys[index], week.clone()));
          kept[index] = Some(week);
        }
        Err(availability) => failure = Some(availability),
      }
    }
    lists_read += pair.len();
    progress(lists_read, 0, 0);
  }
  source.cache.put_patronus_history(&listed);

  let weeks_missing = u32::try_from(kept.iter().filter(|week| week.is_none()).count()).unwrap_or(u32::MAX);
  let fetched_at = kept.iter().flatten().map(|week| week.fetched_at).max();
  let me = kept.iter().rev().flatten().find_map(|week| week.me);
  let mut input = RobotStatsInput {
    target: source.target.to_string(),
    window_start,
    runs_start: now.saturating_sub(RUNS_DAYS * DAY),
    me,
    branch_prefix: source.branch_prefix.to_string(),
    robots: kept.into_iter().flatten().flat_map(|week| week.robots).collect(),
    runs: BTreeMap::new(),
    weeks_missing,
  };
  if input.robots.is_empty() && fetched_at.is_none() {
    return RobotStatsResult {
      availability: failure.unwrap_or(PatronusAvailability::Available),
      stale: false,
      fetched_at: None,
      stats: None,
    };
  }

  // the runs kept, then the numbers of the lists, then the runs not read yet
  let mut wanted: Vec<&HistoryRobot> = input.robots.iter().filter(|robot| needs_run(robot, input.me, input.runs_start)).collect();
  wanted.sort_by_key(|robot| std::cmp::Reverse(robot.start));
  let run_keys: Vec<[u8; 32]> = wanted.iter().map(|robot| key(source, &["run", &robot.id])).collect();
  let kept_runs: Vec<Option<RobotRun>> = source.cache.get_patronus_history(&run_keys);
  let mut missing = Vec::new();
  for (robot, run) in wanted.iter().zip(kept_runs) {
    match run {
      Some(run) => {
        input.runs.insert(robot.id.clone(), run);
      }
      None => missing.push((*robot).clone()),
    }
  }
  missing.truncate(MAX_RUNS_PER_READ);
  if !missing.is_empty() && failure != Some(PatronusAvailability::NotLoggedIn) {
    on_event(RobotStatsEvent::Partial(Box::new(aggregate_robot_stats(&input, calendar, now))));
    let runs_total = missing.len();
    progress(lists_read, 0, runs_total);
    let read = source.service.robot_runs(&missing, &|runs_read| progress(lists_read, runs_read, runs_total)).await;
    info!(read = read.runs.len(), asked = runs_total, "read the runs of the user's robots");
    let entries: Vec<([u8; 32], &RobotRun)> = read.runs.iter().map(|(id, run)| (key(source, &["run", id]), run)).collect();
    source.cache.put_patronus_history(&entries);
    input.runs.extend(read.runs);
    if let Some(read_failure) = read.failure {
      failure = Some(read_failure);
    }
  }

  RobotStatsResult {
    stale: failure.is_some(),
    availability: failure.unwrap_or(PatronusAvailability::Available),
    fetched_at,
    stats: Some(aggregate_robot_stats(&input, calendar, now)),
  }
}

/// The random salt of the owner keys of this cache and server, made once
fn salt(source: &RobotStatsSource<'_>) -> Vec<u8> {
  let key = key(source, &["salt"]);
  if let Some(Some(salt)) = source.cache.get_patronus_history::<Vec<u8>>(&[key]).pop() {
    return salt;
  }
  let salt = uuid::Uuid::new_v4().as_bytes().to_vec();
  source.cache.put_patronus_history(&[(key, &salt)]);
  salt
}

fn key(source: &RobotStatsSource<'_>, parts: &[&str]) -> [u8; 32] {
  let mut hash = Sha256::new().chain_update(KEPT_VERSION).chain_update([0]).chain_update(source.service.base_url());
  for part in parts {
    hash = hash.chain_update([0]).chain_update(part);
  }
  hash.finalize().into()
}

#[cfg(test)]
#[path = "robot_stats_test.rs"]
mod robot_stats_test;
