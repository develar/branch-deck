//! The fix of a mutual dangling reference (`dangling_references.rs`): the change of the use moves into the branch, and
//! it is offered only when a trial build of a check session proves it (`brd check`, the refusal of `brd safe-push`).
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
use domain_types::rewrite::RegroupPlan;
use git_ops::move_hunk::MoveHunkMode;
use object_cache::cache::ObjectCache;
use std::collections::{HashMap, HashSet};
use tracing::debug;

/// A fix of a mutual dangling reference of a branch that a trial build proved
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvenFix {
  /// `commit`, the `fixed_by` of the reference, changes only the file of the use: it moves into the branch, at `path` of
  /// the plan (`parent/leaf` when the branch is stacked)
  MoveCommit { path: String, commit: String },
  /// The change of `file` in `from` (`fixed_by`) moves into `to` (`removed_by`), as `brd move-hunk <from> <to> <file>`
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
    let trial = hunk_trial(self.session, self.plan, self.report, fixed_by, &reference.removed_by, &reference.file)?;
    proves(self.report, &trial.report, &branch.name).then(|| ProvenFix::MoveHunk {
      from: fixed_by.to_string(),
      to: reference.removed_by.clone(),
      file: reference.file.clone(),
    })
  }
}

/// A check of the history after the change of `file` in `from` moved into `to` (`MoveHunkMode::Trial`, written as
/// objects only, HEAD's tree kept)
pub(crate) struct HunkTrial {
  /// The session on the rewritten history
  pub session: CheckSession,
  /// Old id -> new id of each rewritten commit, oldest first
  pub commits: Vec<(String, String)>,
  /// The check of `plan` with the rewritten ids
  pub report: CheckReport,
}

/// The trial of moving the change of `file` in `from` into `to` of `session`, whose check of `plan` is `report`; `None`
/// when the move is refused or its check fails
pub(crate) fn hunk_trial(session: &CheckSession, plan: &RegroupPlan, report: &CheckReport, from: &str, to: &str, file: &str) -> Option<HunkTrial> {
  let params = MoveHunkParams {
    from: from.to_string(),
    to: to.to_string(),
    path: file.to_string(),
    blocks: Vec::new(),
  };
  let options = MoveHunkOptions {
    expect_head: Some(report.head.clone()),
    mode: MoveHunkMode::Trial,
    ..MoveHunkOptions::default()
  };
  let moved = move_hunk_core(session.git_executor(), session.repository_path(), &ObjectCache::disabled(), &params, &options)
    .inspect_err(|error| debug!(%error, "the trial of moving the hunk failed"))
    .ok()?;
  let rewritten = session.with_rewrite(&moved.commits, &moved.trees);
  let plan = session.rewritten_plan(plan, &moved.commits);
  let report = rewritten
    .check(Some(&plan))
    .inspect_err(|error| debug!(%error, "the trial build of the moved hunk failed"))
    .ok()?;
  Some(HunkTrial {
    session: rewritten,
    commits: moved.commits,
    report,
  })
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
