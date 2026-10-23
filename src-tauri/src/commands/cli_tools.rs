//! Settings → Tools: the `teamcity` and `youtrack` CLIs, their status and the action of each row
//! (spec/repository/cli-tools.spec.md), and the Setup dot of what waits for the user
//! (spec/repository/setup-attention.spec.md). The cores are `cli_tools` and `remote_facts::cli_tool_releases`; this is
//! their adapter.

use crate::auto_update::SharedUpdateState;
use crate::command_error::CommandError;
use crate::commands::github::GitHubState;
use crate::commands::patronus::PatronusState;
use crate::commands::youtrack::YouTrackState;
use crate::operation::{operation, run_blocking};
use cli_tools::environment::ToolEnvironment;
use cli_tools::install::{ActionInput, ActionOutput, run_action};
use cli_tools::kept_statuses::KeptStatuses;
use cli_tools::setup::{SetupFacts, setup_entries};
use cli_tools::status::{KnownFacts, read_statuses};
use domain_types::cli_tools::{CliTool, CliToolAction, CliToolStatus};
use domain_types::setup::{SetupEntry, SetupSkip};
use domain_types::update::UpdateStatus;
use remote_facts::cli_tool_releases::{refresh_releases, releases_file};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use sync_utils::timestamp::unix_now;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, State};
use tracing::{instrument, warn};

/// Every window reads the items of the Setup dot again: the statuses of the tools changed
pub const SETUP_CHANGED_EVENT: &str = "setup-changed";

pub struct CliToolsState {
  /// The actions running now, by tool: a cancel sets its flag
  running: Mutex<HashMap<CliTool, Arc<AtomicBool>>>,
  /// The statuses of the read that started last, which the Setup dot takes without running the CLIs again
  statuses: KeptStatuses,
  /// The newest releases, asked of GitHub at most once a day (no lock: two reads at once ask twice at worst)
  releases_file: Option<PathBuf>,
}

impl CliToolsState {
  pub fn new(app_data_dir: Option<&Path>) -> Self {
    Self {
      running: Mutex::default(),
      statuses: KeptStatuses::default(),
      releases_file: app_data_dir.map(releases_file),
    }
  }

  /// The statuses of every tool with the newest releases and the YouTrack the app is signed in to, kept for the Setup
  /// dot
  async fn read(&self, github: &GitHubState, youtrack: &YouTrackState) -> Result<Vec<CliToolStatus>, CommandError> {
    let ticket = self.statuses.start_read();
    let releases = refresh_releases(&github.service, self.releases_file.as_deref(), unix_now()).await;
    let signed_in = youtrack.service.status();
    let signed_in_youtrack = signed_in.url.filter(|_| signed_in.signed_in_as.is_some() && signed_in.problem.is_none());
    let statuses = run_blocking(move || {
      Ok::<_, CommandError>(read_statuses(&ToolEnvironment::from_process(), &|tool| KnownFacts {
        latest: releases.latest(tool).map(str::to_string),
        signed_in_youtrack: signed_in_youtrack.clone(),
      }))
    })
    .await?;
    self.statuses.keep(ticket, &statuses);
    Ok(statuses)
  }
}

/// A line of the output of a running action, or the page a sign-in opened in the browser
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct CliToolProgress {
  pub line: String,
  /// The authorize page of a sign-in, for "Open in Browser" when the browser didn't come up
  pub url: Option<String>,
}

/// What an action did, in the words of its toast
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct CliToolActionDone {
  pub title: String,
  /// Why the skill an install or an update brings along failed, while the CLI itself is done
  pub skill_problem: Option<String>,
}

/// The status of every tool, each read in its own threads, with the newest releases (GitHub, once a day); every window
/// reads the Setup dot again
#[tauri::command]
#[specta::specta]
#[instrument(skip_all)]
pub async fn cli_tools_status(
  app: AppHandle,
  state: State<'_, CliToolsState>,
  github: State<'_, GitHubState>,
  youtrack: State<'_, YouTrackState>,
) -> Result<Vec<CliToolStatus>, CommandError> {
  operation(async {
    let statuses = state.read(&github, &youtrack).await?;
    if let Err(e) = app.emit(SETUP_CHANGED_EVENT, ()) {
      warn!(error = %e, "can't tell the windows that the setup changed");
    }
    Ok(statuses)
  })
  .await
}

/// What waits for the user (the Setup dot): the downloaded update of the app, the tools' updates and expiring sign-ins
/// from their last statuses (read now when none is known), the app's YouTrack sign-in; without what `skips` hides
#[tauri::command]
#[specta::specta]
#[instrument(skip_all, fields(skips = skips.len()))]
pub async fn get_setup_items(
  state: State<'_, CliToolsState>,
  github: State<'_, GitHubState>,
  update_state: State<'_, SharedUpdateState>,
  youtrack: State<'_, YouTrackState>,
  skips: Vec<SetupSkip>,
) -> Result<Vec<SetupEntry>, CommandError> {
  operation(async {
    let statuses = match state.statuses.get() {
      Some(statuses) => statuses,
      None => state.read(&github, &youtrack).await?,
    };
    let app_update = {
      let update = update_state.lock().map_err(|e| CommandError::new(format!("the update state is poisoned: {e}")))?;
      (update.info.is_update_available && update.info.status == UpdateStatus::Downloaded).then(|| update.info.available_version.clone())
    };
    let youtrack_broken = youtrack.service.status().problem.is_some();
    Ok(setup_entries(&SetupFacts {
      app_update: app_update.as_deref(),
      tools: &statuses,
      youtrack_broken,
      skips: &skips,
      now: unix_now(),
    }))
  })
  .await
}

/// An action of a row of Settings → Tools
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RunCliToolActionParams {
  pub tool: CliTool,
  pub action: CliToolAction,
  /// The release an update wants
  pub latest: Option<String>,
  /// The server a sign-in of `teamcity` is to
  pub server: Option<String>,
  /// An install brings the agent skill along
  pub with_skill: bool,
}

/// Runs the action of a row (install, with the agent skill when `with_skill`; update; the skill; the sign-in of
/// `teamcity` to `server`), each line of its output to `progress`; a second action of the same tool while one runs is
/// refused
#[tauri::command]
#[specta::specta]
#[instrument(skip(state, patronus, progress))]
pub async fn run_cli_tool_action(
  state: State<'_, CliToolsState>,
  patronus: State<'_, PatronusState>,
  params: RunCliToolActionParams,
  progress: Channel<CliToolProgress>,
) -> Result<CliToolActionDone, CommandError> {
  operation(async move {
    let RunCliToolActionParams {
      tool,
      action,
      latest,
      server,
      with_skill,
    } = params;
    let canceled = Arc::new(AtomicBool::new(false));
    {
      let mut running = state.running.lock().expect("the running actions");
      if running.contains_key(&tool) {
        return Err(CommandError::new(format!("{} is busy with another action", tool.command())));
      }
      running.insert(tool, Arc::clone(&canceled));
    }
    let result = run_blocking(move || {
      let send = |progress_event: CliToolProgress| {
        if let Err(e) = progress.send(progress_event) {
          warn!(error = %e, "failed to report how far the action is");
        }
      };
      let on_line = |line: &str| {
        send(CliToolProgress {
          line: line.to_string(),
          url: None,
        });
      };
      let on_url = |url: &str| {
        send(CliToolProgress {
          line: "Waiting for the browser…".to_string(),
          url: Some(url.to_string()),
        });
      };
      let input = ActionInput {
        latest: latest.as_deref(),
        server: server.as_deref(),
        with_skill,
      };
      let output = ActionOutput {
        on_line: &on_line,
        on_url: &on_url,
      };
      run_action(&ToolEnvironment::from_process(), tool, action, &input, &output, &|| canceled.load(Ordering::SeqCst)).map_err(CommandError::new)
    })
    .await;
    state.running.lock().expect("the running actions").remove(&tool);
    // the robot prompt finds the CLI again: it may be new, or signed in to other servers
    if tool == CliTool::Teamcity {
      patronus.reset_teamcity().await;
    }
    result.map(|done| CliToolActionDone {
      title: done.title,
      skill_problem: done.skill_problem,
    })
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
