//! Settings → Tools: the `teamcity` and `youtrack` CLIs, their status and the action of each row
//! (openspec/specs/repository/cli-tools/spec.md), and the Setup dot of what waits for the user
//! (openspec/specs/repository/setup-attention/spec.md). The cores are `cli_tools` and `remote_facts::cli_tool_releases`; where the
//! tools run and what they take from outside is the host's (`AppHost::tool_environment`, `tool_releases`,
//! `youtrack_status`: the user's tools in the app, the fakes of a test repository in the e2e server).

use crate::context::AppContext;
use crate::events::{cli_tools_running_changed, setup_changed};
use crate::operation::{operation, run_blocking};
use crate::transport::{AppEvents, Call};
use cli_tools::install::{ActionInput, ActionOutput, run_action};
use cli_tools::kept_statuses::KeptStatuses;
use cli_tools::setup::{SetupFacts, setup_entries};
use cli_tools::status::{FoundTool, KnownFacts, find_statuses};
use domain_types::cli_tools::{CliTool, CliToolAction, CliToolRunning, CliToolStatus, CliToolsRunning};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::setup::{SetupEntry, SetupSkip};
use domain_types::youtrack::YouTrackAuthStatus;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use sync_utils::single_flight::NextFlight;
use tracing::instrument;

/// The tools of the app: the actions that run and the statuses of the read that started last
#[derive(Default)]
pub struct CliToolsState {
  /// The actions running now, by tool, which every window shows (`cli_tools_running`, the event `cli-tools-running`)
  running: Arc<Mutex<Running>>,
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

  /// The actions that run now
  pub fn running_now(&self) -> CliToolsRunning {
    lock(&self.running).snapshot()
  }
}

/// The actions that run, and the version of their last change, which grows with each
#[derive(Default)]
struct Running {
  version: u32,
  actions: HashMap<CliTool, RunningAction>,
}

/// An action that runs: what the windows show of it, and the flag its cancel sets
struct RunningAction {
  action: CliToolAction,
  line: String,
  url: Option<String>,
  canceled: Arc<AtomicBool>,
}

impl Running {
  /// The actions in the order of the rows, at a new version
  fn changed(&mut self) -> CliToolsRunning {
    self.version = self.version.wrapping_add(1);
    self.snapshot()
  }

  fn snapshot(&self) -> CliToolsRunning {
    let actions = CliTool::ALL
      .into_iter()
      .filter_map(|tool| {
        self.actions.get(&tool).map(|running| CliToolRunning {
          tool,
          action: running.action,
          line: running.line.clone(),
          url: running.url.clone(),
        })
      })
      .collect();
    CliToolsRunning { version: self.version, actions }
  }
}

fn lock(running: &Mutex<Running>) -> std::sync::MutexGuard<'_, Running> {
  running.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An action of a tool from its start to its end: each change goes to every window, and its drop ends it there, also
/// when the action panics
struct RunningGuard {
  tool: CliTool,
  running: Arc<Mutex<Running>>,
  events: Arc<dyn AppEvents>,
}

impl RunningGuard {
  /// Starts `action` of `tool`, refused with `CliToolBusy` while another action of it runs (a window that started one
  /// before it heard of the other)
  fn start(state: &CliToolsState, events: &Arc<dyn AppEvents>, tool: CliTool, action: CliToolAction, canceled: Arc<AtomicBool>) -> Result<Self, ApiError> {
    let snapshot = {
      let mut running = lock(&state.running);
      if running.actions.contains_key(&tool) {
        return Err(ApiError::new(
          ErrorCode::CliToolBusy,
          format!("{} runs another action: it shows on its row until it ends", tool.command()),
        ));
      }
      running.actions.insert(
        tool,
        RunningAction {
          action,
          line: String::new(),
          url: None,
          canceled,
        },
      );
      running.changed()
    };
    cli_tools_running_changed(&**events, &snapshot);
    Ok(Self {
      tool,
      running: Arc::clone(&state.running),
      events: Arc::clone(events),
    })
  }

  /// A line of the output, or the page a sign-in opened with its line
  fn progress(&self, line: &str, url: Option<&str>) {
    let snapshot = {
      let mut running = lock(&self.running);
      let Some(action) = running.actions.get_mut(&self.tool) else {
        return;
      };
      line.clone_into(&mut action.line);
      if let Some(url) = url {
        action.url = Some(url.to_string());
      }
      running.changed()
    };
    cli_tools_running_changed(&*self.events, &snapshot);
  }
}

impl Drop for RunningGuard {
  fn drop(&mut self) {
    let snapshot = {
      let mut running = lock(&self.running);
      running.actions.remove(&self.tool);
      running.changed()
    };
    cli_tools_running_changed(&*self.events, &snapshot);
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

/// What an action did, in the words of its toast
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct CliToolActionDone {
  pub title: String,
  /// Why the skill an install or an update brings along failed, while the CLI itself is done
  pub skill_problem: Option<String>,
}

/// Runs the action of a row (install, with the agent skill when `with_skill`; update; the skill; the sign-in of
/// `teamcity` to `server`); every window shows it running with its last line (`cli-tools-running`), so a second action
/// of the same tool while one runs is refused (`CliToolBusy`)
#[instrument(skip_all, fields(tool = ?params.tool, action = ?params.action))]
pub async fn run_cli_tool_action(ctx: &AppContext, params: RunCliToolActionParams) -> Result<CliToolActionDone, ApiError> {
  operation(async move {
    let RunCliToolActionParams {
      tool,
      action,
      latest,
      server,
      with_skill,
    } = params;
    let canceled = Arc::new(AtomicBool::new(false));
    let guard = RunningGuard::start(&ctx.cli_tools, &ctx.events, tool, action, Arc::clone(&canceled))?;
    let environment = ctx.host.tool_environment();
    let result = run_blocking(move || {
      let on_line = |line: &str| guard.progress(line, None);
      let on_url = |url: &str| guard.progress("Waiting for the browser…", Some(url));
      let input = ActionInput {
        latest: latest.as_deref(),
        server: server.as_deref(),
        with_skill,
      };
      let output = ActionOutput {
        on_line: &on_line,
        on_url: &on_url,
      };
      // the guard ends the action in every window when this returns or panics
      run_action(&environment, tool, action, &input, &output, &|| canceled.load(Ordering::SeqCst))
    })
    .await;
    ctx.host.tool_changed(tool).await;
    result.map(|done| CliToolActionDone {
      title: done.title,
      skill_problem: done.skill_problem,
    })
  })
  .await
}

/// The actions of the tools that run now: a window that opens, or loads again, while one runs shows it, then follows
/// the event `cli-tools-running`
#[instrument(skip_all)]
pub fn cli_tools_running(ctx: &AppContext, _call: &Call) -> CliToolsRunning {
  ctx.cli_tools.running_now()
}

/// Ends the action of `tool` that runs, with its whole process group
#[instrument(skip_all, fields(tool = ?tool))]
pub fn cancel_cli_tool_action(ctx: &AppContext, _call: &Call, tool: CliTool) {
  if let Some(running) = lock(&ctx.cli_tools.running).actions.get(&tool) {
    running.canceled.store(true, Ordering::SeqCst);
  }
}

#[cfg(test)]
#[path = "cli_tools_test.rs"]
mod cli_tools_test;
