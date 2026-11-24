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

/// The backend of the test repository `id` for a bridge: a host on the context of the repository; none for a
/// repository the server lacks
pub fn backend_of(state: &Arc<AppState>, id: &str) -> Option<Fallback> {
  let ctx = Arc::clone(&state.repositories.get(id)?.context);
  Some(Fallback::of_context(ctx))
}

/// Serves the bridge `connection` with `backend` on the lines of `incoming` until they end, the backend kept while it
/// serves
pub async fn serve_bridge(connection: Connection, backend: Fallback, incoming: mpsc::UnboundedReceiver<String>) -> ide_host::server::Ended {
  let (link, source) = backend.connect().await;
  let ended = connection.serve(async move { Backend { link, source } }, incoming).await;
  drop(backend);
  ended
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
