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
use crate::sync_overrides::OverridingReporter;
use crate::tauri_command_bridge::{
  CommandResult, Rejection, branch_prefix_now, branch_prefix_of, branch_rules_of, failed, failing_robots, list_pull_requests_of, list_robots, poll_robots, profiles_of,
  readiness_of, repository_by_path, review_requests_of, robot_details_of, robot_problems,
};
use app_api::api_error::{api_error, no_branch_prefix};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use domain_types::ProgressReporter;
use domain_types::error::ApiError;
use domain_types::landing::{DecideLandParams, LandAnswer};
use domain_types::patronus::{
  BranchReviewReadiness, KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusRobotDetails, PatronusRobotsResult, PatronusStartOptions, ReviewReadinessBranch,
};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, PullRequestsResult};
use domain_types::session::{RefreshFactsParams, RepositoryView, SetDemandParams, SyncAnswer, SyncReason, Version, ViewPatch, ViewSubscription};
use object_cache::cache::ObjectCache;
use remote_facts::fact_sources::{FactSources, Read};
use repository_model::facts::Needs;
use repository_model::moved_remote::{MovedRemoteAsk, fetch_in_session, skip_in_session};
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::runtime::{RunSetup, SessionEnvironment};
use repository_model::session::{SessionKey, Subscriber, SubscriberGone, SubscriptionId};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::{Arc, Weak};
use std::time::Duration;
use sync_core::sync::{IssueConfigSource, SyncOptions};
use trial_check::landing_gate::landing_needs_of;

/// What the syncs of the server's sessions take: the issue links read by the sync, the prefix of the local config read
/// now, and the settings of the test repository for the focus
pub struct TestSessionEnvironment {
  pub state: Weak<AppState>,
}

impl SessionEnvironment for TestSessionEnvironment {
  fn run_setup(&self, key: &SessionKey) -> anyhow::Result<RunSetup> {
    let state = self.state.upgrade().ok_or_else(|| anyhow::anyhow!("The test server stopped"))?;
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
}

/// What the scheduler of the facts reads the fakes of a test repository with, as the app reads its services
/// (`list_robots`, `poll_robots`: counted, so the tests check the request policy); a read the server can't make (the
/// repository went away) is a failure of Patronus
pub struct TestFactSources {
  pub state: Weak<AppState>,
}

impl TestFactSources {
  async fn answer(&self, read: impl AsyncFnOnce(&AppState) -> Result<PatronusRobotsResult, Rejection>) -> PatronusRobotsResult {
    let failure = |message: &str| PatronusRobotsResult::from(PatronusAvailability::Error { message: message.to_owned() });
    let Some(state) = self.state.upgrade() else {
      return failure("The test server stopped");
    };
    read(&state).await.unwrap_or_else(|_| failure("The test server cannot read the robots"))
  }
}

impl FactSources for TestFactSources {
  fn robots<'a>(&'a self, key: &'a SessionKey, since: Option<u32>, background: bool) -> Read<'a, PatronusRobotsResult> {
    Box::pin(self.answer(async move |state| list_robots(state, key, since, background).await))
  }

  fn poll<'a>(&'a self, key: &'a SessionKey, known: Vec<KnownRobotStatus>) -> Read<'a, PatronusRobotsResult> {
    Box::pin(self.answer(async move |state| poll_robots(state, key, &known).await))
  }

  fn failing<'a>(&'a self, key: &'a SessionKey, ids: Vec<String>) -> Read<'a, Result<Vec<String>, ApiError>> {
    Box::pin(async move {
      let state = self.state.upgrade().ok_or_else(stopped)?;
      failing_robots(&state, key, &ids).await
    })
  }

  fn problems<'a>(&'a self, key: &'a SessionKey, id: String) -> Read<'a, Result<Vec<PatronusProblem>, ApiError>> {
    Box::pin(async move {
      let state = self.state.upgrade().ok_or_else(stopped)?;
      robot_problems(&state, key, &id).await
    })
  }

  fn robot_details<'a>(&'a self, key: &'a SessionKey, id: String) -> Read<'a, Result<PatronusRobotDetails, ApiError>> {
    Box::pin(async move {
      let state = self.state.upgrade().ok_or_else(stopped)?;
      robot_details_of(&state, key, &id).await
    })
  }

  fn review_requests<'a>(&'a self, key: &'a SessionKey, _github_repository: Option<String>) -> Read<'a, domain_types::review_request::ReviewRequestsResult> {
    Box::pin(async move {
      let failure = |message: String| domain_types::review_request::ReviewRequestsResult::unavailable(PullRequestAvailability::Error { message });
      let Some(state) = self.state.upgrade() else {
        return failure(stopped().message);
      };
      review_requests_of(&state, key).await.unwrap_or_else(|error| failure(error.message))
    })
  }

  fn pull_requests<'a>(&'a self, key: &'a SessionKey) -> Read<'a, PullRequestsResult> {
    Box::pin(async move {
      let Some(state) = self.state.upgrade() else {
        return PullRequestsResult::unavailable(PullRequestAvailability::Error { message: stopped().message });
      };
      list_pull_requests_of(&state, key).await
    })
  }

  fn branch_rules<'a>(&'a self, key: &'a SessionKey, branch: String) -> Read<'a, Result<BranchRules, ApiError>> {
    Box::pin(async move {
      let state = self.state.upgrade().ok_or_else(stopped)?;
      branch_rules_of(&state, key, &branch).await
    })
  }

  fn profiles<'a>(&'a self, key: &'a SessionKey, baseline: String) -> Read<'a, Result<PatronusStartOptions, ApiError>> {
    Box::pin(async move {
      let state = self.state.upgrade().ok_or_else(stopped)?;
      profiles_of(&state, key, baseline).await
    })
  }

  fn readiness<'a>(&'a self, key: &'a SessionKey, baseline: String, asks: Vec<ReviewReadinessBranch>, recheck: bool) -> Read<'a, Result<Vec<BranchReviewReadiness>, ApiError>> {
    Box::pin(async move {
      let state = self.state.upgrade().ok_or_else(stopped)?;
      readiness_of(&state, key, baseline, asks, recheck).await
    })
  }

  /// The test server keeps no answer (no object cache): every readiness is asked
  fn cached_readiness<'a>(&'a self, _key: &'a SessionKey, _baseline: String, asks: Vec<ReviewReadinessBranch>) -> Read<'a, Vec<Option<BranchReviewReadiness>>> {
    Box::pin(async move { vec![None; asks.len()] })
  }

  /// Only a repository whose remote a test moved behind the app's back (`commitOnRemote`): the others set the remote
  /// statuses of their branches (`sync-overrides`), and their pull requests show heads that no fetch may bring in
  fn fetch_moved_remotes<'a>(&'a self, key: &'a SessionKey, asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    Box::pin(async move {
      let Some(state) = self.state.upgrade() else {
        return;
      };
      let moved = repository_by_path(&state, &key.worktree).ok().and_then(|repository| repository.existing_remote()).is_some_and(|host| host.remote_moved_behind());
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

/// The write of the app to a pull request goes into the session of its repository, as the app's commands write it:
/// the version of the view after its patch, none when no session of the repository is open
pub(crate) async fn write_pull_request(state: &AppState, repository_path: &str, write: PullRequestWrite) -> Option<Version> {
  let key = session_key(state, repository_path).await.ok()?;
  state.sessions.write_pull_request(&key, write).await
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
async fn session_key(state: &AppState, repository_path: &str) -> Result<SessionKey, Rejection> {
  let prefix = branch_prefix_of(state, repository_path).await?;
  if prefix.is_empty() {
    return Err(failed("open the session", no_branch_prefix()));
  }
  Ok(SessionKey {
    worktree: repository_path.to_owned(),
    prefix,
  })
}

pub async fn get_snapshot(State(state): State<Arc<AppState>>, Json(request): Json<SessionRequest>) -> CommandResult<Option<RepositoryView>> {
  let key = session_key(&state, &request.repository_path).await?;
  Ok(Json(state.sessions.get_snapshot(&key).await))
}

/// The decision of a start over the facts of the session, as the app's `decide_land`: the same function of the session
pub async fn decide_land(State(state): State<Arc<AppState>>, Json(params): Json<DecideLandParams>) -> CommandResult<Option<LandAnswer>> {
  let key = session_key(&state, &params.repository_path).await?;
  Ok(Json(state.sessions.decide_land(&key, params.land).await))
}

/// The stream of a window's subscription: its answer, then the patches. A call without a channel (a test of this crate
/// that calls the route directly) gets the answer, and the subscription ends with its first patch
pub async fn subscribe_view(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(request): Json<SessionRequest>) -> Response {
  answer_with_progress(progress_channel(&headers), move |progress| async move {
    let key = session_key(&state, &request.repository_path).await?;
    let (id, view) = state
      .sessions
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
