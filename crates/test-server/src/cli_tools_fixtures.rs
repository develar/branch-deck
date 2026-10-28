//! The `teamcity` and `youtrack` CLIs and `brew` of a test repository: fakes in its app data directory
//! (`test_utils::fake_cli_tools`), which the real cores of Settings → Tools run (`cli_tools`), so no test runs the
//! tools of the machine. A test sets what is installed (`POST /repositories/{id}/cli-tools`) and reads what the fakes
//! were asked (`GET`). The newest release of each tool is the fixture's until the app checks the releases itself.

use crate::host::held_or_failed;
use crate::invoke::{CommandResult, Rejection, failed};
use crate::remote_host::HeldAnswer;
use crate::state::{AppState, TestRepository};
use crate::tauri_command_bridge::requesting_repository;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
// jscpd:ignore-start: the e2e server is the second adapter of the cores the app's commands call; the imports name them
use cli_tools::environment::ToolEnvironment;
use cli_tools::install::{ActionInput, ActionOutput, run_action};
use cli_tools::kept_statuses::KeptStatuses;
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

/// The fakes of one repository, made on the first need in `root`. Shared (`Arc`): a handler takes it out of the
/// repository and lets the repository go before it waits, as a guard of the map held over an await would hold up the
/// creation and deletion of the repositories of the other tests
pub struct CliToolsHost {
  root: std::path::PathBuf,
  fakes: OnceLock<FakeCliTools>,
  latest: RwLock<HashMap<CliTool, String>>,
  running: Mutex<HashMap<CliTool, Arc<AtomicBool>>>,
  /// The YouTrack the app is signed in to, as the requests that start the reads found it
  signed_in_youtrack: RwLock<Option<String>>,
  /// The statuses of the read that started last, which the Setup dot takes, as the app keeps them: a fixture forgets
  /// them once it is written, and a read that started before, which may have seen half of it, keeps nothing
  statuses: KeptStatuses,
}

impl CliToolsHost {
  pub fn new(app_data: &std::path::Path) -> Self {
    Self {
      root: app_data.join("cli-tools"),
      fakes: OnceLock::new(),
      latest: RwLock::default(),
      running: Mutex::default(),
      signed_in_youtrack: RwLock::default(),
      statuses: KeptStatuses::default(),
    }
  }

  fn fakes(&self) -> &FakeCliTools {
    self.fakes.get_or_init(|| FakeCliTools::new(&self.root))
  }

  pub(crate) fn environment(&self) -> ToolEnvironment {
    let fakes = self.fakes();
    ToolEnvironment::isolated(&fakes.bin(), &fakes.home())
  }

  /// The statuses on the fakes, kept for the Setup dot
  async fn read(&self) -> Result<Vec<CliToolStatus>, Rejection> {
    let ticket = self.statuses.start_read();
    let environment = self.environment();
    let facts: HashMap<CliTool, KnownFacts> = CliTool::ALL.into_iter().map(|tool| (tool, self.facts(tool))).collect();
    let statuses = tokio::task::spawn_blocking(in_current_span(move || read_statuses(&environment, &|tool| facts.get(&tool).cloned().unwrap_or_default())))
      .await
      .map_err(|join_error| {
        tracing::error!("Cannot read the CLIs: the read panicked: {join_error}");
        Rejection::Panicked
      })?;
    self.statuses.keep(ticket, &statuses);
    Ok(statuses)
  }

  fn facts(&self, tool: CliTool) -> KnownFacts {
    KnownFacts {
      latest: self.latest.read().expect("the latest releases").get(&tool).cloned(),
      signed_in_youtrack: self.signed_in_youtrack.read().expect("the YouTrack of the app").clone(),
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
  // the tool of each newest release before anything is written: a fixture is taken whole or not at all
  let latest = fixture
    .latest
    .iter()
    .map(|(name, version)| CliTool::ALL.into_iter().find(|tool| tool.command() == name).map(|tool| (tool, name, version)))
    .collect::<Option<Vec<_>>>()
    .ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  let fakes = host.fakes();
  if fixture.brew {
    fakes.add_brew();
  }
  for (name, text) in &fixture.files {
    fakes.set(name, text);
  }
  for (tool, name, version) in latest {
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
  // after the fixture, not before: a read of the page meanwhile (the Setup dot of a window that opens) saw half of it
  host.statuses.forget();
  Ok(StatusCode::OK)
}

/// What the fakes were asked, in order: `brew install jetbrains/utils/youtrack`, `teamcity skill install …`
pub async fn get_cli_tools(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
  let repository = repository_of(&state, &id)?;
  let log = repository.cli_tools.fakes().log();
  Ok(Json(serde_json::json!({ "log": log })))
}

pub async fn cli_tools_status(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap) -> CommandResult<serde_json::Value> {
  let (host, remote) = {
    let repository = requesting_repository(&state, &headers)?;
    (Arc::clone(&repository.cli_tools), repository.existing_remote())
  };
  if let Some(remote) = &remote {
    held_or_failed(remote, HeldAnswer::CliTools, "read the tools").await?;
  }
  // the YouTrack the app is signed in to, of the fake YouTrack a test started
  let signed_in = match remote {
    Some(remote) => {
      let status = remote.youtrack_service().status();
      status.url.filter(|_| status.signed_in_as.is_some() && status.problem.is_none())
    }
    None => None,
  };
  *host.signed_in_youtrack.write().expect("the YouTrack of the app") = signed_in;
  let statuses = host.read().await?;
  Ok(Json(serde_json::to_value(statuses).expect("the statuses serialize")))
}

/// The arguments as the page sends them, camelCase as Tauri takes them
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCliToolActionRequest {
  tool: CliTool,
  action: CliToolAction,
  #[serde(default)]
  latest: Option<String>,
  #[serde(default)]
  server: Option<String>,
  #[serde(default)]
  with_skill: bool,
}

/// The action on the fakes; its lines stay unsent, as the page keeps the progress channel
pub async fn run_cli_tool_action(
  State(state): State<Arc<AppState>>,
  headers: axum::http::HeaderMap,
  Json(request): Json<RunCliToolActionRequest>,
) -> CommandResult<serde_json::Value> {
  let repository = requesting_repository(&state, &headers)?;
  let environment = repository.cli_tools.environment();
  let canceled = Arc::new(AtomicBool::new(false));
  {
    let mut running = repository.cli_tools.running.lock().expect("the running actions");
    if running.contains_key(&request.tool) {
      return Err(failed("run the action", format!("{} is busy with another action", request.tool.command())));
    }
    running.insert(request.tool, Arc::clone(&canceled));
  }
  let repository_id = repository.id.clone();
  drop(repository);
  let RunCliToolActionRequest {
    tool,
    action,
    latest,
    server,
    with_skill,
  } = request;
  let result = tokio::task::spawn_blocking(in_current_span(move || {
    let input = ActionInput {
      latest: latest.as_deref(),
      server: server.as_deref(),
      with_skill,
    };
    let output = ActionOutput {
      on_line: &|_| {},
      on_url: &|_| {},
    };
    run_action(&environment, tool, action, &input, &output, &|| canceled.load(Ordering::SeqCst))
  }))
  .await;
  if let Some(repository) = state.repositories.get(&repository_id) {
    repository.cli_tools.running.lock().expect("the running actions").remove(&tool);
  }
  match result {
    Ok(Ok(done)) => Ok(Json(serde_json::json!({ "title": done.title, "skill_problem": done.skill_problem }))),
    Ok(Err(error)) => Err(failed("run the action", error)),
    Err(join_error) => {
      tracing::error!("Cannot run the action: it panicked: {join_error}");
      Err(Rejection::Panicked)
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
  let (app_update, host, remote) = {
    let repository = requesting_repository(&state, &headers)?;
    // the fake services only once a test started them: a page that opens starts none
    (
      crate::updater::available_update(&repository),
      Arc::clone(&repository.cli_tools),
      repository.existing_remote(),
    )
  };
  let statuses = match host.statuses.get() {
    Some(statuses) => statuses,
    None => host.read().await?,
  };
  let youtrack_broken = match remote {
    Some(remote) => remote.youtrack_service().status().problem.is_some(),
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
