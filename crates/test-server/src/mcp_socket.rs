//! The bridge of the IDE on a WebSocket of the e2e server (`GET /repositories/{id}/mcp`): the connection of `ide-host`,
//! one JSON-RPC message per text frame, a client of a backend host on the context of the test repository (its fakes,
//! its settings, the sessions of the server) over the in-process transport, as `branch-deck ide` serves its stdio with
//! the backend of the app or of its own. The fake host of the E2E tests of the page in Air (a Playwright page that plays
//! the host page and air-ide) talks to it; the tests of the app link run the corpus of
//! `docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson` on the same backend (`mcp_corpus_test.rs`). The
//! end of the socket ends the connection, which closes its windows.

use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use ide_host::fallback::Fallback;
use ide_host::server::{Backend, Connection};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::Instrument;

/// The backend of the test repository `id` for a bridge: the one host of the repository (`TestRepository::host`), which
/// the page mock goes through too; none for a repository the server lacks
pub fn backend_of(state: &Arc<AppState>, id: &str) -> Option<Fallback> {
  Some(Fallback::of_host(state.repositories.get(id)?.host()))
}

/// Serves the bridge `connection` with `backend` on the lines of `incoming` until they end, the backend kept while it
/// serves
pub async fn serve_bridge(connection: Connection, backend: Fallback, incoming: mpsc::UnboundedReceiver<String>) -> ide_host::server::Ended {
  let (link, source) = backend.connect().await;
  let ended = connection.serve(async move { Backend { link, source } }, incoming).await;
  drop(backend);
  ended
}

/// The build of the backend of `POST /repositories/{id}/stale-backend`, and the build of the bridge it refuses
const STALE_BUILD: &str = "0.0.0-stale";
const NEW_BUILD: &str = "0.0.0-new";

/// `POST /repositories/{id}/stale-backend`: the binary of the app of the test repository was replaced on disk while its
/// window is open, and a bridge of the new build asks its backend: a host on the context of the repository, stale with
/// a window, refuses the bridge (`otherBuild`) and stays, its hook the app's (`BuildReplaced`), so the main window of the
/// page hears it. Answers the refusal
pub async fn stale_backend(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<axum::Json<serde_json::Value>, StatusCode> {
  let ctx = Arc::clone(&state.repositories.get(&id).ok_or(StatusCode::NOT_FOUND)?.context);
  let replaced = {
    let ctx = Arc::clone(&ctx);
    let told = app_api::app::life::BuildReplaced::default();
    Box::new(move || {
      told.tell(&*ctx.events);
    })
  };
  let hooks = backend_host::host::HostHooks {
    stale: Box::new(|| true),
    has_window: Box::new(|| true),
    restart_to_update: replaced,
    ..backend_host::host::HostHooks::default()
  };
  let host = backend_host::host::Host::of_build(ctx, STALE_BUILD.to_owned(), hooks);
  let client = backend_host::in_process::InProcess::client(&host);
  match ide_host::backend::handshake(ide_host::backend::Transport::InProcess(client), NEW_BUILD).await {
    Err(ide_host::backend::Refusal::OtherBuild { build, exits }) => Ok(axum::Json(serde_json::json!({ "build": build, "exits": exits }))),
    _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
  }
}

/// `GET /repositories/{id}/mcp`: the WebSocket of a bridge of the IDE for the test repository `id`
pub async fn mcp_socket(Path(id): Path<String>, State(state): State<Arc<AppState>>, upgrade: WebSocketUpgrade) -> Response {
  if !state.repositories.contains_key(&id) {
    return StatusCode::NOT_FOUND.into_response();
  }
  upgrade.on_upgrade(move |socket| serve_socket(state, id, socket))
}

async fn serve_socket(state: Arc<AppState>, id: String, mut socket: WebSocket) {
  let Some(backend) = backend_of(&state, &id) else {
    return;
  };
  let (connection, mut out) = Connection::new();
  let (lines, incoming) = mpsc::unbounded_channel();
  let serving = tokio::spawn(serve_bridge(connection, backend, incoming).in_current_span());
  loop {
    tokio::select! {
      received = socket.recv() => match received {
        Some(Ok(Message::Text(text))) => {
          if lines.send(text.as_str().to_owned()).is_err() {
            break;
          }
        }
        Some(Ok(Message::Close(_)) | Err(_)) | None => break,
        Some(Ok(_)) => {}
      },
      sent = out.recv() => match sent {
        Some(text) => {
          if socket.send(Message::Text(text.into())).await.is_err() {
            break;
          }
        }
        None => break,
      },
    }
  }
  drop(lines);
  let _ = serving.await;
}
