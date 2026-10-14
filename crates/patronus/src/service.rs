//! Asks Patronus on behalf of the app with the least load on it: a rejected token is not sent again until the Safe
//! Push CLI renews it, background requests stop while Patronus fails (exponential backoff), and a full list asked by
//! two windows of one repository within a few seconds is fetched once.

use crate::client::{PatronusClient, PatronusError, RobotDto, RobotUpdateRequest};
use crate::token::{TokenLookup, TokenStamp};
use git_ops::model::{BranchKind, strip_branch_namespace};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use sync_types::patronus::{KnownRobotStatus, PatronusAvailability, PatronusPushMode, PatronusRobot, PatronusRobotStatus, PatronusRobotsResult};
use sync_utils::timestamp::{parse_rfc3339_timestamp, unix_now};
use tracing::{info, instrument};

/// The window of a full list, as the Patronus default
pub const FULL_LIST_DAYS: u32 = 7;
/// A full list this fresh is reused instead of asking again
const FULL_LIST_REUSE: Duration = Duration::from_secs(10);
const BACKOFF_FIRST: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(600);

type TokenSource = Box<dyn Fn(u32) -> TokenLookup + Send + Sync>;

pub struct PatronusService {
  client: PatronusClient,
  token_source: TokenSource,
  state: Mutex<ServiceState>,
}

#[derive(Default)]
pub(crate) struct ServiceState {
  /// Patronus rejected this token: background requests wait for a new one
  rejected: Option<TokenStamp>,
  backoff: Option<Backoff>,
  /// By repository: the last full list and when it was fetched
  full_lists: HashMap<String, (Instant, Vec<RobotDto>)>,
  /// By robot id; a finished robot does not change any more
  pub(crate) details: HashMap<String, (Instant, crate::actions::CachedDetails)>,
  /// By `project/repository/target`: the answer of `robot-start-supported`
  pub(crate) start_supported: HashMap<String, (Instant, Result<crate::client::StartSupportedDto, String>)>,
}

struct Backoff {
  until: Instant,
  delay: Duration,
  message: String,
}

impl PatronusService {
  /// `token_source` gets the current Unix seconds, see `token::find_user_token`
  pub fn new(client: PatronusClient, token_source: impl Fn(u32) -> TokenLookup + Send + Sync + 'static) -> Self {
    Self {
      client,
      token_source: Box::new(token_source),
      state: Mutex::new(ServiceState::default()),
    }
  }

  /// The user's robots in the Space repository `repository` (e.g. `ultimate`) started since `since`, or in the last
  /// `FULL_LIST_DAYS` days
  #[instrument(skip(self))]
  pub async fn robots(&self, repository: &str, branch_prefix: &str, since: Option<u32>, background: bool) -> PatronusRobotsResult {
    let token = match self.token(background) {
      Ok(token) => token,
      Err(result) => return result,
    };
    if since.is_none()
      && let Some(robots) = self.fresh_full_list(repository)
    {
      return self.to_result(&robots, branch_prefix);
    }
    let start = since.unwrap_or_else(|| unix_now().saturating_sub(FULL_LIST_DAYS * 86_400));
    let response = self.client.list_robots(&token.value, repository, start).await;
    match self.settle(response, &token.stamp) {
      Ok(robots) => {
        if since.is_none() {
          self.lock().full_lists.insert(repository.to_string(), (Instant::now(), robots.clone()));
        }
        self.to_result(&robots, branch_prefix)
      }
      Err(availability) => PatronusRobotsResult::from(availability),
    }
  }

  /// The robots of `known` whose status changed; the others are not in the result. Always a background request.
  #[instrument(skip(self, known), fields(count = known.len()))]
  pub async fn poll(&self, branch_prefix: &str, known: &[KnownRobotStatus]) -> PatronusRobotsResult {
    let requests: Vec<RobotUpdateRequest> = known
      .iter()
      .filter_map(|robot| {
        Some(RobotUpdateRequest {
          id: robot.id.clone(),
          last_known_status: status_name(robot.status)?.to_string(),
        })
      })
      .collect();
    if requests.is_empty() {
      return PatronusRobotsResult {
        availability: PatronusAvailability::Available,
        robots: Vec::new(),
      };
    }
    let token = match self.token(true) {
      Ok(token) => token,
      Err(result) => return result,
    };
    let response = self.client.fetch_updates(&token.value, &requests).await;
    match self.settle(response, &token.stamp) {
      Ok(robots) => self.to_result(&robots, branch_prefix),
      Err(availability) => PatronusRobotsResult::from(availability),
    }
  }

  /// The token to send, or why nothing is sent
  pub(crate) fn token(&self, background: bool) -> Result<crate::token::Token, PatronusRobotsResult> {
    let token = match (self.token_source)(unix_now()) {
      TokenLookup::Found(token) => token,
      TokenLookup::Missing | TokenLookup::Expired(_) => return Err(PatronusRobotsResult::from(PatronusAvailability::NotLoggedIn)),
    };
    let state = self.lock();
    if background {
      // a rejected token costs Patronus a Space call on every request: wait until the CLI writes a new one
      if state.rejected.as_ref() == Some(&token.stamp) {
        return Err(PatronusRobotsResult::from(PatronusAvailability::NotLoggedIn));
      }
      if let Some(backoff) = state.backoff.as_ref().filter(|backoff| Instant::now() < backoff.until) {
        return Err(PatronusRobotsResult::from(PatronusAvailability::Error { message: backoff.message.clone() }));
      }
    }
    Ok(token)
  }

  /// Records the outcome (rejected token, backoff) and maps a failure to the availability
  pub(crate) fn settle<T>(&self, response: Result<T, PatronusError>, stamp: &TokenStamp) -> Result<T, PatronusAvailability> {
    let mut state = self.lock();
    match response {
      Ok(value) => {
        state.rejected = None;
        state.backoff = None;
        Ok(value)
      }
      Err(PatronusError::Unauthorized) => {
        info!("Patronus rejected the token; background requests wait for a new one");
        state.rejected = Some(stamp.clone());
        Err(PatronusAvailability::NotLoggedIn)
      }
      Err(e) => {
        let message = e.message();
        if e.is_transient() {
          let delay = state.backoff.as_ref().map_or(BACKOFF_FIRST, |backoff| (backoff.delay * 2).min(BACKOFF_MAX));
          info!(delay_seconds = delay.as_secs(), error = %message, "Patronus failed; background requests back off");
          state.backoff = Some(Backoff {
            until: Instant::now() + delay,
            delay,
            message: message.clone(),
          });
        }
        Err(PatronusAvailability::Error { message })
      }
    }
  }

  fn fresh_full_list(&self, repository: &str) -> Option<Vec<RobotDto>> {
    let state = self.lock();
    let (fetched_at, robots) = state.full_lists.get(repository)?;
    (fetched_at.elapsed() < FULL_LIST_REUSE).then(|| robots.clone())
  }

  pub(crate) fn client(&self) -> &PatronusClient {
    &self.client
  }

  fn to_result(&self, robots: &[RobotDto], branch_prefix: &str) -> PatronusRobotsResult {
    let mut robots: Vec<PatronusRobot> = robots.iter().map(|robot| to_robot(robot, branch_prefix, &self.client)).collect();
    robots.sort_by_key(|robot| std::cmp::Reverse(robot.start_time));
    PatronusRobotsResult {
      availability: PatronusAvailability::Available,
      robots,
    }
  }

  pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, ServiceState> {
    self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }
}

pub fn to_robot(robot: &RobotDto, branch_prefix: &str, client: &PatronusClient) -> PatronusRobot {
  let branch_name = robot
    .source_branch
    .as_deref()
    .map(|branch| branch.strip_prefix("refs/heads/").unwrap_or(branch))
    .and_then(|branch| strip_branch_namespace(branch, branch_prefix, BranchKind::Virtual))
    .map(str::to_string);
  PatronusRobot {
    id: robot.id.clone(),
    name: robot.name.clone(),
    status: parse_status(&robot.status),
    push_mode: parse_push_mode(&robot.push_mode),
    branch_name,
    source_revision: robot.source_revision.clone().filter(|revision| !revision.is_empty()),
    target_branch: robot.target_branch.strip_prefix("refs/heads/").unwrap_or(&robot.target_branch).to_string(),
    start_time: parse_rfc3339_timestamp(&robot.start_date_time).unwrap_or(0),
    finish_time: robot.finish_date_time.as_deref().and_then(parse_rfc3339_timestamp),
    cancellation_reason: robot.cancellation_reason.clone().filter(|reason| !reason.is_empty()),
    url: client.robot_url(&robot.id),
  }
}

pub(crate) fn parse_status(status: &str) -> PatronusRobotStatus {
  match status {
    "RUNNING" => PatronusRobotStatus::Running,
    "CANCELING" => PatronusRobotStatus::Canceling,
    "CANCELED" => PatronusRobotStatus::Canceled,
    "FAILURE" => PatronusRobotStatus::Failure,
    "SUCCESS" => PatronusRobotStatus::Success,
    _ => PatronusRobotStatus::Unknown,
  }
}

/// The name Patronus uses; None for a status it would not know
fn status_name(status: PatronusRobotStatus) -> Option<&'static str> {
  match status {
    PatronusRobotStatus::Running => Some("RUNNING"),
    PatronusRobotStatus::Canceling => Some("CANCELING"),
    PatronusRobotStatus::Canceled => Some("CANCELED"),
    PatronusRobotStatus::Failure => Some("FAILURE"),
    PatronusRobotStatus::Success => Some("SUCCESS"),
    PatronusRobotStatus::Unknown => None,
  }
}

pub(crate) fn parse_push_mode(mode: &str) -> PatronusPushMode {
  match mode {
    "DRY_RUN" => PatronusPushMode::DryRun,
    "REBASE" => PatronusPushMode::Rebase,
    "REBASE_AUTOSQUASH" => PatronusPushMode::RebaseAutosquash,
    "SQUASH_ALL" => PatronusPushMode::SquashAll,
    "MERGE" => PatronusPushMode::Merge,
    _ => PatronusPushMode::Unknown,
  }
}
