//! The process `brd mcp`: one connection with air-ide on the stdin and the stdout (`serve`), in a context of the
//! services of the app (`app_api::app::services`, the same the desktop app runs with) on its data directories
//! (`org.develar.branch-deck`), so the desktop app and the IDE can be open on the same repository at once. The stdout
//! carries only the protocol, the log goes to the stderr; the end of the stdin ends the process with 0, its settings
//! saved.

use crate::server::Connection;
use app_api::app::services::{AppServices, ContextSource, ProcessParts, RemoteServices, start_sessions};
use app_api::context::AppContext;
use app_api::events::{settings_changed, settings_save_failed};
use app_api::repository_state::RepositoryStateCache;
use app_settings::service::SettingsService;
use git_executor::git_command_executor::GitCommandExecutor;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use tracing::Instrument;

/// The data directory of the app, its bundle identifier: the same as the app's and `brd`'s
const APP_IDENTIFIER: &str = "org.develar.branch-deck";

/// How long the end of the process waits for the work it leaves (a blocking git of a command)
const SHUTDOWN: Duration = Duration::from_secs(2);

/// Serves air-ide on the stdin and the stdout until the stdin ends: the exit code, 0 then. The caller set up the log on
/// the stderr (`brd mcp` of bd-cli: its warnings, more with -v)
pub fn run() -> i32 {
  let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
    Ok(runtime) => runtime,
    Err(error) => {
      tracing::error!(%error, "brd mcp cannot start its runtime");
      return 1;
    }
  };
  let code = runtime.block_on(serve(tokio::io::stdin(), tokio::io::stdout(), app_data_dir()));
  runtime.shutdown_timeout(SHUTDOWN);
  code
}

/// Serves air-ide on `input` and `output`, one JSON-RPC message per line, in the context of the app data directory
/// `app_data_dir`, until `input` ends: the exit code, 0 then
pub async fn serve(input: impl AsyncRead + Unpin + Send + 'static, output: impl AsyncWrite + Unpin + Send + 'static, app_data_dir: Option<PathBuf>) -> i32 {
  let (connection, mut out) = Connection::new();
  let ctx = match context(&connection, app_data_dir) {
    Ok(ctx) => ctx,
    Err(error) => {
      tracing::error!(%error, "brd mcp cannot open the settings of the app");
      return 1;
    }
  };
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
  connection.serve(Arc::clone(&ctx), incoming).await;
  // the end of the input: the IDE closed the project; the settings changed last reach the file first
  if let Err(error) = ctx.settings.flush() {
    tracing::error!(%error, "the settings changed last are lost");
  }
  reader.abort();
  writer.abort();
  0
}

/// The context `brd mcp` built, which its services find once it exists
#[derive(Default)]
struct McpContext(OnceLock<Arc<AppContext>>);

impl ContextSource for McpContext {
  fn context(&self) -> &AppContext {
    self.0.get().expect("the services of brd mcp run once its context exists")
  }
}

/// The app data directory of the app (`~/Library/Application Support/org.develar.branch-deck` on macOS)
pub fn app_data_dir() -> Option<PathBuf> {
  dirs::data_dir().map(|dir| dir.join(APP_IDENTIFIER))
}

/// The context of the process: the services, the sessions and the settings of the app, the events and the desktop of
/// `connection`
fn context(connection: &Connection, app_data_dir: Option<PathBuf>) -> std::io::Result<Arc<AppContext>> {
  let git = GitCommandExecutor::new();
  let repository_state = RepositoryStateCache::new();
  let remote = Arc::new(RemoteServices::new(env!("CARGO_PKG_VERSION"), git.clone(), app_data_dir.clone()));
  let source = Arc::new(McpContext::default());
  let services = AppServices::new(Arc::clone(&source), remote);
  let runtime = tokio::runtime::Handle::current();
  let events = connection.events();
  let (sessions, registry) = start_sessions(&services, &runtime, Arc::clone(&events), &git, &repository_state);
  // the one writer of the settings in this process: a change the desktop app saved reaches the views as its own
  let settings = Arc::new(SettingsService::open(app_data_dir.unwrap_or_default())?);
  {
    let events = Arc::clone(&events);
    settings.on_save_error(move |error| settings_save_failed(&*events, error.to_string()));
  }
  {
    let events = Arc::clone(&events);
    settings.on_external_change(move |change| settings_changed(&*events, change));
  }
  let ctx = Arc::new(services.into_context(ProcessParts {
    git,
    settings,
    repository_state,
    sessions,
    registry,
    events,
    desktop: connection.desktop(),
    runtime,
  }));
  let _ = source.0.set(Arc::clone(&ctx));
  Ok(ctx)
}

#[cfg(test)]
#[path = "process_test.rs"]
mod process_test;
