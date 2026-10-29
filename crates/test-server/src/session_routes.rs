//! The sessions of the app's commands (`app_api::session`) on this server: the same `Sessions` and `GitSessionHost`, on
//! a `TestClock` that stands still, with the prefix resolved as the server's sync family resolves it
//! (`TestSessionEnvironment`); the commands themselves are the app's, routed by `invoke.rs`. `subscribe_view` is a
//! long-lived event stream of its channel: its answer `{"result": …}` first, then each patch as `{"progress":
//! <ViewPatch>}` until `unsubscribe_view` or the deletion of the repository (`delete_repository` closes its windows and
//! its sessions, so the stream ends and the deletion does not wait for it). The next run of a session takes the
//! overrides a test set for its repository (`sync_overrides`). The clock of the sessions of a repository stands still
//! until its test moves it (`POST /repositories/{id}/clock/advance`, `advance_clock`). A test that starts warm builds the
//! frame of its repository's session (`POST /repositories/{id}/warm-start/build`, kept in `TestRepository::frame`) and
//! starts the app on it (`GET /repositories/{id}/warm-start`, `warm_start`). The facts of the sessions are read
//! by the scheduler of the facts, as the app reads them, over the fakes of each test repository (`TestFactSources`),
//! and the effects of the engine go to the same fakes (`TestServices`).

use crate::host::held_or_failed;
use crate::remote_fixtures::{context_of, session_of, unknown_repository};
use crate::remote_host::HeldAnswer;
use crate::remote_host::RemoteHost;
use crate::review_fixtures::review_requests_of;
use crate::state::AppState;
use crate::state::TestRepository;
use crate::sync_overrides::OverridingReporter;
use app_api::api_error::{api_error, patronus_error};
use app_api::operation::run_blocking;
use app_settings::projects::synced_before;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use domain_types::ProgressReporter;
use domain_types::error::ApiError;
use domain_types::patronus::{
  BranchReviewReadiness, KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusRobotDetails, PatronusRobotsResult, PatronusStartOptions, ReviewReadinessBranch,
};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, PullRequestsResult};
use domain_types::session::{CachedModel, SyncReason};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use patronus::service::PatronusService;
use remote_facts::fact_sources::{FactSources, Read};
use remote_facts::service_sources::{ServiceFactSources, ServiceHost};
use repository_model::facts::Needs;
use repository_model::moved_remote::{MovedRemoteAsk, fetch_in_session, skip_in_session};
use repository_model::runtime::{RunSetup, SessionEnvironment};
use repository_model::session::SessionKey;
use repository_model::warm_start::{CachedSession, frame_of};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;
use sync_core::github_repository::{GitHubRepository, resolve_github_repository};
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
  /// session's sync running until the release. A run under a prefix the config left holds nothing: it builds nothing
  /// and answers `prefix_changed` at once, as the app's does, so the window subscribes to the session of the new prefix,
  /// whose sync is the one held
  fn run_setup(&self, key: &SessionKey) -> anyhow::Result<RunSetup> {
    let state = self.state.upgrade().ok_or_else(|| anyhow::anyhow!("The test server stopped"))?;
    let prefix = state.branch_prefix_now(&key.worktree)?;
    if prefix == key.prefix
      && let Some(host) = state.repository(&key.worktree).ok().and_then(|repository| repository.existing_remote())
    {
      tokio::runtime::Handle::current().block_on(host.pass_held(HeldAnswer::Sync));
    }
    Ok(RunSetup {
      options: SyncOptions {
        issue_config: IssueConfigSource::Load,
        ..Default::default()
      },
      prefix,
    })
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  /// The overrides a test set for the next sync of the repository, taken by this run (`sync_overrides`); those of every
  /// sync stay for the next run
  fn reporter(&self, key: &SessionKey, reporter: Arc<dyn ProgressReporter>) -> Arc<dyn ProgressReporter> {
    let overrides = self.state.upgrade().and_then(|state| {
      let repository = state.repository(&key.worktree).ok()?;
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
      .and_then(|state| state.repository(&key.worktree).ok().map(|repository| repository.store.app().auto_sync_on_focus))
      .unwrap_or(false)
  }

  /// The landing needs by the real core, without the object cache, as every command of the server; the computation holds
  /// the repository as a command does (its deletion waits for it, so no git of it runs in a deleted directory), and a
  /// test that holds the answer (`HeldAnswer::LandingNeeds`) sees the page while they are computed
  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> anyhow::Result<Needs> {
    let state = self.state.upgrade().ok_or_else(|| anyhow::anyhow!("The test server stopped"))?;
    let (in_flight, host) = {
      let repository = state
        .repository(&key.worktree)
        .map_err(|_gone| anyhow::anyhow!("The test repository {} is gone", key.worktree))?;
      (Arc::clone(&repository.in_flight), repository.existing_remote())
    };
    let _command = in_flight.blocking_read();
    // a deletion that came first has removed it and waited for the hold
    state
      .repository(&key.worktree)
      .map_err(|_gone| anyhow::anyhow!("The test repository {} is gone", key.worktree))?;
    if let Some(host) = host {
      tokio::runtime::Handle::current().block_on(host.pass_held(HeldAnswer::LandingNeeds));
    }
    Ok(landing_needs_of(&state.git_executor, &ObjectCache::disabled(), &key.worktree, &key.prefix, landed)?)
  }

  /// A test fails the view of the robots (`HeldAnswer::RepositoryView`) as it fails a read of the bridge
  fn before_patronus_view(&self, key: &SessionKey) -> Result<(), ApiError> {
    let host = self.state.upgrade().and_then(|state| state.repository(&key.worktree).ok()?.existing_remote());
    match host {
      Some(host) if host.fails(HeldAnswer::RepositoryView) => Err(ApiError::internal("the fixture fails this read")),
      _ => Ok(()),
    }
  }

  /// The frame the repository keeps in memory (`TestRepository::frame`); none until a test built one
  fn frame(&self, key: &SessionKey) -> Option<CachedSession> {
    let state = self.state.upgrade()?;
    let repository = state.repository(&key.worktree).ok()?;
    repository.frame.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
  }

  /// In the memory of the repository once a test built a frame of it: the sessions of the other repositories keep none
  fn save_frame(&self, frame: &CachedSession) {
    let Some(state) = self.state.upgrade() else {
      return;
    };
    if let Ok(repository) = state.repository(&frame.key.worktree) {
      let mut kept = repository.frame.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      if kept.is_some() {
        *kept = Some(frame.clone());
      }
    }
  }

  /// The `lastSyncTime` of the repository in the store of its test (`syncedAgoMs` of the setup), as the app's writer
  /// keeps it
  fn synced_before(&self, key: &SessionKey) -> bool {
    self
      .state
      .upgrade()
      .and_then(|state| state.repository(&key.worktree).ok().map(|repository| synced_before(&repository.store, &key.worktree)))
      .unwrap_or(false)
  }

  /// Records the `keep` of the repository (`prunes` of `GET /repositories/{id}/review-requests`), then prunes as the app
  fn prune_review_refs(&self, git: &GitCommandExecutor, key: &SessionKey, keep: &[u32]) {
    if let Some(state) = self.state.upgrade()
      && let Ok(repo) = state.repository(&key.worktree)
    {
      repo.review_prune_keeps.lock().expect("review prune lock").push(keep.to_vec());
    }
    if let Err(error) = sync_core::review_fetch::prune_review_refs(git, &key.worktree, keep) {
      tracing::warn!(worktree = %key.worktree, "Cannot prune the review refs: {error:#}");
    }
  }
}

/// The repository of a session, held as a command holds it while a read of its facts runs: its deletion waits
async fn holding(state: &AppState, key: &SessionKey) -> Result<tokio::sync::OwnedRwLockReadGuard<()>, ApiError> {
  let in_flight = Arc::clone(&state.repository(&key.worktree)?.in_flight);
  Ok(in_flight.read_owned().await)
}

/// The fake services of the repository at `path` (`remote_host.rs`), started now if not yet; the repository is not held
fn remote_of(state: &AppState, path: &str) -> Result<Arc<RemoteHost>, ApiError> {
  Ok(state.repository(path)?.remote(&state.git_executor))
}

/// The Space repository of the repository at `path`, None while no Patronus fixture put it on Space
fn space_of(state: &AppState, path: &str) -> Result<Option<SpaceRepository>, ApiError> {
  Ok(state.repository(path)?.existing_remote().and_then(|host| host.space_repository()))
}

/// Keeps the robots of an answer by id, for the tests that pin a revision the fake resolved
fn keep_answered(state: &AppState, path: &str, result: &PatronusRobotsResult) {
  if let Ok(repository) = state.repository(path) {
    let mut answered = repository.patronus_answered.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    answered.extend(result.robots.iter().map(|robot| (robot.id.clone(), robot.clone())));
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
    calls(&*state.repository(&key.worktree)?).fetch_add(1, Ordering::SeqCst);
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
    let host = self.state()?.repository(&key.worktree)?.existing_remote();
    match host {
      Some(host) => held_or_failed(&host, answer, operation).await,
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
        let repo = state.repository(&key.worktree)?;
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

  /// Held and failed as `get_auth_status` of a window without a session is (`TestHost::before_auth_status`)
  fn auth<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<domain_types::sign_in::AuthStatus, ApiError>> {
    Box::pin(async move {
      self.gate(key, HeldAnswer::AuthStatus, "read the sign-ins").await?;
      self.services.auth(key).await
    })
  }
}

/// The services of each test repository (`ServiceHost`): its fake Patronus and fake GitHub (`RemoteHost`), the Space
/// repository its Patronus fixture names, the GitHub repository resolved with the repository's own global config, the
/// new API of its settings, no object cache; a failed review is Patronus' error. The services of the context of every
/// test repository (`AppState::context_of`), of the commands of app-api that read them by the key of a session, and of
/// the rewrites of its engine
pub struct TestServices {
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
    let repo = state.repository(&params.repository_path)?;
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

/// The services of the test repositories of `state`, as the app's (`AppContext::services` and `rewrites`)
pub fn test_services(state: &Arc<AppState>) -> Arc<TestServices> {
  Arc::new(TestServices {
    state: Arc::downgrade(state),
    git: state.git_executor.clone(),
  })
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
    state
      .repository(&key.worktree)
      .map_or_default(|repo| app_settings::linked_pull_requests::linked(&repo.store, &repository.full_name()))
  }

  fn link_pull_request(&self, key: &SessionKey, repository: &GitHubRepository, link: domain_types::review_request::LinkedPullRequest) -> Result<(), ApiError> {
    let state = self.state()?;
    let repo = state.repository(&key.worktree)?;
    app_settings::linked_pull_requests::link(&repo.store, &repository.full_name(), link).map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(())
  }

  fn forget_linked_pull_requests(&self, key: &SessionKey, repository: &GitHubRepository, numbers: &[u32]) {
    if let Ok(state) = self.state()
      && let Ok(repo) = state.repository(&key.worktree)
    {
      let _ = app_settings::linked_pull_requests::forget(&repo.store, &repository.full_name(), numbers);
    }
  }

  /// The config as the config commands of the test read and write it: the global config of the repository its own file
  fn config_git(&self, key: &SessionKey) -> GitCommandExecutor {
    let state = self.state.upgrade();
    state
      .and_then(|state| state.repository(&key.worktree).ok().map(|repository| repository.context.git.clone()))
      .unwrap_or_else(|| self.git.clone())
  }

  fn patronus(&self, key: &SessionKey) -> Result<Arc<PatronusService>, ApiError> {
    Ok(remote_of(&*self.state()?, &key.worktree)?.patronus_service())
  }

  fn github(&self, key: &SessionKey) -> Result<Arc<github::service::GitHubService>, ApiError> {
    Ok(remote_of(&*self.state()?, &key.worktree)?.github_service())
  }

  fn youtrack(&self, key: &SessionKey) -> Result<Arc<youtrack::service::YouTrackService>, ApiError> {
    Ok(remote_of(&*self.state()?, &key.worktree)?.shared_youtrack_service())
  }

  /// The sign-ins of the test repository: its fakes are its accounts
  fn sign_ins(&self, key: &SessionKey) -> Result<remote_facts::sign_in::SignIns, ApiError> {
    Ok(remote_of(&*self.state()?, &key.worktree)?.sign_ins().clone())
  }

  fn space_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<SpaceRepository>, ApiError>> {
    Box::pin(async move { space_of(&*self.state()?, &key.worktree) })
  }

  fn github_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<GitHubRepository>, ApiError>> {
    Box::pin(async move {
      // the config of the test as its fixtures write it (`branchdeck.githubRepository`), read each time
      let git = self.state()?.repository(&key.worktree)?.context.git.clone();
      let path = key.worktree.clone();
      run_blocking(move || resolve_github_repository(&git, &path)).await
    })
  }

  fn new_api(&self, key: &SessionKey) -> bool {
    self
      .state()
      .is_ok_and(|state| state.repository(&key.worktree).is_ok_and(|repository| repository.store.app().patronus_new_api))
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
      let moved = state
        .repository(&key.worktree)
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

#[derive(Deserialize)]
pub struct ClockAdvanceRequest {
  ms: u64,
}

/// The app quits (`POST /repositories/{id}/sessions/close`): its windows close (`AppContext::forget_windows`) and the
/// sessions of the repository go, so the window that opens it next (a reload) opens a new session, whose facts are
/// asked anew as at a start of the app; answers once each saved its last frame, which the next start reads
/// (`warm_start`)
pub async fn close_sessions(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<StatusCode, StatusCode> {
  let (path, context) = state
    .repositories
    .get(&id)
    .map(|repository| (repository.path.clone(), Arc::clone(&repository.context)))
    .ok_or(StatusCode::NOT_FOUND)?;
  context.forget_windows();
  state.sessions.close_worktree_saved(&path).await;
  Ok(StatusCode::OK)
}

/// The frame of the repository's session as an earlier run of the app left it (`POST /repositories/{id}/warm-start/build`):
/// a sync without a page, as `bd sync` syncs (the session reads no fact without a window), whose table the repository
/// keeps from now on (`TestRepository::frame`), as do the saves of its sessions after it. The session closes then, as
/// the app quits, so the next start (`warm_start`) opens one seeded from the frame. Answers the rows of the frame
pub async fn build_warm_start(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
  let path = state.repositories.get(&id).map(|repository| repository.path.clone()).ok_or_else(unknown_repository)?;
  let key = session_of(&state, &path).await?;
  state
    .sessions
    .sync(&key, SyncReason::Manual)
    .wait()
    .await
    .map_err(|error| fixture_failed("sync before the frame", &error))?;
  let of = key.clone();
  let frame = state
    .sessions
    .read(&key, move |table, facts| frame_of(&of, table, facts))
    .await
    .ok_or_else(|| fixture_failed("build the frame", &ApiError::internal("The session closed while the frame was built")))?;
  let rows = frame.table.order.len();
  *state
    .repository(&path)
    .map_err(|_gone| unknown_repository())?
    .frame
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(frame);
  state.sessions.close_worktree(&path);
  Ok(Json(serde_json::json!({ "rows": rows })))
}

/// The start of the app with the frame of the repository (`GET /repositories/{id}/warm-start`), as
/// `src-tauri/src/warm_start.rs` starts it before the main window: the session of the prefix the config names now opened
/// seeded from the frame (its first sync runs at once for a repository synced before) and what the page gets as
/// `window.__BD_MODEL__`, the path and the seed; null without a frame or with a frame of another prefix
pub async fn warm_start(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<Option<CachedModel>>, (StatusCode, String)> {
  let (path, frame) = {
    let repository = state.repositories.get(&id).ok_or_else(unknown_repository)?;
    let frame = repository.frame.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    (repository.path.clone(), frame)
  };
  let Some(frame) = frame else {
    return Ok(Json(None));
  };
  let key = session_key_now(&state, &path).await?;
  let view = state.sessions.open_seeded(&key, frame).await;
  Ok(Json(view.map(|view| CachedModel { repository_path: path, view })))
}

/// The key of the session of the test repository at `path` by its git config read now, as the start of the app and the
/// subscription of its window resolve it (`AppContext::session_key_now`)
async fn session_key_now(state: &AppState, path: &str) -> Result<SessionKey, (StatusCode, String)> {
  context_of(state, path)?
    .session_key_now(path)
    .await
    .map_err(|error| fixture_failed("resolve the key of the session", &error))
}

/// The rejection of a fixture route that could not `operation`: a failure of the test, not an answer of the app
fn fixture_failed(operation: &str, error: &ApiError) -> (StatusCode, String) {
  (StatusCode::INTERNAL_SERVER_ERROR, format!("Cannot {operation}: {}", error.message))
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
