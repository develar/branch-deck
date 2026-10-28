//! The session commands of the app (`src-tauri/src/commands/session.rs`) on the sessions of this server: the same
//! `Sessions` and `GitSessionHost`, on a `TestClock` that stands still, with the prefix resolved as the server's sync
//! family resolves it. `subscribe_view` is a long-lived event stream of its channel (`channel_commands`): its answer
//! `{"result": …}` first, then each patch as `{"progress": <ViewPatch>}` until `unsubscribe_view` or the deletion of the
//! repository (`delete_repository` closes its sessions, so the stream ends and the deletion does not wait for it); a
//! failure answers `{"result": {"status": "error", "error": <ApiError>}}` on the stream, the JSON of the app. The next run
//! of a session takes the overrides a test set for its repository (`sync_overrides`). The clock of the sessions of a
//! repository stands still until its test moves it (`POST /repositories/{id}/clock/advance`, `advance_clock`). The facts of the sessions are read by the scheduler of the
//! facts, as the app reads them, over the fakes of each test repository (`TestFactSources`); a window asks one again with
//! `refresh_facts`.

use crate::channel_commands::{Progress, answer_with_progress, progress_channel};
use crate::remote_host::HeldAnswer;
use crate::state::AppState;
use crate::state::TestRepository;
use crate::sync_overrides::OverridingReporter;
use crate::tauri_command_bridge::{
  CommandResult, Rejection, branch_prefix_now, branch_prefix_of, failed, github_of, held_or_failed, holding, keep_answered, patronus_new_api_of, rejected, remote_of,
  repository_by_path, requesting_repository, review_requests_of, space_of,
};
use app_api::api_error::{api_error, no_branch_prefix, patronus_error};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use domain_types::ProgressReporter;
use domain_types::error::ApiError;
use domain_types::operation::{CancelOperationParams, ExecuteParams, Plan, PrepareParams};
use domain_types::patronus::{
  BranchReviewReadiness, KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusRobotDetails, PatronusRobotsResult, PatronusStartOptions, ReviewReadinessBranch,
};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, PullRequestsResult};
use domain_types::session::{RefreshFactsParams, RepositoryView, SetDemandParams, SyncAnswer, SyncReason, Version, ViewPatch, ViewSubscription};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use patronus::service::PatronusService;
use remote_facts::fact_sources::{FactSources, Read};
use remote_facts::service_sources::{ServiceFactSources, ServiceHost};
use repository_model::facts::Needs;
use repository_model::moved_remote::{MovedRemoteAsk, fetch_in_session, skip_in_session};
use repository_model::runtime::{RunSetup, SessionEnvironment};
use repository_model::session::{SessionKey, Subscriber, SubscriberGone, SubscriptionId};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;
use sync_core::github_repository::GitHubRepository;
use sync_core::patronus::SpaceRepository;
use sync_core::sync::{IssueConfigSource, SyncOptions};
use trial_check::landing_gate::landing_needs_of;

/// What the syncs of the server's sessions take: the issue links read by the sync, the prefix of the local config read
/// now, and the settings of the test repository for the focus
pub struct TestSessionEnvironment {
  pub state: Weak<AppState>,
}

impl SessionEnvironment for TestSessionEnvironment {
  /// The options of a sync of the app; a test that holds the sync runs of the repository (`HeldAnswer::Sync`) sees the
  /// session's sync running until the release
  fn run_setup(&self, key: &SessionKey) -> anyhow::Result<RunSetup> {
    let state = self.state.upgrade().ok_or_else(|| anyhow::anyhow!("The test server stopped"))?;
    if let Some(host) = repository_by_path(&state, &key.worktree).ok().and_then(|repository| repository.existing_remote()) {
      tokio::runtime::Handle::current().block_on(host.pass_held(HeldAnswer::Sync));
    }
    Ok(RunSetup {
      options: SyncOptions {
        issue_config: IssueConfigSource::Load,
        ..Default::default()
      },
      prefix: branch_prefix_now(&state, &key.worktree)?,
    })
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  /// The overrides a test set for the next sync of the repository, taken by this run (`sync_overrides`); those of every
  /// sync stay for the next run
  fn reporter(&self, key: &SessionKey, reporter: Arc<dyn ProgressReporter>) -> Arc<dyn ProgressReporter> {
    let overrides = self.state.upgrade().and_then(|state| {
      let repository = repository_by_path(&state, &key.worktree).ok()?;
      let mut set = repository.sync_overrides.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      if set.as_ref().is_some_and(|overrides| overrides.every) {
        set.clone()
      } else {
        set.take()
      }
    });
    match overrides {
      Some(overrides) => Arc::new(OverridingReporter { inner: reporter, overrides }),
      None => reporter,
    }
  }

  /// "Sync on focus" of the settings of the test repository; a repository already deleted syncs nothing
  fn syncs_on_focus(&self, key: &SessionKey) -> bool {
    self
      .state
      .upgrade()
      .and_then(|state| repository_by_path(&state, &key.worktree).ok().map(|repository| repository.store.app().auto_sync_on_focus))
      .unwrap_or(false)
  }

  /// The landing needs by the real core, without the object cache, as every command of the server; the computation holds
  /// the repository as a command does (its deletion waits for it, so no git of it runs in a deleted directory), and a
  /// test that holds the answer (`HeldAnswer::LandingNeeds`) sees the page while they are computed
  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> anyhow::Result<Needs> {
    let state = self.state.upgrade().ok_or_else(|| anyhow::anyhow!("The test server stopped"))?;
    let (in_flight, host) = {
      let repository = repository_by_path(&state, &key.worktree).map_err(|_gone| anyhow::anyhow!("The test repository {} is gone", key.worktree))?;
      (Arc::clone(&repository.in_flight), repository.existing_remote())
    };
    let _command = in_flight.blocking_read();
    // a deletion that came first has removed it and waited for the hold
    repository_by_path(&state, &key.worktree).map_err(|_gone| anyhow::anyhow!("The test repository {} is gone", key.worktree))?;
    if let Some(host) = host {
      tokio::runtime::Handle::current().block_on(host.pass_held(HeldAnswer::LandingNeeds));
    }
    Ok(landing_needs_of(&state.git_executor, &ObjectCache::disabled(), &key.worktree, &key.prefix, landed)?)
  }

  /// A test fails the view of the robots (`HeldAnswer::RepositoryView`) as it fails a read of the bridge
  fn before_patronus_view(&self, key: &SessionKey) -> Result<(), ApiError> {
    let host = self.state.upgrade().and_then(|state| repository_by_path(&state, &key.worktree).ok()?.existing_remote());
    match host {
      Some(host) if host.fails(HeldAnswer::RepositoryView) => Err(ApiError::internal("the fixture fails this read")),
      _ => Ok(()),
    }
  }

  /// Records the `keep` of the repository (`prunes` of `GET /repositories/{id}/review-requests`), then prunes as the app
  fn prune_review_refs(&self, git: &GitCommandExecutor, key: &SessionKey, keep: &[u32]) {
    if let Some(state) = self.state.upgrade()
      && let Ok(repo) = repository_by_path(&state, &key.worktree)
    {
      repo.review_prune_keeps.lock().expect("review prune lock").push(keep.to_vec());
    }
    if let Err(error) = sync_core::review_fetch::prune_review_refs(git, &key.worktree, keep) {
      tracing::warn!(worktree = %key.worktree, "Cannot prune the review refs: {error:#}");
    }
  }
}

/// What the scheduler of the facts reads the fakes of a test repository with: the reads of the app
/// (`ServiceFactSources`) over the services of the repository (`TestServices`), each counted, held or failed as its
/// tests check it, and the robots answered kept for them (`keep_answered`); the review requests come from their fixture.
/// A read the server can't make (the repository went away) is a failure of the service
pub struct TestFactSources {
  state: Weak<AppState>,
  services: ServiceFactSources<TestServices>,
}

impl TestFactSources {
  pub fn new(state: &Arc<AppState>) -> Self {
    let services = TestServices {
      state: Arc::downgrade(state),
      git: state.git_executor.clone(),
    };
    Self {
      state: Arc::downgrade(state),
      services: ServiceFactSources::new(services),
    }
  }

  fn state(&self) -> Result<Arc<AppState>, ApiError> {
    self.state.upgrade().ok_or_else(stopped)
  }

  /// Counts a read of the repository of `key` by `calls`: the tests check the request policy
  fn count(&self, key: &SessionKey, calls: impl FnOnce(&TestRepository) -> &AtomicU32) -> Result<Arc<AppState>, ApiError> {
    let state = self.state()?;
    calls(&*repository_by_path(&state, &key.worktree).map_err(rejected)?).fetch_add(1, Ordering::SeqCst);
    Ok(state)
  }

  /// A list or a poll of the robots, counted by `calls`, its robots kept for the tests that pin a revision the fake
  /// resolved
  async fn robots_read(&self, key: &SessionKey, calls: impl FnOnce(&TestRepository) -> &AtomicU32, read: Read<'_, PatronusRobotsResult>) -> PatronusRobotsResult {
    let state = match self.count(key, calls) {
      Ok(state) => state,
      Err(error) => return PatronusRobotsResult::from(PatronusAvailability::Error { message: error.message }),
    };
    let result = read.await;
    keep_answered(&state, &key.worktree, &result);
    result
  }

  /// The fake of the repository of `key` passes a held `answer` (`/remote-gates`), or fails it as its fixture says;
  /// nothing holds a repository without fakes
  async fn gate(&self, key: &SessionKey, answer: HeldAnswer, operation: &str) -> Result<(), ApiError> {
    let host = repository_by_path(&*self.state()?, &key.worktree).map_err(rejected)?.existing_remote();
    match host {
      Some(host) => held_or_failed(&host, answer, operation).await.map_err(rejected),
      None => Ok(()),
    }
  }
}

impl FactSources for TestFactSources {
  fn robots<'a>(&'a self, key: &'a SessionKey, since: Option<u32>, background: bool) -> Read<'a, PatronusRobotsResult> {
    Box::pin(self.robots_read(key, |repository| &repository.patronus_list_calls, self.services.robots(key, since, background)))
  }

  fn poll<'a>(&'a self, key: &'a SessionKey, known: Vec<KnownRobotStatus>) -> Read<'a, PatronusRobotsResult> {
    Box::pin(self.robots_read(key, |repository| &repository.patronus_poll_calls, self.services.poll(key, known)))
  }

  fn failing<'a>(&'a self, key: &'a SessionKey, ids: Vec<String>) -> Read<'a, Result<Vec<String>, ApiError>> {
    self.services.failing(key, ids)
  }

  fn problems<'a>(&'a self, key: &'a SessionKey, id: String) -> Read<'a, Result<Vec<PatronusProblem>, ApiError>> {
    self.services.problems(key, id)
  }

  fn robot_details<'a>(&'a self, key: &'a SessionKey, id: String) -> Read<'a, Result<PatronusRobotDetails, ApiError>> {
    Box::pin(async move {
      self.count(key, |repository| &repository.patronus_detail_calls)?;
      self.services.robot_details(key, id).await
    })
  }

  fn review_requests<'a>(&'a self, key: &'a SessionKey, _github_repository: Option<String>, reviews: bool) -> Read<'a, domain_types::review_request::ReviewRequestsResult> {
    Box::pin(async move {
      let failure = |message: String| domain_types::review_request::ReviewRequestsResult::unavailable(PullRequestAvailability::Error { message });
      let Some(state) = self.state.upgrade() else {
        return failure(stopped().message);
      };
      review_requests_of(&state, key, reviews).await.unwrap_or_else(|error| failure(error.message))
    })
  }

  /// Counted; the listing holds the repository as a command does: its deletion waits for it (the fake reads the heads
  /// from its files)
  fn pull_requests<'a>(&'a self, key: &'a SessionKey) -> Read<'a, PullRequestsResult> {
    Box::pin(async move {
      let held = match self.count(key, |repository| &repository.pull_request_calls) {
        Ok(state) => holding(&state, key).await,
        Err(error) => Err(error),
      };
      match held {
        Ok(_command) => self.services.pull_requests(key).await,
        Err(error) => PullRequestsResult::unavailable(PullRequestAvailability::Error { message: error.message }),
      }
    })
  }

  fn branch_rules<'a>(&'a self, key: &'a SessionKey, branch: String) -> Read<'a, Result<BranchRules, ApiError>> {
    Box::pin(async move {
      let _command = holding(&*self.state()?, key).await?;
      self.gate(key, HeldAnswer::BranchRules, "read the rules of the branch").await?;
      self.services.branch_rules(key, branch).await
    })
  }

  fn profiles<'a>(&'a self, key: &'a SessionKey, baseline: String) -> Read<'a, Result<PatronusStartOptions, ApiError>> {
    Box::pin(async move {
      let _command = holding(&*self.state()?, key).await?;
      self.gate(key, HeldAnswer::Profiles, "read the Safe Push options").await?;
      self.services.profiles(key, baseline).await
    })
  }

  /// Without the object cache, so every call asks: the fake Patronus answers the pre-check of a branch whose tip is the
  /// tip of a branch of `POST /repositories/{id}/review-answers`, else by the owners of its paths
  /// (`remote_fixtures::review_check_answer`). Each call is logged with its subjects and its `recheck`
  /// (`GET /repositories/{id}/review-answers`)
  fn readiness<'a>(&'a self, key: &'a SessionKey, baseline: String, asks: Vec<ReviewReadinessBranch>, recheck: bool) -> Read<'a, Result<Vec<BranchReviewReadiness>, ApiError>> {
    Box::pin(async move {
      let state = self.state()?;
      let _command = holding(&state, key).await?;
      {
        let repo = repository_by_path(&state, &key.worktree).map_err(rejected)?;
        let branches: Vec<&str> = asks.iter().map(|ask| ask.branch_name.as_str()).collect();
        let mut calls = repo.review_readiness_calls.lock().expect("review readiness calls lock");
        calls.push(serde_json::json!({"branches": branches, "recheck": recheck}));
      }
      self.services.readiness(key, baseline, asks, recheck).await
    })
  }

  /// The test server keeps no answer (no object cache): every readiness is asked
  fn cached_readiness<'a>(&'a self, _key: &'a SessionKey, _baseline: String, asks: Vec<ReviewReadinessBranch>) -> Read<'a, Vec<Option<BranchReviewReadiness>>> {
    Box::pin(async move { vec![None; asks.len()] })
  }

  fn fetch_moved_remotes<'a>(&'a self, key: &'a SessionKey, asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    self.services.fetch_moved_remotes(key, asks)
  }

  /// Held and failed as `get_auth_status` of the bridge is
  fn auth<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<domain_types::sign_in::AuthStatus, ApiError>> {
    Box::pin(async move {
      self.gate(key, HeldAnswer::AuthStatus, "read the sign-ins").await?;
      self.services.auth(key).await
    })
  }
}

/// The services of each test repository (`ServiceHost`): its fake Patronus and fake GitHub (`RemoteHost`), the Space
/// repository its Patronus fixture names, the GitHub repository resolved with the repository's own global config, the
/// new API of its settings, no object cache; a failed review is Patronus' error
struct TestServices {
  state: Weak<AppState>,
  git: GitCommandExecutor,
}

/// A rewrite of a test repository answers a core's failure as the app's commands do
impl operations::rewrite::RewriteHost for TestServices {
  fn rewrite_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  fn undo_error(&self, error: anyhow::Error) -> ApiError {
    app_api::api_error::undo_error(error)
  }

  /// Counts the fetch of the repository (`fetchCalls` of `GET /repositories/{id}/review-requests`) and fails it with the
  /// `fetchError` of its fixture; an empty head of a request is the branch it names (else `HEAD`), an empty base the
  /// baseline (else an earlier commit of `HEAD`), so the review has a range on the template
  fn review_fetch(&self, mut params: domain_types::review_request::FetchReviewHeadParams) -> Result<domain_types::review_request::FetchReviewHeadParams, ApiError> {
    let state = self.state()?;
    let repo = repository_by_path(&state, &params.repository_path).map_err(rejected)?;
    repo.review_fetch_calls.fetch_add(1, Ordering::SeqCst);
    let fetch_error = repo.review_requests.read().unwrap_or_else(std::sync::PoisonError::into_inner).fetch_error.clone();
    if let Some(message) = fetch_error {
      return Err(ApiError::internal(message));
    }
    let git = &self.git;
    let path = params.repository_path.clone();
    let commit = |rev: &str| git.execute_command(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{rev}^{{commit}}")], &path);
    if params.head_ref_oid.is_empty() {
      params.head_ref_oid = commit(&format!("refs/heads/{}", params.head_ref_name)).or_else(|_| commit("HEAD")).map_err(api_error)?;
    }
    if params.base_ref_oid.is_empty() {
      let baseline = history_rewrite::baseline::detect_baseline_branch(git, &path).and_then(|baseline| commit(&baseline));
      params.base_ref_oid = match baseline {
        Ok(baseline) if baseline != params.head_ref_oid => baseline,
        // a template with fewer commits has a shorter range
        _ => ["HEAD~2", "HEAD~1", "HEAD"].iter().find_map(|rev| commit(rev).ok()).unwrap_or_default(),
      };
    }
    Ok(params)
  }
}

/// The services a start whose answer was lost of the registry of the server is settled with: the fakes of its test
/// repository, as the app's are its own (`RegistryFiles::settled_with`)
pub fn settle_host(state: &Weak<AppState>, git: GitCommandExecutor) -> Arc<dyn ServiceHost> {
  Arc::new(TestServices { state: Weak::clone(state), git })
}

impl TestServices {
  fn state(&self) -> Result<Arc<AppState>, ApiError> {
    self.state.upgrade().ok_or_else(stopped)
  }
}

impl ServiceHost for TestServices {
  fn git(&self) -> GitCommandExecutor {
    self.git.clone()
  }

  /// The links of a test repository live in its own settings (`TestRepository::store`), as the app's in its app data
  fn linked_pull_requests(&self, key: &SessionKey, repository: &GitHubRepository) -> Vec<domain_types::review_request::LinkedPullRequest> {
    let Ok(state) = self.state() else {
      return Vec::new();
    };
    repository_by_path(&state, &key.worktree).map_or_default(|repo| app_settings::linked_pull_requests::linked(&repo.store, &repository.full_name()))
  }

  fn link_pull_request(&self, key: &SessionKey, repository: &GitHubRepository, link: domain_types::review_request::LinkedPullRequest) -> Result<(), ApiError> {
    let state = self.state()?;
    let repo = repository_by_path(&state, &key.worktree).map_err(rejected)?;
    app_settings::linked_pull_requests::link(&repo.store, &repository.full_name(), link).map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(())
  }

  fn forget_linked_pull_requests(&self, key: &SessionKey, repository: &GitHubRepository, numbers: &[u32]) {
    if let Ok(state) = self.state()
      && let Ok(repo) = repository_by_path(&state, &key.worktree)
    {
      let _ = app_settings::linked_pull_requests::forget(&repo.store, &repository.full_name(), numbers);
    }
  }

  /// The config as the config commands of the test read and write it: the global config of the repository its own file
  fn config_git(&self, key: &SessionKey) -> GitCommandExecutor {
    let state = self.state.upgrade();
    state
      .and_then(|state| crate::tauri_command_bridge::config_git(&state, &key.worktree).ok())
      .unwrap_or_else(|| self.git.clone())
  }

  fn patronus(&self, key: &SessionKey) -> Result<Arc<PatronusService>, ApiError> {
    Ok(remote_of(&*self.state()?, &key.worktree).map_err(rejected)?.patronus_service())
  }

  fn github(&self, key: &SessionKey) -> Result<Arc<github::service::GitHubService>, ApiError> {
    Ok(remote_of(&*self.state()?, &key.worktree).map_err(rejected)?.github_service())
  }

  fn youtrack(&self, key: &SessionKey) -> Result<Arc<youtrack::service::YouTrackService>, ApiError> {
    Ok(remote_of(&*self.state()?, &key.worktree).map_err(rejected)?.shared_youtrack_service())
  }

  /// The sign-ins of the test repository: its fakes are its accounts
  fn sign_ins(&self, key: &SessionKey) -> Result<remote_facts::sign_in::SignIns, ApiError> {
    Ok(remote_of(&*self.state()?, &key.worktree).map_err(rejected)?.sign_ins().clone())
  }

  fn space_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<SpaceRepository>, ApiError>> {
    Box::pin(async move { Ok(space_of(&*self.state()?, &key.worktree).map_err(rejected)?.map(|(_, space)| space)) })
  }

  fn github_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<GitHubRepository>, ApiError>> {
    Box::pin(async move {
      let state = self.state()?;
      github_of(&state, &key.worktree, None).await.map(|(_, repository)| repository).map_err(rejected)
    })
  }

  fn new_api(&self, key: &SessionKey) -> bool {
    self.state().is_ok_and(|state| patronus_new_api_of(&state, &key.worktree).unwrap_or(false))
  }

  fn object_cache<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, ObjectCache> {
    Box::pin(async { ObjectCache::disabled() })
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    patronus_error(format!("{error:#}"))
  }

  /// Only a repository whose remote a test moved behind the app's back (`commitOnRemote`): the others set the remote
  /// statuses of their branches (`sync-overrides`), and their pull requests show heads that no fetch may bring in
  fn fetch_moved_remotes<'a>(&'a self, key: &'a SessionKey, asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    Box::pin(async move {
      let Some(state) = self.state.upgrade() else {
        return;
      };
      let moved = repository_by_path(&state, &key.worktree)
        .ok()
        .and_then(|repository| repository.existing_remote())
        .is_some_and(|host| host.remote_moved_behind());
      if moved {
        fetch_in_session(&state.sessions, &state.git_executor, key, asks).await;
      } else {
        skip_in_session(&state.sessions, key, asks).await;
      }
    })
  }
}

fn stopped() -> ApiError {
  ApiError::internal("The test server stopped")
}

/// Adds a pull request to Reviews by its link, as the app's `link_pull_request`: the same core over the fake GitHub
pub async fn link_pull_request(
  State(state): State<Arc<AppState>>,
  Json(params): Json<domain_types::review_request::LinkPullRequestParams>,
) -> CommandResult<domain_types::review_request::LinkedPullRequestAnswer> {
  let key = session_key(&state, &params.repository_path).await?;
  let (entry, kept) = remote_facts::reviews::link_pull_request(&test_services(&state), &state.sessions, &key, &params.input, sync_utils::timestamp::unix_now()).await?;
  Ok(Json(domain_types::review_request::LinkedPullRequestAnswer { entry, kept }))
}

/// Removes a link of Reviews, as the app's `unlink_pull_request`
pub async fn unlink_pull_request(State(state): State<Arc<AppState>>, Json(params): Json<domain_types::review_request::UnlinkPullRequestParams>) -> CommandResult<()> {
  let key = session_key(&state, &params.repository_path).await?;
  Ok(Json(
    remote_facts::reviews::unlink_pull_request(&test_services(&state), &state.sessions, &key, params.number).await?,
  ))
}

/// The services of the test repositories of `state`, as the app's
fn test_services(state: &Arc<AppState>) -> TestServices {
  TestServices {
    state: Arc::downgrade(state),
    git: state.git_executor.clone(),
  }
}

/// The patches of a session to the event stream of `subscribe_view`
struct StreamSubscriber(Progress);

impl Subscriber for StreamSubscriber {
  fn send(&self, patch: &ViewPatch) -> Result<(), SubscriberGone> {
    if self.0.try_send(patch) { Ok(()) } else { Err(SubscriberGone) }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRequest {
  repository_path: String,
}

/// The session key of the test repository at `repository_path`, as the app's `session_key`: refused without a prefix
pub(crate) async fn session_key(state: &AppState, repository_path: &str) -> Result<SessionKey, Rejection> {
  let prefix = branch_prefix_of(state, repository_path).await?;
  if prefix.is_empty() {
    return Err(failed("open the session", no_branch_prefix()));
  }
  Ok(SessionKey {
    worktree: repository_path.to_owned(),
    prefix,
  })
}

/// The group a typed message or branch name puts a commit into, with the grouping of the session's table (`preview_group`
/// of the app)
pub async fn preview_group(
  State(state): State<Arc<AppState>>,
  Json(params): Json<domain_types::commit::PreviewGroupParams>,
) -> CommandResult<Option<domain_types::commit::CommitGroup>> {
  let key = session_key(&state, &params.repository_path).await?;
  Ok(Json(state.sessions.preview_group(&key, &params.message).await))
}

/// The check of a typed branch name (`validate_branch_name` of the app)
pub async fn validate_branch_name(Json(params): Json<domain_types::branch_names::ValidateBranchNameParams>) -> Json<domain_types::branch_names::BranchNameCheck> {
  Json(domain_types::branch_names::check_branch_name(&params.name, params.rules))
}

/// The blocks of a commit body (`commit_body_blocks` of the app)
pub async fn commit_body_blocks(Json(params): Json<domain_types::commit::CommitBodyParams>) -> Json<Vec<domain_types::commit_body::CommitBodyBlock>> {
  Json(sync_utils::commit_body::body_blocks(sync_utils::commit_body::message_body(&params.message)))
}

pub async fn get_snapshot(State(state): State<Arc<AppState>>, Json(request): Json<SessionRequest>) -> CommandResult<Option<RepositoryView>> {
  let key = session_key(&state, &request.repository_path).await?;
  Ok(Json(state.sessions.get_snapshot(&key).await))
}

/// The plan of an intent over the facts of the session, as the app's `prepare`: the same function of the engine
pub async fn prepare(State(state): State<Arc<AppState>>, Json(params): Json<PrepareParams>) -> CommandResult<Option<Plan>> {
  let key = session_key(&state, &params.repository_path).await?;
  Ok(Json(operations::engine::prepare(&state.sessions, &key, params.intent).await?))
}

/// The events of an operation to the event stream of the call that runs it
struct StreamEvents(Progress);

impl operations::events::EventSink for StreamEvents {
  fn send(&self, event: domain_types::operation::OperationEvent) {
    self.0.send(&event);
  }
}

/// Runs an intent on the plan the page showed, as the app's `execute`: the same engine (`operations::engine::execute`)
/// over the services of the test repository (its fake Patronus and fake GitHub, `TestServices`); its steps, the
/// progress of its push and its outcome go to the event stream of its channel as `{"progress": <OperationEvent>}`, the
/// answer last
pub async fn execute(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(params): Json<ExecuteParams>) -> Response {
  answer_with_progress(progress_channel(&headers), move |progress| async move {
    let key = session_key(&state, &params.repository_path).await?;
    let services = TestServices {
      state: Arc::downgrade(&state),
      git: state.git_executor.clone(),
    };
    let events: Arc<dyn operations::events::EventSink> = Arc::new(StreamEvents(progress));
    let accepted = operations::land_execute::Accepted {
      fingerprint: params.fingerprint,
      confirmations: params.accepted,
    };
    let hosts = operations::engine::Hosts {
      services: &services,
      rewrites: &services,
    };
    let outcome = Box::pin(operations::engine::execute(&state.registry, &key, params.intent, accepted, hosts, events)).await;
    Ok(Json(outcome.map_err(|error| failed("execute", error))?))
  })
  .await
  .into_response()
}

/// Stops an operation of the server, as the app's `cancel_operation`
pub async fn cancel_operation(State(state): State<Arc<AppState>>, Json(params): Json<CancelOperationParams>) -> Json<bool> {
  Json(state.registry.cancel(&params.op))
}

/// The stream of a window's subscription: its answer, then the patches. A call without a channel (a test of this crate
/// that calls the route directly) gets the answer, and the subscription ends with its first patch
pub async fn subscribe_view(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(request): Json<SessionRequest>) -> Response {
  answer_with_progress(progress_channel(&headers), move |progress| async move {
    let key = session_key(&state, &request.repository_path).await?;
    // Activity reads the journal of the repository, which the settings of the page remember, as the app's do; before
    // the subscription, whose patches follow its answer
    if let Some(journal) = state.registry.journal_of(&key).await
      && let Ok(repository) = requesting_repository(&state, &headers)
      && let Err(error) = app_settings::journals::remember_journal(&repository.store, &request.repository_path, &journal.to_string_lossy())
    {
      tracing::warn!(%error, "cannot remember the journal of the repository");
    }
    // through the registry, as the app subscribes: the window watches the operations registry of the repository
    let (id, view) = state
      .registry
      .subscribe(&key, Box::new(StreamSubscriber(progress)))
      .await
      .ok_or_else(|| failed("subscribe to the view", ApiError::internal("The session closed while the window subscribed")))?;
    Ok(Json(ViewSubscription { id: id.0, view }))
  })
  .await
  .into_response()
}

#[derive(Deserialize)]
pub struct UnsubscribeRequest {
  id: u32,
}

pub async fn unsubscribe_view(State(state): State<Arc<AppState>>, Json(request): Json<UnsubscribeRequest>) -> Json<()> {
  state.sessions.unsubscribe(SubscriptionId(request.id));
  Json(())
}

pub async fn set_demand(State(state): State<Arc<AppState>>, Json(params): Json<SetDemandParams>) -> Json<()> {
  state.sessions.set_demand(SubscriptionId(params.subscription), &params.demand);
  Json(())
}

#[derive(Deserialize)]
pub struct ClockAdvanceRequest {
  ms: u64,
}

/// The app quits (`POST /repositories/{id}/sessions/close`): the sessions of the repository go with the subscriptions of
/// their windows, so the window that opens it next (a reload) opens a new session, whose facts are asked anew as at a
/// start of the app
pub async fn close_sessions(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<StatusCode, StatusCode> {
  let path = state.repositories.get(&id).map(|repository| repository.path.clone()).ok_or(StatusCode::NOT_FOUND)?;
  state.sessions.close_worktree(&path);
  Ok(StatusCode::OK)
}

/// Moves the clock of the sessions of the repository `id` (`AppState::session_clocks`) by `ms`: the timers of its
/// sessions that decide what to ask and when wake (`advanceBackendClock` of wait-helpers.ts), and the sessions idle for
/// `IDLE_LIMIT` since go (`Sessions::sweep`). The sessions of the other repositories, the time of the page
/// (`page.clock`) and the 16 ms merge of the patches do not move with it
pub async fn advance_clock(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(request): Json<ClockAdvanceRequest>) -> Result<Json<()>, StatusCode> {
  let path = state.repositories.get(&id).map(|repository| repository.path.clone()).ok_or(StatusCode::NOT_FOUND)?;
  state.session_clocks.of_worktree(&path).advance(Duration::from_millis(request.ms));
  state.sessions.sweep();
  Ok(Json(()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowFocusRequest {
  repository_path: String,
  focused: bool,
}

pub async fn set_window_focus(State(state): State<Arc<AppState>>, Json(request): Json<WindowFocusRequest>) -> CommandResult<Option<SyncAnswer>> {
  let key = session_key(&state, &request.repository_path).await?;
  if request.focused {
    // as the app: a gained focus reads the operations registry again, where no watch would tell
    state.registry.reread(&key).await;
  }
  match state.sessions.set_window_focus(&key, request.focused) {
    Some(ticket) => ticket.wait().await.map(|answer| Json(Some(answer))).map_err(|error| failed("sync on focus", error)),
    None => Ok(Json(None)),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
  repository_path: String,
  reason: SyncReason,
}

pub async fn sync(State(state): State<Arc<AppState>>, Json(request): Json<SyncRequest>) -> CommandResult<SyncAnswer> {
  let key = session_key(&state, &request.repository_path).await?;
  state.sessions.sync(&key, request.reason).wait().await.map(Json).map_err(|error| failed("sync", error))
}

/// Asks the session of the repository to read a fact again, as the app's `refresh_facts`: the version after the patch
/// of that read, null when nothing was asked
pub async fn refresh_facts(State(state): State<Arc<AppState>>, Json(params): Json<RefreshFactsParams>) -> CommandResult<Option<Version>> {
  let key = session_key(&state, &params.repository_path).await?;
  Ok(Json(state.sessions.refresh(&key, params.family, params.reason, params.subject).await))
}
