//! Settings → Tools: the `teamcity` and `youtrack` CLIs, their status and the action of each row
//! (spec/repository/cli-tools.spec.md), and the Setup dot of what waits for the user
//! (spec/repository/setup-attention.spec.md). The cores are `cli_tools` and `remote_facts::cli_tool_releases`; where the
//! tools run and what they take from outside is the host's (`AppHost::tool_environment`, `tool_releases`,
//! `youtrack_status`: the user's tools in the app, the fakes of a test repository in the e2e server).

use crate::context::AppContext;
use crate::events::setup_changed;
use crate::operation::{operation, run_blocking};
use crate::transport::{Call, Sink};
use cli_tools::install::{ActionInput, ActionOutput, run_action};
use cli_tools::kept_statuses::KeptStatuses;
use cli_tools::setup::{SetupFacts, setup_entries};
use cli_tools::status::{FoundTool, KnownFacts, find_statuses};
use domain_types::cli_tools::{CliTool, CliToolAction, CliToolStatus};
use domain_types::error::ApiError;
use domain_types::setup::{SetupEntry, SetupSkip};
use domain_types::youtrack::YouTrackAuthStatus;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use sync_utils::single_flight::NextFlight;
use tracing::{instrument, warn};

/// The tools of the app: the actions that run and the statuses of the read that started last
#[derive(Default)]
pub struct CliToolsState {
  /// The actions running now, by tool: a cancel sets its flag
  running: Mutex<HashMap<CliTool, Arc<AtomicBool>>>,
  /// The statuses of the read that started last, which the Setup dot takes without running the CLIs again
  statuses: KeptStatuses,
  /// The reads of the statuses: the windows that ask while one runs (the Setup dot of each, Settings → Tools) share the
  /// one after it, which starts after what changed meanwhile (an action ended, an install in a terminal)
  reads: NextFlight<Result<Vec<CliToolStatus>, ApiError>>,
}

impl CliToolsState {
  /// The statuses kept are forgotten: the tools changed outside the app (a fixture of the e2e server)
  pub fn forget_statuses(&self) {
    self.statuses.forget();
  }

  fn running(&self) -> std::sync::MutexGuard<'_, HashMap<CliTool, Arc<AtomicBool>>> {
    self.running.lock().unwrap_or_else(PoisonError::into_inner)
  }
}

/// The YouTrack the app is signed in to, by its URL, while its sign-in has no problem
fn signed_in_youtrack(status: Option<YouTrackAuthStatus>) -> Option<String> {
  let status = status?;
  status.url.filter(|_| status.signed_in_as.is_some() && status.problem.is_none())
}

/// The statuses of every tool with the newest releases and the YouTrack the app is signed in to, kept for the Setup dot
async fn read(ctx: &AppContext) -> Result<Vec<CliToolStatus>, ApiError> {
  let environment = ctx.host.tool_environment();
  let signed_in = || signed_in_youtrack(ctx.host.youtrack_status());
  read_once(&ctx.cli_tools, ctx.host.tool_releases(), move || find_statuses(&environment), signed_in).await
}

/// `read` of the parts of the host: the tools are found (`find`, off the async workers) while the newest releases are
/// asked (`releases`); a caller that comes while a read runs gets the answer of the read after it, which every caller
/// that came meanwhile shares, as the read running may have started before what it asks about
async fn read_once(
  state: &CliToolsState,
  releases: impl Future<Output = HashMap<CliTool, String>> + Send,
  find: impl FnOnce() -> Vec<FoundTool> + Send + 'static,
  signed_in_youtrack: impl FnOnce() -> Option<String> + Send,
) -> Result<Vec<CliToolStatus>, ApiError> {
  state
    .reads
    .run(|| async {
      let ticket = state.statuses.start_read();
      let (releases, found) = tokio::join!(releases, run_blocking(move || Ok::<_, ApiError>(find())));
      let signed_in_youtrack = signed_in_youtrack();
      let statuses: Vec<CliToolStatus> = found?
        .into_iter()
        .map(|found| {
          let facts = KnownFacts {
            latest: releases.get(&found.tool()).cloned(),
            signed_in_youtrack: signed_in_youtrack.clone(),
          };
          found.status(&facts)
        })
        .collect();
      state.statuses.keep(ticket, &statuses);
      Ok(statuses)
    })
    .await
}

/// The status of every tool, each read in its own threads, with the newest releases (GitHub, once a day); every window
/// reads the Setup dot again
#[instrument(skip_all)]
pub async fn cli_tools_status(ctx: &AppContext) -> Result<Vec<CliToolStatus>, ApiError> {
  operation(async {
    ctx.host.before_tools_status().await?;
    let statuses = read(ctx).await?;
    setup_changed(&*ctx.events);
    Ok(statuses)
  })
  .await
}

/// What waits for the user (the Setup dot): the downloaded update or failed update check of the app, the tools' updates
/// and expiring sign-ins from their last statuses (read now when none is known), the app's YouTrack sign-in; without
/// what `skips` hides
#[instrument(skip_all, fields(skips = skips.len()))]
pub async fn get_setup_items(ctx: &AppContext, skips: Vec<SetupSkip>) -> Result<Vec<SetupEntry>, ApiError> {
  operation(async {
    let statuses = match ctx.cli_tools.statuses.get() {
      Some(statuses) => statuses,
      None => read(ctx).await?,
    };
    let app_update = ctx.desktop.updater.downloaded();
    let app_update_check_failure = ctx.desktop.updater.check_failure();
    let youtrack_broken = ctx.host.youtrack_status().is_some_and(|status| status.problem.is_some());
    Ok(setup_entries(&SetupFacts {
      app_update: app_update.as_deref(),
      app_update_check_failure: app_update_check_failure.as_deref(),
      tools: &statuses,
      youtrack_broken,
      skips: &skips,
      now: ctx.host.now(),
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

/// Runs the action of a row (install, with the agent skill when `with_skill`; update; the skill; the sign-in of
/// `teamcity` to `server`), each line of its output to `progress`; a second action of the same tool while one runs is
/// refused
#[instrument(skip_all, fields(tool = ?params.tool, action = ?params.action))]
pub async fn run_cli_tool_action(ctx: &AppContext, params: RunCliToolActionParams, progress: Sink<CliToolProgress>) -> Result<CliToolActionDone, ApiError> {
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
      let mut running = ctx.cli_tools.running();
      if running.contains_key(&tool) {
        return Err(ApiError::internal(format!("{} is busy with another action", tool.command())));
      }
      running.insert(tool, Arc::clone(&canceled));
    }
    let environment = ctx.host.tool_environment();
    let result = run_blocking(move || {
      let send = |event: CliToolProgress| {
        if progress.send(event).is_err() {
          warn!("the page of the action went away: its lines go nowhere");
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
      run_action(&environment, tool, action, &input, &output, &|| canceled.load(Ordering::SeqCst))
    })
    .await;
    ctx.cli_tools.running().remove(&tool);
    ctx.host.tool_changed(tool).await;
    result.map(|done| CliToolActionDone {
      title: done.title,
      skill_problem: done.skill_problem,
    })
  })
  .await
}

/// Ends the action of `tool` that runs, with its whole process group
#[instrument(skip_all, fields(tool = ?tool))]
pub fn cancel_cli_tool_action(ctx: &AppContext, _call: &Call, tool: CliTool) {
  if let Some(canceled) = ctx.cli_tools.running().get(&tool) {
    canceled.store(true, Ordering::SeqCst);
  }
}

#[cfg(test)]
#[path = "cli_tools_test.rs"]
mod cli_tools_test;
