//! Where the commits ahead of the baseline go in one sync: the branch of each commit and the commits each branch and the
//! branches it is built on have, so a conflict of a sync names the branch of each commit it likely needs
//! (`missing_commits` of the trial build finds them by the changed files instead).

use domain_types::BranchError;
use domain_types::commit::Commit;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// Where the commits ahead of the baseline go in one sync, for the missing commits of its conflicts
#[derive(Default)]
pub struct CommitOwners {
  /// Original commit id -> the branch it is in
  branch_of: HashMap<String, String>,
  /// Branch -> the original ids of its commits and of the commits of the branches it is built on
  present: HashMap<String, HashSet<String>>,
}

impl CommitOwners {
  /// `base_of` names the branch a branch is built on, `None` for the base of the sync
  pub fn new<'s>(branches: &IndexMap<String, Vec<Commit>>, base_of: impl Fn(&str) -> Option<&'s str>) -> Self {
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
    Self { branch_of, present }
  }

  /// Drops from the missing commits of a conflict of `branch` the commits that the branch and the branches it is built
  /// on have (their copies have other ids, so `git log ^<virtual tip>` keeps them), and names the branch of the others
  pub fn refine(&self, branch: &str, error: &mut BranchError) {
    let BranchError::MergeConflict(info) = error else { return };
    let present = self.present.get(branch);
    let missing = &mut info.conflict_analysis.missing_commits;
    missing.retain(|commit| present.is_none_or(|present| !present.contains(&commit.hash)));
    for commit in missing.iter_mut() {
      commit.branch = self.branch_of.get(&commit.hash).cloned();
    }
  }
}
