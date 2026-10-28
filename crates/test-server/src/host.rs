//! What the app of a test repository gives the commands of app-api beside the services of its sessions
//! (`app_api::transport::AppHost`): the accounts of the repository's fakes (`RemoteHost`, started on the first need),
//! no `teamcity` CLI, the branch names of its fixture (E2E never runs claude), the page's clock in UTC, so the weeks
//! and the hours of the Safe Push statistics of a test are fixed, and the fakes of the CLIs of Settings → Tools
//! (`cli_tools_fixtures.rs`). A read of a status the test holds waits at the gate of its answer (`held_or_failed`). The
//! events of the app go to the pages of the repository on a stream of each (`TestEvents`). The desktop of the app
//! (`TestDesktop`) builds no window: it records each open and each window menu (`GET /repositories/{id}/windows`), takes
//! the build of a page a test opened (`POST /repositories/{id}/windows/built`), picks the folder of its fixture
//! (`POST /repositories/{id}/dialog-pick`) and restarts nothing; its agent CLI (`TestAgentCli`) installs nothing

use crate::cli_tools_fixtures::CliToolsHost;
use crate::invoke::{Rejection, event_stream, failed};
use crate::remote_host::{HeldAnswer, PAGE_CLOCK_START, RemoteHost};
use crate::state::AppState;
use crate::tauri_command_bridge::rejected;
use crate::updater::NO_UPDATER_MESSAGE;
use app_api::app::agent_cli::{AgentCliCommand, AgentCliInstalled, AgentCliStatus};
use app_api::app::windows::{BuiltPage, OpenSubWindowParams, Shown, SubWindowOpen, show_when_settled};
use app_api::transport::{AgentCliHost, AppEvents, AppHost, DialogHost, Later, MenuHost, RestartHost, WindowHost, WindowId};
use app_settings::service::SettingsService;
use app_settings::windows::window_snapshot;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use cli_tools::environment::ToolEnvironment;
// jscpd:ignore-start: the implementation of `AppHost` imports the types of its signature, as `transport.rs` does
use domain_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use domain_types::calendar::Calendar;
use domain_types::cli_tools::CliTool;
use domain_types::error::ApiError;
use domain_types::youtrack::YouTrackAuthStatus;
use git_executor::git_command_executor::GitCommandExecutor;
use patronus::teamcity_cli::TeamCityCli;
use remote_facts::fact_sources::Read;
use remote_facts::sign_in::SignIns;
// jscpd:ignore-end
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use sync_core::branch_name_suggestion::clean_names;
use tokio::sync::mpsc;
use youtrack::service::YouTrackService;

/// The host of the app of the test repository `repository` (`AppState::context_of`)
pub struct TestHost {
  pub state: Weak<AppState>,
  /// The id of the test repository
  pub repository: String,
  /// The fakes of its CLIs
  pub cli_tools: Arc<CliToolsHost>,
}

impl TestHost {
  /// The fakes of the test repository, started now if not yet
  fn remote(&self) -> Result<Arc<RemoteHost>, ApiError> {
    let state = self.state.upgrade().ok_or_else(|| ApiError::internal("The test server stopped"))?;
    let repository = state.repositories.get(&self.repository);
    let repository = repository.ok_or_else(|| ApiError::internal(format!("The test repository {} is gone", self.repository)))?;
    Ok(repository.remote(&state.git_executor))
  }

  /// The fakes of the test repository once a test started them: a page that opens starts none
  fn existing_remote(&self) -> Option<Arc<RemoteHost>> {
    self.state.upgrade()?.repositories.get(&self.repository)?.existing_remote()
  }
}

impl AppHost for TestHost {
  fn sign_ins(&self) -> Result<SignIns, ApiError> {
    Ok(self.remote()?.sign_ins().clone())
  }

  fn youtrack(&self) -> Result<Arc<YouTrackService>, ApiError> {
    Ok(self.remote()?.shared_youtrack_service())
  }

  /// Held and failed as the test says (`HeldAnswer::AuthStatus`)
  fn before_auth_status(&self) -> Read<'_, Result<(), ApiError>> {
    Box::pin(async move {
      let remote = self.remote()?;
      held_or_failed(&remote, HeldAnswer::AuthStatus, "read the sign-ins").await.map_err(rejected)
    })
  }

  /// Nothing of the developer's machine: no `teamcity` CLI
  fn teamcity(&self) -> Read<'_, Option<TeamCityCli>> {
    Box::pin(async { None })
  }

  /// The fixture of the repository after its delay (`POST /repositories/{id}/branch-name-suggestions`), counted, by
  /// default two names from the words of the commit messages; blocking, as the app's `claude` is
  fn suggest_branch_names(&self, _git: &GitCommandExecutor, params: &SuggestBranchNamesParams, _is_superseded: &dyn Fn() -> bool) -> BranchNameSuggestionOutcome {
    let fixture = self.state.upgrade().and_then(|state| {
      let repository = state.repositories.get(&self.repository)?;
      repository.branch_name_suggestion_calls.fetch_add(1, Ordering::SeqCst);
      let fixture = repository.branch_name_suggestions.read().expect("branch name suggestion fixture lock").clone();
      Some(fixture)
    });
    let Some(fixture) = fixture else {
      return BranchNameSuggestionOutcome::Error {
        message: format!("The test repository {} is gone", self.repository),
      };
    };
    std::thread::sleep(std::time::Duration::from_millis(fixture.delay_ms));
    if let Some(outcome) = fixture.outcome {
      return outcome;
    }
    let words: Vec<&str> = params
      .commits
      .iter()
      .flat_map(|commit| commit.message.split_whitespace())
      .filter(|word| word.len() > 3)
      .collect();
    let names = clean_names(&[
      format!("fix-{}", words.first().unwrap_or(&"issue")),
      format!("update-{}", words.get(1).unwrap_or(&"feature")),
    ]);
    BranchNameSuggestionOutcome::Suggested { names }
  }

  /// The start of the page's clock (`PAGE_CLOCK_START`)
  fn now(&self) -> u32 {
    PAGE_CLOCK_START
  }

  fn calendar(&self) -> Calendar {
    Calendar::utc()
  }

  fn tool_environment(&self) -> ToolEnvironment {
    self.cli_tools.environment()
  }

  /// The releases of the fixture (`POST /repositories/{id}/cli-tools`)
  fn tool_releases(&self) -> Read<'_, HashMap<CliTool, String>> {
    Box::pin(async move { self.cli_tools.latest() })
  }

  /// Held and failed as the test says (`HeldAnswer::CliTools`), once a test started the fakes
  fn before_tools_status(&self) -> Read<'_, Result<(), ApiError>> {
    Box::pin(async move {
      match self.existing_remote() {
        Some(remote) => held_or_failed(&remote, HeldAnswer::CliTools, "read the tools").await.map_err(rejected),
        None => Ok(()),
      }
    })
  }

  /// No `teamcity` CLI to look for again
  fn tool_changed(&self, _tool: CliTool) -> Read<'_, ()> {
    Box::pin(async {})
  }

  /// The fake YouTrack a test started
  fn youtrack_status(&self) -> Option<YouTrackAuthStatus> {
    self.existing_remote().map(|remote| remote.youtrack_service().status())
  }
}

/// Waits while a test holds `answer` (`/remote-gates`), then fails it while the test fails it: a read of a status the
/// server gives itself
pub(crate) async fn held_or_failed(host: &RemoteHost, answer: HeldAnswer, operation: &str) -> Result<(), Rejection> {
  host.pass_held(answer).await;
  if host.fails(answer) {
    return Err(failed(operation, "the fixture fails this read"));
  }
  Ok(())
}

/// The events of the app of a test repository to its pages (`AppEvents`), as the app sends them to its windows: each
/// page of the repository subscribes when it starts (`GET /repositories/{id}/events?window=<label>`,
/// `subscribe_events`), and an event goes to every page, or to the pages of one window; what a page emits for the other
/// windows (`emit` of `@tauri-apps/api/event`) goes the same way (`POST /repositories/{id}/events`, `emit_page_event`),
/// as the event plugin of Tauri sends it, the page itself included. The streams end with the repository (`close`)
pub struct TestEvents {
  /// The stream of each page that listens, by its window; none once the repository went
  pages: Mutex<Option<Vec<PageStream>>>,
}

struct PageStream {
  window: String,
  sender: mpsc::UnboundedSender<Value>,
}

impl Default for TestEvents {
  fn default() -> Self {
    Self {
      pages: Mutex::new(Some(Vec::new())),
    }
  }
}

impl TestEvents {
  /// The stream of a new page of `window`, its first message `{"subscribed": true}`; none once the repository went
  pub fn subscribe(&self, window: String) -> Option<mpsc::UnboundedReceiver<Value>> {
    let mut pages = self.pages.lock().unwrap_or_else(PoisonError::into_inner);
    let pages = pages.as_mut()?;
    let (sender, receiver) = mpsc::unbounded_channel();
    sender.send(json!({ "subscribed": true })).ok()?;
    pages.push(PageStream { window, sender });
    Some(receiver)
  }

  /// Sends `event` to the pages of `window`, or of every window; `source`: the window of the page that emitted it, none
  /// for an event of the app. A page that went away leaves
  fn send(&self, window: Option<&str>, event: &str, payload: Value, source: Option<&str>) {
    let mut message = json!({ "event": event, "window": window, "source": source });
    message["payload"] = payload;
    let mut pages = self.pages.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(pages) = pages.as_mut() {
      pages.retain(|page| window.is_some_and(|window| window != page.window) || page.sender.send(message.clone()).is_ok());
    }
  }

  /// The repository went: the stream of each page ends
  pub fn close(&self) {
    self.pages.lock().unwrap_or_else(PoisonError::into_inner).take();
  }
}

impl AppEvents for TestEvents {
  fn emit(&self, event: &str, payload: Value) {
    self.send(None, event, payload, None);
  }

  fn emit_to(&self, window: &WindowId, event: &str, payload: Value) {
    self.send(Some(window.as_str()), event, payload, None);
  }
}

#[derive(Deserialize)]
pub struct EventsQuery {
  /// The label of the window of the page (`main`, `settings`, …)
  window: String,
}

/// The event stream of a page of the test repository `id`, as SSE: `{"subscribed": true}`, then each event the page
/// hears, `{ event, payload, window, source }`, until the repository goes. Outside `hold_repository`: the stream lasts
/// as long as the page, which the deletion of the repository does not wait for
pub async fn subscribe_events(Path(id): Path<String>, Query(query): Query<EventsQuery>, State(state): State<Arc<AppState>>) -> Response {
  let receiver = state.repositories.get(&id).and_then(|repository| {
    // a page subscribes once, when it loads, before its first command
    repository.desktop.page_loaded(&query.window);
    repository.events.subscribe(query.window)
  });
  let Some(receiver) = receiver else {
    return StatusCode::GONE.into_response();
  };
  event_stream(receiver)
}

/// An event a page of the repository emits (`emit`, `emit_to` of `@tauri-apps/api/event`)
#[derive(Deserialize)]
pub struct PageEvent {
  event: String,
  #[serde(default)]
  payload: Value,
  /// The window it goes to; none: every window
  #[serde(default)]
  window: Option<String>,
  /// The window of the page that emits it
  source: String,
}

/// Sends the event of a page to the pages of the repository `id` it names, as the event plugin of the app does
pub async fn emit_page_event(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(event): Json<PageEvent>) -> StatusCode {
  let Some(repository) = state.repositories.get(&id) else {
    return StatusCode::GONE;
  };
  repository.events.send(event.window.as_deref(), &event.event, event.payload, Some(&event.source));
  StatusCode::OK
}

/// The desktop of the app of a test repository: the e2e server builds no window and opens no menu, it records them for
/// the test (`GET /repositories/{id}/windows`), and a test opens the page of a window itself (`openLinkedSubWindow`);
/// its folder dialog picks the folder of the fixture (`POST /repositories/{id}/dialog-pick`), else the repository's
/// own; its restart fails, as its updater's install does
pub struct TestDesktop {
  /// The writer of the settings of the repository: a window starts from its copy, which the open records
  settings: Arc<SettingsService>,
  /// The folder a pick answers without a fixture: the test repository
  repository_path: String,
  windows: Mutex<TestWindows>,
  pick: Mutex<Pick>,
}

/// What the folder dialog of the test repository picks
#[derive(Clone)]
enum Pick {
  /// The repository's own folder: no fixture
  Repository,
  /// The folder of the fixture
  Folder(String),
  /// Nothing: the user cancels
  Canceled,
}

#[derive(Default)]
struct TestWindows {
  /// The labels of the windows built (an open of the app, a page a test opened)
  open: HashSet<String>,
  /// The page each window shows, counted by its loads (its subscription to the events, `subscribe_events`): an open
  /// is of the page that asked for it, a test reads the opens of the page it holds
  pages: HashMap<String, u32>,
  /// Each open of a window, as a test reads it (`getOpenedSubWindows`)
  opens: Vec<Value>,
  /// The windows a window menu was asked of, in order
  menus: Vec<String>,
}

impl TestDesktop {
  pub fn new(settings: &Arc<SettingsService>, repository_path: &str) -> Self {
    Self {
      settings: Arc::clone(settings),
      repository_path: repository_path.to_owned(),
      windows: Mutex::default(),
      pick: Mutex::new(Pick::Repository),
    }
  }

  fn windows(&self) -> std::sync::MutexGuard<'_, TestWindows> {
    self.windows.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// A page of `window` loaded: the opens it asks for are its own
  fn page_loaded(&self, window: &str) {
    *self.windows().pages.entry(window.to_owned()).or_default() += 1;
  }

  /// An open the page of `opener` asked for: its parameters, its data as the page sends it, and the settings the window
  /// starts from, the writer's copy now (`window_snapshot`) with its version
  fn record_open(&self, opener: &WindowId, params: &OpenSubWindowParams) {
    let (version, store) = window_snapshot(&self.settings, &params.window_id);
    let data = serde_json::from_str(&params.data).unwrap_or_else(|_| Value::String(params.data.clone()));
    let mut windows = self.windows();
    let page = windows.pages.get(opener.as_str()).copied().unwrap_or_default();
    let open = json!({
      "opener": opener.as_str(),
      "page": page,
      "windowId": params.window_id,
      "url": params.url,
      "title": params.title,
      "data": data,
      "store": store,
      "storeVersion": version,
    });
    windows.opens.push(open);
  }
}

impl WindowHost for TestDesktop {
  fn is_open(&self, label: &str) -> bool {
    self.windows().open.contains(label)
  }

  fn reopen(&self, opener: &WindowId, params: &OpenSubWindowParams, _update: Option<&str>, _front: bool) -> Result<(), ApiError> {
    self.record_open(opener, params);
    Ok(())
  }

  fn build(&self, opener: &WindowId, params: &OpenSubWindowParams, _page_script: &str, _zoom: f64) -> Result<(), ApiError> {
    self.record_open(opener, params);
    self.windows().open.insert(params.window_id.clone());
    Ok(())
  }

  /// No window to show
  fn show(&self, _label: &str, _shown: Shown) -> Later<'_, ()> {
    Box::pin(async {})
  }
}

impl DialogHost for TestDesktop {
  fn pick_folder(&self, _title: &str) -> Option<String> {
    let pick = self.pick.lock().unwrap_or_else(PoisonError::into_inner).clone();
    match pick {
      Pick::Repository => Some(self.repository_path.clone()),
      Pick::Folder(folder) => Some(folder),
      Pick::Canceled => None,
    }
  }
}

impl MenuHost for TestDesktop {
  /// No native menu: the check is the request, which a test reads
  fn set_auto_sync_checked(&self, checked: bool) -> Result<(), ApiError> {
    tracing::debug!(checked, "Auto-sync on Focus checked");
    Ok(())
  }

  fn show_window_menu(&self, window: &WindowId) -> Result<(), ApiError> {
    self.windows().menus.push(window.as_str().to_owned());
    Ok(())
  }
}

impl RestartHost for TestDesktop {
  fn restart(&self) -> ApiError {
    ApiError::internal(NO_UPDATER_MESSAGE)
  }
}

/// The bd the copied prompts name: a fixed path, the app's `agent_cli_command` reads the running binary
pub const AGENT_CLI: &str = "/Users/test/.local/bin/bd";

/// The skill files the fake install names
const AGENT_SKILLS: [&str; 2] = ["/Users/test/.claude/skills/branch-deck/SKILL.md", "/Users/test/.agents/skills/branch-deck/SKILL.md"];

/// The agent CLI of a test repository: `bd install` without its files, which leaves the rules of `bd workspace` to the
/// global config of the repository, as the app reads the developer's; the command of a POSIX shell, with the repository
/// path as it is (test paths are plain)
#[derive(Default)]
pub struct TestAgentCli {
  installed: AtomicBool,
}

impl AgentCliHost for TestAgentCli {
  fn install(&self) -> Result<AgentCliInstalled, ApiError> {
    self.installed.store(true, Ordering::SeqCst);
    Ok(AgentCliInstalled {
      bd: AGENT_CLI.to_owned(),
      on_path: true,
      skills: AGENT_SKILLS.map(str::to_owned).to_vec(),
      agents: None,
    })
  }

  fn command(&self, repository_path: &str) -> Result<AgentCliCommand, ApiError> {
    Ok(AgentCliCommand {
      command: format!("{AGENT_CLI} -C {repository_path}"),
      powershell: false,
    })
  }

  /// The skills once `install_agent_cli` ran, else none
  fn status(&self) -> AgentCliStatus {
    if self.installed.load(Ordering::SeqCst) {
      AgentCliStatus {
        bd: Some(AGENT_CLI.to_owned()),
        skills: AGENT_SKILLS.map(str::to_owned).to_vec(),
      }
    } else {
      AgentCliStatus { bd: None, skills: Vec::new() }
    }
  }
}

/// What the desktop of the test repository `id` recorded: `{ opens, menus, pages }`, each open of a window
/// (`{ opener, page, windowId, url, title, data, store, storeVersion }`: the window and the page of it that asked), the
/// window of each window menu, in order, and the page each window shows now
pub async fn get_windows(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
  let repository = state.repositories.get(&id).ok_or(StatusCode::GONE)?;
  let windows = repository.desktop.windows();
  Ok(Json(json!({ "opens": windows.opens, "menus": windows.menus, "pages": windows.pages })))
}

/// The page a test opened for a window: `main`, or a sub-window from the snapshot of the settings at `storeVersion`
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuiltWindow {
  label: String,
  page: BuiltKind,
  #[serde(default)]
  store_version: u32,
  /// The open the page names (`__SUB_WINDOW_OPEN__`)
  open_id: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum BuiltKind {
  Main,
  Sub,
}

/// `POST /repositories/{id}/windows/built`: the window of a page a test opens is built, as the app builds it before its
/// page loads (`SubWindowState::built`): the load of a sub-window's page takes the changes of the settings after its
/// snapshot (`sub_window_loaded`), and its stages set the deadlines of the window, as in the app
pub async fn built_window(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(built): Json<BuiltWindow>) -> StatusCode {
  let Some(repository) = state.repositories.get(&id) else {
    return StatusCode::GONE;
  };
  let page = match built.page {
    BuiltKind::Main => BuiltPage::Main,
    BuiltKind::Sub => BuiltPage::Sub {
      settings_version: built.store_version,
    },
  };
  let open = SubWindowOpen {
    id: built.open_id,
    opened_at: 0.0,
  };
  let deadline = repository.context.sub_windows.built(&built.label, open, page);
  repository.desktop.windows().open.insert(built.label.clone());
  show_when_settled(&repository.context, built.label, deadline);
  StatusCode::OK
}

/// What the folder dialog of the test repository picks from now on: `path`, or nothing (`null`: the user cancels)
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogPick {
  path: Option<String>,
}

/// `POST /repositories/{id}/dialog-pick`
pub async fn set_dialog_pick(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(pick): Json<DialogPick>) -> StatusCode {
  let Some(repository) = state.repositories.get(&id) else {
    return StatusCode::GONE;
  };
  *repository.desktop.pick.lock().unwrap_or_else(PoisonError::into_inner) = pick.path.map_or(Pick::Canceled, Pick::Folder);
  StatusCode::OK
}
