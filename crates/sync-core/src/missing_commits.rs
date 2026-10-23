//! The commits a conflicting commit likely needs: the older commits outside its branch and the branches it is built on
//! that change a conflicting file. A sync names the branch of each (`commit_owners`); a trial build (`check`) finds
//! them by the changed files of each commit.

use crate::check::{BranchCheck, CheckReport};
use crate::diff_scan::CommitDiff;
use std::collections::{HashMap, HashSet};

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
