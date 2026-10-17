//! An inverted stack (`bd check`): a branch conflicts, or has a dangling reference, because it needs a commit of a
//! branch stacked on it. Two fixes are tried, each by a trial build of the plan with it, and only those that build
//! both branches are offered: the commits of the parent that need the child move into the child, or the parent stacks
//! on the child.

use crate::check::{BranchCheck, CheckOutcome, CheckReport, CheckSession};
use crate::dangling_fix::plan_path;
use crate::missing_commits::missing_commits;
use crate::regroup::RegroupPlan;
use std::collections::{BTreeMap, HashMap, HashSet};

/// How a fix of an inverted stack regroups
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvertedStackFixKind {
  /// The commits of the parent that need the child move into it
  MoveToChild,
  /// The parent stacks on the child
  Swap,
}

/// A fix of an inverted stack: what to add to the plan of `bd group`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvertedStackFix {
  pub kind: InvertedStackFixKind,
  /// Plan path -> the commits (full ids) it gets
  pub branches: BTreeMap<String, Vec<String>>,
  pub unstack: Vec<String>,
}

/// A branch that needs a commit of a branch stacked on it, with the fixes a trial build passes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvertedStack {
  /// The parent
  pub branch: String,
  /// The branch stacked on it that has the commit it needs
  pub child: String,
  pub fixes: Vec<InvertedStackFix>,
  /// The change in the child uses what the parent adds: the child can't be built without it, which the trial build
  /// doesn't see, so no swap is tried
  pub mutual: bool,
}

/// Whether `branch` is built on `ancestor`, directly or through its parents in `report`
fn is_built_on(report: &CheckReport, branch: &BranchCheck, ancestor: &str) -> bool {
  let mut current = branch.built_on.as_deref();
  for _ in 0..report.branches.len() {
    let Some(name) = current else { return false };
    if name == ancestor {
      return true;
    }
    current = report.branches.iter().find(|check| check.name == name).and_then(|check| check.built_on.as_deref());
  }
  false
}

/// `branch` of `report`, a check of `plan` in `session`, conflicts, or has a dangling reference, because it needs a
/// commit of a branch stacked on it: the fixes a trial build of `plan` with them passes; `None` for no such case.
/// `files_of` holds the changed files of each commit (`missing_commits::changed_files`)
pub fn inverted_stack(
  session: &CheckSession,
  plan: &RegroupPlan,
  report: &CheckReport,
  branch: &BranchCheck,
  files_of: &HashMap<String, HashSet<String>>,
) -> Option<InvertedStack> {
  // the commits of the branch that need something, and what they need
  let (mut own, needed): (Vec<String>, Vec<String>) = match &branch.outcome {
    CheckOutcome::Conflict { commit, files } => (vec![commit.clone()], missing_commits(report, branch, commit, files, files_of)),
    CheckOutcome::Ok { .. } => report
      .dangling
      .get(&branch.name)
      .into_iter()
      .flatten()
      .filter(|reference| reference.is_confirmed())
      .filter_map(|reference| Some((reference.removed_by.clone(), reference.fixed_by.clone()?)))
      .unzip(),
    _ => return None,
  };
  let group_of: HashMap<&str, Option<&str>> = report.regroup.commits.iter().map(|commit| (commit.id.as_str(), commit.group.as_deref())).collect();
  let child = needed
    .iter()
    .filter_map(|id| group_of.get(id.as_str()).copied().flatten())
    .filter_map(|group| report.branches.iter().find(|check| check.name == group))
    .find(|check| is_built_on(report, check, &branch.name))?;

  let mutual = report
    .dangling
    .get(&branch.name)
    .into_iter()
    .flatten()
    .any(|reference| reference.is_confirmed() && reference.mutual);
  own.sort();
  own.dedup();
  let candidates = [
    InvertedStackFix {
      kind: InvertedStackFixKind::MoveToChild,
      branches: BTreeMap::from([(plan_path(child), own)]),
      unstack: Vec::new(),
    },
    InvertedStackFix {
      kind: InvertedStackFixKind::Swap,
      branches: BTreeMap::from([(format!("{}/{}", child.name, branch.name), Vec::new())]),
      unstack: vec![child.name.clone()],
    },
  ];
  let fixes = candidates
    .into_iter()
    .filter(|fix| !(mutual && fix.kind == InvertedStackFixKind::Swap))
    .filter(|fix| {
      let mut fixed = plan.clone();
      for (path, commits) in &fix.branches {
        fixed.branches.entry(path.clone()).or_default().extend(commits.iter().cloned());
      }
      fixed.unstack.extend(fix.unstack.iter().cloned());
      // a fix the regroup refuses, or one after which either branch still fails, is no fix
      session
        .check_with_dangling_of(Some(&fixed), &[branch.name.clone(), child.name.clone()])
        .is_ok_and(|trial| trial.builds_cleanly(&branch.name) && trial.builds_cleanly(&child.name))
    })
    .collect();
  Some(InvertedStack {
    branch: branch.name.clone(),
    child: child.name.clone(),
    fixes,
    mutual,
  })
}
