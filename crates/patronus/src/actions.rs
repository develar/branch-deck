//! What the user does with a robot (start, restart, cancel) and what the chip popover asks on demand (the checks, the
//! problems, the merge profiles, the progress of the running checks). Each is one request or a few in parallel; the
//! answers that don't change are cached: the details of a finished robot for the session, of a running one for a
//! minute, the progress of a build for a minute, the profiles for 10 minutes.

use crate::client::{BranchRobotStartRequest, BuildProgressDto, ProblemDto, RobotDto, SpaceReviewDto, StartSupportedDto, TeamCityCheckDto};
use crate::service::{PatronusService, parse_push_mode, parse_status, robot_profile, to_robot};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use sync_types::patronus::{
  PatronusAvailability, PatronusChecks, PatronusMergeRequest, PatronusProblem, PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotDetails, PatronusRobotProfile,
  PatronusRobotProgress, PatronusRobotStatus, PatronusStartOptions,
};
use tracing::instrument;

/// The details of a running robot change: asked again after this
const RUNNING_DETAILS_TTL: Duration = Duration::from_secs(60);
/// Patronus caches the progress of a build for a minute: asking more often would read the same answer
const BUILD_PROGRESS_TTL: Duration = Duration::from_secs(60);
/// The profiles of a branch change rarely
const START_SUPPORTED_TTL: Duration = Duration::from_secs(600);
const START_SUPPORTED_ERROR_TTL: Duration = Duration::from_secs(60);
/// Failed check names kept for the popover
const MAX_FAILED_NAMES: usize = 3;

/// The details without the branch name, which depends on the branch prefix of the caller
#[derive(Clone)]
pub struct CachedDetails {
  robot: RobotDto,
  checks: Option<PatronusChecks>,
  problems: Vec<PatronusProblem>,
  progress: Option<PatronusRobotProgress>,
}

/// A failed action, for the user
pub type ActionError = String;

impl PatronusService {
  /// The robot with its profile, checks and error problems
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
    let details = CachedDetails {
      robot,
      // the checks and the problems are extras: the details show without them
      checks: checks.as_deref().and_then(summarize_checks),
      problems: problems.unwrap_or_default().into_iter().map(to_problem).collect(),
      progress,
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

  /// The error problems of a finished robot, asked once per session (the chip names a missing review); the details
  /// popover fills the same cache
  #[instrument(skip(self))]
  pub async fn robot_problems(&self, robot_id: &str) -> Result<Vec<PatronusProblem>, ActionError> {
    if let Some(problems) = self.caches.problems.get(robot_id) {
      return Ok(problems);
    }
    let token = self.token(true).map_err(|result| availability_message(result.availability))?;
    let response = self.client().get_problems(&token.value, robot_id).await;
    let problems: Vec<PatronusProblem> = self.settle(response, &token.stamp).map_err(availability_message)?.into_iter().map(to_problem).collect();
    self.caches.problems.insert(robot_id.to_string(), problems.clone());
    Ok(problems)
  }

  /// The merge profiles of the target branch and the profile of the last robot of the branch as the default
  #[instrument(skip(self))]
  pub async fn start_options(&self, project_key: &str, repository: &str, target_branch: &str, last_robot_id: Option<&str>) -> Result<PatronusStartOptions, ActionError> {
    let supported = self.cached_start_supported(project_key, repository, target_branch).await?;
    let profiles: Vec<PatronusProfile> = supported
      .profiles
      .iter()
      .filter(|profile| !profile.is_emergency)
      .map(|profile| PatronusProfile {
        id: profile.id.clone(),
        name: profile.name.clone(),
        description: profile.description.clone().filter(|description| !description.is_empty()),
        allowed_push_modes: profile.allowed_push_modes.iter().map(|mode| parse_push_mode(mode)).collect(),
      })
      .collect();
    let default_profile_id = match last_robot_id {
      // the last robot is usually in the cache: its popover was open
      Some(robot_id) => self
        .cached_details(robot_id)
        .await
        .ok()
        .and_then(|details| robot_profile(&details.robot)?.id)
        .filter(|id| profiles.iter().any(|profile| &profile.id == id)),
      None => None,
    };
    Ok(PatronusStartOptions {
      target_branch: target_branch.to_string(),
      supported: supported.supported,
      profiles,
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
  #[allow(clippy::too_many_arguments)]
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
      robot.profile = Some(PatronusRobotProfile { id: profile_id, name: None });
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
          summary.failed_names.push(check.build_configuration_name.clone());
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
    check_name: last.map(|(_, check, _)| check.build_configuration_name.clone()),
    check_percent: last.map(|(_, _, build)| seconds(build.percentage_complete.min(100))),
    overtime_seconds: last.filter(|(left, _, _)| *left <= 0).map(|(left, _, _)| seconds(-left)),
    queued,
    probably_hanging,
  })
}

fn to_problem(problem: ProblemDto) -> PatronusProblem {
  PatronusProblem {
    title: problem.title,
    subsystem: problem.subsystem.filter(|subsystem| !subsystem.is_empty()),
  }
}
