use crate::commands::branch_prefix::PrefixInputs;
use crate::commands::github::{GitHubState, app_user_agent};
use crate::commands::pull_requests::github_repository;
use crate::operation::{operation, run_blocking};
use crate::repository_state::RepositoryStateCache;
use app_api::api_error::patronus_error;
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::calendar::Calendar;
use domain_types::error::ApiError;
use domain_types::patronus::{
  GetPatronusTargetBranchParams, PatronusAvailability, PatronusCommandComment, PatronusCommandReply, PatronusCommandSubject, PostPatronusCommandParams,
  ReadPatronusCommandReplyParams,
};
use domain_types::robot_failure::{GetRobotCommitMapParams, GetRobotFailureParams, RobotCommitMap, RobotFailureReport};
use domain_types::robot_stats::{GetRobotStatsParams, RobotStatsEvent, RobotStatsResult};
use git_executor::git_command_executor::GitCommandExecutor;
use moka::sync::Cache;
use patronus::client::{PATRONUS_URL, PatronusClient};
use patronus::service::PatronusService;
use patronus::teamcity_cli::TeamCityCli;
use patronus::token::find_user_token;
use remote_facts::robot_stats::{RobotStatsSource, robot_stats_core};
use std::sync::Arc;
use std::time::Duration;
use sync_core::patronus::{SpaceRepository, find_space_repository, find_target_branch, required_target_branch};
use sync_core::robot_commits::{RobotSubject, robot_failure_report, robot_subject_commit_map};
use sync_utils::timestamp::unix_now;
use tauri::State;
use tauri::ipc::Channel;
use tracing::instrument;

/// The Patronus client of the app: one HTTP client (kept-alive connection), the token and backoff state, and the
/// Space repository of each repository, so a poll starts no git process
pub struct PatronusState {
  pub(crate) service: Arc<PatronusService>,
  /// The Space repository (None = not on Space) by repository path. Re-read now and then: the remote can change.
  repositories: Cache<String, Option<SpaceRepository>>,
  /// The `teamcity` CLI with an authenticated server, once found; looked for again on each full failure until then (the
  /// user may log in meanwhile)
  teamcity: tokio::sync::Mutex<Option<TeamCityCli>>,
  /// By repository path: a read of the statistics that asks Patronus waits for the one asking, then reads what it kept
  stats_reads: Cache<String, Arc<tokio::sync::Mutex<()>>>,
}

impl PatronusState {
  pub fn new(app_version: &str) -> Self {
    let user_agent = app_user_agent(app_version);
    // the variable of the Safe Push CLI, for the staging and dev instances
    let base_url = std::env::var("SAFEPUSH_PATRONUS_URL").map_or_else(|_| PATRONUS_URL.to_string(), |url| url.trim_end_matches('/').to_string());
    Self {
      service: Arc::new(PatronusService::new(PatronusClient::new(base_url, &user_agent), find_user_token)),
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

  /// The CLI is looked for again on the next full failure: it was installed, updated or signed in to other servers
  pub async fn reset_teamcity(&self) {
    *self.teamcity.lock().await = None;
  }

  /// The Space repository `origin` pushes to: one `remote get-url --push` (`find_space_repository`) on a miss, for the
  /// actions and the reads of the facts (`AppServices`). The GitHub repository of its branches (a mirror of the Space
  /// one) is the app's, kept while the config stays as it is (`RepositoryStateCache::github_repository`)
  pub(crate) async fn space_repository(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<SpaceRepository>, ApiError> {
    if let Some(space) = self.repositories.get(repository_path) {
      return Ok(space);
    }
    let git = git_executor.clone();
    let path = repository_path.to_string();
    let space = run_blocking(move || find_space_repository(&git, &path)).await?;
    self.repositories.insert(repository_path.to_string(), space.clone());
    Ok(space)
  }
}

/// The branch a robot pushes to, from the baseline: the branch of `origin` it tracks
async fn target_branch(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str) -> Result<String, ApiError> {
  let git = git_executor.clone();
  let path = repository_path.to_string();
  let baseline = baseline_branch.to_string();
  run_blocking(move || required_target_branch(&git, &path, &baseline)).await
}

/// The Safe Push statistics of the repository (`spec/repository/safe-push-statistics.spec.md`): everyone's robots into
/// the target of the period, kept in the object cache, the progress and the numbers of the lists through `events`
/// before the runs are read
#[tauri::command]
#[instrument(skip(git_executor, repository_state, patronus, settings, prefixes, events))]
#[specta::specta]
pub async fn get_robot_stats(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  patronus: State<'_, PatronusState>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: GetRobotStatsParams,
  events: Channel<RobotStatsEvent>,
) -> Result<RobotStatsResult, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let Some(repository) = patronus.space_repository(&git_executor, &params.repository_path).await? else {
      return Ok(RobotStatsResult {
        availability: PatronusAvailability::NotPatronus,
        stale: false,
        fetched_at: None,
        stats: None,
      });
    };
    let target = target_branch(&git_executor, &params.repository_path, &params.baseline_branch).await?;
    let branch_prefix = inputs.resolve(&params.repository_path).await?;
    let reads = patronus.stats_reads.get_with(params.repository_path.clone(), || Arc::new(tokio::sync::Mutex::new(())));
    // the first open runs `rev-parse` and opens LMDB: off the async workers
    let cache = {
      let (git, repository_state, path) = ((*git_executor).clone(), (*repository_state).clone(), params.repository_path.clone());
      run_blocking(move || anyhow::Ok(repository_state.object_cache(&git, &path))).await?
    };
    let source = RobotStatsSource {
      service: &patronus.service,
      cache: &cache,
      repository: &repository.name,
      target: &target,
      branch_prefix: &branch_prefix,
      reads: Some(&reads),
    };
    let calendar = Calendar::from_zone(jiff::tz::TimeZone::system());
    let send = |event: RobotStatsEvent| {
      if let Err(e) = events.send(event) {
        tracing::debug!(error = %e, "the statistics window is gone");
      }
    };
    Ok(robot_stats_core(&source, &calendar, unix_now(), params.period, params.refresh, &send).await)
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
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: GetRobotFailureParams,
) -> Result<RobotFailureReport, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let branch_prefix = inputs.resolve(&params.repository_path).await?;
    let failure = if params.full {
      let teamcity = patronus.teamcity().await;
      patronus.service.robot_failure_full(&branch_prefix, &params.robot_id, teamcity.as_ref()).await
    } else {
      patronus.service.robot_failure(&branch_prefix, &params.robot_id).await
    }
    .map_err(patronus_error)?;
    let git = git_executor.inner().clone();
    run_blocking(move || robot_failure_report(&git, &params.repository_path, &branch_prefix, &params.ancestors, &params.baseline_branch, failure)).await
  })
  .await
}

/// Which commits of the branches a running robot covers, by the hashes of the checked-out branch: the commit rows mark
/// the commits added or changed since it started (`robot_subject_commit_map`). No commits for a stack without members.
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = ?params.branch_name, stack_root = ?params.stack_root, ancestors = params.ancestors.len()))]
#[specta::specta]
pub async fn get_robot_commit_map(
  git_executor: State<'_, GitCommandExecutor>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: GetRobotCommitMapParams,
) -> Result<RobotCommitMap, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let subject = RobotSubject {
      branch_name: params.branch_name.as_deref(),
      stack_root: params.stack_root.as_deref(),
    };
    robot_subject_commit_map(
      &inputs.git,
      &params.repository_path,
      &branch_prefix,
      subject,
      &params.source_revision,
      &params.baseline_branch,
      &params.ancestors,
    )
    .map(Option::unwrap_or_default)
  })
  .await
}

/// Starts a Safe Push (`/patronus merge`) or a Dry Run (`/patronus dry-run`) of a pull request of someone else the user
/// reviewed: the comment whose merge deletes its branch, as a merge on GitHub does. The start of the user's own branch
/// is `execute` of a `Land`, whose engine comments and reads the reply itself. The posted comment; the bot answers with
/// a reaction (`read_patronus_command_reply`)
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github))]
#[specta::specta]
pub async fn post_patronus_command(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  params: PostPatronusCommandParams,
) -> Result<PatronusCommandComment, ApiError> {
  operation(async move {
    let repository = github_repository(&git_executor, &repository_state, &params.repository_path, params.github_repository.as_deref())
      .await?
      .ok_or_else(unknown_github_repository)?;
    remote_facts::patronus_command::post_patronus_command(
      &github.service,
      &params.repository_path,
      &repository,
      params.pull_request_number,
      params.mode,
      PatronusCommandSubject::ReviewedPullRequest,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))
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
) -> Result<PatronusCommandReply, ApiError> {
  operation(async move {
    let repository = github_repository(&git_executor, &repository_state, &params.repository_path, params.github_repository.as_deref())
      .await?
      .ok_or_else(unknown_github_repository)?;
    remote_facts::patronus_command::read_patronus_command_reply(&github.service, &params.repository_path, &repository, params.pull_request_number, &params.comment_id)
      .await
      .map_err(|e| ApiError::internal(e.to_string()))
  })
  .await
}

fn unknown_github_repository() -> ApiError {
  ApiError::internal(remote_facts::patronus_command::UNKNOWN_GITHUB_REPOSITORY_MESSAGE)
}

/// The branch Patronus pushes into for the baseline (`master` for `origin/master`); None when the baseline is no branch
/// of origin
#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn get_patronus_target_branch(git_executor: State<'_, GitCommandExecutor>, params: GetPatronusTargetBranchParams) -> Result<Option<String>, ApiError> {
  let git = (*git_executor).clone();
  run_blocking(move || find_target_branch(&git, &params.repository_path, &params.baseline_branch)).await
}
