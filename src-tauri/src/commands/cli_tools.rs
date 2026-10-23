//! Settings → Tools: the `teamcity` and `youtrack` CLIs, their status and the action of each row
//! (spec/repository/cli-tools.spec.md). The cores are `cli_tools`; this is their adapter.

use crate::command_error::CommandError;
use crate::commands::patronus::PatronusState;
use crate::operation::{operation, run_blocking};
use cli_tools::environment::ToolEnvironment;
use cli_tools::install::run_action;
use cli_tools::status::{KnownFacts, read_statuses};
use domain_types::cli_tools::{CliTool, CliToolAction, CliToolStatus};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// The actions running now, by tool: a cancel sets its flag
#[derive(Default)]
pub struct CliToolsState {
  running: Mutex<HashMap<CliTool, Arc<AtomicBool>>>,
}

/// A line of the output of a running action
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct CliToolProgress {
  pub line: String,
}

/// What an action did, in the words of its toast
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct CliToolActionDone {
  pub title: String,
}

/// The status of every tool, each read in its own threads
#[tauri::command]
#[specta::specta]
#[instrument]
pub async fn cli_tools_status() -> Result<Vec<CliToolStatus>, CommandError> {
  run_blocking(|| Ok::<_, CommandError>(read_statuses(&ToolEnvironment::from_process(), &|_| KnownFacts::default()))).await
}

/// Runs `action` of `tool` (install, update, the skill), each line of its output to `progress`; a second action of the
/// same tool while one runs is refused
#[tauri::command]
#[specta::specta]
#[instrument(skip(state, patronus, progress))]
pub async fn run_cli_tool_action(
  state: State<'_, CliToolsState>,
  patronus: State<'_, PatronusState>,
  tool: CliTool,
  action: CliToolAction,
  latest: Option<String>,
  progress: Channel<CliToolProgress>,
) -> Result<CliToolActionDone, CommandError> {
  operation(async {
    let canceled = Arc::new(AtomicBool::new(false));
    {
      let mut running = state.running.lock().expect("the running actions");
      if running.contains_key(&tool) {
        return Err(CommandError::new(format!("{} is busy with another action", tool.command())));
      }
      running.insert(tool, Arc::clone(&canceled));
    }
    let result = run_blocking(move || {
      let on_line = |line: &str| {
        if let Err(e) = progress.send(CliToolProgress { line: line.to_string() }) {
          warn!(error = %e, "failed to report a line of the action");
        }
      };
      run_action(&ToolEnvironment::from_process(), tool, action, latest.as_deref(), &on_line, &|| {
        canceled.load(Ordering::SeqCst)
      })
      .map_err(CommandError::new)
    })
    .await;
    state.running.lock().expect("the running actions").remove(&tool);
    // the robot prompt finds the CLI again: it may be new, or signed in to other servers
    if tool == CliTool::Teamcity {
      patronus.reset_teamcity().await;
    }
    result.map(|done| CliToolActionDone { title: done.title })
  })
  .await
}

/// Ends the action of `tool` that runs, with its whole process group
#[tauri::command]
#[specta::specta]
#[instrument(skip(state))]
#[expect(clippy::needless_pass_by_value, reason = "Tauri hands a command its state by value")]
pub fn cancel_cli_tool_action(state: State<'_, CliToolsState>, tool: CliTool) {
  if let Some(canceled) = state.running.lock().expect("the running actions").get(&tool) {
    canceled.store(true, Ordering::SeqCst);
  }
}
