//! What the user does with a robot (start, restart, cancel) and what the chip popover asks on demand (the checks, the
//! problems, the merge profiles). Each is one request or a few in parallel; the answers that don't change are cached:
//! the details of a finished robot for the session, of a running one for a minute, the profiles for 10 minutes.

use crate::client::{BranchRobotStartRequest, MERGE_PROFILE_ID_OPTION, MERGE_PROFILE_NAME_OPTION, ProblemDto, RobotDto, SpaceReviewDto, StartSupportedDto, TeamCityCheckDto};
use crate::service::{PatronusService, parse_push_mode, parse_status, to_robot};
use std::time::{Duration, Instant};
use sync_types::patronus::{
  PatronusAvailability, PatronusChecks, PatronusMergeRequest, PatronusProblem, PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotDetails, PatronusRobotStatus,
  PatronusStartOptions,
};
use tracing::instrument;

/// The details of a running robot change: asked again after this
const RUNNING_DETAILS_TTL: Duration = Duration::from_secs(60);
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
}

/// A failed action, for the user
pub type ActionError = String;

impl PatronusService {
  /// The robot with its profile, checks and error problems
  #[instrument(skip(self))]
  pub async fn robot_details(&self, branch_prefix: &str, robot_id: &str) -> Result<PatronusRobotDetails, ActionError> {
    let details = self.cached_details(robot_id).await?;
    let robot = to_robot(&details.robot, branch_prefix, self.client());
    Ok(PatronusRobotDetails {
      profile_id: details.robot.options.get(MERGE_PROFILE_ID_OPTION).cloned().filter(|id| !id.is_empty()),
      profile_name: details.robot.options.get(MERGE_PROFILE_NAME_OPTION).cloned().filter(|name| !name.is_empty()),
      robot,
      checks: details.checks,
      problems: details.problems,
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
    let details = CachedDetails {
      robot,
      // the checks and the problems are extras: the details show without them
      checks: checks.ok().and_then(|checks| summarize_checks(&checks)),
      problems: problems.unwrap_or_default().into_iter().map(to_problem).collect(),
    };
    if parse_status(&details.robot.status).is_finished() {
      self.caches.problems.insert(robot_id.to_string(), details.problems.clone());
    }
    self.caches.details.insert(robot_id.to_string(), (Instant::now(), details.clone()));
    Ok(details)
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
        .and_then(|details| details.robot.options.get(MERGE_PROFILE_ID_OPTION).cloned())
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
      merge_profile_id: profile_id,
      space_review: merge_request.map(|merge_request| SpaceReviewDto {
        project_key: merge_request.project_key,
        review_key: merge_request.review_key,
      }),
    };
    let token = self.action_token()?;
    let response = self.client().start_robot(&token.value, &request).await;
    let robot_id = self.settle(response, &token.stamp).map_err(availability_message)?;
    self.fetch_robot(branch_prefix, &robot_id).await
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

fn to_problem(problem: ProblemDto) -> PatronusProblem {
  PatronusProblem {
    title: problem.title,
    subsystem: problem.subsystem.filter(|subsystem| !subsystem.is_empty()),
  }
}
