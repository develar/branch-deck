//! The commits a conflicting commit likely needs: the older commits outside its branch and the branches it is built on
//! that change a conflicting file. A sync names the branch of each; a trial build (`check`) finds them by the changed
//! files of each commit.

use crate::check::{BranchCheck, CheckReport};
use crate::diff_scan::CommitDiff;
use git_ops::commit_list::Commit;
use git_ops::model::BranchError;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// Where the commits ahead of the baseline go in one sync, for the missing commits of its conflicts
#[derive(Default)]
pub(crate) struct CommitOwners {
  /// Original commit id -> the branch it is in
  branch_of: HashMap<String, String>,
  /// Branch -> the original ids of its commits and of the commits of the branches it is built on
  present: HashMap<String, HashSet<String>>,
}

impl CommitOwners {
  /// `base_of` names the branch a branch is built on, `None` for the base of the sync
  pub(crate) fn new<'s>(branches: &IndexMap<String, Vec<Commit>>, base_of: impl Fn(&str) -> Option<&'s str>) -> Self {
    let branch_of: HashMap<String, String> = branches
      .iter()
      .flat_map(|(name, commits)| commits.iter().map(move |commit| (commit.id.clone(), name.clone())))
      .collect();
    let present = branches
      .keys()
      .map(|name| {
        let mut ids = HashSet::new();
        let mut seen = HashSet::new();
        let mut current = Some(name.as_str());
        while let Some(branch) = current
          && seen.insert(branch)
        {
          ids.extend(branches.get(branch).into_iter().flatten().map(|commit| commit.id.clone()));
          current = base_of(branch);
        }
        (name.clone(), ids)
      })
      .collect();
    CommitOwners { branch_of, present }
  }

  /// Drops from the missing commits of a conflict of `branch` the commits that the branch and the branches it is built
  /// on have (their copies have other ids, so `git log ^<virtual tip>` keeps them), and names the branch of the others
  pub(crate) fn refine(&self, branch: &str, error: &mut BranchError) {
    let BranchError::MergeConflict(info) = error else { return };
    let present = self.present.get(branch);
    let missing = &mut info.conflict_analysis.missing_commits;
    missing.retain(|commit| present.is_none_or(|present| !present.contains(&commit.hash)));
    for commit in missing.iter_mut() {
      commit.branch = self.branch_of.get(&commit.hash).cloned();
    }
  }
}

/// Commit id -> every path its changes name, old paths of renames included
pub fn changed_files(diffs: &[CommitDiff]) -> HashMap<String, HashSet<String>> {
  diffs
    .iter()
    .map(|diff| {
      let files = diff
        .files
        .iter()
        .flat_map(|change| std::iter::once(change.path.clone()).chain(change.old_path.clone()))
        .collect();
      (diff.id.clone(), files)
    })
    .collect()
}

/// The commits older than `commit` that change one of `files` and are neither in the branch nor in the branches it is
/// built on, oldest first; `files_of` holds the changed files of each commit (`changed_files`)
pub fn missing_commits(report: &CheckReport, branch: &BranchCheck, commit: &str, files: &[String], files_of: &HashMap<String, HashSet<String>>) -> Vec<String> {
  let mut present: HashSet<&str> = branch.commits.iter().map(String::as_str).collect();
  let mut ancestor = branch.built_on.as_deref();
  while let Some(name) = ancestor {
    let Some(parent) = report.branches.iter().find(|check| check.name == name) else { break };
    present.extend(parent.commits.iter().map(String::as_str));
    ancestor = parent.built_on.as_deref();
  }
  report
    .regroup
    .commits
    .iter()
    .map(|regrouped| regrouped.id.as_str())
    .take_while(|id| *id != commit)
    .filter(|id| !present.contains(id))
    .filter(|id| files_of.get(*id).is_some_and(|changed| files.iter().any(|file| changed.contains(file))))
    .map(str::to_string)
    .collect()
}

#[cfg(test)]
#[path = "missing_commits_test.rs"]
mod missing_commits_test;
