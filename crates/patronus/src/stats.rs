//! The reads of the Safe Push statistics (`remote_facts::robot_stats`, `spec/repository/safe-push-statistics.spec.md`):
//! everyone's robots into a target in a window, and what the checks, problems and changes of one of the user's finished
//! robots tell. Foreground requests: the user opened the statistics. Of others only what the rules count is kept: no
//! name, no email, no branch, the owner as a salted hash.

use crate::actions::finished_failure;
use crate::client::{PatronusClient, PatronusError, RobotDto, RobotOwnerDto, TeamCityCheckDto};
use crate::service::{PatronusService, parse_status};
use domain_types::patronus::{PatronusAvailability, PatronusRobotStatus};
use domain_types::robot_stats::{CheckRun, HistoryRobot, HistoryRobotKind, RobotRun};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use sync_utils::timestamp::parse_rfc3339_timestamp;
use tokio::sync::Semaphore;
use tracing::{Instrument, instrument};

/// The robots whose runs are read at once, each with two requests
pub const MAX_RUN_READS: usize = 4;

/// The robots of a window, and who asks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryList {
  pub me: Option<u64>,
  pub robots: Vec<HistoryRobot>,
}

impl PatronusService {
  /// Everyone's robots into `target` started in `[start, end)`, the owners hashed with `salt`
  #[instrument(skip(self, salt))]
  pub async fn robot_history(&self, repository: &str, target: &str, start: u32, end: u32, salt: &[u8]) -> Result<HistoryList, PatronusAvailability> {
    let token = self.token(false).map_err(|result| result.availability)?;
    let response = self.client().list_target_robots(&token.value, repository, target, start, end).await;
    let (me, robots) = self.settle(response, &token.stamp)?;
    let me = me.as_ref().map(|me| owner_key(salt, me));
    Ok(HistoryList {
      me,
      robots: robots.iter().filter_map(|robot| to_history_robot(robot, salt, me)).collect(),
    })
  }

  /// What finished robots of the user tell, at most `MAX_RUN_READS` robots at once, each with two requests: their
  /// checks, and of a failed one its problems, of a merged one its changes. `on_read` gets the count read so far. A
  /// robot Patronus fails for is left out (asked again by the next read); a rejected token stops the reads
  #[instrument(skip_all, fields(count = robots.len()))]
  pub async fn robot_runs(&self, robots: &[HistoryRobot], on_read: &(dyn Fn(usize) + Sync)) -> RunsRead {
    let token = match self.token(false) {
      Ok(token) => token,
      Err(result) => {
        return RunsRead {
          runs: Vec::new(),
          failure: Some(result.availability),
        };
      }
    };
    let requests = Arc::new(Semaphore::new(MAX_RUN_READS));
    let mut pending = tokio::task::JoinSet::new();
    for robot in robots {
      let client = self.client().clone();
      let token = token.value.clone();
      let requests = Arc::clone(&requests);
      let (id, status) = (robot.id.clone(), robot.status);
      pending.spawn(
        async move {
          let _permit = requests.acquire_owned().await;
          let read = read_run(&client, &token, &id, status).await;
          (id, read)
        }
        .in_current_span(),
      );
    }
    let mut read = RunsRead { runs: Vec::new(), failure: None };
    let mut answered = 0;
    while let Some(Ok((id, answer))) = pending.join_next().await {
      answered += 1;
      on_read(answered);
      match self.settle(answer, &token.stamp) {
        Ok(run) => read.runs.push((id, run)),
        Err(PatronusAvailability::NotLoggedIn) => {
          pending.abort_all();
          read.failure = Some(PatronusAvailability::NotLoggedIn);
          break;
        }
        Err(failure) => read.failure = Some(failure),
      }
    }
    read
  }
}

/// The runs a read got, and the last failure of Patronus, if any
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunsRead {
  pub runs: Vec<(String, RobotRun)>,
  pub failure: Option<PatronusAvailability>,
}

async fn read_run(client: &PatronusClient, token: &str, id: &str, status: PatronusRobotStatus) -> Result<RobotRun, PatronusError> {
  let checks = client.get_teamcity_checks(token, id);
  Ok(match status {
    PatronusRobotStatus::Failure => {
      let (checks, problems) = tokio::join!(checks, client.get_problems(token, id));
      let (checks, problems) = (checks?, problems?);
      let (kind, failed_checks) = finished_failure(&checks, &problems);
      RobotRun {
        failure_kind: Some(kind),
        failed_checks,
        checks: check_runs(&checks),
        commits: None,
      }
    }
    PatronusRobotStatus::Success => {
      let (checks, changes) = tokio::join!(checks, client.get_robot_changes(token, id));
      let (checks, changes) = (checks?, changes?);
      RobotRun {
        failure_kind: None,
        failed_checks: Vec::new(),
        checks: check_runs(&checks),
        commits: changes.commits_number.filter(|_| changes.kind == "DELIVERED"),
      }
    }
    _ => RobotRun {
      failure_kind: None,
      failed_checks: Vec::new(),
      checks: check_runs(&checks.await?),
      commits: None,
    },
  })
}

/// The key of an owner: the first 8 bytes of `sha256(salt, type, id)`
pub fn owner_key(salt: &[u8], owner: &RobotOwnerDto) -> u64 {
  let digest = Sha256::new()
    .chain_update(salt)
    .chain_update(owner.kind.as_bytes())
    .chain_update([0])
    .chain_update(owner.id.as_bytes())
    .finalize();
  let mut bytes = [0; 8];
  bytes.copy_from_slice(&digest[..8]);
  u64::from_be_bytes(bytes)
}

/// A robot of a list as the statistics keep it; None without a start time or an owner
pub fn to_history_robot(robot: &RobotDto, salt: &[u8], me: Option<u64>) -> Option<HistoryRobot> {
  let owner_dto = robot.owner.as_ref()?;
  let owner = owner_key(salt, owner_dto);
  let mine = me == Some(owner);
  Some(HistoryRobot {
    id: robot.id.clone(),
    owner,
    user: owner_dto.kind == "USER",
    kind: robot_kind(robot.kind.as_deref()),
    status: parse_status(&robot.status),
    dry_run: robot.push_mode == "DRY_RUN",
    emergency: robot.is_emergency,
    start: parse_rfc3339_timestamp(&robot.start_date_time)?,
    finish: robot.finish_date_time.as_deref().and_then(parse_rfc3339_timestamp),
    source_robot_id: robot.source_robot_id.clone().filter(|id| !id.is_empty()),
    source_branch: robot
      .source_branch
      .as_deref()
      .filter(|_| mine)
      .map(|branch| branch.strip_prefix("refs/heads/").unwrap_or(branch).to_string()),
  })
}

fn robot_kind(kind: Option<&str>) -> HistoryRobotKind {
  match kind {
    Some("SAFE_PUSH") => HistoryRobotKind::SafePush,
    Some("WEB_UI") => HistoryRobotKind::WebUi,
    Some("SPACE_SAFE_MERGE") => HistoryRobotKind::SpaceSafeMerge,
    Some("GITHUB_PR") => HistoryRobotKind::GithubPr,
    Some("CUSTOM") => HistoryRobotKind::Custom,
    Some(kind) if kind.ends_with("CHERRY_PICK") => HistoryRobotKind::CherryPick,
    _ => HistoryRobotKind::Other,
  }
}

/// The checks that passed or failed, with how many attempts they took
fn check_runs(checks: &[TeamCityCheckDto]) -> Vec<CheckRun> {
  checks
    .iter()
    .filter(|check| matches!(check.status.as_str(), "SUCCESS" | "FAILURE"))
    .map(|check| CheckRun {
      name: check.label().to_string(),
      passed: check.status == "SUCCESS",
      attempts: u32::try_from(check.attempts.len()).unwrap_or(u32::MAX).max(1),
    })
    .collect()
}

#[cfg(test)]
#[path = "stats_test.rs"]
mod stats_test;
