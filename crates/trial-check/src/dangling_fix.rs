//! The fix of a mutual dangling reference (`dangling_references.rs`): the change of the use moves into the branch, and
//! it is offered only when a trial build of a check session proves it (`bd check`, the refusal of `bd safe-push`).
//!
//! - **Move the commit**: `fixed_by` changes only the file of the use, so the plan with it moved into the branch is
//!   checked.
//! - **Move the hunk**: else its change of that file moves into `removed_by`, written as objects only
//!   (`MoveHunkMode::Trial`), and the history after it is checked. A hunk move costs the rewrite of the commits above
//!   the older of the two plus a check of every branch, so a prover tries at most `MAX_HUNK_TRIALS`.
//! - **Proven** when the branch builds without a proven dangling reference afterwards and no branch that built breaks.

use crate::check::{BranchCheck, CheckReport, CheckSession};
use crate::dangling_references::DanglingReference;
use crate::move_hunk::{MoveHunkOptions, MoveHunkParams, move_hunk_core};
use git_ops::move_hunk::MoveHunkMode;
use history_rewrite::regroup::RegroupPlan;
use object_cache::cache::ObjectCache;
use std::collections::{HashMap, HashSet};
use tracing::debug;

/// A fix of a mutual dangling reference of a branch that a trial build proved
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvenFix {
  /// `commit`, the `fixed_by` of the reference, changes only the file of the use: it moves into the branch, at `path` of
  /// the plan (`parent/leaf` when the branch is stacked)
  MoveCommit { path: String, commit: String },
  /// The change of `file` in `from` (`fixed_by`) moves into `to` (`removed_by`), as `bd move-hunk <from> <to> <file>`
  /// moves it: the whole change of the file
  MoveHunk { from: String, to: String, file: String },
}

/// How many hunk moves one prover tries: each writes the rewritten commits and builds every branch on them
const MAX_HUNK_TRIALS: usize = 2;

/// Proves the fixes of the mutual dangling references of `report`, a check of `plan` in `session`, by trial builds of
/// the plan with them
pub struct DanglingFixProver<'a> {
  session: &'a CheckSession,
  plan: &'a RegroupPlan,
  report: &'a CheckReport,
  /// The changed files of each commit (`missing_commits::changed_files`)
  files_of: &'a HashMap<String, HashSet<String>>,
  hunk_trials: usize,
}

impl<'a> DanglingFixProver<'a> {
  pub const fn new(session: &'a CheckSession, plan: &'a RegroupPlan, report: &'a CheckReport, files_of: &'a HashMap<String, HashSet<String>>) -> Self {
    Self {
      session,
      plan,
      report,
      files_of,
      hunk_trials: 0,
    }
  }

  /// The proven fix of `reference`, a proven mutual dangling reference of `branch`; `None` when neither move builds, or
  /// the hunk moves of this prover are used up
  pub fn prove(&mut self, branch: &BranchCheck, reference: &DanglingReference) -> Option<ProvenFix> {
    let fixed_by = reference.fixed_by.as_deref()?;
    let only_the_file = self.files_of.get(fixed_by).is_some_and(|files| files.iter().all(|path| *path == reference.file));
    if only_the_file {
      let path = plan_path(branch);
      let mut plan = self.plan.clone();
      // the commit leaves the branch the plan gives it
      for ids in plan.branches.values_mut() {
        ids.retain(|id| !fixed_by.starts_with(id.as_str()));
      }
      plan.unassign.retain(|id| !fixed_by.starts_with(id.as_str()));
      plan.branches.entry(path.clone()).or_default().push(fixed_by.to_string());
      let trial = self
        .session
        .check(Some(&plan))
        .inspect_err(|error| debug!(%error, "the trial of moving the commit failed"))
        .ok()?;
      return proves(self.report, &trial, &branch.name).then(|| ProvenFix::MoveCommit {
        path,
        commit: fixed_by.to_string(),
      });
    }

    if self.hunk_trials == MAX_HUNK_TRIALS {
      return None;
    }
    self.hunk_trials += 1;
    let params = MoveHunkParams {
      from: fixed_by.to_string(),
      to: reference.removed_by.clone(),
      path: reference.file.clone(),
      blocks: Vec::new(),
    };
    let options = MoveHunkOptions {
      expect_head: Some(self.report.head.clone()),
      mode: MoveHunkMode::Trial,
      ..MoveHunkOptions::default()
    };
    let moved = move_hunk_core(self.session.git_executor(), self.session.repository_path(), &ObjectCache::disabled(), &params, &options)
      .inspect_err(|error| debug!(%error, "the trial of moving the hunk failed"))
      .ok()?;
    let session = self.session.with_rewrite(&moved.commits, &moved.trees);
    let plan = self.session.rewritten_plan(self.plan, &moved.commits);
    let trial = session
      .check(Some(&plan))
      .inspect_err(|error| debug!(%error, "the trial build of the moved hunk failed"))
      .ok()?;
    proves(self.report, &trial, &branch.name).then(|| ProvenFix::MoveHunk {
      from: fixed_by.to_string(),
      to: reference.removed_by.clone(),
      file: reference.file.clone(),
    })
  }
}

/// Whether `trial` proves a fix of `branch`, which fails in `report`: it builds, and every branch that built still does
/// (or has no commits left)
fn proves(report: &CheckReport, trial: &CheckReport, branch: &str) -> bool {
  trial.builds_cleanly(branch)
    && report
      .branches
      .iter()
      .filter(|check| report.builds_cleanly(&check.name))
      .all(|check| trial.branches.iter().all(|built| built.name != check.name) || trial.builds_cleanly(&check.name))
}

/// The path of `branch` in a plan: `parent/leaf` when it is stacked
pub fn plan_path(branch: &BranchCheck) -> String {
  branch
    .declared_parent
    .as_ref()
    .map_or_else(|| branch.name.clone(), |parent| format!("{parent}/{}", branch.name))
}
