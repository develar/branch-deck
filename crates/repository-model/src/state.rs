//! What a session sent its windows of the table (docs/design/rust-domain-authority.md, "Представление: снимок и
//! патчи"): `View` keeps the view as it was sent last, at its version. `diff` compares the table state with it part by
//! part (`PartialEq`) and answers the patch of the parts that differ, cloning only those; `snapshot` is the view at the
//! version of the last patch, for a window that opens or lost a patch. Rust decides equality, so the page keeps the
//! identity of every part a patch does not carry.

use crate::patch::{apply_patch, branch_change, changed, changed_commits, diff_keyed};
use crate::table::TableState;
use domain_types::session::{ArchivedChanges, TableChanges, TableView, Version, ViewPatch};
use std::collections::BTreeMap;
use std::hash::{BuildHasher, RandomState};

/// The view of a session as its windows hold it
#[derive(Clone, Debug)]
pub struct View {
  last_sent: TableView,
}

impl View {
  /// An empty view at the start of `epoch`; the first `diff` sends everything the table has
  #[must_use]
  pub fn new(epoch: u32) -> Self {
    Self {
      last_sent: TableView {
        version: Version { epoch, seq: 0 },
        ..TableView::default()
      },
    }
  }

  #[must_use]
  pub const fn version(&self) -> Version {
    self.last_sent.version
  }

  /// The view at the version of the last patch
  #[must_use]
  pub fn snapshot(&self) -> TableView {
    self.last_sent.clone()
  }

  /// The patch from the view sent last to `state`, and the view moves to it; `None` when no part differs
  pub fn diff(&mut self, state: &TableState) -> Option<ViewPatch> {
    let last = &self.last_sent;
    let parts = &state.parts;
    let mut branches = BTreeMap::new();
    for (name, branch) in &parts.branches {
      if let Some(change) = branch_change(last.branches.get(name), branch) {
        branches.insert(name.clone(), change);
      }
    }
    let removed_branches = last.branches.keys().filter(|name| !parts.branches.contains_key(*name)).cloned().collect();
    let (stacks, removed_stacks) = diff_keyed(&last.stacks, &parts.stacks);
    let (last_archived, archived) = (&last.sections.archived, &parts.sections.archived);
    let (integration, removed_integration) = diff_keyed(&last_archived.integration, &archived.integration);
    let archived_changes = ArchivedChanges {
      names: changed(&last_archived.names, &archived.names),
      newly_archived: changed(&last_archived.newly_archived, &archived.newly_archived),
      integration,
      removed_integration,
    };
    let changes = TableChanges {
      sync: changed(&last.sync, &state.sync),
      header: changed(&last.header, &parts.header),
      order: changed(&last.order, &parts.order),
      branches,
      removed_branches,
      stacks,
      removed_stacks,
      unassigned: changed_commits(&last.sections.unassigned, &parts.sections.unassigned),
      archived: (archived_changes != ArchivedChanges::default()).then_some(archived_changes),
      unapplied: changed(&last.sections.unapplied, &parts.sections.unapplied),
      landed: changed(&last.sections.landed, &parts.sections.landed),
    };
    if changes.is_empty() {
      return None;
    }
    let patch = ViewPatch {
      base: self.last_sent.version,
      next: self.last_sent.version.next(),
      changes,
    };
    // the view moves as a window's does: by the patch, so only the parts that changed are copied
    if let Err(rejected) = apply_patch(&mut self.last_sent, patch.clone()) {
      unreachable!("a patch from the view applies to it: {rejected}");
    }
    Some(patch)
  }
}

/// A random epoch of a session: random per process and per call
#[must_use]
pub fn new_epoch() -> u32 {
  // each `RandomState` has its own random keys; the hash of nothing is as random as they are
  let hash = RandomState::new().hash_one(());
  u32::try_from(hash >> 32).unwrap_or_default()
}

#[cfg(test)]
#[path = "state_test.rs"]
mod state_test;
