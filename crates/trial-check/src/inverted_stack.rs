//! An inverted stack (`brd check`): a branch conflicts, has a dangling reference or a missing declaration, because it
//! needs a commit of a branch stacked on it. The fixes are tried, each by a trial build of the plan with it, and only
//! those that build both branches are offered: the commits of the parent that need the child move into the child, or
//! the parent stacks on the child; for a missing declaration also the child's commit moves into the parent where it
//! changes only the file the use needs, else that change of the file moves into the parent's commit (`brd move-hunk`).
//! A fix of a missing declaration is proven when the trial has none of the links it was for and no other that the
//! check didn't have (`missing_declarations`).

use crate::check::{BranchCheck, CheckOutcome, CheckReport, CheckSession};
use crate::dangling_fix::{hunk_trial, plan_path};
use crate::missing_commits::missing_commits;
use crate::missing_declarations::{MissingDeclaration, missing_declarations};
use domain_types::rewrite::RegroupPlan;
use std::collections::{BTreeMap, HashMap, HashSet};

/// How a fix of an inverted stack regroups
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvertedStackFixKind {
  /// The commits of the parent that need the child move into it
  MoveToChild,
  /// The parent stacks on the child
  Swap,
  /// The child's commit that the parent needs changes only the file of the need: it moves into the parent
  MoveToParent,
  /// The change of the file of the need in the child's commit moves into the parent's commit that needs it
  /// (`brd move-hunk <from> <to> <file>`)
  MoveHunk,
}

/// A fix of an inverted stack: what to add to the plan of `brd group`, or the hunk to move
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvertedStackFix {
  pub kind: InvertedStackFixKind,
  /// Plan path -> the commits (full ids) it gets
  pub branches: BTreeMap<String, Vec<String>>,
  pub unstack: Vec<String>,
  /// Of `MoveHunk`: the commit the change is in, the commit it moves into and the file (full ids)
  pub hunk: Option<(String, String, String)>,
}

/// What a commit of the parent needs of a commit of the child
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvertedNeed {
  /// The commit of the parent (full id)
  pub commit: String,
  /// The commit of the child it needs (full id)
  pub needs: String,
  /// The file whose change it needs: of a missing declaration or a dangling reference; `None` for a conflict
  pub file: Option<String>,
}

/// A branch that needs a commit of a branch stacked on it, with the fixes a trial build passes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvertedStack {
  /// The parent
  pub branch: String,
  /// The branch stacked on it that has the commit it needs
  pub child: String,
  /// What the parent needs of the child
  pub needs: Vec<InvertedNeed>,
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

/// A missing declaration without its branch, which a fix may change: the commit of the use, the name, the place of the
/// use and the commit it needs, each commit by its id before a rewrite (`renamed`: new id -> old id)
type MissingKey = (String, String, String, u32, String);

fn missing_key(missing: &MissingDeclaration, renamed: &HashMap<&str, &str>) -> MissingKey {
  let old = |id: &str| renamed.get(id).map_or_else(|| id.to_string(), |old| (*old).to_string());
  (
    old(&missing.used_by),
    missing.identifier.clone(),
    missing.file.clone(),
    missing.line,
    old(&missing.declared_by),
  )
}

/// Whether `trial`, a check in `session` (after a rewrite, `renamed`: new id -> old id), proves a fix of the missing
/// declarations `fixed` of the check whose missing declarations are `before`: the trial has none of `fixed` and none
/// that `before` lacks
fn fixes_missing(session: &CheckSession, trial: &CheckReport, renamed: &HashMap<&str, &str>, before: &[MissingDeclaration], fixed: &[&MissingDeclaration]) -> bool {
  let none: HashMap<&str, &str> = HashMap::new();
  let known: HashSet<MissingKey> = before.iter().map(|missing| missing_key(missing, &none)).collect();
  let fixed: HashSet<MissingKey> = fixed.iter().map(|missing| missing_key(missing, &none)).collect();
  missing_declarations(session, trial).is_ok_and(|after| {
    after
      .iter()
      .map(|missing| missing_key(missing, renamed))
      .all(|key| known.contains(&key) && !fixed.contains(&key))
  })
}

/// `branch` of `report`, a check of `plan` in `session`, conflicts, has a dangling reference or a missing declaration
/// (`missing`, those of every branch of the check), because it needs a commit of a branch stacked on it: the fixes a
/// trial build of `plan` with them passes; `None` for no such case. `files_of` holds the changed files of each commit
/// (`missing_commits::changed_files`)
pub fn inverted_stack(
  session: &CheckSession,
  plan: &RegroupPlan,
  report: &CheckReport,
  branch: &BranchCheck,
  missing: &[MissingDeclaration],
  files_of: &HashMap<String, HashSet<String>>,
) -> Option<InvertedStack> {
  // the commits of the branch that need something, and what they need
  let mut needs: Vec<InvertedNeed> = match &branch.outcome {
    CheckOutcome::Conflict { commit, files } => missing_commits(report, branch, commit, files, files_of)
      .into_iter()
      .map(|needs| InvertedNeed {
        commit: commit.clone(),
        needs,
        file: None,
      })
      .collect(),
    CheckOutcome::Ok { .. } => report
      .dangling
      .get(&branch.name)
      .into_iter()
      .flatten()
      .filter(|reference| reference.is_confirmed())
      .filter_map(|reference| {
        Some(InvertedNeed {
          commit: reference.removed_by.clone(),
          needs: reference.fixed_by.clone()?,
          file: Some(reference.file.clone()),
        })
      })
      .collect(),
    _ => return None,
  };
  let own_missing: Vec<&MissingDeclaration> = if matches!(branch.outcome, CheckOutcome::Ok { .. }) {
    missing.iter().filter(|missing| missing.branch == branch.name).collect()
  } else {
    Vec::new()
  };
  let group_of: HashMap<&str, Option<&str>> = report.regroup.commits.iter().map(|commit| (commit.id.as_str(), commit.group.as_deref())).collect();
  // the branch stacked on this one, at any depth, that has the commit `id`
  let child_of = |id: &str| {
    group_of
      .get(id)
      .copied()
      .flatten()
      .and_then(|group| report.branches.iter().find(|check| check.name == group))
      .filter(|check| is_built_on(report, check, &branch.name))
  };
  let child = needs
    .iter()
    .map(|need| need.needs.as_str())
    .chain(own_missing.iter().map(|missing| missing.declared_by.as_str()))
    .find_map(child_of)?;
  let of_child = |id: &str| child_of(id).is_some_and(|check| check.name == child.name);
  // the missing declarations the commits of the child would give the branch
  let fixed: Vec<&MissingDeclaration> = own_missing.into_iter().filter(|missing| of_child(&missing.declared_by)).collect();
  needs.retain(|need| of_child(&need.needs));
  for missing in &fixed {
    let need = InvertedNeed {
      commit: missing.used_by.clone(),
      needs: missing.declared_by.clone(),
      file: Some(missing.change_file.clone()),
    };
    if !needs.contains(&need) {
      needs.push(need);
    }
  }

  let mutual = report
    .dangling
    .get(&branch.name)
    .into_iter()
    .flatten()
    .any(|reference| reference.is_confirmed() && reference.mutual);
  let mut own: Vec<String> = needs.iter().map(|need| need.commit.clone()).collect();
  own.sort();
  own.dedup();
  let mut candidates = vec![
    InvertedStackFix {
      kind: InvertedStackFixKind::MoveToChild,
      branches: BTreeMap::from([(plan_path(child), own)]),
      unstack: Vec::new(),
      hunk: None,
    },
    InvertedStackFix {
      kind: InvertedStackFixKind::Swap,
      branches: BTreeMap::from([(format!("{}/{}", child.name, branch.name), Vec::new())]),
      unstack: vec![child.name.clone()],
      hunk: None,
    },
  ];
  // the change the first missing declaration needs moves into the parent: its whole commit where it changes only the
  // file, else that change into the commit of the use (below)
  let first = fixed.first().copied();
  let whole = first.filter(|missing| {
    files_of
      .get(&missing.declared_by)
      .is_some_and(|files| files.iter().all(|path| *path == missing.change_file))
  });
  if let Some(missing) = whole {
    candidates.push(InvertedStackFix {
      kind: InvertedStackFixKind::MoveToParent,
      branches: BTreeMap::from([(plan_path(branch), vec![missing.declared_by.clone()])]),
      unstack: Vec::new(),
      hunk: None,
    });
  }
  let proves = |trial_session: &CheckSession, trial: &CheckReport, renamed: &HashMap<&str, &str>| {
    trial.builds_cleanly(&branch.name) && trial.builds_cleanly(&child.name) && (fixed.is_empty() || fixes_missing(trial_session, trial, renamed, missing, &fixed))
  };
  let mut fixes: Vec<InvertedStackFix> = candidates
    .into_iter()
    .filter(|fix| !(mutual && fix.kind == InvertedStackFixKind::Swap))
    .filter(|fix| {
      let mut fixed_plan = plan.clone();
      for (path, commits) in &fix.branches {
        // a commit the fix gives a branch leaves the one the plan gives it
        for ids in fixed_plan.branches.values_mut() {
          ids.retain(|id| !commits.iter().any(|commit| commit.starts_with(id.as_str())));
        }
        fixed_plan.branches.entry(path.clone()).or_default().extend(commits.iter().cloned());
      }
      fixed_plan.unstack.extend(fix.unstack.iter().cloned());
      // a fix the regroup refuses, or one after which either branch still fails, is no fix
      session
        .check_with_dangling_of(Some(&fixed_plan), &[branch.name.clone(), child.name.clone()])
        .is_ok_and(|trial| proves(session, &trial, &HashMap::new()))
    })
    .collect();
  if let Some(missing) = first.filter(|_| whole.is_none())
    && let Some(trial) = hunk_trial(session, plan, report, &missing.declared_by, &missing.used_by, &missing.change_file)
  {
    let renamed: HashMap<&str, &str> = trial.commits.iter().map(|(old, new)| (new.as_str(), old.as_str())).collect();
    if proves(&trial.session, &trial.report, &renamed) {
      fixes.push(InvertedStackFix {
        kind: InvertedStackFixKind::MoveHunk,
        branches: BTreeMap::new(),
        unstack: Vec::new(),
        hunk: Some((missing.declared_by.clone(), missing.used_by.clone(), missing.change_file.clone())),
      });
    }
  }
  Some(InvertedStack {
    branch: branch.name.clone(),
    child: child.name.clone(),
    needs,
    fixes,
    mutual,
  })
}
