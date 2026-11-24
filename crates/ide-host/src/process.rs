//! The process of the bridge, `branch-deck ide` (`brd mcp`, `branch-deck cli mcp`): one connection with air-ide on the
//! stdin and the stdout (`serve`), served by the one backend of the user, the desktop app, which the bridge joins or
//! starts without a window (`start.rs`), or by a backend of its own process (`fallback.rs`). Its data is the app's
//! (`org.develar.branch-deck`), so the desktop app and the IDE show the same sessions. The stdout carries only the
//! protocol, the log goes to the stderr.
//!
//! The exit code: 0 when the stdin ended (the IDE closed the project) and then the backend's connection, 1 when the
//! backend ended it first or the fallback found a backend of its build to join: the IDE starts the bridge again.

use crate::fallback::{Fallback, watch_for_own_backend};
use crate::server::{Backend, Connection, Ended};
use crate::start::{Reach, reach};
use backend_host::identity::build_id;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;

/// How long the end of the process waits for the work it leaves (a blocking git of a command)
const SHUTDOWN: Duration = Duration::from_secs(2);

/// How long the end of the bridge waits for its last messages to reach the IDE
const FLUSH: Duration = Duration::from_millis(500);

/// The exit code of an end the IDE asked for
pub const EXIT_ENDED: i32 = 0;
/// The exit code of an unexpected end, after which the IDE starts the bridge again
pub const EXIT_AGAIN: i32 = 1;

/// Serves air-ide on the stdin and the stdout until either the stdin or the backend ends: the exit code. The caller set
/// up the log on the stderr (`brd mcp` of bd-cli: its warnings, more with -v)
pub fn run() -> i32 {
  let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
    Ok(runtime) => runtime,
    Err(error) => {
      tracing::error!(%error, "the bridge cannot start its runtime");
      return EXIT_AGAIN;
    }
  };
  let app = std::env::current_exe().ok().and_then(|executable| crate::start::app_of(&executable));
  let code = runtime.block_on(serve(tokio::io::stdin(), tokio::io::stdout(), app_data_dir(), app));
  runtime.shutdown_timeout(SHUTDOWN);
  code
}

/// Serves air-ide on `input` and `output`, one JSON-RPC message per line, with the backend of the app at `app` on the
/// app data directory `app_data_dir` (none: the bridge knows no app, and serves alone), until `input` or the backend
/// ends: the exit code
pub async fn serve(input: impl AsyncRead + Unpin + Send + 'static, output: impl AsyncWrite + Unpin + Send + 'static, app_data_dir: Option<PathBuf>, app: Option<PathBuf>) -> i32 {
  let (connection, mut out) = Connection::new();
  let writer = tokio::spawn(
    async move {
      let mut output = output;
      while let Some(message) = out.recv().await {
        let written = async {
          output.write_all(message.as_bytes()).await?;
          output.write_all(b"\n").await?;
          output.flush().await
        };
        if written.await.is_err() {
          break;
        }
      }
    }
    .in_current_span(),
  );
  let (lines, incoming) = mpsc::unbounded_channel();
  let reader = tokio::spawn(
    async move {
      let mut input = BufReader::new(input).lines();
      while let Ok(Some(line)) = input.next_line().await {
        if lines.send(line).is_err() {
          break;
        }
      }
    }
    .in_current_span(),
  );
  let build = build_id();
  // the backend of the process, when the bridge serves alone: it lives while the bridge serves
  let fallback: Arc<Mutex<Option<Fallback>>> = Arc::default();
  let (watch, watched) = oneshot::channel::<PathBuf>();
  let backend = {
    let (fallback, build) = (Arc::clone(&fallback), build.clone());
    async move {
      let data_dir = app_data_dir.clone().unwrap_or_default();
      match reach(&data_dir, app.as_deref(), &build).await {
        Reach::Joined(link, source) => {
          tracing::info!("the bridge joined the backend of the app");
          Some(Backend { link, source })
        }
        Reach::Fallback { why, dir } => {
          tracing::info!(?why, "the bridge serves the IDE with a backend of its own");
          let own = match Fallback::open(app_data_dir) {
            Ok(own) => own,
            Err(error) => {
              tracing::error!(%error, "the bridge cannot open the settings of the app");
              return None;
            }
          };
          let (link, source) = own.connect().await;
          *fallback.lock().unwrap_or_else(PoisonError::into_inner) = Some(own);
          if let Some(dir) = dir.filter(|_| why.may_come_back()) {
            let _ = watch.send(dir);
          }
          Some(Backend { link, source })
        }
      }
    }
  };
  let (opened, mut failed) = oneshot::channel::<()>();
  let backend = async move {
    if let Some(backend) = backend.await {
      return backend;
    }
    let _ = opened.send(());
    std::future::pending().await
  };
  let way_back = {
    let build = build.clone();
    async move {
      match watched.await {
        Ok(dir) => watch_for_own_backend(&dir, &build).await,
        Err(_) => std::future::pending().await,
      }
    }
  };
  let code = tokio::select! {
    ended = connection.serve(backend, incoming) => match ended {
      Ended::Input => EXIT_ENDED,
      Ended::Backend => {
        tracing::warn!("the backend ended the connection of the bridge");
        EXIT_AGAIN
      }
    },
    () = way_back => EXIT_AGAIN,
    // the sender goes once the backend is ready: only its message ends the bridge
    Ok(()) = &mut failed => EXIT_AGAIN,
  };
  let own = fallback.lock().unwrap_or_else(PoisonError::into_inner).take();
  if let Some(own) = own {
    // the IDE closed the project: the settings changed last reach the file first
    own.flush();
  }
  reader.abort();
  // what the bridge wrote last reaches the IDE
  if tokio::time::timeout(FLUSH, writer).await.is_err() {
    tracing::debug!("a task of the bridge still holds its link to the IDE");
  }
  code
}

/// The app data directory of the app whose binary runs the bridge (`app_settings::identity`): the one the backend of that
/// app keys its socket with, a check copy's own
pub fn app_data_dir() -> Option<PathBuf> {
  app_settings::identity::app_data_dir()
}

#[cfg(test)]
#[path = "process_test.rs"]
pub(crate) mod process_test;
