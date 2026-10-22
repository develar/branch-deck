//! Asks Patronus on behalf of the app with the least load on it: a rejected token is not sent again until the Safe
//! Push CLI renews it, background requests stop while Patronus fails (exponential backoff), and a full list asked by
//! two windows of one repository within a few seconds is fetched once.

use crate::client::{MERGE_PROFILE_ID_OPTION, MERGE_PROFILE_NAME_OPTION, PatronusClient, PatronusError, REVIEW_APPROVAL_CHECK_SKIP_OPTION, RobotDto, RobotUpdateRequest};
use crate::token::{TokenLookup, TokenStamp};
use domain_types::branch_names::{BranchKind, strip_branch_namespace};
use domain_types::patronus::{
  KnownRobotStatus, PatronusAvailability, PatronusPullRequestLink, PatronusPushMode, PatronusRobot, PatronusRobotProfile, PatronusRobotStatus, PatronusRobotsResult,
};
use moka::sync::Cache;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::sync::Mutex;
use std::time::{Duration, Instant};
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
  pub(crate) caches: Caches,
}

#[derive(Default)]
pub(crate) struct ServiceState {
  /// Patronus rejected this token: background requests wait for a new one
  rejected: Option<TokenStamp>,
  backoff: Option<Backoff>,
  /// Patronus answered 404 to `robots/progress` (its flag is off for the user): the builds are asked instead until then
  pub(crate) progress_api_rests_until: Option<Instant>,
}

/// The answers of Patronus, each with when it was fetched: the readers decide how long an answer holds. Bounded, so a
/// long session does not grow them with every push (the least used entries go first)
pub(crate) struct Caches {
  /// By repository: the last full list
  full_lists: Cache<String, (Instant, Vec<RobotDto>)>,
  /// By robot id; a finished robot does not change any more
  pub(crate) details: Cache<String, (Instant, crate::actions::CachedDetails)>,
  /// By `project/repository/target`: the answer of `robot-start-supported`
  pub(crate) start_supported: Cache<String, (Instant, Result<crate::client::StartSupportedDto, String>)>,
  /// By `repository/target/tip/base`, of a repository only on Space: the review requirement and how long it holds
  pub(crate) reviews: Cache<String, (Instant, Duration, domain_types::patronus::PatronusReviewRequirement)>,
  /// By `repository/source/target`: the open merge request, None = none (asked again after a while)
  pub(crate) merge_requests: Cache<String, (Instant, Option<crate::client::MergeRequestDto>)>,
  /// By robot id, of finished robots: the error problems
  pub(crate) problems: Cache<String, Vec<domain_types::patronus::PatronusProblem>>,
  /// By build id: its progress, so the details open in two windows ask it once a minute
  pub(crate) build_progress: Cache<String, (Instant, crate::client::BuildProgressDto)>,
  /// By robot id: its progress of the new API (`robots/progress`), kept as a build's is
  pub(crate) robot_progress: Cache<String, (Instant, Option<domain_types::patronus::PatronusRobotProgress>)>,
  /// By `robot id/with the teamcity CLI`, of finished robots: the failed builds of the full failure
  pub(crate) failed_builds: Cache<String, crate::actions::FailedBuilds>,
}

impl Caches {
  fn new() -> Self {
    Self {
      full_lists: Cache::new(16),
      details: Cache::new(200),
      start_supported: Cache::new(64),
      reviews: Cache::builder().max_capacity(500).support_invalidation_closures().build(),
      merge_requests: Cache::new(500),
      problems: Cache::new(1_000),
      build_progress: Cache::new(500),
      robot_progress: Cache::new(200),
      failed_builds: Cache::new(50),
    }
  }
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
      caches: Caches::new(),
    }
  }

  /// The user's robots in the Space repository `repository` (e.g. `ultimate`) started since `since`, or in the last
  /// `FULL_LIST_DAYS` days. With `new_api` the list names the merge profile of a robot where Patronus put it (`mergeProfile`)
  #[instrument(skip(self))]
  pub async fn robots(&self, repository: &str, branch_prefix: &str, since: Option<u32>, background: bool, new_api: bool) -> PatronusRobotsResult {
    let token = match self.token(background) {
      Ok(token) => token,
      Err(result) => return result,
    };
    if since.is_none()
      && let Some(robots) = self.fresh_full_list(repository)
    {
      return self.to_result(&robots, branch_prefix, new_api);
    }
    let start = since.unwrap_or_else(|| unix_now().saturating_sub(FULL_LIST_DAYS * 86_400));
    let response = self.client.list_robots(&token.value, repository, start).await;
    match self.settle(response, &token.stamp) {
      Ok(robots) => {
        if since.is_none() {
          self.caches.full_lists.insert(repository.to_string(), (Instant::now(), robots.clone()));
        }
        self.to_result(&robots, branch_prefix, new_api)
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
        github_repository: None,
      };
    }
    let token = match self.token(true) {
      Ok(token) => token,
      Err(result) => return result,
    };
    let response = self.client.fetch_updates(&token.value, &requests).await;
    match self.settle(response, &token.stamp) {
      // a poll answers the options, where the profile has always been
      Ok(robots) => self.to_result(&robots, branch_prefix, false),
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
    let (fetched_at, robots) = self.caches.full_lists.get(repository)?;
    (fetched_at.elapsed() < FULL_LIST_REUSE).then_some(robots)
  }

  pub(crate) const fn client(&self) -> &PatronusClient {
    &self.client
  }

  /// The server, e.g. `PATRONUS_URL`: part of the key of what is kept of it
  pub fn base_url(&self) -> &str {
    self.client.base_url()
  }

  /// Who asks, for a key of an answer that depends on the user: the server and a hash of the current token (a renewed
  /// token is another key, which costs one more question per branch every 14 days); None without a token. The Space
  /// user is not known locally: the `name` of the token file is the CLI client, which another user's login keeps.
  pub fn identity(&self) -> Option<String> {
    let TokenLookup::Found(token) = (self.token_source)(unix_now()) else {
      return None;
    };
    let digest = Sha256::digest(token.value.as_bytes());
    Some(format!("{} {}", self.client.base_url(), to_hex(&digest[..16])))
  }

  /// `listed_profiles`: the new API, the profile a list names (`mergeProfile`) counts
  fn to_result(&self, robots: &[RobotDto], branch_prefix: &str, listed_profiles: bool) -> PatronusRobotsResult {
    let mut robots: Vec<PatronusRobot> = robots
      .iter()
      .map(|robot| {
        let mut known = to_robot(robot, branch_prefix, &self.client);
        if listed_profiles && known.profile.is_none() {
          known.profile = listed_profile(robot);
        }
        known
      })
      .collect();
    robots.sort_by_key(|robot| std::cmp::Reverse(robot.start_time));
    PatronusRobotsResult {
      availability: PatronusAvailability::Available,
      robots,
      github_repository: None,
    }
  }

  pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, ServiceState> {
    self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }
}

/// The robot as the app shows it: its source branch named by its kind, a virtual branch (`branch_name`) or the stack
/// branch of a whole stack (`stack_root`); neither for another branch or a temporary ref
pub fn to_robot(robot: &RobotDto, branch_prefix: &str, client: &PatronusClient) -> PatronusRobot {
  let source = robot.source_branch.as_deref().map(|branch| branch.strip_prefix("refs/heads/").unwrap_or(branch));
  let named = |kind: BranchKind| source.and_then(|branch| strip_branch_namespace(branch, branch_prefix, kind)).map(str::to_string);
  PatronusRobot {
    id: robot.id.clone(),
    name: robot.name.clone(),
    status: parse_status(&robot.status),
    push_mode: parse_push_mode(&robot.push_mode),
    branch_name: named(BranchKind::Virtual),
    stack_root: named(BranchKind::Stack),
    source_revision: robot.source_revision.clone().filter(|revision| !revision.is_empty()),
    target_branch: robot.target_branch.strip_prefix("refs/heads/").unwrap_or(&robot.target_branch).to_string(),
    start_time: parse_rfc3339_timestamp(&robot.start_date_time).unwrap_or(0),
    finish_time: robot.finish_date_time.as_deref().and_then(parse_rfc3339_timestamp),
    cancellation_reason: robot.cancellation_reason.clone().filter(|reason| !reason.is_empty()),
    url: client.robot_url(&robot.id),
    pull_request: robot.github_pr_url.as_deref().and_then(parse_pull_request_link),
    profile: robot_profile(robot),
  }
}

/// The merge profile in the options of an answer that has them (a poll, the details, a start); None from a list, which
/// lacks the options: unknown, not "none"
pub fn robot_profile(robot: &RobotDto) -> Option<PatronusRobotProfile> {
  let options = robot.options.as_ref()?;
  let option = |key: &str| options.get(key).filter(|value| !value.is_empty()).cloned();
  Some(PatronusRobotProfile {
    id: option(MERGE_PROFILE_ID_OPTION),
    name: option(MERGE_PROFILE_NAME_OPTION),
    skips_review: option(REVIEW_APPROVAL_CHECK_SKIP_OPTION).map(|value| value == "true"),
  })
}

/// The merge profile a list names with the new API (`mergeProfile`); None while Patronus leaves it null, for a robot
/// without one as while its flag is off for the user: unknown either way, as from a list without it
pub fn listed_profile(robot: &RobotDto) -> Option<PatronusRobotProfile> {
  let profile = robot.merge_profile.as_ref()?;
  Some(PatronusRobotProfile {
    id: Some(profile.id.clone()),
    name: Some(profile.name.clone()),
    skips_review: Some(profile.skips_review),
  })
}

/// `https://github.com/{owner}/{name}/pull/{number}`
fn parse_pull_request_link(url: &str) -> Option<PatronusPullRequestLink> {
  let (_, number) = url.trim_end_matches('/').rsplit_once("/pull/")?;
  Some(PatronusPullRequestLink {
    number: number.parse().ok()?,
    url: url.to_string(),
  })
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
const fn status_name(status: PatronusRobotStatus) -> Option<&'static str> {
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

fn to_hex(bytes: &[u8]) -> String {
  bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
    let _ = write!(hex, "{byte:02x}");
    hex
  })
}
