//! The session commands of the app (`src-tauri/src/commands/session.rs`) on the sessions of this server: the same
//! `Sessions` and `GitSessionHost`, on a `TestClock` that stands still, with the prefix resolved as the server's sync
//! family resolves it. `subscribe_view` is a long-lived event stream of its channel (`channel_commands`): its answer
//! `{"result": …}` first, then each patch as `{"progress": <ViewPatch>}` until `unsubscribe_view` or the deletion of the
//! repository (`delete_repository` closes its sessions, so the stream ends and the deletion does not wait for it); a
//! failure answers `{"result": {"status": "error", "error": <ApiError>}}` on the stream, the JSON of the app. The next run
//! of a session takes the overrides a test set for its repository (`sync_overrides`). A push the page answered (the
//! templates have no remote) reaches the session through `pushed_in_page`. The clock of the sessions stands still until
//! a test moves it (`POST /clock/advance`, `advance_clock`). The facts of the sessions are read by the scheduler of the
//! facts, as the app reads them, over the fakes of each test repository (`TestFactSources`); a window asks one again with
//! `refresh_facts`.

use crate::channel_commands::{Progress, answer_with_progress, progress_channel};
use crate::remote_host::HeldAnswer;
use crate::state::AppState;
use crate::sync_overrides::OverridingReporter;
use crate::tauri_command_bridge::{
  CommandResult, Rejection, branch_prefix_now, branch_prefix_of, branch_rules_of, failed, failing_robots, list_pull_requests_of, list_robots, poll_robots, profiles_of,
  repository_by_path, robot_problems,
};
use app_api::api_error::{api_error, no_branch_prefix};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use domain_types::error::ApiError;
use domain_types::patronus::{KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusRobotsResult, PatronusStartOptions};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, PullRequestsResult};
use domain_types::session::{RefreshFactsParams, RepositoryView, SetDemandParams, SyncAnswer, SyncReason, Version, ViewPatch, ViewSubscription};
use domain_types::{ProgressReporter, RemoteStatusUpdate};
use object_cache::cache::ObjectCache;
use remote_facts::fact_sources::{FactSources, Read};
use repository_model::facts::Needs;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::runtime::{RunSetup, SessionEnvironment, pushed_heads};
use repository_model::session::{SessionKey, Subscriber, SubscriberGone, SubscriptionId};
use repository_model::table::RemoteSubject;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::{Arc, Weak};
use std::time::Duration;
use sync_core::push::StackPushBranch;
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

/// Moves the clock of the sessions (`AppState::session_clock`) by `ms`: the timers of the sessions that decide what to
/// ask and when wake (`advanceBackendClock` of wait-helpers.ts), and the sessions idle for `IDLE_LIMIT` since go
/// (`Sessions::sweep`). The time of the page (`page.clock`) and the 16 ms merge of the patches do not move with it
pub async fn advance_clock(State(state): State<Arc<AppState>>, Json(request): Json<ClockAdvanceRequest>) -> Json<()> {
  state.session_clock.advance(Duration::from_millis(request.ms));
  state.sessions.sweep();
  Json(())
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushedInPageRequest {
  repository_path: String,
  /// The branches of the push, as the page sent them to `push_branch` or `push_branches`
  branches: Vec<StackPushBranch>,
  /// The remote statuses the page's answer has, in the order of the branches
  remote_statuses: Vec<RemoteStatusUpdate>,
  /// The answer said the remote moved: nothing was pushed
  #[serde(default)]
  moved: bool,
}

/// A push the page answered (`routePush` of push-helpers.ts: the templates name the repository itself as `origin`, so a
/// test of a moved remote or of the progress fakes the remote there): its outcome goes into the table of the session
/// of the repository as the end of the app's push writes it (`TableState::end_push`), and the answer is the version of
/// the view whose patch has it (`None`: no window shows the repository), which the page's answer names as the app's
/// command does. Unlike the app's push (`Sessions::push`) it moved no remote-tracking ref, so the inputs the session
/// knows stay as they are
pub async fn pushed_in_page(State(state): State<Arc<AppState>>, Json(request): Json<PushedInPageRequest>) -> CommandResult<Option<Version>> {
  let key = session_key(&state, &request.repository_path).await?;
  let subjects: Vec<RemoteSubject> = request.branches.iter().map(RemoteSubject::of).collect();
  let heads = pushed_heads(&subjects, &request.remote_statuses);
  let statuses = request.remote_statuses;
  let applied = state.sessions.apply(&key, move |table| {
    table.begin_push(&subjects);
    table.end_push(&subjects, Some(statuses));
  });
  let version = applied.version().await;
  if !request.moved && !heads.is_empty() {
    // the pull requests are listed as after the app's push
    state.sessions.tell_push(&key, heads);
  }
  Ok(Json(version))
}

/// Asks the session of the repository to read a fact again, as the app's `refresh_facts`: the version after the patch
/// of that read, null when nothing was asked
pub async fn refresh_facts(State(state): State<Arc<AppState>>, Json(params): Json<RefreshFactsParams>) -> CommandResult<Option<Version>> {
  let key = session_key(&state, &params.repository_path).await?;
  Ok(Json(state.sessions.refresh(&key, params.family, params.reason).await))
}
