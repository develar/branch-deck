use crate::command_error::CommandError;
use crate::commands::github::{GitHubState, app_user_agent};
use crate::commands::pull_requests::github_repository;
use crate::operation::{operation, run_blocking};
use crate::repository_state::RepositoryStateCache;
use domain_types::calendar::Calendar;
use domain_types::patronus::{
  BranchReviewReadiness, GetBranchReviewReadinessParams, GetPatronusRobotsParams, GetPatronusStartOptionsParams, GetPatronusTargetBranchParams, PatronusAvailability,
  PatronusCommandComment, PatronusCommandReply, PatronusFailingRobotsParams, PatronusProblem, PatronusRobot, PatronusRobotDetails, PatronusRobotParams, PatronusRobotStatus,
  PatronusRobotsResult, PatronusStartOptions, PollPatronusRobotsParams, PostPatronusCommandParams, ReadPatronusCommandReplyParams, StartPatronusRobotParams,
};
use domain_types::robot_failure::{GetRobotCommitMapParams, GetRobotFailureParams, RobotCommitMap, RobotFailureReport};
use domain_types::robot_stats::{GetRobotStatsParams, RobotStatsEvent, RobotStatsResult};
use git_executor::git_command_executor::GitCommandExecutor;
use moka::sync::Cache;
use patronus::client::{PATRONUS_URL, PatronusClient};
use patronus::service::PatronusService;
use patronus::teamcity_cli::TeamCityCli;
use patronus::token::find_user_token;
use std::sync::Arc;
use std::time::Duration;
use sync_core::landing_gate::{LandingCheck, LandingGateParams, landing_check};
use sync_core::patronus::{SpaceRepository, find_space_repository, find_target_branch, prepare_robot_start, required_target_branch};
use sync_core::review_readiness::{branch_review_readiness, prepare_review_readiness};
use sync_core::robot_commits::{RobotSubject, robot_failure_report, robot_subject_commit_map};
use sync_core::robot_stats::{RobotStatsSource, robot_stats_core};
use sync_core::target_conflict::{TargetCheck, TargetConflictParams, target_conflict_core};
use sync_utils::timestamp::unix_now;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// The Patronus client of the app: one HTTP client (kept-alive connection), the token and backoff state, and the
/// Space repository of each repository, so a poll starts no git process
pub struct PatronusState {
  service: PatronusService,
  /// The Space repository (None = not on Space) by repository path. Re-read now and then: the remote can change.
  repositories: Cache<String, Option<SpaceRepository>>,
  /// The `teamcity` CLI with an authenticated server, once found; looked for again on each full failure until then (the
  /// user may log in meanwhile)
  teamcity: tokio::sync::Mutex<Option<TeamCityCli>>,
  /// By repository path: a read of the statistics waits for the one running, then reads what it kept
  stats_reads: Cache<String, Arc<tokio::sync::Mutex<()>>>,
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
      stats_reads: Cache::new(100),
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

  /// The user's robots for the checks of an action (the app's push, `PatronusService::robots_for_checks`): none off Space,
  /// or when the Space repository can't be read
  pub(crate) async fn robots_for_checks(&self, git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str) -> Vec<PatronusRobot> {
    match self.space_repository(git_executor, repository_path).await {
      Ok(Some(space)) => self.service.robots_for_checks(&space.name, branch_prefix).await,
      Ok(None) => Vec::new(),
      Err(error) => {
        warn!(error = ?error, "no robots for the checks: the Space repository is unknown");
        Vec::new()
      }
    }
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
  run_blocking(move || required_target_branch(&git, &path, &baseline)).await
}

/// The Safe Push statistics of the repository (`spec/repository/safe-push-statistics.spec.md`): everyone's robots into
/// the target of the last 70 days, kept in the object cache, the progress and the numbers of the lists through `events`
/// before the runs are read
#[tauri::command]
#[instrument(skip(git_executor, repository_state, patronus, events))]
#[specta::specta]
pub async fn get_robot_stats(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  params: GetRobotStatsParams,
  events: Channel<RobotStatsEvent>,
) -> Result<RobotStatsResult, CommandError> {
  operation(async move {
    let Some(repository) = patronus.space_repository(&git_executor, &params.repository_path).await? else {
      return Ok(RobotStatsResult {
        availability: PatronusAvailability::NotPatronus,
        stale: false,
        fetched_at: None,
        stats: None,
      });
    };
    let target = target_branch(&git_executor, &params.repository_path, &params.baseline_branch).await?;
    let read = patronus.stats_reads.get_with(params.repository_path.clone(), || Arc::new(tokio::sync::Mutex::new(())));
    let _reading = read.lock().await;
    let cache = repository_state.object_cache(&git_executor, &params.repository_path);
    let source = RobotStatsSource {
      service: &patronus.service,
      cache: &cache,
      repository: &repository.name,
      target: &target,
      branch_prefix: &params.branch_prefix,
    };
    let calendar = Calendar::from_zone(jiff::tz::TimeZone::system());
    let send = |event: RobotStatsEvent| {
      if let Err(e) = events.send(event) {
        tracing::debug!(error = %e, "the statistics window is gone");
      }
    };
    Ok(robot_stats_core(&source, &calendar, unix_now(), params.refresh, &send).await)
  })
  .await
}

/// The user's Patronus robots in the Space repository `origin` pushes to: the last 7 days, or the ones started since
/// `since`. When Patronus can't be asked, the availability says why.
#[tauri::command]
#[instrument(skip(git_executor, repository_state, patronus))]
#[specta::specta]
pub async fn get_patronus_robots(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  params: GetPatronusRobotsParams,
) -> Result<PatronusRobotsResult, CommandError> {
  operation(async move {
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
  })
  .await
}

/// The robots of `params.robots` whose status changed since the app saw it; the others are not in the result
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_prefix = %params.branch_prefix, robots = params.robots.len()))]
#[specta::specta]
pub async fn poll_patronus_robots(patronus: State<'_, PatronusState>, params: PollPatronusRobotsParams) -> Result<PatronusRobotsResult, CommandError> {
  operation(async move { Ok(patronus.service.poll(&params.branch_prefix, &params.robots).await) }).await
}

/// The robot with its merge profile, CI checks, error problems and whether it will fail, for the chip popover
#[tauri::command]
#[instrument(skip(patronus))]
#[specta::specta]
pub async fn get_patronus_robot_details(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<PatronusRobotDetails, CommandError> {
  operation(async move {
    patronus
      .service
      .robot_details(&params.branch_prefix, &params.robot_id, params.new_api)
      .await
      .map_err(CommandError::new)
  })
  .await
}

/// Why the robot failed, with the commit map of its branch, for the prompt for an agent: fast from the details, or full
/// with the failed builds, their tests and the log tails of the `teamcity` CLI
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, robot_id = %params.robot_id, full = params.full))]
#[specta::specta]
pub async fn get_robot_failure(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  params: GetRobotFailureParams,
) -> Result<RobotFailureReport, CommandError> {
  operation(async move {
    let failure = if params.full {
      let teamcity = patronus.teamcity().await;
      patronus.service.robot_failure_full(&params.branch_prefix, &params.robot_id, teamcity.as_ref()).await
    } else {
      patronus.service.robot_failure(&params.branch_prefix, &params.robot_id).await
    }
    .map_err(CommandError::new)?;
    let git = git_executor.inner().clone();
    run_blocking(move || robot_failure_report(&git, &params.repository_path, &params.branch_prefix, &params.ancestors, &params.baseline_branch, failure)).await
  })
  .await
}

/// Which commits of the branches a running robot covers, by the hashes of the checked-out branch: the commit rows mark
/// the commits added or changed since it started (`robot_subject_commit_map`). No commits for a stack without members.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = ?params.branch_name, stack_root = ?params.stack_root, ancestors = params.ancestors.len()))]
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

/// The branch a start pushes to and its merge profiles; the profile of the branch's last robot is the default
#[tauri::command]
#[instrument(skip(git_executor, patronus))]
#[specta::specta]
pub async fn get_patronus_start_options(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  params: GetPatronusStartOptionsParams,
) -> Result<PatronusStartOptions, CommandError> {
  operation(async move {
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
  })
  .await
}

/// Starts a Safe Push or a Dry Run of the pushed virtual branch, or with `stack` of the pushed stack branch of the whole
/// stack; the frontend pushes it first. First the landing gate and the target (`sync_core::patronus::prepare_robot_start`,
/// which the test server runs too), as `bd safe-push` refuses it: a proven dangling reference or a missing declaration
/// refuses the start unless `allow` lets it. The new robot.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, stack = params.stack, mode = ?params.mode, landed_commits = params.landed_commits.len()))]
#[specta::specta]
pub async fn start_patronus_robot(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  params: StartPatronusRobotParams,
) -> Result<PatronusRobot, CommandError> {
  operation(async move {
    let (git, repository_state_cache, start) = ((*git_executor).clone(), (*repository_state).clone(), params.clone());
    let branches = run_blocking(move || {
      let cache = repository_state_cache.object_cache(&git, &start.repository_path);
      prepare_robot_start(&git, &cache, &start)
    })
    .await?;
    let repository = patronus.required_space_repository(&git_executor, &params.repository_path).await?;
    patronus
      .service
      .start(
        &repository.name,
        &params.branch_prefix,
        &branches.source_branch,
        &branches.target_branch,
        params.mode,
        params.profile_id,
        params.merge_request,
      )
      .await
      .map_err(CommandError::new)
  })
  .await
}

/// What refuses the start of a Safe Push or a Dry Run, as `bd safe-push` refuses it (`sync_core::landing_gate`): the
/// refusals left after `allow`, each with its fix; asked before the push, through the object cache of the repository
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, stack = params.stack, landed_commits = params.landed_commits.len()))]
#[specta::specta]
pub async fn check_landing_gate(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  params: LandingGateParams,
) -> Result<LandingCheck, CommandError> {
  let (git, repository_state) = ((*git_executor).clone(), (*repository_state).clone());
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    landing_check(&git, &cache, &params)
  })
  .await
}

/// Whether Patronus can rebase the branch a start pushes onto the newest target, as `bd safe-push` checks it: the fetch
/// of the target, then the replay of the branch onto its tip; a failed fetch or replay is in the result, never an error
#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn check_target_conflict(git_executor: State<'_, GitCommandExecutor>, params: TargetConflictParams) -> Result<TargetCheck, CommandError> {
  let git = git_executor.inner().clone();
  run_blocking(move || Ok::<_, CommandError>(target_conflict_core(&git, &params))).await
}

/// Starts the robot again on the newest commit of its branch; the new robot
#[tauri::command]
#[instrument(skip(patronus))]
#[specta::specta]
pub async fn restart_patronus_robot(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<PatronusRobot, CommandError> {
  operation(async move { patronus.service.restart(&params.branch_prefix, &params.robot_id).await.map_err(CommandError::new) }).await
}

/// Asks Patronus to cancel the robot; it answers it canceling
#[tauri::command]
#[instrument(skip(patronus))]
#[specta::specta]
pub async fn cancel_patronus_robot(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<PatronusRobot, CommandError> {
  operation(async move { patronus.service.cancel(&params.branch_prefix, &params.robot_id).await.map_err(CommandError::new) }).await
}

/// The current status of one robot, read right before a push into its branch
#[tauri::command]
#[instrument(skip(patronus))]
#[specta::specta]
pub async fn get_patronus_robot_status(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<PatronusRobotStatus, CommandError> {
  operation(async move { patronus.service.robot_status(&params.robot_id).await.map_err(CommandError::new) }).await
}

/// The error problems of a finished robot, once per session: the chip names a missing review
#[tauri::command]
#[instrument(skip(patronus))]
#[specta::specta]
pub async fn get_patronus_robot_problems(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<Vec<PatronusProblem>, CommandError> {
  operation(async move { patronus.service.robot_problems(&params.robot_id).await.map_err(CommandError::new) }).await
}

/// Of the running robots, the ones a failed required check dooms (Patronus' warning), asked after each poll until known
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, robots = params.robot_ids.len()))]
#[specta::specta]
pub async fn get_patronus_failing_robots(patronus: State<'_, PatronusState>, params: PatronusFailingRobotsParams) -> Result<Vec<String>, CommandError> {
  operation(async move { patronus.service.failing_robots(&params.robot_ids).await.map_err(CommandError::new) }).await
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
  operation(async move {
    let space = patronus.required_space_repository(&git_executor, &params.repository_path).await?;
    let github = github_repository(&git_executor, &repository_state, &params.repository_path, None).await?;
    let (inputs, cache, params) = {
      let (git, repository_state) = ((*git_executor).clone(), (*repository_state).clone());
      run_blocking(move || {
        let inputs = prepare_review_readiness(&git, space, github, &params)?;
        let cache = repository_state.object_cache(&git, &params.repository_path);
        anyhow::Ok((inputs, cache, params))
      })
      .await?
    };
    branch_review_readiness(
      &git_executor,
      &params.repository_path,
      &cache,
      &patronus.service,
      &inputs.repository(),
      &params.branches,
      params.recheck,
    )
    .await
    .map_err(CommandError::from)
  })
  .await
}

/// Starts a Safe Push (`/patronus merge`) or a Dry Run (`/patronus dry-run`) from the GitHub pull request of the
/// branch: Patronus links the robot to the pull request only from its comment. The posted comment; the bot answers
/// with a reaction (`read_patronus_command_reply`).
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn post_patronus_command(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: PostPatronusCommandParams,
) -> Result<PatronusCommandComment, CommandError> {
  operation(async move {
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
  })
  .await
}

/// How Patronus answered the command comment: pending, accepted (the robot started) or rejected with the reason
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn read_patronus_command_reply(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: ReadPatronusCommandReplyParams,
) -> Result<PatronusCommandReply, CommandError> {
  operation(async move {
    let repository = github_repository(&git_executor, &repository_state, &params.repository_path, params.github_repository.as_deref())
      .await?
      .ok_or_else(unknown_github_repository)?;
    sync_core::patronus_command::read_patronus_command_reply(&github.service, &params.repository_path, &repository, params.pull_request_number, &params.comment_id)
      .await
      .map_err(|e| CommandError::new(e.to_string()))
  })
  .await
}

fn unknown_github_repository() -> CommandError {
  CommandError::new(sync_core::patronus_command::UNKNOWN_GITHUB_REPOSITORY_MESSAGE)
}

/// The branch Patronus pushes into for the baseline (`master` for `origin/master`); None when the baseline is no branch
/// of origin
#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn get_patronus_target_branch(git_executor: State<'_, GitCommandExecutor>, params: GetPatronusTargetBranchParams) -> Result<Option<String>, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || find_target_branch(&git, &params.repository_path, &params.baseline_branch)).await
}
