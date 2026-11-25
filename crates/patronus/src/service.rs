//! Asks Patronus on behalf of the app with the least load on it: a rejected token is not sent again until the Safe
//! Push CLI renews it, background requests stop while Patronus fails (exponential backoff), and a full list asked by
//! two windows of one repository within a few seconds is fetched once. The list of a check before an action of the app
//! (a push, a start, the landing check) is never answered from a kept one: a robot started elsewhere (the IDE, `bd`)
//! since that list would hold nothing.
//!
//! The robots are the user's (`owner=me`) and, whoever started them, those of the pushed branches of the user (a
//! reviewer's Safe Merge from the merge request): Patronus filters by one source branch per list, so a full window asks
//! one list per pushed branch, and a short incremental one asks everyone's robots of its minutes once.

use crate::client::{
  MERGE_PROFILE_ID_OPTION, MERGE_PROFILE_NAME_OPTION, PatronusClient, PatronusError, REVIEW_APPROVAL_CHECK_SKIP_OPTION, RobotDto, RobotListFilter, RobotOwnerDto,
  RobotUpdateRequest,
};
use crate::token::{TokenLookup, TokenStamp};
use domain_types::branch_names::{BranchKind, strip_branch_namespace};
use domain_types::patronus::{
  KnownRobotStatus, PatronusAvailability, PatronusPullRequestLink, PatronusPushMode, PatronusRobot, PatronusRobotProfile, PatronusRobotStatus, PatronusRobotsResult, RobotsAsk,
};
use moka::sync::Cache;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sync_utils::timestamp::{parse_rfc3339_timestamp, unix_now};
use tokio::sync::Semaphore;
use tracing::{Instrument as _, info, instrument};

/// The window of a full list, as the Patronus default
pub const FULL_LIST_DAYS: u32 = 7;
/// A full list this fresh is reused instead of asking again
const FULL_LIST_REUSE: Duration = Duration::from_secs(10);
/// An incremental list of a window at most this long (Unix seconds) asks everyone's robots of the repository once
/// instead of one list per pushed branch: a few robots a minute on the monorepo, against a week of them in megabytes
pub const EVERYONE_WINDOW: u32 = 30 * 60;
/// The lists of one read in flight at once
const MAX_LISTS: usize = 4;
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
  /// Counts `forget_full_lists`: a full list that was in flight when the robots changed is not kept
  full_list_generation: u64,
  /// Who asks, as the last list answered: a poll and the details, which do not say it, tell the robots of someone else
  /// by it
  me: Option<RobotOwnerDto>,
}

/// What a list answered: who asks, and the robots
type Listed = (Option<RobotOwnerDto>, Vec<RobotDto>);

/// One list of a read: whose robots, kept under it for the page when of the full window
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ListKey {
  Mine,
  Source(String),
  Everyone,
}

impl ListKey {
  fn filter(&self) -> RobotListFilter<'_> {
    match self {
      Self::Mine => RobotListFilter::Mine,
      Self::Source(branch) => RobotListFilter::SourceBranch(branch),
      Self::Everyone => RobotListFilter::Everyone,
    }
  }
}

/// A list of a read and the start of its window
struct ListRequest {
  key: ListKey,
  start: u32,
  /// Of the full window: kept for the page, and answered from a kept one with `reuse`
  full: bool,
}

/// What the lists of a read answered: who asks, the robots of every list that answered (by id once), and the failure of
/// the first that did not
struct Fetched {
  me: Option<RobotOwnerDto>,
  robots: Vec<RobotDto>,
  failure: Option<PatronusAvailability>,
}

/// The answers of Patronus, each with when it was fetched: the readers decide how long an answer holds. Bounded, so a
/// long session does not grow them with every push (the least used entries go first)
pub(crate) struct Caches {
  /// By repository and list (the user's, a pushed branch's): the last list of the full window, for the page's lists
  /// only (`FULL_LIST_REUSE`); dropped when the robots changed (an action of the app, a poll that saw a change). Written
  /// only under the service lock (`keep_full_list`, `forget_full_lists`)
  full_lists: Cache<(String, ListKey), (Instant, Listed)>,
  /// By robot id; a finished robot does not change any more
  pub(crate) details: Cache<String, (Instant, crate::actions::CachedDetails)>,
  /// By `project/repository/target`: the answer of `robot-start-supported`
  pub(crate) start_supported: Cache<String, (Instant, Result<crate::client::StartSupportedDto, String>)>,
  /// By `repository/target/tip/base`, of a repository only on Space: the review requirement and how long it holds
  pub(crate) reviews: Cache<String, (Instant, Duration, domain_types::patronus::PatronusReviewRequirement)>,
  /// By `repository/source/target`: the open merge request, None = none (asked again after a while)
  pub(crate) merge_requests: Cache<String, (Instant, Option<crate::client::MergeRequestDto>)>,
  /// By robot id, of finished robots: the problems of every severity as Patronus answered them, which the details of
  /// the robot and its failure read too
  pub(crate) problems: Cache<String, Arc<[crate::client::ProblemDto]>>,
  /// By robot id: the problems requests in flight, one for the details of its card and the read ahead that ask at once
  pub(crate) problem_reads: sync_utils::single_flight::SingleFlight<String, Result<Arc<[crate::client::ProblemDto]>, PatronusError>>,
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
      full_lists: Cache::new(256),
      details: Cache::new(200),
      start_supported: Cache::new(64),
      reviews: Cache::builder().max_capacity(500).support_invalidation_closures().build(),
      merge_requests: Cache::new(500),
      problems: Cache::new(1_000),
      problem_reads: sync_utils::single_flight::SingleFlight::default(),
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

  /// The robots of `ask` in the Space repository `repository` (e.g. `ultimate`), for the page: the user's, and those of
  /// the pushed branches `ask.sources` whoever started them, started since `ask.since` or in the last `FULL_LIST_DAYS`
  /// days. A list of the full window fetched within `FULL_LIST_REUSE` (another window) answers again. With `new_api` the
  /// list names the merge profile of a robot where Patronus put it (`mergeProfile`). A list that failed fails the read:
  /// a full one without the robots of a branch would drop them from the session
  #[instrument(skip(self, ask), fields(since = ask.since, sources = ask.sources.len(), new_sources = ask.new_sources.len()))]
  pub async fn robots(&self, repository: &str, branch_prefix: &str, ask: &RobotsAsk, background: bool, new_api: bool) -> PatronusRobotsResult {
    let fetched = match self.fetch(repository, ask, background, true).await {
      Ok(fetched) => fetched,
      Err(result) => return result,
    };
    match fetched.failure {
      Some(availability) => PatronusRobotsResult::from(availability),
      None => self.to_result(&fetched.robots, branch_prefix, new_api, fetched.me.as_ref()),
    }
  }

  /// The lists `ask` needs, at most `MAX_LISTS` in flight, each settled (a rejected token, the backoff); answered from
  /// a kept list of the full window only with `reuse`, and a list of the full window fetched is kept for the page.
  /// Everyone's robots of a short window are kept only when the user's or of `ask.sources`: a robot of someone else on
  /// a temporary ref or a branch of their own is never the robot of a branch of the user
  async fn fetch(&self, repository: &str, ask: &RobotsAsk, background: bool, reuse: bool) -> Result<Fetched, PatronusRobotsResult> {
    let token = self.token(background)?;
    let requests = list_requests(ask, unix_now());
    let generation = self.lock().full_list_generation;
    let mut answers: Vec<(ListKey, Listed)> = Vec::new();
    let mut pending = tokio::task::JoinSet::new();
    let lists = Arc::new(Semaphore::new(MAX_LISTS));
    for request in requests {
      if reuse
        && request.full
        && let Some((me, robots)) = self.fresh_full_list(repository, &request.key)
      {
        answers.push((request.key, (me, robots)));
        continue;
      }
      let (client, token, repository, lists) = (self.client.clone(), token.value.clone(), repository.to_string(), Arc::clone(&lists));
      pending.spawn(
        async move {
          let _permit = lists.acquire_owned().await;
          let response = client.list_robots(&token, &repository, request.start, request.key.filter()).await;
          (request, response)
        }
        .in_current_span(),
      );
    }
    let mut failure = None;
    while let Some(joined) = pending.join_next().await {
      // a list that panicked answered nothing: the read fails, as a full one without it would drop its robots
      let Ok((request, response)) = joined else {
        failure.get_or_insert_with(|| PatronusAvailability::Error {
          message: "a list of the robots failed".to_string(),
        });
        continue;
      };
      match self.settle(response, &token.stamp) {
        Ok((me, robots)) => {
          if request.full {
            self.keep_full_list(repository, &request.key, me.as_ref(), &robots, generation);
          }
          answers.push((request.key, (me, robots)));
        }
        Err(availability) => {
          failure.get_or_insert(availability);
        }
      }
    }
    let me = answers.iter().find_map(|(_, (me, _))| me.clone()).or_else(|| self.me());
    if let Some(me) = &me {
      self.lock().me = Some(me.clone());
    }
    let sources: HashSet<&str> = ask.sources.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    let mut robots = Vec::new();
    for (key, (_, listed)) in answers {
      for robot in listed {
        let kept = key != ListKey::Everyone || is_mine(&robot, me.as_ref()) || source_branch(&robot).is_some_and(|branch| sources.contains(branch));
        if kept && seen.insert(robot.id.clone()) {
          robots.push(robot);
        }
      }
    }
    Ok(Fetched { me, robots, failure })
  }

  /// The robots for the check of the app's push (`repository_model::push_guard`): the user's and those of `sources`
  /// (the pushed branches that may hold it) whoever started them, of the full window asked now, never kept ones (a robot
  /// the IDE, `bd` or a reviewer started since the page's list holds too), as `brd push` reads its holds; the lists it
  /// fetched answer the page's next list. Best effort: of a list that failed, none, and none when Patronus can't be asked
  /// (no token, a rejected token, or while the requests back off after a failure: a background request, so a Patronus
  /// that is down holds no push up), logged; the page checked the robots it knows before
  pub async fn robots_for_checks(&self, repository: &str, branch_prefix: &str, sources: Vec<String>) -> Vec<PatronusRobot> {
    match self.fetch(repository, &RobotsAsk::full(sources), true, false).await {
      Ok(fetched) => {
        if let Some(availability) = &fetched.failure {
          info!(availability = ?availability, "a list for the checks failed: its robots hold nothing");
        }
        self.to_result(&fetched.robots, branch_prefix, false, fetched.me.as_ref()).robots
      }
      Err(result) => {
        info!(availability = ?result.availability, "no robots for the checks: nothing holds");
        Vec::new()
      }
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
      Ok(robots) => {
        if !robots.is_empty() {
          // the robots changed: a kept full list, or one in flight, would show a robot that finished running
          self.forget_full_lists();
        }
        self.to_result(&robots, branch_prefix, false, self.me().as_ref())
      }
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

  fn fresh_full_list(&self, repository: &str, key: &ListKey) -> Option<Listed> {
    let (fetched_at, listed) = self.caches.full_lists.get(&(repository.to_string(), key.clone()))?;
    (fetched_at.elapsed() < FULL_LIST_REUSE).then_some(listed)
  }

  /// Keeps the list `key` of the full window of `repository` fetched at generation `generation` of
  /// `forget_full_lists`, unless the robots changed while it was in flight (a cancel from another window, a poll that
  /// saw a robot finish): that list would show the canceled robot running until the next one
  fn keep_full_list(&self, repository: &str, key: &ListKey, me: Option<&RobotOwnerDto>, robots: &[RobotDto], generation: u64) {
    let state = self.lock();
    if state.full_list_generation == generation {
      self
        .caches
        .full_lists
        .insert((repository.to_string(), key.clone()), (Instant::now(), (me.cloned(), robots.to_vec())));
    }
  }

  /// Who asks, as the last list answered; None before the first
  pub(crate) fn me(&self) -> Option<RobotOwnerDto> {
    self.lock().me.clone()
  }

  /// A start, a restart, a cancel or a poll changed the robots, or a reconcile found the robot of a start whose answer was
  /// lost (`operations::reconcile`): the next full list of the page asks Patronus, and a list in flight is not kept. A
  /// list reused within `FULL_LIST_REUSE` would show a robot the page just canceled, or one a poll saw finish, running,
  /// and would hide the robot of a start that a list kept from before lacks
  pub fn forget_full_lists(&self) {
    let mut state = self.lock();
    state.full_list_generation += 1;
    self.caches.full_lists.invalidate_all();
  }

  pub(crate) const fn client(&self) -> &PatronusClient {
    &self.client
  }

  /// The server, e.g. `PATRONUS_URL`: part of the key of what is kept of it
  pub fn base_url(&self) -> &str {
    self.client.base_url()
  }

  /// The robot page in the web UI
  pub fn robot_url(&self, robot_id: &str) -> String {
    self.client.robot_url(robot_id)
  }

  /// The web UI, which lists the robots of the user first: where a start whose answer was lost is looked for
  pub fn web_url(&self) -> String {
    self.client.web_url().to_string()
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

  /// `listed_profiles`: the new API, the profile a list names (`mergeProfile`) counts; `me`: who asks, which tells the
  /// robots someone else started
  fn to_result(&self, robots: &[RobotDto], branch_prefix: &str, listed_profiles: bool, me: Option<&RobotOwnerDto>) -> PatronusRobotsResult {
    let mut robots: Vec<PatronusRobot> = robots
      .iter()
      .map(|robot| {
        let mut known = to_robot(robot, branch_prefix, &self.client, me);
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
/// branch of a whole stack (`stack_root`); neither for another branch or a temporary ref. Of an owner who is not `me`
/// it names who started it (`started_by`); without `me` (no list answered yet) none
pub fn to_robot(robot: &RobotDto, branch_prefix: &str, client: &PatronusClient, me: Option<&RobotOwnerDto>) -> PatronusRobot {
  let source = source_branch(robot);
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
    started_by: started_by(robot, me),
  }
}

/// The source branch of a robot without `refs/heads/`
fn source_branch(robot: &RobotDto) -> Option<&str> {
  robot.source_branch.as_deref().map(|branch| branch.strip_prefix("refs/heads/").unwrap_or(branch))
}

/// The robot is of who asks; unknown (no `me`, no owner) is not
fn is_mine(robot: &RobotDto, me: Option<&RobotOwnerDto>) -> bool {
  matches!((robot.owner.as_ref(), me), (Some(owner), Some(me)) if owner.is(me))
}

/// The name of the owner who is not `me`; of an owner without a name, its id
fn started_by(robot: &RobotDto, me: Option<&RobotOwnerDto>) -> Option<String> {
  let (owner, me) = (robot.owner.as_ref()?, me?);
  (!owner.is(me)).then(|| owner.name.clone().filter(|name| !name.is_empty()).unwrap_or_else(|| owner.id.clone()))
}

/// The lists of `ask` at `now`: of no pushed branch the user's robots of its window; of a short incremental window
/// everyone's robots of it once, with the full window of the branches pushed since the last list; else the full window
/// of the user's robots and of each pushed branch
fn list_requests(ask: &RobotsAsk, now: u32) -> Vec<ListRequest> {
  let full_start = now.saturating_sub(FULL_LIST_DAYS * 86_400);
  let full = |key: ListKey| ListRequest {
    key,
    start: full_start,
    full: true,
  };
  let sources = |branches: &[String]| branches.iter().map(|branch| full(ListKey::Source(branch.clone()))).collect::<Vec<_>>();
  match ask.since {
    _ if ask.sources.is_empty() => vec![match ask.since {
      Some(since) => ListRequest {
        key: ListKey::Mine,
        start: since,
        full: false,
      },
      None => full(ListKey::Mine),
    }],
    Some(since) if now.saturating_sub(since) <= EVERYONE_WINDOW => {
      let mut requests = vec![ListRequest {
        key: ListKey::Everyone,
        start: since,
        full: false,
      }];
      requests.extend(sources(&ask.new_sources));
      requests
    }
    _ => {
      let mut requests = vec![full(ListKey::Mine)];
      requests.extend(sources(&ask.sources));
      requests
    }
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
