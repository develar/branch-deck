use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::commands::github::{GitHubState, app_user_agent};
use crate::commands::pull_requests::github_repository;
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_names::{to_final_branch_name, to_stack_branch_name};
use git_ops::revs::is_object_id;
use moka::sync::Cache;
use patronus::client::{PATRONUS_URL, PatronusClient};
use patronus::service::PatronusService;
use patronus::teamcity_cli::TeamCityCli;
use patronus::token::find_user_token;
use std::time::Duration;
use sync_core::patronus::{SpaceRepository, find_space_repository, find_target_branch};
use sync_core::review_readiness::{ReadinessRepository, branch_review_readiness};
use sync_core::robot_commits::{RobotSubject, robot_failure_report, robot_subject_commit_map};
use sync_core::robot_landing::robot_landings;
use sync_core::target_conflict::{TargetCheck, TargetConflictParams, target_conflict_core};
use sync_types::patronus::{
  BranchReviewReadiness, GetBranchReviewReadinessParams, GetPatronusRobotsParams, GetPatronusStartOptionsParams, GetPatronusTargetBranchParams, PatronusAvailability,
  PatronusCommandComment, PatronusCommandReply, PatronusFailingRobotsParams, PatronusProblem, PatronusRobot, PatronusRobotDetails, PatronusRobotParams, PatronusRobotStatus,
  PatronusRobotsResult, PatronusStartOptions, PollPatronusRobotsParams, PostPatronusCommandParams, ReadPatronusCommandReplyParams, StartPatronusRobotParams,
};
use sync_types::robot_failure::{GetRobotCommitMapParams, GetRobotFailureParams, RobotCommitMap, RobotFailureReport};
use sync_types::robot_landing::{GetRobotLandingsParams, RobotLanding};
use tauri::State;
use tracing::instrument;

/// The Patronus client of the app: one HTTP client (kept-alive connection), the token and backoff state, and the
/// Space repository of each repository, so a poll starts no git process
pub struct PatronusState {
  service: PatronusService,
  /// The Space repository (None = not on Space) by repository path. Re-read now and then: the remote can change.
  repositories: Cache<String, Option<SpaceRepository>>,
  /// The `teamcity` CLI with an authenticated server, once found; looked for again on each full failure until then (the
  /// user may log in meanwhile)
  teamcity: tokio::sync::Mutex<Option<TeamCityCli>>,
}

impl PatronusState {
  pub fn new(app_version: &str) -> Self {
    let user_agent = app_user_agent(app_version);
    // the variable of the Safe Push CLI, for the staging and dev instances
    let base_url = std::env::var("SAFEPUSH_PATRONUS_URL").map_or_else(|_| PATRONUS_URL.to_string(), |url| url.trim_end_matches('/').to_string());
    Self {
      service: PatronusService::new(PatronusClient::new(base_url, &user_agent), find_user_token),
      repositories: Cache::builder().max_capacity(100).time_to_live(Duration::from_secs(600)).build(),
      teamcity: tokio::sync::Mutex::new(None),
    }
  }

  async fn teamcity(&self) -> Option<TeamCityCli> {
    let mut teamcity = self.teamcity.lock().await;
    if teamcity.is_none() {
      *teamcity = TeamCityCli::discover().await;
    }
    teamcity.clone()
  }

  /// The Space repository `origin` pushes to: one `remote get-url --push` (`find_space_repository`) on a miss. The GitHub
  /// repository of its branches (a mirror of the Space one) is the app's, kept while the config stays as it is
  /// (`RepositoryStateCache::github_repository`)
  pub(crate) async fn space_repository(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<SpaceRepository>, CommandError> {
    if let Some(space) = self.repositories.get(repository_path) {
      return Ok(space);
    }
    let git = git_executor.clone();
    let path = repository_path.to_string();
    let space = run_blocking(move || find_space_repository(&git, &path)).await?;
    self.repositories.insert(repository_path.to_string(), space.clone());
    Ok(space)
  }

  /// The Space repository, for an action: not on Space is an error
  async fn required_space_repository(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<SpaceRepository, CommandError> {
    self.space_repository(git_executor, repository_path).await?.ok_or_else(not_on_space)
  }
}

fn not_on_space() -> CommandError {
  CommandError::new("The repository is not on Space: Patronus can't push it")
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
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  params: GetPatronusRobotsParams,
) -> Result<PatronusRobotsResult, CommandError> {
  let Some(repository) = patronus.space_repository(&git_executor, &params.repository_path).await? else {
    return Ok(PatronusRobotsResult::from(PatronusAvailability::NotPatronus));
  };
  let mut result = patronus
    .service
    .robots(&repository.name, &params.branch_prefix, params.since, params.background, params.new_api)
    .await;
  result.github_repository = github_repository(&git_executor, &repository_state, &params.repository_path, None)
    .await?
    .map(|github| github.full_name());
  Ok(result)
}

/// The robots of `params.robots` whose status changed since the app saw it; the others are not in the result
#[tauri::command]
#[specta::specta]
pub async fn poll_patronus_robots(patronus: State<'_, PatronusState>, params: PollPatronusRobotsParams) -> Result<PatronusRobotsResult, CommandError> {
  Ok(patronus.service.poll(&params.branch_prefix, &params.robots).await)
}

/// The robot with its merge profile, CI checks, error problems and whether it will fail, for the chip popover
#[tauri::command]
#[specta::specta]
pub async fn get_patronus_robot_details(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<PatronusRobotDetails, CommandError> {
  patronus
    .service
    .robot_details(&params.branch_prefix, &params.robot_id, params.new_api)
    .await
    .map_err(CommandError::new)
}

/// Why the robot failed, with the commit map of its branch, for the prompt for an agent: fast from the details, or full
/// with the failed builds, their tests and the log tails of the `teamcity` CLI
#[tauri::command]
#[specta::specta]
pub async fn get_robot_failure(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  params: GetRobotFailureParams,
) -> Result<RobotFailureReport, CommandError> {
  let failure = if params.full {
    let teamcity = patronus.teamcity().await;
    patronus.service.robot_failure_full(&params.branch_prefix, &params.robot_id, teamcity.as_ref()).await
  } else {
    patronus.service.robot_failure(&params.branch_prefix, &params.robot_id).await
  }
  .map_err(CommandError::new)?;
  let git = git_executor.inner().clone();
  run_blocking(move || robot_failure_report(&git, &params.repository_path, &params.branch_prefix, &params.ancestors, &params.baseline_branch, failure)).await
}

/// Which commits of the branches a running robot covers, by the hashes of the checked-out branch: the commit rows mark
/// the commits added or changed since it started (`robot_subject_commit_map`). No commits for a stack without members.
#[tauri::command]
#[specta::specta]
pub async fn get_robot_commit_map(git_executor: State<'_, GitCommandExecutor>, params: GetRobotCommitMapParams) -> Result<RobotCommitMap, CommandError> {
  let git = git_executor.inner().clone();
  run_blocking(move || {
    let subject = RobotSubject {
      branch_name: params.branch_name.as_deref(),
      stack_root: params.stack_root.as_deref(),
    };
    robot_subject_commit_map(
      &git,
      &params.repository_path,
      &params.branch_prefix,
      subject,
      &params.source_revision,
      &params.baseline_branch,
      &params.ancestors,
    )
    .map(Option::unwrap_or_default)
  })
  .await
}

/// What merged robots landed of the branches, by content: the commits each tested against the branches as the last sync
/// built them, and the commits added after them. The app asks it when a change below a robot's commit (an amend, a new
/// commit of a branch it is built on, any change of the whole stack) gave the branches new copies, so their hashes no
/// longer tell (`robot_landings`)
#[tauri::command]
#[specta::specta]
pub async fn get_robot_landings(git_executor: State<'_, GitCommandExecutor>, params: GetRobotLandingsParams) -> Result<Vec<RobotLanding>, CommandError> {
  let git = git_executor.inner().clone();
  run_blocking(move || robot_landings(&git, &params.repository_path, &params.branch_prefix, &params.baseline_branch, &params.robots)).await
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
    .start_options(
      &repository.project_key,
      &repository.name,
      &target,
      params.last_robot_id.as_deref(),
      params.last_robot_profile,
    )
    .await
    .map_err(CommandError::new)
}

/// Starts a Safe Push or a Dry Run of the pushed virtual branch, or with `stack` of the pushed stack branch of the whole
/// stack; the frontend pushes it first. The new robot.
#[tauri::command]
#[specta::specta]
pub async fn start_patronus_robot(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  params: StartPatronusRobotParams,
) -> Result<PatronusRobot, CommandError> {
  let repository = patronus.required_space_repository(&git_executor, &params.repository_path).await?;
  let target = target_branch(&git_executor, &params.repository_path, &params.baseline_branch).await?;
  let source = if params.stack {
    to_stack_branch_name(&params.branch_prefix, &params.branch_name)?
  } else {
    to_final_branch_name(&params.branch_prefix, &params.branch_name)?
  };
  let source_branch = format!("refs/heads/{source}");
  patronus
    .service
    .start(
      &repository.name,
      &params.branch_prefix,
      &source_branch,
      &target,
      params.mode,
      params.profile_id,
      params.merge_request,
    )
    .await
    .map_err(CommandError::new)
}

/// Whether Patronus can rebase the branch a start pushes onto the newest target, as `bd safe-push` checks it: the fetch
/// of the target, then the replay of the branch onto its tip; a failed fetch or replay is in the result, never an error
#[tauri::command]
#[specta::specta]
pub async fn check_target_conflict(git_executor: State<'_, GitCommandExecutor>, params: TargetConflictParams) -> Result<TargetCheck, CommandError> {
  let git = git_executor.inner().clone();
  run_blocking(move || Ok::<_, CommandError>(target_conflict_core(&git, &params))).await
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

/// The error problems of a finished robot, once per session: the chip names a missing review
#[tauri::command]
#[specta::specta]
pub async fn get_patronus_robot_problems(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<Vec<PatronusProblem>, CommandError> {
  patronus.service.robot_problems(&params.robot_id).await.map_err(CommandError::new)
}

/// Of the running robots, the ones a failed required check dooms (Patronus' warning), asked after each poll until known
#[tauri::command]
#[specta::specta]
pub async fn get_patronus_failing_robots(patronus: State<'_, PatronusState>, params: PatronusFailingRobotsParams) -> Result<Vec<String>, CommandError> {
  patronus.service.failing_robots(&params.robot_ids).await.map_err(CommandError::new)
}

/// Whether a Safe Push of each branch would pass the code-owner review, for the tip and the start the page shows:
/// Patronus judges the approvals of the pull request the robot would start from. One `git log` for the commits of all
/// branches, their paths from the object cache, then one pre-check per branch whose kept answer doesn't hold
/// (`sync_core::review_readiness`), one after another; a failure of Patronus backs the others off. The answers are in
/// the order of `params.branches`, each with its input.
// `instrument` must come before `specta::specta`, which rejects the expressions of `fields`
#[tauri::command]
#[instrument(skip_all, fields(branches = params.branches.len(), recheck = params.recheck))]
#[specta::specta]
pub async fn get_branch_review_readiness(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  params: GetBranchReviewReadinessParams,
) -> Result<Vec<BranchReviewReadiness>, CommandError> {
  if let Some(branch) = params.branches.iter().find(|branch| !is_object_id(&branch.tip)) {
    return Err(CommandError::new(format!("Not a commit id: {}", branch.tip)));
  }
  let space = patronus.required_space_repository(&git_executor, &params.repository_path).await?;
  let github = github_repository(&git_executor, &repository_state, &params.repository_path, None).await?;
  let target = target_branch(&git_executor, &params.repository_path, &params.baseline_branch).await?;
  let cache = {
    let (git, path, repository_state) = ((*git_executor).clone(), params.repository_path.clone(), (*repository_state).clone());
    run_blocking(move || anyhow::Ok(repository_state.object_cache(&git, &path))).await?
  };
  let repository = ReadinessRepository {
    space: &space,
    github: github.as_ref(),
    target: &target,
    baseline: &params.baseline_branch,
    branch_prefix: &params.branch_prefix,
  };
  branch_review_readiness(
    &git_executor,
    &params.repository_path,
    &cache,
    &patronus.service,
    &repository,
    &params.branches,
    params.recheck,
  )
  .await
  .map_err(CommandError::from)
}

/// Starts a Safe Push (`/patronus merge`) or a Dry Run (`/patronus dry-run`) from the GitHub pull request of the
/// branch: Patronus links the robot to the pull request only from its comment. The posted comment; the bot answers
/// with a reaction (`read_patronus_command_reply`).
#[tauri::command]
#[specta::specta]
pub async fn post_patronus_command(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: PostPatronusCommandParams,
) -> Result<PatronusCommandComment, CommandError> {
  let repository = github_repository(&git_executor, &repository_state, &params.repository_path, params.github_repository.as_deref())
    .await?
    .ok_or_else(unknown_github_repository)?;
  sync_core::patronus_command::post_patronus_command(
    &github.service,
    &params.repository_path,
    &repository,
    params.pull_request_number,
    params.mode,
    params.subject,
  )
  .await
  .map_err(|e| CommandError::new(e.to_string()))
}

/// How Patronus answered the command comment: pending, accepted (the robot started) or rejected with the reason
#[tauri::command]
#[specta::specta]
pub async fn read_patronus_command_reply(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: ReadPatronusCommandReplyParams,
) -> Result<PatronusCommandReply, CommandError> {
  let repository = github_repository(&git_executor, &repository_state, &params.repository_path, params.github_repository.as_deref())
    .await?
    .ok_or_else(unknown_github_repository)?;
  sync_core::patronus_command::read_patronus_command_reply(&github.service, &params.repository_path, &repository, params.pull_request_number, &params.comment_id)
    .await
    .map_err(|e| CommandError::new(e.to_string()))
}

/// The repository of the pull request a `/patronus` comment goes to is the one the review requests name, else the
/// mirror of the Space repository: neither is known
fn unknown_github_repository() -> CommandError {
  CommandError::new("The GitHub repository of the branches is unknown: the Space settings name no GitHub mirror; run git config branchdeck.githubRepository <owner>/<name>")
}

/// The branch Patronus pushes into for the baseline (`master` for `origin/master`); None when the baseline is no branch
/// of origin
#[tauri::command]
#[specta::specta]
pub async fn get_patronus_target_branch(git_executor: State<'_, GitCommandExecutor>, params: GetPatronusTargetBranchParams) -> Result<Option<String>, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || find_target_branch(&git, &params.repository_path, &params.baseline_branch)).await
}
