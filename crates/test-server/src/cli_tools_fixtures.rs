//! The `teamcity` and `youtrack` CLIs and `brew` of a test repository: fakes in its app data directory
//! (`test_utils::fake_cli_tools`), where the commands of Settings → Tools of app-api run the real cores (`cli_tools`,
//! `AppHost::tool_environment` of `TestHost`), so no test runs the tools of the machine. A test sets what is installed
//! (`POST /repositories/{id}/cli-tools`) and reads what the fakes were asked (`GET`). The newest release of each tool
//! is the fixture's (`AppHost::tool_releases`), the app checks GitHub.

use crate::state::{AppState, TestRepository};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use cli_tools::environment::ToolEnvironment;
use domain_types::cli_tools::CliTool;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock, PoisonError, RwLock};
use test_utils::fake_cli_tools::FakeCliTools;

/// The fakes of one repository, made on the first need in `root`, and the newest release of each tool
pub struct CliToolsHost {
  root: std::path::PathBuf,
  fakes: OnceLock<FakeCliTools>,
  latest: RwLock<HashMap<CliTool, String>>,
}

impl CliToolsHost {
  pub fn new(app_data: &std::path::Path) -> Self {
    Self {
      root: app_data.join("cli-tools"),
      fakes: OnceLock::new(),
      latest: RwLock::default(),
    }
  }

  fn fakes(&self) -> &FakeCliTools {
    self.fakes.get_or_init(|| FakeCliTools::new(&self.root))
  }

  /// Where the tools of the test repository run: its fakes alone
  pub fn environment(&self) -> ToolEnvironment {
    let fakes = self.fakes();
    ToolEnvironment::isolated(&fakes.bin(), &fakes.home())
  }

  /// The newest release of each tool, as the fixture set it
  pub fn latest(&self) -> HashMap<CliTool, String> {
    self.latest.read().unwrap_or_else(PoisonError::into_inner).clone()
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
    host.latest.write().unwrap_or_else(PoisonError::into_inner).insert(tool, version.clone());
  }
  for installed in &fixture.installed {
    if installed.unmanaged {
      fakes.install_unmanaged(installed.tool.command(), &installed.version);
    } else {
      fakes.install(installed.tool.command(), &installed.version);
    }
  }
  // after the fixture, not before: a read of the page meanwhile (the Setup dot of a window that opens) saw half of it
  repository.context.cli_tools.forget_statuses();
  Ok(StatusCode::OK)
}

/// What the fakes were asked, in order: `brew install jetbrains/utils/youtrack`, `teamcity skill install …`
pub async fn get_cli_tools(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
  let repository = repository_of(&state, &id)?;
  let log = repository.cli_tools.fakes().log();
  Ok(Json(serde_json::json!({ "log": log })))
}
