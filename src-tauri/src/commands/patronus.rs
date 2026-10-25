use crate::commands::branch_prefix::PrefixInputs;
use crate::commands::github::{GitHubState, app_user_agent};
use crate::commands::pull_requests::github_repository;
use crate::commands::session::SessionsState;
use crate::operation::{operation, run_blocking};
use crate::repository_state::RepositoryStateCache;
use app_api::api_error::{api_error, patronus_error};
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::calendar::Calendar;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{LandingCheck, LandingGateParams};
use domain_types::patronus::{
  BranchReviewReadiness, GetPatronusTargetBranchParams, KnownRobotStatus, PatronusAvailability, PatronusCommandComment, PatronusCommandReply, PatronusCommandSubject,
  PatronusProblem, PatronusRobot, PatronusRobotDetails, PatronusRobotParams, PatronusRobotStatus, PatronusRobotsResult, PatronusStartOptions, PostPatronusCommandParams,
  ReadPatronusCommandReplyParams, ReviewReadinessBranch, StartPatronusRobotParams,
};
use domain_types::robot_failure::{GetRobotCommitMapParams, GetRobotFailureParams, RobotCommitMap, RobotFailureReport};
use domain_types::robot_stats::{GetRobotStatsParams, RobotStatsEvent, RobotStatsResult};
use git_executor::git_command_executor::GitCommandExecutor;
use moka::sync::Cache;
use object_cache::cache::ObjectCache;
use patronus::client::{PATRONUS_URL, PatronusClient};
use patronus::service::PatronusService;
use patronus::teamcity_cli::TeamCityCli;
use patronus::token::find_user_token;
use remote_facts::review_readiness::{ReadinessInputs, branch_review_readiness, kept_review_readiness, prepare_review_readiness};
use remote_facts::robot_stats::{RobotStatsSource, robot_stats_core};
use repository_model::actions::{NOT_ON_SPACE, RemoteActions};
use repository_model::session::SessionKey;
use std::sync::Arc;
use std::time::Duration;
use sync_core::patronus::{SpaceRepository, find_space_repository, find_target_branch, required_target_branch};
use sync_core::robot_commits::{RobotSubject, robot_failure_report, robot_subject_commit_map};
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
  /// By repository path: a read of the statistics that asks Patronus waits for the one asking, then reads what it kept
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

  /// The CLI is looked for again on the next full failure: it was installed, updated or signed in to other servers
  pub async fn reset_teamcity(&self) {
    *self.teamcity.lock().await = None;
  }

  /// The Space repository `origin` pushes to: one `remote get-url --push` (`find_space_repository`) on a miss. The GitHub
  /// repository of its branches (a mirror of the Space one) is the app's, kept while the config stays as it is
  /// (`RepositoryStateCache::github_repository`)
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

  /// The user's robots of the repository at `repository_path`, its branches named under `branch_prefix`, in the Space
  /// repository `origin` pushes to: the last 7 days, or the ones started since `since`, with the GitHub
  /// repository the branches are mirrored to (`AppFactSources`, the reads of the session's scheduler). Off Space, or when
  /// git can't tell the remote, the availability says so. With the new API of Settings → Patronus (`new_api`) the list
  /// names the merge profile of each robot (`mergeProfile`, behind the `robot.list.merge.profile.enabled` flag of
  /// Patronus); off, it leaves the profile unknown
  pub(crate) async fn robots(
    &self,
    git_executor: &GitCommandExecutor,
    repository_state: &RepositoryStateCache,
    (repository_path, branch_prefix): (&str, &str),
    since: Option<u32>,
    background: bool,
    new_api: bool,
  ) -> PatronusRobotsResult {
    let repository = match self.space_repository(git_executor, repository_path).await {
      Ok(Some(repository)) => repository,
      Ok(None) => return PatronusRobotsResult::from(PatronusAvailability::NotPatronus),
      Err(error) => return PatronusRobotsResult::from(PatronusAvailability::Error { message: error.message }),
    };
    let mut result = self.service.robots(&repository.name, branch_prefix, since, background, new_api).await;
    match github_repository(git_executor, repository_state, repository_path, None).await {
      Ok(github) => result.github_repository = github.map(|github| github.full_name()),
      Err(error) => warn!(error = ?error, "the robots name no GitHub repository: it can't be read"),
    }
    result
  }

  /// The robots of `known` whose status changed since the session saw it, the others not in the answer
  pub(crate) async fn poll(&self, branch_prefix: &str, known: &[KnownRobotStatus]) -> PatronusRobotsResult {
    self.service.poll(branch_prefix, known).await
  }

  /// Of the running robots `ids`, the ones a failed required check dooms (Patronus' warning), asked after each answer
  /// until known
  pub(crate) async fn failing(&self, ids: &[String]) -> Result<Vec<String>, ApiError> {
    self.service.failing_robots(ids).await.map_err(patronus_error)
  }

  /// The error problems of a finished robot (the service keeps them: a finished robot doesn't change)
  pub(crate) async fn problems(&self, id: &str) -> Result<Vec<PatronusProblem>, ApiError> {
    self.service.robot_problems(id).await.map_err(patronus_error)
  }

  /// The Space repository, for an action: not on Space is an error
  async fn required_space_repository(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<SpaceRepository, ApiError> {
    self.space_repository(git_executor, repository_path).await?.ok_or_else(not_on_space)
  }

  /// The merge profiles of the target `baseline` pushes to, for the session of the repository (`AppFactSources`): the
  /// default of the last robot is the page's
  pub(crate) async fn profiles(&self, git_executor: &GitCommandExecutor, repository_path: &str, baseline: &str) -> Result<PatronusStartOptions, ApiError> {
    let repository = self.required_space_repository(git_executor, repository_path).await?;
    let target = target_branch(git_executor, repository_path, baseline).await?;
    self
      .service
      .start_options(&repository.project_key, &repository.name, &target, None, None)
      .await
      .map_err(patronus_error)
  }

  /// The code-owner review of `asks` against `baseline`, for the session of `key` (`AppFactSources`): one `git log`
  /// for the commits of all, their paths from the object cache, then one pre-check per ask whose kept answer doesn't
  /// hold (`remote_facts::review_readiness`); `recheck` asks Patronus again, in the foreground. The answers in the
  /// order of `asks`
  pub(crate) async fn readiness(
    &self,
    git: &GitCommandExecutor,
    repository_state: &RepositoryStateCache,
    key: &SessionKey,
    baseline: &str,
    asks: Vec<ReviewReadinessBranch>,
    recheck: bool,
  ) -> Result<Vec<BranchReviewReadiness>, ApiError> {
    let (inputs, cache, asks) = self.readiness_inputs(git, repository_state, key, baseline, asks).await?;
    branch_review_readiness(git, &key.worktree, &cache, &self.service, &inputs.repository(), &asks, recheck)
      .await
      .map_err(api_error)
  }

  /// Of `asks`, the answers the object cache keeps that still hold, without Patronus (`kept_review_readiness`); none when
  /// the repository is not on Space or its inputs can't be read (the pre-check says why)
  pub(crate) async fn kept_readiness(
    &self,
    git: &GitCommandExecutor,
    repository_state: &RepositoryStateCache,
    key: &SessionKey,
    baseline: &str,
    asks: Vec<ReviewReadinessBranch>,
  ) -> Vec<Option<BranchReviewReadiness>> {
    let count = asks.len();
    match self.readiness_inputs(git, repository_state, key, baseline, asks).await {
      Ok((inputs, cache, asks)) => kept_review_readiness(git, &key.worktree, &cache, &self.service, &inputs.repository(), &asks).await,
      Err(_) => vec![None; count],
    }
  }

  /// What the readiness of `asks` is read with: the Space and GitHub repositories, the target of the baseline, the
  /// object cache of the repository
  async fn readiness_inputs(
    &self,
    git: &GitCommandExecutor,
    repository_state: &RepositoryStateCache,
    key: &SessionKey,
    baseline: &str,
    asks: Vec<ReviewReadinessBranch>,
  ) -> Result<(ReadinessInputs, ObjectCache, Vec<ReviewReadinessBranch>), ApiError> {
    let space = self.required_space_repository(git, &key.worktree).await?;
    let github = github_repository(git, repository_state, &key.worktree, None).await?;
    let (git, repository_state, key, baseline) = (git.clone(), repository_state.clone(), key.clone(), baseline.to_owned());
    run_blocking(move || {
      let inputs = prepare_review_readiness(&git, space, github, &key.prefix, &key.worktree, &baseline, &asks)?;
      let cache = repository_state.object_cache(&git, &key.worktree);
      anyhow::Ok((inputs, cache, asks))
    })
    .await
  }
}

fn not_on_space() -> ApiError {
  ApiError::new(ErrorCode::NotOnSpace, NOT_ON_SPACE)
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

/// The robot with its merge profile, CI checks, error problems and whether it will fail, for the chip popover. With the
/// new API of Settings → Patronus the progress of a running robot comes in one request (`GET /robots/progress`, behind
/// the `robot.progress.enabled` flag of Patronus) instead of one per running check; the builds are asked as before
/// while Patronus answers 404
#[tauri::command]
#[instrument(skip(git_executor, patronus, settings, prefixes))]
#[specta::specta]
pub async fn get_patronus_robot_details(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: PatronusRobotParams,
) -> Result<PatronusRobotDetails, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let new_api = inputs.patronus_new_api();
    let branch_prefix = inputs.resolve(&params.repository_path).await?;
    patronus.service.robot_details(&branch_prefix, &params.robot_id, new_api).await.map_err(patronus_error)
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

/// Starts a Safe Push or a Dry Run of the pushed virtual branch, or with `stack` of the pushed stack branch of the whole
/// stack; the frontend pushes it first. The whole sequence is `RemoteActions::start_robot` (`start_guard`), which the test
/// server runs too: the user's robots (one list; a failed list fails the start), the checks of the kept table (parked,
/// landed whole, not built, a robot on its last commit or one that lands a part of it), the landing gate with the
/// commits the view names landed and the target, as `bd safe-push` refuses them; then Patronus. The new robot, which the
/// session of the repository polls from now on (`SessionsState::write_robot`).
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, stack = params.stack, mode = ?params.mode))]
#[specta::specta]
pub async fn start_patronus_robot(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  sessions: State<'_, SessionsState>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: StartPatronusRobotParams,
) -> Result<PatronusRobot, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let branch_prefix = inputs.resolve(&params.repository_path).await?;
    let space = patronus.space_repository(&git_executor, &params.repository_path).await?;
    let actions = remote_actions(&patronus, space.as_ref(), &git_executor, &sessions, &branch_prefix);
    let robot = actions.start_robot(&params).await.map_err(api_error)?;
    sessions.write_robot(&params.repository_path, branch_prefix, &robot).await;
    Ok(robot)
  })
  .await
}

/// What the app's actions on the remote run on (`repository_model::actions`): the one service of the app and the Space
/// repository of the repository (none off Space: a robot action is refused, a push holds nothing), the sessions with the
/// landing gate of the app, and the prefix the command resolved for it (`PrefixInputs`)
pub(crate) fn remote_actions<'a>(
  patronus: &'a PatronusState,
  space: Option<&'a SpaceRepository>,
  git_executor: &'a GitCommandExecutor,
  sessions: &'a SessionsState,
  branch_prefix: &'a str,
) -> RemoteActions<'a> {
  RemoteActions::new(
    space.map(|space| (&patronus.service, space)),
    git_executor,
    sessions.sessions(),
    sessions.landing_gate(),
    branch_prefix,
  )
}

/// What refuses the start of a Safe Push or a Dry Run, as `bd safe-push` refuses it (`trial_check::landing_gate`): the
/// refusals left after `allow`, each with its fix; asked before the push, through the object cache of the repository, with
/// the commits merged Safe Pushes landed as the repository view names them on the robots Rust reads
/// (`RemoteActions::check_landing`; none off Space, or when the Space repository can't be read)
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, stack = params.stack))]
#[specta::specta]
pub async fn check_landing_gate(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  sessions: State<'_, SessionsState>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: LandingGateParams,
) -> Result<LandingCheck, ApiError> {
  operation(async {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let branch_prefix = inputs.resolve(&params.repository_path).await?;
    let space = patronus.space_repository(&git_executor, &params.repository_path).await.unwrap_or_else(|error| {
      warn!(error = ?error, "no robots for the landing check: the Space repository is unknown");
      None
    });
    let actions = remote_actions(&patronus, space.as_ref(), &git_executor, &sessions, &branch_prefix);
    actions.check_landing(&params).await.map_err(api_error)
  })
  .await
}

/// Whether Patronus can rebase the branch a start pushes onto the newest target, as `bd safe-push` checks it: the fetch
/// of the target, then the replay of the branch onto its tip; a failed fetch or replay is in the result, never an error
#[tauri::command]
#[instrument(skip(git_executor, settings, prefixes))]
#[specta::specta]
pub async fn check_target_conflict(
  git_executor: State<'_, GitCommandExecutor>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: TargetConflictParams,
) -> Result<TargetCheck, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    anyhow::Ok(target_conflict_core(&inputs.git, &branch_prefix, &params))
  })
  .await
}

// jscpd:ignore-start: the restart and the cancel are two commands of the page with the same states, one action apart
/// Starts the robot again on the newest commit of its branch, unless the checks of its subject refuse it as they refuse
/// a start (`RemoteActions::restart_robot`: a robot not in the user's list or of a temporary ref is refused); the new robot
#[tauri::command]
#[instrument(skip(git_executor, patronus, sessions, settings, prefixes))]
#[specta::specta]
pub async fn restart_patronus_robot(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  sessions: State<'_, SessionsState>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: PatronusRobotParams,
) -> Result<PatronusRobot, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    act_on_robot(inputs, &patronus, &sessions, &params, RobotAction::Restart).await
  })
  .await
}

/// Asks Patronus to cancel a running robot of the user's list (`RemoteActions::cancel_robot`); it answers it canceling
#[tauri::command]
#[instrument(skip(git_executor, patronus, sessions, settings, prefixes))]
#[specta::specta]
pub async fn cancel_patronus_robot(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  sessions: State<'_, SessionsState>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: PatronusRobotParams,
) -> Result<PatronusRobot, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    act_on_robot(inputs, &patronus, &sessions, &params, RobotAction::Cancel).await
  })
  .await
}

// jscpd:ignore-end

/// What the user does with a robot of the list
enum RobotAction {
  Restart,
  Cancel,
}

/// The restart or the cancel of the robot of `params` on the Space repository of the repository (`start_guard`)
async fn act_on_robot(
  inputs: PrefixInputs,
  patronus: &PatronusState,
  sessions: &SessionsState,
  params: &PatronusRobotParams,
  action: RobotAction,
) -> Result<PatronusRobot, ApiError> {
  let git_executor = inputs.git.clone();
  let branch_prefix = inputs.resolve(&params.repository_path).await?;
  let space = patronus.space_repository(&git_executor, &params.repository_path).await?;
  let actions = remote_actions(patronus, space.as_ref(), &git_executor, sessions, &branch_prefix);
  let robot = match action {
    RobotAction::Restart => actions.restart_robot(params).await,
    RobotAction::Cancel => actions.cancel_robot(params).await,
  }
  .map_err(api_error)?;
  sessions.write_robot(&params.repository_path, branch_prefix, &robot).await;
  Ok(robot)
}

/// The current status of one robot, read right before a push into its branch
#[tauri::command]
#[instrument(skip(patronus))]
#[specta::specta]
pub async fn get_patronus_robot_status(patronus: State<'_, PatronusState>, params: PatronusRobotParams) -> Result<PatronusRobotStatus, ApiError> {
  operation(async move { patronus.service.robot_status(&params.robot_id).await.map_err(patronus_error) }).await
}

/// Starts a Safe Push (`/patronus merge`) or a Dry Run (`/patronus dry-run`) from the GitHub pull request of the
/// branch: Patronus links the robot to the pull request only from its comment. The comment of the user's own branch
/// comes after the checks of `start_patronus_robot` (`RemoteActions::check_start_from_pull_request`), which the test
/// server runs too. The posted comment; the bot answers with a reaction (`read_patronus_command_reply`).
#[tauri::command]
#[instrument(skip(git_executor, repository_state, github, patronus, sessions, settings, prefixes))]
#[specta::specta]
#[expect(clippy::too_many_arguments, reason = "the states of the app a Tauri command takes one by one")]
pub async fn post_patronus_command(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  github: State<'_, GitHubState>,
  patronus: State<'_, PatronusState>,
  sessions: State<'_, SessionsState>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: PostPatronusCommandParams,
) -> Result<PatronusCommandComment, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let repository = github_repository(&git_executor, &repository_state, &params.repository_path, params.github_repository.as_deref())
      .await?
      .ok_or_else(unknown_github_repository)?;
    if params.subject == PatronusCommandSubject::OwnBranch {
      let branch_prefix = inputs.resolve(&params.repository_path).await?;
      let space = patronus.space_repository(&git_executor, &params.repository_path).await?;
      remote_actions(&patronus, space.as_ref(), &git_executor, &sessions, &branch_prefix)
        .check_start_from_pull_request(&params)
        .await
        .map_err(api_error)?;
    }
    remote_facts::patronus_command::post_patronus_command(
      &github.service,
      &params.repository_path,
      &repository,
      params.pull_request_number,
      params.mode,
      params.subject,
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
