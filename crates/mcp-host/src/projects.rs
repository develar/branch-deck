//! The repositories of the main view in the IDE (openspec/specs/repository/ide-embedding/spec.md, "Repositories"): the
//! Git roots of the project (`roots/list`) are the recent projects the views of the connection see (`recentProjects`),
//! the root the main view shows first, so the page selects it and its selector lists the roots, as it does the recent
//! projects in the desktop app. A pick of the page (its write of `recentProjects` with another root first) is
//! remembered by the set of the roots (`app_settings::ide_roots`), and the next start of the project shows it again;
//! what the page changes of a root (its collapsed cards, its issue links) goes into the recent projects of the desktop
//! app in their place, whose order and selection stay the desktop app's.

use serde_json::{Value, json};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The key of the recent projects in the settings
pub const RECENT_PROJECTS_KEY: &str = "recentProjects";

/// The roots of the project and the one the main view shows
#[derive(Default)]
pub struct Projects(Mutex<State>);

#[derive(Default)]
struct State {
  /// The Git roots of the project, as `roots/list` answered last
  roots: Vec<String>,
  /// The root the main view shows
  shown: Option<String>,
}

impl Projects {
  fn state(&self) -> MutexGuard<'_, State> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// The Git roots of the project
  pub fn roots(&self) -> Vec<String> {
    self.state().roots.clone()
  }

  /// The root the main view shows; none without a Git root
  pub fn shown(&self) -> Option<String> {
    self.state().shown.clone()
  }

  /// The IDE named `roots`, of which the user picked `remembered` last: the main view shows it, else the root it shows
  /// while that is still a root, else the first. Whether the roots or the root shown changed
  pub fn set_roots(&self, roots: Vec<String>, remembered: Option<String>) -> bool {
    let mut state = self.state();
    let kept = state.shown.clone().filter(|shown| roots.contains(shown));
    let shown = remembered.filter(|root| roots.contains(root)).or(kept).or_else(|| roots.first().cloned());
    let changed = state.roots != roots || state.shown != shown;
    state.roots = roots;
    state.shown = shown;
    changed
  }

  /// The page picked `path`: whether it is a root that the main view did not show, which it shows now
  pub fn pick(&self, path: &str) -> bool {
    let mut state = self.state();
    if state.shown.as_deref() == Some(path) || !state.roots.iter().any(|root| root == path) {
      return false;
    }
    state.shown = Some(path.to_owned());
    true
  }

  /// The recent projects as the views see them, of `recent`, the recent projects of the settings: the roots, the root
  /// shown first, each with what the recent projects keep of it
  pub fn seen(&self, recent: &Value) -> Value {
    let state = self.state();
    let ordered = state.shown.iter().chain(state.roots.iter().filter(|root| Some(*root) != state.shown.as_ref()));
    Value::Array(ordered.map(|root| project_of(recent, root).cloned().unwrap_or_else(|| json!({ "path": root }))).collect())
  }

  /// A change of the settings (`SettingsChange`, as its JSON) as the views see it: the recent projects of the roots
  pub fn seen_change(&self, change: &mut Value) {
    if change["key"] == RECENT_PROJECTS_KEY
      && let Some(value) = change.get_mut("value")
    {
      *value = self.seen(value);
    }
  }

  /// Every change of the settings in `value`, an answer of a command, as the views see it
  pub fn seen_changes(&self, value: &mut Value) {
    match value {
      Value::Object(object) if object.get("key").is_some_and(Value::is_string) && object.contains_key("version") => self.seen_change(value),
      Value::Object(object) => object.values_mut().for_each(|value| self.seen_changes(value)),
      Value::Array(items) => items.iter_mut().for_each(|value| self.seen_changes(value)),
      _ => {}
    }
  }
}

/// The project of `path` in the recent projects `recent`
fn project_of<'a>(recent: &'a Value, path: &str) -> Option<&'a Value> {
  recent.as_array()?.iter().find(|project| project["path"] == path)
}

/// The recent projects `recent` of the settings with `seen`, what the page wrote of its roots: each project of a root in
/// its place, a root the recent projects lack at their end; their order and every other project stay
pub fn merged(recent: &Value, seen: &Value) -> Value {
  let mut projects = recent.as_array().cloned().unwrap_or_default();
  for project in seen.as_array().into_iter().flatten() {
    let Some(path) = project["path"].as_str() else {
      continue;
    };
    match projects.iter_mut().find(|known| known["path"] == path) {
      Some(known) => known.clone_from(project),
      None => projects.push(project.clone()),
    }
  }
  Value::Array(projects)
}

#[cfg(test)]
#[path = "projects_test.rs"]
mod projects_test;
