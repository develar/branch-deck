//! The `teamcity` and `youtrack` CLIs and `brew` of a test repository: fakes in its app data directory
//! (`test_utils::fake_cli_tools`), which the real cores of Settings → Tools run (`cli_tools`), so no test runs the
//! tools of the machine. A test sets what is installed (`POST /repositories/{id}/cli-tools`) and reads what the fakes
//! were asked (`GET`). The newest release of each tool is the fixture's until the app checks the releases itself.

use crate::state::{AppState, TestRepository};
use crate::tauri_command_bridge::{CommandFailure, CommandResult, requesting_repository};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
// jscpd:ignore-start: the e2e server is the second adapter of the cores the app's commands call; the imports name them
use cli_tools::environment::ToolEnvironment;
use cli_tools::install::run_action;
use cli_tools::setup::{SetupFacts, setup_entries};
use cli_tools::status::{KnownFacts, read_statuses};
use domain_types::cli_tools::{CliTool, CliToolAction, CliToolStatus};
use domain_types::setup::SetupSkip;
// jscpd:ignore-end
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use sync_utils::span::in_current_span;
use test_utils::fake_cli_tools::FakeCliTools;

/// The fakes of one repository, made on the first need
#[derive(Default)]
pub struct CliToolsHost {
  fakes: OnceLock<FakeCliTools>,
  latest: RwLock<HashMap<CliTool, String>>,
  running: Mutex<HashMap<CliTool, Arc<AtomicBool>>>,
  /// The last statuses read, which the Setup dot takes, as the app keeps them, by the generation of the fixture they
  /// were read with: a fixture drops them, and a read that started before it keeps nothing
  statuses: Mutex<(u64, Option<Vec<CliToolStatus>>)>,
}

impl CliToolsHost {
  fn fakes(&self, repository: &TestRepository) -> &FakeCliTools {
    self.fakes.get_or_init(|| FakeCliTools::new(&repository.app_data.join("cli-tools")))
  }

  fn environment(&self, repository: &TestRepository) -> ToolEnvironment {
    let fakes = self.fakes(repository);
    ToolEnvironment::isolated(&fakes.bin(), &fakes.home())
  }

  /// The statuses on the fakes, kept for the Setup dot
  async fn read(&self, repository: &TestRepository) -> Result<Vec<CliToolStatus>, CommandFailure> {
    let generation = self.statuses.lock().expect("the statuses").0;
    let environment = self.environment(repository);
    let facts: HashMap<CliTool, KnownFacts> = CliTool::ALL.into_iter().map(|tool| (tool, self.facts(tool))).collect();
    let statuses = tokio::task::spawn_blocking(in_current_span(move || read_statuses(&environment, &|tool| facts.get(&tool).cloned().unwrap_or_default())))
      .await
      .map_err(|join_error| {
        tracing::error!("Cannot read the CLIs: the read panicked: {join_error}");
        CommandFailure::Panicked
      })?;
    let mut kept = self.statuses.lock().expect("the statuses");
    if kept.0 == generation {
      kept.1 = Some(statuses.clone());
    }
    Ok(statuses)
  }

  fn facts(&self, tool: CliTool) -> KnownFacts {
    KnownFacts {
      latest: self.latest.read().expect("the latest releases").get(&tool).cloned(),
    }
  }
}

/// A tool installed before the test starts
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledTool {
  pub tool: CliTool,
  pub version: String,
  /// By its install script, not by Homebrew
  #[serde(default)]
  pub unmanaged: bool,
}

/// The fakes from now on; a field left out changes nothing
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliToolsFixture {
  #[serde(default)]
  pub installed: Vec<InstalledTool>,
  /// The machine has Homebrew
  #[serde(default)]
  pub brew: bool,
  /// The newest release of a tool, which `brew upgrade` and `teamcity update` install
  #[serde(default)]
  pub latest: BTreeMap<String, String>,
  /// State files of the fakes by name: `teamcity.auth`, `youtrack.auth` (the `auth status --json`), `teamcity.skill`
  /// (the bundled skill), `brew.fail` (the stderr of a failing brew), `brew.hang` (a brew that runs for a minute)
  #[serde(default)]
  pub files: BTreeMap<String, String>,
}

fn repository_of<'a>(state: &'a AppState, id: &str) -> Result<dashmap::mapref::one::Ref<'a, String, TestRepository>, StatusCode> {
  state.repositories.get(id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)
}

pub async fn set_cli_tools(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<CliToolsFixture>) -> Result<StatusCode, StatusCode> {
  let repository = repository_of(&state, &id)?;
  let host = &repository.cli_tools;
  let fakes = host.fakes(&repository);
  {
    let mut kept = host.statuses.lock().expect("the statuses");
    *kept = (kept.0 + 1, None);
  }
  if fixture.brew {
    fakes.add_brew();
  }
  for (name, text) in &fixture.files {
    fakes.set(name, text);
  }
  for (name, version) in &fixture.latest {
    let tool = CliTool::ALL.into_iter().find(|tool| tool.command() == name).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    fakes.set_latest(name, version);
    host.latest.write().expect("the latest releases").insert(tool, version.clone());
  }
  for installed in &fixture.installed {
    if installed.unmanaged {
      fakes.install_unmanaged(installed.tool.command(), &installed.version);
    } else {
      fakes.install(installed.tool.command(), &installed.version);
    }
  }
  Ok(StatusCode::OK)
}

/// What the fakes were asked, in order: `brew install jetbrains/utils/youtrack`, `teamcity skill install …`
pub async fn get_cli_tools(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
  let repository = repository_of(&state, &id)?;
  let log = repository.cli_tools.fakes(&repository).log();
  Ok(Json(serde_json::json!({ "log": log })))
}

pub async fn cli_tools_status(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap) -> CommandResult<serde_json::Value> {
  let repository = requesting_repository(&state, &headers)?;
  let statuses = repository.cli_tools.read(&repository).await?;
  Ok(Json(serde_json::to_value(statuses).expect("the statuses serialize")))
}

#[derive(Deserialize)]
pub struct RunCliToolActionRequest {
  tool: CliTool,
  action: CliToolAction,
  #[serde(default)]
  latest: Option<String>,
}

/// The action on the fakes; its lines stay unsent, as the page keeps the progress channel
pub async fn run_cli_tool_action(
  State(state): State<Arc<AppState>>,
  headers: axum::http::HeaderMap,
  Json(request): Json<RunCliToolActionRequest>,
) -> CommandResult<serde_json::Value> {
  let repository = requesting_repository(&state, &headers)?;
  let environment = repository.cli_tools.environment(&repository);
  let canceled = Arc::new(AtomicBool::new(false));
  {
    let mut running = repository.cli_tools.running.lock().expect("the running actions");
    if running.contains_key(&request.tool) {
      return Err(CommandFailure::error("run the action", format!("{} is busy with another action", request.tool.command())));
    }
    running.insert(request.tool, Arc::clone(&canceled));
  }
  let repository_id = repository.id.clone();
  drop(repository);
  let RunCliToolActionRequest { tool, action, latest } = request;
  let result = tokio::task::spawn_blocking(in_current_span(move || {
    run_action(&environment, tool, action, latest.as_deref(), &|_| {}, &|| canceled.load(Ordering::SeqCst))
  }))
  .await;
  if let Some(repository) = state.repositories.get(&repository_id) {
    repository.cli_tools.running.lock().expect("the running actions").remove(&tool);
  }
  match result {
    Ok(Ok(done)) => Ok(Json(serde_json::json!({ "title": done.title }))),
    Ok(Err(error)) => Err(CommandFailure::error("run the action", error)),
    Err(join_error) => {
      tracing::error!("Cannot run the action: it panicked: {join_error}");
      Err(CommandFailure::Panicked)
    }
  }
}

#[derive(Deserialize)]
pub struct CancelCliToolActionRequest {
  tool: CliTool,
}

pub async fn cancel_cli_tool_action(
  State(state): State<Arc<AppState>>,
  headers: axum::http::HeaderMap,
  Json(request): Json<CancelCliToolActionRequest>,
) -> CommandResult<serde_json::Value> {
  let repository = requesting_repository(&state, &headers)?;
  if let Some(canceled) = repository.cli_tools.running.lock().expect("the running actions").get(&request.tool) {
    canceled.store(true, Ordering::SeqCst);
  }
  Ok(Json(serde_json::Value::Null))
}

#[derive(Deserialize)]
pub struct SetupItemsRequest {
  skips: Vec<SetupSkip>,
}

/// The items of the Setup dot as the app derives them: the update of the fixture (`updater.rs`), the CLIs on the fakes,
/// the sign-in of the fake YouTrack; now is the start of the page's clock, which the skips of the page are counted from
pub async fn get_setup_items(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap, Json(request): Json<SetupItemsRequest>) -> CommandResult<serde_json::Value> {
  let repository = requesting_repository(&state, &headers)?;
  let app_update = crate::updater::available_update(&repository);
  let cached = repository.cli_tools.statuses.lock().expect("the statuses").1.clone();
  let statuses = match cached {
    Some(statuses) => statuses,
    None => repository.cli_tools.read(&repository).await?,
  };
  // the fake services only once a test started them: a page that opens starts none
  let remote = repository.existing_remote();
  drop(repository);
  let youtrack_broken = match remote {
    Some(remote) => remote.youtrack_service().status().await.problem.is_some(),
    None => false,
  };
  let entries = setup_entries(&SetupFacts {
    app_update: app_update.as_deref(),
    tools: &statuses,
    youtrack_broken,
    skips: &request.skips,
    now: crate::remote_host::PAGE_CLOCK_START,
  });
  Ok(Json(serde_json::to_value(entries).expect("the entries serialize")))
}
