use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::to_final_branch_name;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use moka::sync::Cache;
use patronus::client::{PATRONUS_URL, PatronusClient};
use patronus::service::PatronusService;
use patronus::token::find_user_token;
use std::time::Duration;
use sync_core::patronus::{SpaceRepository, find_space_repository, find_target_branch};
use sync_types::patronus::{
  GetPatronusRobotsParams, GetPatronusStartOptionsParams, PatronusAvailability, PatronusRobot, PatronusRobotDetails, PatronusRobotParams, PatronusRobotStatus,
  PatronusRobotsResult, PatronusStartOptions, PollPatronusRobotsParams, StartPatronusRobotParams,
};
use tauri::State;

/// The Patronus client of the app: one HTTP client (kept-alive connection), the token and backoff state, and the
/// Space repository of each repository, so a poll starts no git process
pub struct PatronusState {
  service: PatronusService,
  /// By repository path; None = not on Space. Re-read now and then: the remote can change.
  repositories: Cache<String, Option<SpaceRepository>>,
}

impl PatronusState {
  pub fn new(app_version: &str) -> Self {
    let user_agent = format!("Branch Deck/{app_version} ({}; {})", std::env::consts::OS, std::env::consts::ARCH);
    // the variable of the Safe Push CLI, for the staging and dev instances
    let base_url = std::env::var("SAFEPUSH_PATRONUS_URL").map_or_else(|_| PATRONUS_URL.to_string(), |url| url.trim_end_matches('/').to_string());
    Self {
      service: PatronusService::new(PatronusClient::new(base_url, &user_agent), find_user_token),
      repositories: Cache::builder().max_capacity(100).time_to_live(Duration::from_secs(600)).build(),
    }
  }

  async fn space_repository(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<SpaceRepository>, CommandError> {
    if let Some(repository) = self.repositories.get(repository_path) {
      return Ok(repository);
    }
    let git = git_executor.clone();
    let path = repository_path.to_string();
    let repository = run_blocking(move || find_space_repository(&git, &path)).await?;
    self.repositories.insert(repository_path.to_string(), repository.clone());
    Ok(repository)
  }

  /// The Space repository, for an action: not on Space is an error
  async fn required_space_repository(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<SpaceRepository, CommandError> {
    self
      .space_repository(git_executor, repository_path)
      .await?
      .ok_or_else(|| CommandError::new("The repository is not on Space: Patronus can't push it"))
  }
}

/// The branch a robot pushes to, from the baseline: the branch of `origin` it tracks
async fn target_branch(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str) -> Result<String, CommandError> {
  let git = git_executor.clone();
  let path = repository_path.to_string();
  let baseline = baseline_branch.to_string();
  run_blocking(move || find_target_branch(&git, &path, &baseline))
    .await?
    .ok_or_else(|| CommandError::new(format!("The baseline {baseline_branch} is not a branch of origin: Patronus pushes into a branch of origin")))
}

/// The user's Patronus robots in the Space repository `origin` pushes to: the last 7 days, or the ones started since
/// `since`. When Patronus can't be asked, the availability says why.
#[tauri::command]
#[specta::specta]
pub async fn get_patronus_robots(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  params: GetPatronusRobotsParams,
) -> Result<PatronusRobotsResult, CommandError> {
  let Some(repository) = patronus.space_repository(&git_executor, &params.repository_path).await? else {
    return Ok(PatronusRobotsResult::from(PatronusAvailability::NotPatronus));
  };
  Ok(patronus.service.robots(&repository.name, &params.branch_prefix, params.since, params.background).await)
}

/// The robots of `params.robots` whose status changed since the app saw it; the others are not in the result
#[tauri::command]
#[specta::specta]
pub async fn poll_patronus_robots(patronus: State<'_, PatronusState>, params: PollPatronusRobotsParams) -> Result<PatronusRobotsResult, CommandError> {
  Ok(patronus.service.poll(&params.branch_prefix, &params.robots).await)
}

/// The robot with its merge profile, CI checks and error problems, for the chip popover
#[tauri::command]
#[specta::specta]
pub async fn get_patronus_robot_details(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<PatronusRobotDetails, CommandError> {
  patronus.service.robot_details(&params.branch_prefix, &params.robot_id).await.map_err(CommandError::new)
}

/// The branch a start pushes to and its merge profiles; the profile of the branch's last robot is the default
#[tauri::command]
#[specta::specta]
pub async fn get_patronus_start_options(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  params: GetPatronusStartOptionsParams,
) -> Result<PatronusStartOptions, CommandError> {
  let repository = patronus.required_space_repository(&git_executor, &params.repository_path).await?;
  let target = target_branch(&git_executor, &params.repository_path, &params.baseline_branch).await?;
  patronus
    .service
    .start_options(&repository.project_key, &repository.name, &target, params.last_robot_id.as_deref())
    .await
    .map_err(CommandError::new)
}

/// Starts a Safe Push or a Dry Run of the pushed virtual branch; the frontend pushes it first. The new robot.
#[tauri::command]
#[specta::specta]
pub async fn start_patronus_robot(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  params: StartPatronusRobotParams,
) -> Result<PatronusRobot, CommandError> {
  let repository = patronus.required_space_repository(&git_executor, &params.repository_path).await?;
  let target = target_branch(&git_executor, &params.repository_path, &params.baseline_branch).await?;
  let source_branch = format!("refs/heads/{}", to_final_branch_name(&params.branch_prefix, &params.branch_name)?);
  patronus
    .service
    .start(
      &repository.name,
      &params.branch_prefix,
      &source_branch,
      &target,
      params.mode,
      params.profile_id,
      params.just_pushed,
    )
    .await
    .map_err(CommandError::new)
}

/// Starts the robot again on the newest commit of its branch; the new robot
#[tauri::command]
#[specta::specta]
pub async fn restart_patronus_robot(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<PatronusRobot, CommandError> {
  patronus.service.restart(&params.branch_prefix, &params.robot_id).await.map_err(CommandError::new)
}

/// Asks Patronus to cancel the robot; it answers it canceling
#[tauri::command]
#[specta::specta]
pub async fn cancel_patronus_robot(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<PatronusRobot, CommandError> {
  patronus.service.cancel(&params.branch_prefix, &params.robot_id).await.map_err(CommandError::new)
}

/// The current status of one robot, read right before a push into its branch
#[tauri::command]
#[specta::specta]
pub async fn get_patronus_robot_status(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<PatronusRobotStatus, CommandError> {
  patronus.service.robot_status(&params.robot_id).await.map_err(CommandError::new)
}
