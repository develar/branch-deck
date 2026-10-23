//! The commits and the regroup plans of the unit tests of the crates that build on this one

use crate::regroup::RegroupPlan;
use domain_types::commit::Commit;
use std::collections::BTreeMap;

/// A commit as the commit list reads it, with fixed metadata; the subject is the first line of `message`
pub fn commit(id: &str, message: &str) -> Commit {
  let subject = message.split('\n').next().unwrap().to_string();
  Commit {
    id: id.to_string(),
    stripped_subject: subject.clone(),
    message: message.to_string(),
    author_name: "Test Author".to_string(),
    author_email: "test@example.com".to_string(),
    author_timestamp: 1_234_567_890,
    committer_timestamp: 1_234_567_890,
    author_timezone: String::new(),
    committer_timezone: String::new(),
    subject,
    parent_id: None,
    tree_id: "tree".to_string(),
    note: None,
    mapped_commit_id: None,
    mapped_source: None,
    mapped_resolution: None,
  }
}

/// A plan of `branches` (path, commits) and `unassign`
#[must_use]
pub fn plan(branches: &[(&str, &[&String])], unassign: &[&String]) -> RegroupPlan {
  RegroupPlan {
    branches: branches
      .iter()
      .map(|(name, ids)| ((*name).to_string(), ids.iter().map(|id| (*id).clone()).collect()))
      .collect::<BTreeMap<_, _>>(),
    unassign: unassign.iter().map(|id| (*id).clone()).collect(),
    ..RegroupPlan::default()
  }
}

/// A plan that puts the commits of `keys` into the issue group `name`
#[must_use]
pub fn issue_group_plan(name: &str, keys: &[&str]) -> RegroupPlan {
  RegroupPlan {
    issue_groups: BTreeMap::from([(name.to_string(), keys.iter().map(|key| (*key).to_string()).collect())]),
    ..RegroupPlan::default()
  }
}
