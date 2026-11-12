//! `brd mcp` on a WebSocket of the e2e server (`GET /repositories/{id}/mcp`): the connection of `mcp-host`, one JSON-RPC
//! message per text frame, served in an `AppContext` of the test repository (its fakes, its settings, the sessions of
//! the server) with the events and the desktop of the connection, as `brd mcp` serves it on its stdio. The fake host of
//! the E2E tests of the page in Air (a Playwright page that plays the host page and air-ide) talks to it; the tests of
//! the app link run the corpus of `docs/design/ide-embedding/protocol/corpus/branch-deck-app.ndjson` on the same
//! context (`mcp_corpus_test.rs`). The end of the socket ends the connection and forgets its windows.

use crate::state::{AppState, TestHosts};
use app_api::context::AppContext;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use mcp_host::server::Connection;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::Instrument;

/// The context of the test repository `id` for `connection`: the app of the repository, with the events and the
/// desktop of the connection; none for a repository the server lacks
pub fn context_of_connection(state: &Arc<AppState>, id: &str, connection: &Connection) -> Option<Arc<AppContext>> {
  let hosts = {
    let repository = state.repositories.get(id)?;
    TestHosts {
      settings: Arc::clone(&repository.store),
      events: Arc::clone(&repository.events),
      desktop: Arc::clone(&repository.desktop),
      updater: Arc::clone(&repository.updater),
      cli_tools: Arc::clone(&repository.cli_tools),
    }
  };
  let mut ctx = state.context_of(id, &hosts);
  ctx.events = connection.events();
  ctx.desktop = connection.desktop();
  Some(Arc::new(ctx))
}

/// `GET /repositories/{id}/mcp`: the WebSocket of a connection of `brd mcp` for the test repository `id`
pub async fn mcp_socket(Path(id): Path<String>, State(state): State<Arc<AppState>>, upgrade: WebSocketUpgrade) -> Response {
  if !state.repositories.contains_key(&id) {
    return StatusCode::NOT_FOUND.into_response();
  }
  upgrade.on_upgrade(move |socket| serve_socket(state, id, socket))
}

async fn serve_socket(state: Arc<AppState>, id: String, mut socket: WebSocket) {
  let (connection, mut out) = Connection::new();
  let Some(ctx) = context_of_connection(&state, &id, &connection) else {
    return;
  };
  let (lines, incoming) = mpsc::unbounded_channel();
  let serving = tokio::spawn(connection.serve(ctx, incoming).in_current_span());
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
