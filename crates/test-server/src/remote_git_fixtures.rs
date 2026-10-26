//! The remote of a test repository: the bare `.git/upstream.git` every template names as `origin`
//! (`test_utils::remote_git`), which the app's pushes and fetches reach as a real remote, and what a test does to it
//! behind the app's back. `GET /repositories/{id}/remote` answers the branches it has and the pushes it took from the
//! app, in order (`pushes`: each the branches it moved, to their commits, null for a deleted one); `POST …/remote/push`
//! (`{branch, revision, earlierBuild}`) pushes a commit of the repository to a branch of it as `git push --force` in a
//! terminal does (the remote-tracking ref moves too), or an earlier build of its commits: the same commits by content
//! under other ids, as the sync rebuilt them since; `POST …/remote/commit` (`{branch, subject, author, timestamp, files}`) writes a
//! commit onto a branch of it as someone else's push does (the remote-tracking ref stays); `POST …/remote/fetch`
//! fetches it as `git fetch origin` in a terminal does (the remote-tracking refs move to what it has);
//! `POST …/remote/refusal` (`{message}`) makes it refuse every push with that message, `null` take them again. A push
//! held at the remote is the held answer `push` of `remote-gates` (`RemoteHost::hold_answers`).
//!
//! The remote answers a push in its pre-receive hook, written on the first hold or refusal: the hook posts the refs of
//! the push to `FakeRemote`, the fake remote of the host on a loopback port of its own, which waits at the gate while a
//! test holds the push and refuses it with the message of a refusal. The remote has the pack by then and no ref moved,
//! so a held push shows the progress git reported, and a refused one moved nothing.

use crate::remote_fixtures::{FixtureResult, host_of, unknown_repository};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use sync_utils::span::in_current_span;
use test_utils::fake_server::{FakeGate, FakeServer};
use test_utils::git_test_utils::pre_receive_hook;
use test_utils::remote_git::{RemoteCommit, bare_origin, commit_on_remote, earlier_build, fetch_in_terminal, push_in_terminal, remote_branches, remote_pushes};

/// What the fake remote answers the pre-receive hook: the gate of a held push, the message of a refusal
#[derive(Default)]
struct RemoteAnswers {
  /// The gate every push waits at while a test holds `push`; an opened one lets them through
  gate: Mutex<Option<Arc<FakeGate>>>,
  /// Why the remote refuses every push; None: it takes them
  refusal: Mutex<Option<String>>,
}

/// The fake remote of a test repository, which its pre-receive hook asks, started with the host of the repository
pub struct FakeRemote {
  answers: Arc<RemoteAnswers>,
  server: FakeServer,
  repository_path: std::path::PathBuf,
  /// The hook is written: once, on the first hold or refusal
  hook: Mutex<bool>,
}

impl FakeRemote {
  pub fn start(repository_path: &str) -> Self {
    let answers = Arc::new(RemoteAnswers::default());
    let router = axum::Router::new().route("/pre-receive", axum::routing::post(pre_receive)).with_state(Arc::clone(&answers));
    Self {
      answers,
      server: FakeServer::serve(router),
      repository_path: repository_path.into(),
      hook: Mutex::new(false),
    }
  }

  /// Holds every push from now on at a new gate, which the caller opens
  ///
  /// # Errors
  /// When the repository has no remote of its own (a template that names none)
  pub fn hold(&self) -> anyhow::Result<Arc<FakeGate>> {
    self.write_hook()?;
    let gate = Arc::new(FakeGate::default());
    *lock(&self.answers.gate) = Some(Arc::clone(&gate));
    Ok(gate)
  }

  /// Refuses every push from now on with `message`, or takes them again
  ///
  /// # Errors
  /// When the repository has no remote of its own
  pub fn refuse(&self, message: Option<String>) -> anyhow::Result<()> {
    self.write_hook()?;
    *lock(&self.answers.refusal) = message;
    Ok(())
  }

  /// The pre-receive hook of the remote, which posts the refs of a push (its stdin) to the fake and prints the message
  /// of a refusal, which git shows the pusher as `remote: <message>`
  fn write_hook(&self) -> anyhow::Result<()> {
    let mut written = lock(&self.hook);
    if !*written {
      let bare = bare_origin(&self.repository_path);
      anyhow::ensure!(bare.is_dir(), "the repository has no remote of its own: its template names none");
      pre_receive_hook(&bare, &format!("exec curl --silent --fail-with-body --data-binary @- '{}/pre-receive'", self.server.url));
      *written = true;
    }
    Ok(())
  }
}

/// The answer of the pre-receive hook: after the gate while the push is held, then 403 with the message of a refusal
async fn pre_receive(State(answers): State<Arc<RemoteAnswers>>) -> (StatusCode, String) {
  let gate = lock(&answers.gate).clone();
  if let Some(gate) = gate {
    gate.pass().await;
  }
  let refusal = lock(&answers.refusal).clone();
  match refusal {
    Some(message) => (StatusCode::FORBIDDEN, format!("{message}\n")),
    None => (StatusCode::OK, String::new()),
  }
}

/// The value of `mutex`, also after a panic of another holder: a fixture keeps no invariant across its lock
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
  mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The path of the repository `id`
fn path_of(state: &AppState, id: &str) -> Result<std::path::PathBuf, (StatusCode, String)> {
  Ok(state.repositories.get(id).ok_or_else(unknown_repository)?.path.clone().into())
}

/// Runs the git of a fixture off the runtime; its failure is a conflict of the fixture with the repository
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Result<T, (StatusCode, String)> {
  tokio::task::spawn_blocking(in_current_span(work))
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::CONFLICT, format!("{e:#}")))
}

/// The branches of the remote, by name without `refs/heads/`, and the pushes it took from the app, in order
pub async fn get_remote(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
  let path = path_of(&state, &id)?;
  let (branches, pushes): (BTreeMap<String, String>, _) = blocking(move || Ok((remote_branches(&path)?, remote_pushes(&path)?))).await?;
  Ok(Json(serde_json::json!({ "branches": branches, "pushes": pushes })))
}

/// `POST /repositories/{id}/remote/fetch`: fetches the remote as `git fetch origin` in a terminal does
pub async fn fetch_in_terminal_route(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> FixtureResult {
  let path = path_of(&state, &id)?;
  blocking(move || fetch_in_terminal(&path)).await?;
  Ok(StatusCode::OK)
}

/// `POST /repositories/{id}/remote/push`: the branch of the remote and the commit of the repository pushed to it, or with
/// `earlierBuild` an earlier build of its commits after `origin/master` (`test_utils::remote_git::earlier_build`)
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushInTerminalRequest {
  branch: String,
  revision: String,
  #[serde(default)]
  earlier_build: bool,
}

/// The baseline of the templates, which an earlier build of a virtual branch is built on
const TEMPLATE_BASELINE: &str = "origin/master";

/// Pushes a commit of the repository to a branch of the remote as `git push --force` in a terminal does; the commit
pub async fn push_in_terminal_route(
  Path(id): Path<String>,
  State(state): State<Arc<AppState>>,
  Json(request): Json<PushInTerminalRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
  let path = path_of(&state, &id)?;
  let commit = blocking(move || {
    let revision = if request.earlier_build {
      earlier_build(&path, TEMPLATE_BASELINE, &request.revision)?
    } else {
      request.revision
    };
    push_in_terminal(&path, &request.branch, &revision)
  })
  .await?;
  Ok(Json(serde_json::json!({ "commitId": commit })))
}

/// `POST /repositories/{id}/remote/commit`: a commit someone else pushed onto `branch`
#[derive(Deserialize)]
pub struct RemoteCommitRequest {
  branch: String,
  subject: String,
  /// `Name <email>`
  author: String,
  timestamp: i64,
  files: Vec<(String, String)>,
}

/// Writes a commit onto a branch of the remote as someone else's push does; the commit
pub async fn commit_on_remote_route(
  Path(id): Path<String>,
  State(state): State<Arc<AppState>>,
  Json(request): Json<RemoteCommitRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
  let (host, path) = host_of(&state, &id)?;
  let path = std::path::PathBuf::from(path);
  let commit = RemoteCommit {
    subject: request.subject,
    author: request.author,
    timestamp: request.timestamp,
    files: request.files,
  };
  let id = blocking(move || commit_on_remote(&path, &request.branch, &commit)).await?;
  host.note_remote_moved_behind();
  Ok(Json(serde_json::json!({ "commitId": id })))
}

/// `POST /repositories/{id}/remote/refusal`: why the remote refuses every push, `null`: it takes them
#[derive(Deserialize)]
pub struct RemoteRefusalRequest {
  message: Option<String>,
}

/// Makes the remote refuse every push with the message, or take them again
pub async fn set_remote_refusal(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(request): Json<RemoteRefusalRequest>) -> FixtureResult {
  let (host, _) = host_of(&state, &id)?;
  host.git_remote().refuse(request.message).map_err(|e| (StatusCode::CONFLICT, format!("{e:#}")))?;
  Ok(StatusCode::OK)
}
