//! The hosts of app-api in an IDE (openspec/specs/repository/ide-embedding/spec.md, "Events", "Windows", "Hosts"): a
//! window of the desktop app is an instance of a view of the IDE (`Instances`): the main window the instance of `main`,
//! a sub-window the instance its `x-air-app/openView` answered, its label the instance itself (`review:3`), so two
//! review tabs are two windows. An event of the app is `x-air-app/event` (`McpEvents`), an open of a sub-window
//! `x-air-app/openView` (`McpWindows`). The menu, the updater, the restart and the agent CLI do nothing, and no folder
//! dialog is offered: the roots of the project name the repositories.

use crate::link::Link;
use crate::projects::Projects;
use app_api::app::agent_cli::{AgentCliCommand, AgentCliInstalled, AgentCliStatus, IdeApp};
use app_api::app::windows::{OpenSubWindowParams, Shown};
use app_api::events::SETTINGS_CHANGED_EVENT;
use app_api::transport::{AgentCliHost, AppEvents, Desktop, DialogHost, Later, MenuHost, RestartHost, Updater, WindowHost, WindowId};
use app_settings::windows::MAIN_WINDOW;
use domain_types::error::ApiError;
use domain_types::update::{UpdateInfo, UpdateStatus};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::Instrument;

/// The view of the main window
pub const MAIN_VIEW: &str = "main";

/// Why a host of the desktop does nothing in the IDE
const IN_THE_IDE: &str = "Branch Deck in the IDE updates, restarts and installs with the desktop app";

/// The instances of the views the IDE shows, as windows of the app
#[derive(Default)]
pub struct Instances(Mutex<InstancesState>);

#[derive(Default)]
struct InstancesState {
  /// The params of each instance shown: those of its `viewShown`, else those brd opened it with
  params: HashMap<String, Value>,
  /// The instance of each sub-window brd opened, by its window id
  opened: HashMap<String, String>,
}

/// The view of the instance `instance` (`review` of `review:3`)
pub fn view_of(instance: &str) -> &str {
  instance.split_once(':').map_or(instance, |(view, _)| view)
}

/// The window of `instance`: the main window for the view `main`, else the instance itself
pub fn label_of(instance: &str) -> String {
  if view_of(instance) == MAIN_VIEW { MAIN_WINDOW.to_owned() } else { instance.to_owned() }
}

impl Instances {
  fn state(&self) -> std::sync::MutexGuard<'_, InstancesState> {
    self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  /// The IDE shows `instance` with `params`
  pub fn shown(&self, instance: &str, params: Value) {
    self.state().params.insert(instance.to_owned(), params);
  }

  /// The place of `instance` closed
  pub fn closed(&self, instance: &str) {
    let mut state = self.state();
    state.params.remove(instance);
    state.opened.retain(|_, opened| opened != instance);
  }

  /// The params `instance` is shown with
  pub fn params(&self, instance: &str) -> Option<Value> {
    self.state().params.get(instance).cloned()
  }

  /// The instance of the window `label`: the instance of `main` for the main window, the instance of a sub-window brd
  /// opened by its window id, or the label itself, which is an instance
  pub fn instance_of(&self, label: &str) -> Option<String> {
    let state = self.state();
    if label == MAIN_WINDOW {
      return state.params.keys().find(|instance| view_of(instance) == MAIN_VIEW).cloned();
    }
    if let Some(instance) = state.opened.get(label) {
      return Some(instance.clone());
    }
    state.params.contains_key(label).then(|| label.to_owned())
  }

  /// The IDE answered an open of the window `window_id` with `instance`: whether it is the instance that window showed
  /// already, which then takes `params` as its own
  fn opened(&self, window_id: &str, instance: &str, params: Value) -> bool {
    let mut state = self.state();
    let before = state.opened.insert(window_id.to_owned(), instance.to_owned());
    state.params.insert(instance.to_owned(), params);
    before.as_deref() == Some(instance)
  }
}

/// The events of the app as `x-air-app/event`: to every view, or to the instance of a window; a change of the recent
/// projects as the views see them (`Projects::seen_change`)
pub struct McpEvents {
  pub link: Arc<Link>,
  pub instances: Arc<Instances>,
  pub projects: Arc<Projects>,
}

impl McpEvents {
  fn seen(&self, event: &str, mut payload: Value) -> Value {
    if event == SETTINGS_CHANGED_EVENT {
      self.projects.seen_change(&mut payload);
    }
    payload
  }
}

impl AppEvents for McpEvents {
  fn emit(&self, event: &str, payload: Value) {
    self.link.notify("x-air-app/event", json!({ "event": event, "payload": self.seen(event, payload) }));
  }

  fn emit_to(&self, window: &WindowId, event: &str, payload: Value) {
    // a window that is not open hears nothing, as in Tauri
    if let Some(instance) = self.instances.instance_of(window.as_str()) {
      self
        .link
        .notify("x-air-app/event", json!({ "instance": instance, "event": event, "payload": self.seen(event, payload) }));
    }
  }
}

/// The sub-windows of the app as views of the IDE: an open is `x-air-app/openView` of the view its URL names, the review
/// keyed by its target, so one target is one editor tab; the IDE shows a view itself
pub struct McpWindows {
  pub link: Arc<Link>,
  pub instances: Arc<Instances>,
}

impl McpWindows {
  /// Asks the IDE to open the view of `params`, for `opener`; once it answered, a view it showed already takes the new
  /// data (`init-data-updated`) and its title
  fn open(&self, opener: &WindowId, params: &OpenSubWindowParams) {
    let data: Value = serde_json::from_str(&params.data).unwrap_or(Value::Null);
    let view = view_of_url(&params.url);
    let view_params = json!({ "data": data });
    let mut request = json!({ "view": view, "key": key_of(&view, &params.window_id, &data), "title": params.title, "params": view_params });
    if let Some(opener) = self.instances.instance_of(opener.as_str()) {
      request["opener"] = Value::String(opener);
    }
    let answer = self.link.request("x-air-app/openView", Some(request));
    let (link, instances, window_id, title) = (Arc::clone(&self.link), Arc::clone(&self.instances), params.window_id.clone(), params.title.clone());
    tokio::spawn(
      async move {
        let instance = match answer.await {
          Ok(Ok(result)) => result["instance"].as_str().map(str::to_owned),
          Ok(Err(error)) => {
            tracing::warn!(%error, view, "the IDE opens no view");
            None
          }
          Err(_) => None,
        };
        let Some(instance) = instance else {
          return;
        };
        if instances.opened(&window_id, &instance, view_params) {
          link.notify(
            "x-air-app/event",
            json!({ "instance": instance, "event": "init-data-updated", "payload": { "data": data } }),
          );
          link.ask("x-air-app/setViewTitle", json!({ "instance": instance, "title": title }));
        }
      }
      .in_current_span(),
    );
  }
}

impl WindowHost for McpWindows {
  fn is_open(&self, label: &str) -> bool {
    self.instances.instance_of(label).is_some()
  }

  fn reopen(&self, opener: &WindowId, params: &OpenSubWindowParams, _update: Option<&str>, _front: bool) -> Result<(), ApiError> {
    self.open(opener, params);
    Ok(())
  }

  fn build(&self, opener: &WindowId, params: &OpenSubWindowParams, _page_script: &str, _zoom: f64) -> Result<(), ApiError> {
    self.open(opener, params);
    Ok(())
  }

  /// The IDE shows the place of a view itself
  fn show(&self, _label: &str, _shown: Shown) -> Later<'_, ()> {
    Box::pin(async {})
  }
}

/// The view of the URL of a sub-window (`/review`, `/settings`)
pub fn view_of_url(url: &str) -> String {
  let path = url.split(['?', '#']).next().unwrap_or_default();
  path.trim_start_matches('/').trim_end_matches(".html").to_owned()
}

/// The key of a view: a review by its target (the branch, the pull request, the commit), so the IDE shows one tab per
/// target; any other view by its window id
pub fn key_of(view: &str, window_id: &str, data: &Value) -> String {
  if view != "review" {
    return window_id.to_owned();
  }
  let kind = data["kind"].as_str().unwrap_or("review");
  let target = ["branchName", "number", "commitId"].iter().find_map(|field| match &data[*field] {
    Value::String(text) => Some(text.clone()),
    Value::Number(number) => Some(number.to_string()),
    _ => None,
  });
  target.map_or_else(|| window_id.to_owned(), |target| format!("{kind}:{target}"))
}

/// The desktop of the app in the IDE: its windows are views; the dialog, the menu, the restart, the updater and the
/// agent CLI do nothing
pub fn desktop(link: &Arc<Link>, instances: &Arc<Instances>) -> Desktop {
  let nothing = Arc::new(Nothing);
  Desktop {
    windows: Arc::new(McpWindows {
      link: Arc::clone(link),
      instances: Arc::clone(instances),
    }),
    dialog: Arc::clone(&nothing) as Arc<dyn DialogHost>,
    menu: Arc::clone(&nothing) as Arc<dyn MenuHost>,
    restart: Arc::clone(&nothing) as Arc<dyn RestartHost>,
    updater: Arc::clone(&nothing) as Arc<dyn Updater>,
    agent_cli: nothing,
  }
}

/// A host of the desktop that has no place in the IDE
struct Nothing;

impl DialogHost for Nothing {
  /// No folder dialog: the roots of the project name the repositories
  fn pick_folder(&self, _title: &str) -> Option<String> {
    None
  }
}

impl MenuHost for Nothing {
  fn set_auto_sync_checked(&self, _checked: bool) -> Result<(), ApiError> {
    Ok(())
  }

  fn show_window_menu(&self, _window: &WindowId) -> Result<(), ApiError> {
    Ok(())
  }
}

impl RestartHost for Nothing {
  fn restart(&self) -> ApiError {
    ApiError::internal(IN_THE_IDE)
  }
}

/// No update: the app updates with the desktop app
impl Updater for Nothing {
  fn check(&self) -> Later<'_, Result<UpdateInfo, ApiError>> {
    Box::pin(async { Ok(up_to_date()) })
  }

  fn check_failure(&self) -> Option<String> {
    None
  }

  fn status(&self) -> Result<UpdateInfo, ApiError> {
    Ok(up_to_date())
  }

  fn install(&self) -> Later<'_, Result<(), ApiError>> {
    Box::pin(async { Err(ApiError::internal(IN_THE_IDE)) })
  }

  fn downloaded(&self) -> Option<String> {
    None
  }
}

fn up_to_date() -> UpdateInfo {
  UpdateInfo {
    current_version: env!("CARGO_PKG_VERSION").to_owned(),
    available_version: String::new(),
    is_update_available: false,
    status: UpdateStatus::Idle,
  }
}

/// The agent CLI and the app file of the IDE are installed from the desktop app
impl AgentCliHost for Nothing {
  fn install(&self) -> Result<AgentCliInstalled, ApiError> {
    Err(ApiError::internal(IN_THE_IDE))
  }

  fn command(&self, _repository_path: &str) -> Result<AgentCliCommand, ApiError> {
    Err(ApiError::internal(IN_THE_IDE))
  }

  /// Nothing to install here: the page offers no install of `brd` or of the app file of the IDE
  fn status(&self) -> AgentCliStatus {
    AgentCliStatus {
      bd: None,
      skills: Vec::new(),
      ide_app: None,
      installable: false,
    }
  }

  fn install_ide_app(&self) -> Result<IdeApp, ApiError> {
    Err(ApiError::internal(IN_THE_IDE))
  }

  fn remove_ide_app(&self) -> Result<(), ApiError> {
    Err(ApiError::internal(IN_THE_IDE))
  }
}

#[cfg(test)]
#[path = "hosts_test.rs"]
mod hosts_test;
