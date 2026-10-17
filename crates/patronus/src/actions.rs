//! What the user does with a robot (start, restart, cancel) and what the chip popover asks on demand (the checks, the
//! problems, the merge profiles, the progress of the running checks, why it failed). Each is one request or a few in
//! parallel; the answers that don't change are cached: the details of a finished robot for the session, of a running
//! one for a minute, the progress of a build for a minute, the profiles for 10 minutes, the failed builds of a finished
//! robot for the session.

use crate::ci_text::{cut_head, strip_ansi};
use crate::client::{
  BranchRobotStartRequest, BuildDto, BuildProgressDto, MergeProfileDto, ProblemDto, RobotDto, SpaceReviewDto, StartSupportedDto, TeamCityCheckAttemptDetailsDto,
  TeamCityCheckAttemptDto, TeamCityCheckDto,
};
use crate::service::{PatronusService, parse_push_mode, parse_status, robot_profile, to_robot};
use crate::teamcity_cli::TeamCityCli;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sync_types::patronus::{
  PatronusAvailability, PatronusChecks, PatronusMergeRequest, PatronusProblem, PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotDetails, PatronusRobotProfile,
  PatronusRobotProgress, PatronusRobotStatus, PatronusStartOptions,
};
use sync_types::robot_failure::{BuildProblem, FailedBuild, FailedCheck, FailedTest, OmittedFailures, RobotFailure, RobotFailureKind, RobotFailureProblem};
use tokio::sync::Semaphore;
use tracing::{info, instrument};

/// The details of a running robot change: asked again after this
const RUNNING_DETAILS_TTL: Duration = Duration::from_secs(60);
/// Patronus caches the progress of a build for a minute: asking more often would read the same answer
const BUILD_PROGRESS_TTL: Duration = Duration::from_secs(60);
/// The profiles of a branch change rarely
const START_SUPPORTED_TTL: Duration = Duration::from_secs(600);
const START_SUPPORTED_ERROR_TTL: Duration = Duration::from_secs(60);
/// Failed check names kept for the popover
const MAX_FAILED_NAMES: usize = 3;
/// The limits of a robot failure, so a prompt for an agent stays readable: the failed builds, the names of failed tests,
/// the tests with their message, the lines of a message, the bytes of a message and of problem details
const MAX_FAILED_BUILDS: usize = 5;
const MAX_FAILED_TEST_NAMES: usize = 40;
const MAX_TEST_MESSAGES: usize = 20;
const MAX_MESSAGE_LINES: usize = 60;
/// A line of a message can be a whole serialized object
const MAX_MESSAGE_BYTES: usize = 8 * 1024;
const MAX_DETAILS_BYTES: usize = 4 * 1024;
/// Patronus allows 100 test messages in 10 s
const MAX_TEST_MESSAGE_REQUESTS: usize = 8;
/// The subsystems of Patronus' detectors of a massive failure of tests or builds
const MASSIVE_FAILURE_SUBSYSTEM_PREFIX: &str = "MASSIVE_";

/// The details without the branch name, which depends on the branch prefix of the caller
#[derive(Clone)]
pub struct CachedDetails {
  robot: RobotDto,
  checks: Option<PatronusChecks>,
  problems: Vec<PatronusProblem>,
  failing: bool,
  progress: Option<PatronusRobotProgress>,
  /// The checks and the problems of every severity as Patronus answered them, for the failure of the robot; None when the
  /// checks could not be read
  raw_checks: Option<Arc<[TeamCityCheckDto]>>,
  raw_problems: Arc<[ProblemDto]>,
}

/// The failed builds of a full failure and what their limits left out
#[derive(Clone)]
pub struct FailedBuilds {
  builds: Vec<FailedBuild>,
  omitted: OmittedFailures,
}

/// A failed action, for the user
pub type ActionError = String;

impl PatronusService {
  /// The robot with its profile, checks, error problems and whether it will fail
  #[instrument(skip(self))]
  pub async fn robot_details(&self, branch_prefix: &str, robot_id: &str) -> Result<PatronusRobotDetails, ActionError> {
    let details = self.cached_details(robot_id).await?;
    let robot = to_robot(&details.robot, branch_prefix, self.client());
    let profile = robot_profile(&details.robot);
    Ok(PatronusRobotDetails {
      profile_id: profile.as_ref().and_then(|profile| profile.id.clone()),
      profile_name: profile.and_then(|profile| profile.name),
      robot,
      checks: details.checks,
      problems: details.problems,
      failing: details.failing,
      progress: details.progress,
    })
  }

  async fn cached_details(&self, robot_id: &str) -> Result<CachedDetails, ActionError> {
    if let Some((fetched_at, details)) = self.caches.details.get(robot_id)
      && (parse_status(&details.robot.status).is_finished() || fetched_at.elapsed() < RUNNING_DETAILS_TTL)
    {
      return Ok(details);
    }
    let token = self.action_token()?;
    let client = self.client();
    let (robot, checks, problems) = tokio::join!(
      client.get_robot(&token.value, robot_id),
      client.get_teamcity_checks(&token.value, robot_id),
      client.get_problems(&token.value, robot_id),
    );
    let robot = self.settle(robot, &token.stamp).map_err(availability_message)?;
    let checks = checks.ok();
    // only while the details of a running robot are open: TeamCity's estimate of its running checks
    let progress = match &checks {
      Some(checks) if parse_status(&robot.status) == PatronusRobotStatus::Running => self.checks_progress(&token, checks).await,
      _ => None,
    };
    // the checks and the problems are extras: the details show without them
    let raw_problems: Arc<[ProblemDto]> = problems.unwrap_or_default().into();
    let (problems, failing) = split_problems(raw_problems.to_vec());
    let details = CachedDetails {
      robot,
      checks: checks.as_deref().and_then(summarize_checks),
      problems,
      failing,
      progress,
      raw_checks: checks.map(Arc::from),
      raw_problems,
    };
    if parse_status(&details.robot.status).is_finished() {
      self.caches.problems.insert(robot_id.to_string(), details.problems.clone());
    }
    self.caches.details.insert(robot_id.to_string(), (Instant::now(), details.clone()));
    Ok(details)
  }

  /// The progress of the current build of each running check, in parallel, each cached for a minute; None before a check
  /// runs. A build Patronus can't answer for is left out.
  async fn checks_progress(&self, token: &crate::token::Token, checks: &[TeamCityCheckDto]) -> Option<PatronusRobotProgress> {
    let builds: Vec<String> = checks
      .iter()
      .filter(|check| check.status == "RUNNING")
      .filter_map(|check| check.running_build_id().map(str::to_string))
      .collect();
    let mut progress = HashMap::new();
    let mut pending = tokio::task::JoinSet::new();
    for build_id in builds {
      if let Some((fetched_at, known)) = self.caches.build_progress.get(&build_id)
        && fetched_at.elapsed() < BUILD_PROGRESS_TTL
      {
        progress.insert(build_id, known);
        continue;
      }
      let client = self.client().clone();
      let token = token.value.clone();
      pending.spawn(async move {
        let answer = client.get_build_progress(&token, &build_id).await;
        (build_id, answer)
      });
    }
    while let Some(Ok((build_id, answer))) = pending.join_next().await {
      if let Ok(answer) = self.settle(answer, &token.stamp) {
        self.caches.build_progress.insert(build_id.clone(), (Instant::now(), answer.clone()));
        progress.insert(build_id, answer);
      }
    }
    summarize_progress(checks, &progress)
  }

  /// Why the robot failed, or fails while it runs, from its details (cached as `robot_details`): the failed checks by
  /// their last attempt and the error problems; no request when the details are known. `builds` is None.
  #[instrument(skip(self))]
  pub async fn robot_failure(&self, branch_prefix: &str, robot_id: &str) -> Result<RobotFailure, ActionError> {
    let details = self.cached_details(robot_id).await?;
    let checks = failure_checks(&details)?;
    Ok(robot_failure_of(
      to_robot(&details.robot, branch_prefix, self.client()),
      &details.robot,
      checks,
      &details.raw_problems,
    ))
  }

  /// `robot_failure` with the failed builds of the failed attempts: their problems and failed tests, the messages of
  /// the first tests, and, through `teamcity` when it serves the build, the log tail of a build that failed not only on
  /// tests and whether a test failed before. The builds of a finished robot are asked once per session.
  #[instrument(skip(self, teamcity))]
  pub async fn robot_failure_full(&self, branch_prefix: &str, robot_id: &str, teamcity: Option<&TeamCityCli>) -> Result<RobotFailure, ActionError> {
    let details = self.cached_details(robot_id).await?;
    let checks = failure_checks(&details)?;
    let mut failure = robot_failure_of(to_robot(&details.robot, branch_prefix, self.client()), &details.robot, checks, &details.raw_problems);
    let finished = parse_status(&details.robot.status).is_finished();
    let key = format!("{robot_id}/{}", teamcity.is_some());
    let failed = if let Some(failed) = self.caches.failed_builds.get(&key).filter(|_| finished) {
      failed
    } else {
      let attempts: Vec<&str> = failed_attempts(checks, failure.running).filter_map(|(_, attempt)| attempt.id.as_deref()).collect();
      let failed = self.failed_builds(&attempts, teamcity).await?;
      if finished {
        self.caches.failed_builds.insert(key, failed.clone());
      }
      failed
    };
    failure.builds = Some(failed.builds);
    failure.omitted = OmittedFailures {
      truncated: failure.omitted.truncated || failed.omitted.truncated,
      ..failed.omitted
    };
    Ok(failure)
  }

  /// The failed builds of the attempts in parallel, without the composite ones and the ignored problems and tests, those
  /// that failed not only on tests first, in the limits; then the messages of the first tests, at most
  /// `MAX_TEST_MESSAGE_REQUESTS` at once, and what the CLI adds. A build in the chains of two attempts is listed once.
  async fn failed_builds(&self, attempt_ids: &[&str], teamcity: Option<&TeamCityCli>) -> Result<FailedBuilds, ActionError> {
    let token = self.action_token()?;
    let mut pending = tokio::task::JoinSet::new();
    for (index, attempt_id) in attempt_ids.iter().enumerate() {
      let client = self.client().clone();
      let token = token.value.clone();
      let attempt_id = (*attempt_id).to_string();
      pending.spawn(async move { (index, client.get_attempt_details(&token, &attempt_id).await) });
    }
    let mut answers: Vec<Option<TeamCityCheckAttemptDetailsDto>> = vec![None; attempt_ids.len()];
    while let Some(joined) = pending.join_next().await {
      let (index, answer) = joined.map_err(|e| format!("Cannot read the failed builds: {e}"))?;
      answers[index] = Some(self.settle(answer, &token.stamp).map_err(availability_message)?);
    }

    let mut omitted = OmittedFailures::default();
    let mut seen = HashSet::new();
    let mut builds: Vec<FailedBuild> = Vec::new();
    // by build: whether its log tells why it failed
    let mut wants_log: Vec<bool> = Vec::new();
    // the ids of `get_test_run_details` of each listed test, by build and test index
    let mut test_runs: Vec<(usize, usize, [String; 3])> = Vec::new();
    let mut listed_tests = 0;
    let mut candidates: Vec<BuildDto> = answers
      .into_iter()
      .flatten()
      .flat_map(|details| details.builds)
      .filter(|build| !build.is_composite && seen.insert(build.build_id.clone()))
      .collect();
    // a build that failed not only on tests first (a compilation error explains the failed tests of the builds after it)
    candidates.sort_by_key(|build| build.is_failed_tests_only_failure);
    for build in candidates {
      if builds.len() == MAX_FAILED_BUILDS {
        omitted.builds += 1;
        continue;
      }
      // a build that failed only on tests has its failures in the tests, not in its log
      let log = !build.is_failed_tests_only_failure;
      let (failed_build, runs) = to_failed_build(build, &mut listed_tests, &mut omitted);
      test_runs.extend(runs.into_iter().map(|(test, ids)| (builds.len(), test, ids)));
      wants_log.push(log && !failed_build.problems.is_empty());
      builds.push(failed_build);
    }

    let test_runs: Vec<_> = test_runs.into_iter().take(MAX_TEST_MESSAGES).collect();
    let messages = self.test_messages(&token, test_runs.iter().map(|(_, _, ids)| ids.clone()).collect()).await;
    for ((build, test, _), message) in test_runs.iter().zip(messages) {
      if let Some(message) = message {
        let (message, cut) = cut_head(&strip_ansi(&message), MAX_MESSAGE_LINES, MAX_MESSAGE_BYTES);
        omitted.truncated |= cut;
        builds[*build].failed_tests[*test].message = Some(message);
      }
    }
    let without_message = builds.iter().flat_map(|build| &build.failed_tests).filter(|test| test.message.is_none()).count();
    omitted.messages = u32::try_from(without_message).unwrap_or(u32::MAX);

    if let Some(teamcity) = teamcity {
      omitted.truncated |= add_teamcity_details(teamcity, &mut builds, &wants_log).await;
    }
    info!(builds = builds.len(), ?omitted, "read the failed builds");
    Ok(FailedBuilds { builds, omitted })
  }

  /// The failure messages of the test runs, in their order, at most `MAX_TEST_MESSAGE_REQUESTS` requests at once; None
  /// for a run Patronus can't answer for
  async fn test_messages(&self, token: &crate::token::Token, test_runs: Vec<[String; 3]>) -> Vec<Option<String>> {
    let requests = Arc::new(Semaphore::new(MAX_TEST_MESSAGE_REQUESTS));
    let mut pending = tokio::task::JoinSet::new();
    let count = test_runs.len();
    for (index, [build_id, test_id, test_run_id]) in test_runs.into_iter().enumerate() {
      let client = self.client().clone();
      let token = token.value.clone();
      let requests = Arc::clone(&requests);
      pending.spawn(async move {
        let _permit = requests.acquire_owned().await;
        (index, client.get_test_run_details(&token, &build_id, &test_id, &test_run_id).await)
      });
    }
    let mut messages = vec![None; count];
    while let Some(Ok((index, answer))) = pending.join_next().await {
      messages[index] = self.settle(answer, &token.stamp).ok();
    }
    messages
  }

  /// The error problems of a finished robot, asked once per session (the chip names a missing review); the details
  /// popover fills the same cache
  #[instrument(skip(self))]
  pub async fn robot_problems(&self, robot_id: &str) -> Result<Vec<PatronusProblem>, ActionError> {
    if let Some(problems) = self.caches.problems.get(robot_id) {
      return Ok(problems);
    }
    let token = self.token(true).map_err(|result| availability_message(result.availability))?;
    let response = self.client().get_problems(&token.value, robot_id).await;
    let (problems, _) = split_problems(self.settle(response, &token.stamp).map_err(availability_message)?);
    self.caches.problems.insert(robot_id.to_string(), problems.clone());
    Ok(problems)
  }

  /// Of the running robots `robot_ids`, the ones that will fail: a required check failed and Patronus registered its
  /// warning (`ROBOT_IS_FAILING_PROBLEM`). The problems of each in parallel; a robot Patronus can't answer for is left
  /// out. Not cached: the caller stops asking for a robot once it is failing, and a running robot's problems change.
  #[instrument(skip(self, robot_ids), fields(count = robot_ids.len()))]
  pub async fn failing_robots(&self, robot_ids: &[String]) -> Result<Vec<String>, ActionError> {
    if robot_ids.is_empty() {
      return Ok(Vec::new());
    }
    let token = self.token(true).map_err(|result| availability_message(result.availability))?;
    let mut pending = tokio::task::JoinSet::new();
    for robot_id in robot_ids {
      let client = self.client().clone();
      let token = token.value.clone();
      let robot_id = robot_id.clone();
      pending.spawn(async move {
        let answer = client.get_problems(&token, &robot_id).await;
        (robot_id, answer)
      });
    }
    let mut failing = Vec::new();
    while let Some(Ok((robot_id, answer))) = pending.join_next().await {
      if let Ok(problems) = self.settle(answer, &token.stamp)
        && problems.iter().any(ProblemDto::is_robot_failing)
      {
        failing.push(robot_id);
      }
    }
    // the order of the request, not of the answers
    failing.sort_by_key(|id| robot_ids.iter().position(|known| known == id));
    Ok(failing)
  }

  /// The merge profiles of the target branch and the profile of the last robot of the branch as the default; the
  /// emergency profiles apart. Patronus says before a start only of an emergency profile that it skips the review:
  /// for the profile of the last robot its options say it
  #[instrument(skip(self))]
  pub async fn start_options(&self, project_key: &str, repository: &str, target_branch: &str, last_robot_id: Option<&str>) -> Result<PatronusStartOptions, ActionError> {
    let supported = self.cached_start_supported(project_key, repository, target_branch).await?;
    // the last robot is usually in the cache: its popover was open
    let last_profile = match last_robot_id {
      Some(robot_id) => self.cached_details(robot_id).await.ok().and_then(|details| robot_profile(&details.robot)),
      None => None,
    };
    let to_profile = |profile: &MergeProfileDto| PatronusProfile {
      id: profile.id.clone(),
      name: profile.name.clone(),
      description: profile.description.clone().filter(|description| !description.is_empty()),
      allowed_push_modes: profile.allowed_push_modes.iter().map(|mode| parse_push_mode(mode)).collect(),
      skips_review: if profile.is_emergency {
        Some(true)
      } else {
        last_profile
          .as_ref()
          .filter(|last| last.id.as_deref() == Some(profile.id.as_str()))
          .and_then(|last| last.skips_review)
      },
    };
    let (emergency, regular): (Vec<&MergeProfileDto>, Vec<&MergeProfileDto>) = supported.profiles.iter().partition(|profile| profile.is_emergency);
    let profiles: Vec<PatronusProfile> = regular.into_iter().map(to_profile).collect();
    let emergency_profiles = emergency.into_iter().map(to_profile).collect();
    // never an emergency profile: a start with one is chosen each time
    let default_profile_id = last_profile.and_then(|last| last.id).filter(|id| profiles.iter().any(|profile| &profile.id == id));
    Ok(PatronusStartOptions {
      target_branch: target_branch.to_string(),
      supported: supported.supported,
      profiles,
      emergency_profiles,
      default_profile_id,
    })
  }

  async fn cached_start_supported(&self, project_key: &str, repository: &str, target_branch: &str) -> Result<StartSupportedDto, ActionError> {
    let key = format!("{project_key}/{repository}/{target_branch}");
    if let Some((fetched_at, answer)) = self.caches.start_supported.get(&key) {
      let ttl = if answer.is_ok() { START_SUPPORTED_TTL } else { START_SUPPORTED_ERROR_TTL };
      if fetched_at.elapsed() < ttl {
        return answer;
      }
    }
    let token = self.action_token()?;
    let response = self.client().get_start_supported(&token.value, project_key, repository, target_branch).await;
    let answer = self.settle(response, &token.stamp).map_err(availability_message);
    self.caches.start_supported.insert(key, (Instant::now(), answer.clone()));
    answer
  }

  /// Starts a Safe Push or a Dry Run of the pushed virtual branch `source_branch` (`refs/heads/…`) into `target_branch`.
  /// Never repeated: Patronus fetches a branch pushed right before while it handles the start.
  #[expect(clippy::too_many_arguments, reason = "the fields of one start request of Patronus")]
  #[instrument(skip(self))]
  pub async fn start(
    &self,
    repository: &str,
    branch_prefix: &str,
    source_branch: &str,
    target_branch: &str,
    mode: PatronusPushMode,
    profile_id: Option<String>,
    merge_request: Option<PatronusMergeRequest>,
  ) -> Result<PatronusRobot, ActionError> {
    let mode = match mode {
      PatronusPushMode::DryRun => "DRY_RUN",
      PatronusPushMode::RebaseAutosquash => "REBASE_AUTOSQUASH",
      other => return Err(format!("Branch Deck starts a Safe Push or a Dry Run, not {other:?}")),
    };
    let request = BranchRobotStartRequest {
      kind: "ROBOT_WEB_UI",
      repository: repository.to_string(),
      source_branch: source_branch.to_string(),
      target_branch: target_branch.to_string(),
      mode,
      source_branch_delete_policy: "NEVER",
      merge_profile_id: profile_id.clone(),
      space_review: merge_request.map(|merge_request| SpaceReviewDto {
        project_key: merge_request.project_key,
        review_key: merge_request.review_key,
      }),
    };
    let token = self.action_token()?;
    let response = self.client().start_robot(&token.value, &request).await;
    let robot_id = self.settle(response, &token.stamp).map_err(availability_message)?;
    let mut robot = self.fetch_robot(branch_prefix, &robot_id).await?;
    // the robot knows the profile it was started with, also before Patronus has written it into its options
    if robot.profile.as_ref().is_none_or(|profile| profile.id.is_none()) && profile_id.is_some() {
      robot.profile = Some(PatronusRobotProfile {
        id: profile_id,
        name: None,
        skips_review: None,
      });
    }
    Ok(robot)
  }

  /// Starts the robot again on the newest commit of its branch: a new robot
  #[instrument(skip(self))]
  pub async fn restart(&self, branch_prefix: &str, robot_id: &str) -> Result<PatronusRobot, ActionError> {
    let token = self.action_token()?;
    let response = self.client().restart_robot(&token.value, robot_id).await;
    let robot_id = self.settle(response, &token.stamp).map_err(availability_message)?;
    self.fetch_robot(branch_prefix, &robot_id).await
  }

  /// Asks Patronus to cancel the robot; it is canceling until its checks stopped
  #[instrument(skip(self))]
  pub async fn cancel(&self, branch_prefix: &str, robot_id: &str) -> Result<PatronusRobot, ActionError> {
    let token = self.action_token()?;
    let response = self.client().cancel_robot(&token.value, robot_id).await;
    self.settle(response, &token.stamp).map_err(availability_message)?;
    self.caches.details.invalidate(robot_id);
    self.fetch_robot(branch_prefix, robot_id).await
  }

  /// The current status of one robot (before a push into its branch)
  #[instrument(skip(self))]
  pub async fn robot_status(&self, robot_id: &str) -> Result<PatronusRobotStatus, ActionError> {
    let token = self.action_token()?;
    let response = self.client().get_robot_status(&token.value, robot_id).await;
    self.settle(response, &token.stamp).map(|status| parse_status(&status)).map_err(availability_message)
  }

  async fn fetch_robot(&self, branch_prefix: &str, robot_id: &str) -> Result<PatronusRobot, ActionError> {
    let token = self.action_token()?;
    let response = self.client().get_robot(&token.value, robot_id).await;
    let robot = self.settle(response, &token.stamp).map_err(availability_message)?;
    Ok(to_robot(&robot, branch_prefix, self.client()))
  }

  pub(crate) fn action_token(&self) -> Result<crate::token::Token, ActionError> {
    self.token(false).map_err(|result| availability_message(result.availability))
  }
}

/// What the user reads when an action can't be done
pub(crate) fn availability_message(availability: PatronusAvailability) -> ActionError {
  match availability {
    PatronusAvailability::NotLoggedIn => "Run ./safePush.cmd once to log in to Patronus".to_string(),
    PatronusAvailability::Error { message } => message,
    PatronusAvailability::Available | PatronusAvailability::NotPatronus => "Patronus is not available for the repository".to_string(),
  }
}

/// None before Patronus started any check
pub fn summarize_checks(checks: &[TeamCityCheckDto]) -> Option<PatronusChecks> {
  if checks.is_empty() {
    return None;
  }
  let mut summary = PatronusChecks {
    failed_names: Vec::new(),
    failed: 0,
    running: 0,
    passed: 0,
    canceled: 0,
  };
  for check in checks {
    match check.status.as_str() {
      "FAILURE" => {
        summary.failed += 1;
        if summary.failed_names.len() < MAX_FAILED_NAMES {
          summary.failed_names.push(check.label().to_string());
        }
      }
      "QUEUED" | "RUNNING" => summary.running += 1,
      "CANCELED" => summary.canceled += 1,
      // SUCCESS, SKIPPED
      _ => summary.passed += 1,
    }
  }
  Some(summary)
}

/// How far a robot is by the estimates of the CI: the running check that ends last (its estimate minus its running time),
/// its name and percent; when every running check ran past its estimate, the one closest to its estimate and by how
/// much; the queued checks; whether one probably hangs. None before a check runs.
pub fn summarize_progress(checks: &[TeamCityCheckDto], progress: &HashMap<String, BuildProgressDto>) -> Option<PatronusRobotProgress> {
  let mut queued = 0;
  let mut probably_hanging = false;
  let mut running = false;
  // the check that ends last: the most seconds left, or the least overtime
  let mut last: Option<(i64, &TeamCityCheckDto, &BuildProgressDto)> = None;
  for check in checks {
    match check.status.as_str() {
      "QUEUED" => queued += 1,
      "RUNNING" => {
        running = true;
        let Some(build) = check.running_build_id().and_then(|build_id| progress.get(build_id)) else {
          continue;
        };
        if build.queued {
          queued += 1;
          continue;
        }
        probably_hanging |= build.probably_hanging == Some(true);
        if let Some(estimate) = build.estimated_total_running_seconds {
          let left = estimate - build.elapsed_running_seconds;
          if last.is_none_or(|(known, _, _)| left > known) {
            last = Some((left, check, build));
          }
        }
      }
      _ => {}
    }
  }
  if !running && queued == 0 {
    return None;
  }
  let seconds = |value: i64| u32::try_from(value.max(0)).unwrap_or(u32::MAX);
  Some(PatronusRobotProgress {
    remaining_seconds: last.filter(|(left, _, _)| *left > 0).map(|(left, _, _)| seconds(left)),
    check_name: last.map(|(_, check, _)| check.label().to_string()),
    check_percent: last.map(|(_, _, build)| seconds(build.percentage_complete.min(100))),
    overtime_seconds: last.filter(|(left, _, _)| *left <= 0).map(|(left, _, _)| seconds(-left)),
    queued,
    probably_hanging,
  })
}

/// The checks of the details, for a failure: an error when Patronus did not answer them, as the failure would be unknown
fn failure_checks(details: &CachedDetails) -> Result<&[TeamCityCheckDto], ActionError> {
  details.raw_checks.as_deref().ok_or_else(|| "Patronus did not answer the checks of the robot".to_string())
}

/// The failure of a robot from its checks and problems: the last attempt of each failed check, and of a running robot
/// also the running checks whose current attempt has a failed build or test; the error problems
pub fn robot_failure_of(robot: PatronusRobot, dto: &RobotDto, checks: &[TeamCityCheckDto], problems: &[ProblemDto]) -> RobotFailure {
  let running = robot.status == PatronusRobotStatus::Running;
  let mut omitted = OmittedFailures::default();
  let checks: Vec<FailedCheck> = failed_attempts(checks, running)
    .map(|(check, attempt)| FailedCheck {
      name: check.label().to_string(),
      build_configuration_id: check.build_configuration_id.clone().filter(|id| !id.is_empty()),
      attempt: attempt.number + 1,
      attempt_limit: check.attempt_limit,
      build_id: attempt.build_id.clone(),
      build_url: attempt.build_url.clone().filter(|url| !url.is_empty()),
      failed_tests: attempt.failed_tests_number,
      failed_builds: attempt.failed_builds_number,
      failed_to_start: attempt.failed_to_start_builds_number,
      running: check.status != "FAILURE",
    })
    .collect();
  let problems: Vec<RobotFailureProblem> = problems
    .iter()
    .filter(|problem| problem.is_error())
    .map(|problem| RobotFailureProblem {
      title: problem.title.clone(),
      subsystem: problem.subsystem.clone().filter(|subsystem| !subsystem.is_empty()),
      details: problem.details_markdown.as_deref().filter(|details| !details.trim().is_empty()).map(|details| {
        let (details, cut) = cut_head(details, usize::MAX, MAX_DETAILS_BYTES);
        omitted.truncated |= cut;
        details
      }),
    })
    .collect();
  RobotFailure {
    kind: failure_kind(&checks, &problems),
    robot,
    running,
    test_revision: dto.test_reference_revision.clone().filter(|revision| !revision.is_empty()),
    target_revision: dto.target_revision.clone().filter(|revision| !revision.is_empty()),
    checks,
    problems,
    builds: None,
    omitted,
  }
}

/// The checks that failed with their last attempt; of a running robot also the running checks whose current attempt
/// has a failed build or test already
fn failed_attempts(checks: &[TeamCityCheckDto], running: bool) -> impl Iterator<Item = (&TeamCityCheckDto, &TeamCityCheckAttemptDto)> {
  checks.iter().filter_map(move |check| {
    let attempt = check.last_attempt()?;
    let failed = match check.status.as_str() {
      "FAILURE" => true,
      "RUNNING" => running && (attempt.failed_builds_number.unwrap_or(0) > 0 || attempt.failed_tests_number.unwrap_or(0) > 0),
      _ => false,
    };
    failed.then_some((check, attempt))
  })
}

/// What failed: builds that never started (and no test) in every failed check, a massive failure Patronus detected,
/// checks, only a problem of Patronus, or nothing known
fn failure_kind(checks: &[FailedCheck], problems: &[RobotFailureProblem]) -> RobotFailureKind {
  let infrastructure = |check: &FailedCheck| check.failed_to_start.unwrap_or(0) > 0 && check.failed_tests.unwrap_or(0) == 0;
  let massive = problems.iter().any(|problem| {
    problem
      .subsystem
      .as_deref()
      .is_some_and(|subsystem| subsystem.starts_with(MASSIVE_FAILURE_SUBSYSTEM_PREFIX))
  });
  if !checks.is_empty() && checks.iter().all(infrastructure) {
    RobotFailureKind::Infrastructure
  } else if massive {
    RobotFailureKind::Massive
  } else if !checks.is_empty() {
    RobotFailureKind::Checks
  } else if !problems.is_empty() {
    RobotFailureKind::Patronus
  } else {
    RobotFailureKind::Unknown
  }
}

/// A failed build without its ignored problems and tests, its problem details cut; its tests listed while fewer than
/// `MAX_FAILED_TEST_NAMES` are, with the ids of their messages
fn to_failed_build(build: BuildDto, listed_tests: &mut usize, omitted: &mut OmittedFailures) -> (FailedBuild, Vec<(usize, [String; 3])>) {
  let problems = build
    .problems
    .into_iter()
    .filter(|problem| !problem.is_ignored_problem)
    .map(|problem| {
      let (details, cut) = cut_head(&strip_ansi(&problem.details), usize::MAX, MAX_DETAILS_BYTES);
      omitted.truncated |= cut;
      BuildProblem {
        kind: problem.kind.filter(|kind| !kind.is_empty()),
        details,
      }
    })
    .collect();
  let mut failed_tests = Vec::new();
  let mut test_runs = Vec::new();
  for test in build.failed_tests.into_iter().filter(|test| !test.is_ignored_failure) {
    if *listed_tests == MAX_FAILED_TEST_NAMES {
      omitted.tests += 1;
      continue;
    }
    *listed_tests += 1;
    test_runs.push((failed_tests.len(), [test.build_id, test.test_id, test.test_run_id]));
    failed_tests.push(FailedTest {
      name: test.name,
      url: test.url.filter(|url| !url.is_empty()),
      message: None,
      first_failed_before: None,
    });
  }
  let failed_build = FailedBuild {
    name: build.build_configuration_name,
    project: build.full_project_name.filter(|project| !project.is_empty()),
    build_id: build.build_id,
    build_url: build.build_url.filter(|url| !url.is_empty()),
    failed_to_start: build.is_failed_to_start,
    problems,
    failed_tests,
    log_tail: None,
  };
  (failed_build, test_runs)
}

/// The log tails (of the builds in `wants_log`) and the `firstFailed` of the builds of the CLI's server, all at once
/// (the CLI limits its processes); whether a log was cut
async fn add_teamcity_details(teamcity: &TeamCityCli, builds: &mut [FailedBuild], wants_log: &[bool]) -> bool {
  let mut pending = tokio::task::JoinSet::new();
  for (index, build) in builds.iter().enumerate() {
    let Some(server) = build.build_url.as_deref().and_then(|url| teamcity.server_for(url)) else {
      continue;
    };
    if wants_log[index] {
      let (teamcity, server, build_id) = (teamcity.clone(), server.to_string(), build.build_id.clone());
      pending.spawn(async move { (index, TeamCityAnswer::Log(teamcity.log_tail(&server, &build_id).await)) });
    }
    if !build.failed_tests.is_empty() {
      let (teamcity, server, build_id) = (teamcity.clone(), server.to_string(), build.build_id.clone());
      pending.spawn(async move { (index, TeamCityAnswer::FirstFailed(teamcity.first_failed_before(&server, &build_id).await)) });
    }
  }
  let mut truncated = false;
  while let Some(Ok((index, answer))) = pending.join_next().await {
    match answer {
      TeamCityAnswer::Log(Some(tail)) => {
        truncated |= tail.truncated;
        builds[index].log_tail = Some(tail.text);
      }
      TeamCityAnswer::FirstFailed(Some(first_failed)) => {
        for test in &mut builds[index].failed_tests {
          test.first_failed_before = first_failed.get(&test.name).copied();
        }
      }
      TeamCityAnswer::Log(None) | TeamCityAnswer::FirstFailed(None) => {}
    }
  }
  truncated
}

enum TeamCityAnswer {
  Log(Option<crate::teamcity_cli::LogTail>),
  FirstFailed(Option<HashMap<String, bool>>),
}

/// The errors of a robot, for the user, and whether Patronus warns that the running robot will fail
fn split_problems(problems: Vec<ProblemDto>) -> (Vec<PatronusProblem>, bool) {
  let failing = problems.iter().any(ProblemDto::is_robot_failing);
  let errors = problems
    .into_iter()
    .filter(ProblemDto::is_error)
    .map(|problem| PatronusProblem {
      title: problem.title,
      subsystem: problem.subsystem.filter(|subsystem| !subsystem.is_empty()),
    })
    .collect();
  (errors, failing)
}
