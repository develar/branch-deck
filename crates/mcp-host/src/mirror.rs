//! The mirror of the view of a session (openspec/specs/repository/ide-embedding/spec.md, "View"): the state of a mirror
//! is the `RepositoryView` of the session, each top-level field one part, its `version` too, which the page compares
//! with the versions the commands answer (`SyncAnswer.version`, `Outcome.versionAfter`); a `ViewPatch` becomes one
//! `x-air-app/viewChanged`, its ops by the table rule of Air: a keyed part (`branches`, `stacks`, …) sets each changed
//! key and removes each removed one, any other part that changed is replaced whole, with its value in the view after the
//! patch (`repository_model::patch::apply_patch`, as the page's store applies it), and the part `version` last. The news are the part `news`, the last news of the session, which the page tells by
//! id, so a resync tells none twice.

use domain_types::session::{RepositoryView, TableChanges, ViewPatch};
use repository_model::patch::{PatchRejected, apply_patch};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The view of a mirror as brd keeps it, to which its patches apply
pub struct MirrorView {
  view: RepositoryView,
}

impl MirrorView {
  pub const fn new(view: RepositoryView) -> Self {
    Self { view }
  }

  /// The view as the patches so far made it
  pub const fn view(&self) -> &RepositoryView {
    &self.view
  }

  /// The state of the mirror: the view, its version with it
  pub fn state(&self) -> Value {
    serde_json::to_value(&self.view).unwrap_or_else(|_| json!({}))
  }

  /// Applies `patch` and gives the ops of its change, the version after it last; none when it changed no part
  ///
  /// # Errors
  /// The patch does not apply to the view (another epoch, a lost patch)
  pub fn apply(&mut self, patch: ViewPatch) -> Result<Vec<Value>, PatchRejected> {
    let changes = patch.changes.clone();
    apply_patch(&mut self.view, patch)?;
    let mut ops = ops(&self.view, &changes);
    if !ops.is_empty() {
      ops.push(json!({ "op": "part", "part": "version", "value": self.view.version }));
    }
    Ok(ops)
  }

  /// Applies `patch` when its base is the version of the view: the patches that came before the answer of the
  /// subscription and that its view holds already are left out
  pub fn catch_up(&mut self, patch: ViewPatch) {
    if patch.base == self.view.version {
      let _ = apply_patch(&mut self.view, patch);
    }
  }
}

/// The ops of `changes` over `view`, the view after them, in the order of the parts of the view, `order` after the keyed
/// parts whose keys it names
pub fn ops(view: &RepositoryView, changes: &TableChanges) -> Vec<Value> {
  let mut ops = Ops::default();
  ops.part(changes.cached.is_some(), "cached", &view.cached);
  ops.part(changes.sync.is_some(), "sync", &view.sync);
  ops.part(changes.header.is_some(), "header", &view.header);
  ops.keyed("branches", &view.branches, changes.branches.keys(), &changes.removed_branches);
  ops.keyed("stacks", &view.stacks, changes.stacks.keys(), &changes.removed_stacks);
  ops.keyed("pushes", &view.pushes, changes.pushes.keys(), &changes.removed_pushes);
  ops.keyed(
    "openPullRequests",
    &view.open_pull_requests,
    changes.open_pull_requests.keys(),
    &changes.removed_open_pull_requests,
  );
  ops.keyed("lands", &view.lands, changes.lands.keys(), &changes.removed_lands);
  ops.keyed("landedStates", &view.landed_states, changes.landed_states.keys(), &changes.removed_landed_states);
  ops.keyed("nextSteps", &view.next_steps, changes.next_steps.keys(), &changes.removed_next_steps);
  ops.keyed("actions", &view.actions, changes.actions.keys(), &changes.removed_actions);
  ops.keyed("attention", &view.attention, changes.attention.keys(), &changes.removed_attention);
  ops.keyed("githubLayers", &view.github_layers, changes.github_layers.keys(), &changes.removed_github_layers);
  ops.keyed("reviewGroups", &view.review_groups, changes.review_groups.keys(), &changes.removed_review_groups);
  ops.keyed("operations", &view.operations, changes.operations.keys(), &changes.removed_operations);
  ops.part(changes.order.is_some(), "order", &view.order);
  ops.part(changes.undo.is_some(), "undo", &view.undo);
  let sections = changes.unassigned.is_some() || changes.archived.is_some() || changes.unapplied.is_some() || changes.landed.is_some();
  ops.part(sections, "sections", &view.sections);
  ops.part(changes.reviews.is_some(), "reviews", &view.reviews);
  ops.part(changes.patronus.is_some(), "patronus", &view.patronus);
  ops.part(changes.route.is_some(), "route", &view.route);
  ops.part(changes.facts.is_some(), "facts", &view.facts);
  ops.part(!changes.news.is_empty(), "news", &view.news);
  ops.0
}

#[derive(Default)]
struct Ops(Vec<Value>);

impl Ops {
  /// Replaces the part `part` with `value` when it `changed`
  fn part(&mut self, changed: bool, part: &str, value: &impl Serialize) {
    if changed {
      self.0.push(json!({ "op": "part", "part": part, "value": value }));
    }
  }

  /// Sets each key of `set` to its value in `entries`, and removes each of `removed`
  fn keyed<'a, K: Serialize + Ord + 'a, V: Serialize>(&mut self, part: &str, entries: &BTreeMap<K, V>, set: impl Iterator<Item = &'a K>, removed: &[K]) {
    for key in set {
      if let Some(value) = entries.get(key) {
        self.0.push(json!({ "op": "set", "part": part, "key": key_of(key), "value": value }));
      }
    }
    for key in removed {
      self.0.push(json!({ "op": "remove", "part": part, "key": key_of(key) }));
    }
  }
}

/// A key of a part as the JSON of the view names it: a string, an operation id as its text
fn key_of(key: &impl Serialize) -> String {
  match serde_json::to_value(key) {
    Ok(Value::String(key)) => key,
    Ok(other) => other.to_string(),
    Err(_) => String::new(),
  }
}

#[cfg(test)]
#[path = "mirror_test.rs"]
mod mirror_test;
