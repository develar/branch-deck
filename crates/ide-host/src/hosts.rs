//! The windows of the bridge in an IDE (openspec/specs/repository/ide-embedding/spec.md, "Windows"): a window of the
//! desktop app is an instance of a view of the IDE (`Instances`): the main window the instance of `main`, a sub-window
//! the instance its `x-air-app/openView` answered, its label in the backend the instance itself (`review:3`), so two
//! review tabs are two windows of the connection of the bridge. An open of a sub-window, the host request
//! `x-branch-deck/openWindow`, is `x-air-app/openView` of the view its URL names (`open_view_of`).

use app_settings::windows::MAIN_WINDOW;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// The view of the main window
pub const MAIN_VIEW: &str = "main";

/// The instances of the views the IDE shows, as windows of the app
#[derive(Default)]
pub struct Instances(Mutex<InstancesState>);

#[derive(Default)]
struct InstancesState {
  /// The params of each instance shown: those of its `viewShown`, else those brd opened it with
  params: HashMap<String, Value>,
  /// The instance of each sub-window brd opened, by its window id
  opened: HashMap<String, String>,
  /// The instances whose place the IDE hides (`viewHidden`), until it shows them again
  hidden: HashSet<String>,
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
    let mut state = self.state();
    state.hidden.remove(instance);
    state.params.insert(instance.to_owned(), params);
  }

  /// The IDE hides the place of `instance` (another tab, a hidden tool window); it stays open
  pub fn hidden(&self, instance: &str) {
    self.state().hidden.insert(instance.to_owned());
  }

  /// The place of `instance` closed
  pub fn closed(&self, instance: &str) {
    let mut state = self.state();
    state.params.remove(instance);
    state.hidden.remove(instance);
    state.opened.retain(|_, opened| opened != instance);
  }

  /// The IDE shows the main view: open, and not hidden
  pub fn main_shown(&self) -> bool {
    let state = self.state();
    state.params.keys().any(|instance| view_of(instance) == MAIN_VIEW && !state.hidden.contains(instance))
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
  pub fn opened(&self, window_id: &str, instance: &str, params: Value) -> bool {
    let mut state = self.state();
    let before = state.opened.insert(window_id.to_owned(), instance.to_owned());
    state.params.insert(instance.to_owned(), params);
    before.as_deref() == Some(instance)
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

/// The open of a sub-window, the params of `x-branch-deck/openWindow` (`OpenSubWindowParams` with its opener), as the
/// IDE takes it
pub struct OpenView {
  /// The request `x-air-app/openView`: the view of the URL, its key, its title and its data, the instance of the opener
  pub request: Value,
  /// The params of the view (`{data}`), which the instance the IDE answers is shown with
  pub params: Value,
  /// The data of the open, which a view shown already takes (`init-data-updated`)
  pub data: Value,
  pub window_id: String,
  pub title: String,
}

/// The open of the params `params` of `x-branch-deck/openWindow`, for `opener`, the instance of the window that asks
pub fn open_view_of(params: &Value, opener: Option<String>) -> OpenView {
  let data: Value = params["data"].as_str().and_then(|data| serde_json::from_str(data).ok()).unwrap_or(Value::Null);
  let view = view_of_url(params["url"].as_str().unwrap_or_default());
  let window_id = params["windowId"].as_str().unwrap_or_default().to_owned();
  let title = params["title"].as_str().unwrap_or_default().to_owned();
  let view_params = json!({ "data": data });
  let mut request = json!({ "view": view, "key": key_of(&view, &window_id, &data), "title": title, "params": view_params });
  if let Some(opener) = opener {
    request["opener"] = Value::String(opener);
  }
  OpenView {
    request,
    params: view_params,
    data,
    window_id,
    title,
  }
}

#[cfg(test)]
#[path = "hosts_test.rs"]
mod hosts_test;
