//! Builders of the types for the tests of other crates, behind the feature `fixtures` that only their dev-dependencies
//! enable

use crate::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use crate::pull_request::{PullRequest, PullRequestState};

/// A Safe Push of the virtual branch `branch_name` (`None`: of a temporary ref, matched by its revision) at the revision
/// `tip`, started at `start_time` (Unix seconds)
pub fn robot(id: &str, branch_name: Option<&str>, status: PatronusRobotStatus, start_time: u32) -> PatronusRobot {
  PatronusRobot {
    id: id.to_string(),
    name: "IJPL-1 fix".to_string(),
    status,
    push_mode: PatronusPushMode::RebaseAutosquash,
    branch_name: branch_name.map(str::to_string),
    stack_root: None,
    source_revision: Some("tip".to_string()),
    target_branch: "master".to_string(),
    start_time,
    finish_time: None,
    cancellation_reason: None,
    url: format!("https://patronus/robot/{id}"),
    pull_request: None,
    profile: None,
  }
}

/// A Safe Push of the whole stack of `root`, started on its stack branch at `revision`
pub fn stack_robot(id: &str, root: &str, status: PatronusRobotStatus, revision: &str) -> PatronusRobot {
  PatronusRobot {
    stack_root: Some(root.to_string()),
    source_revision: Some(revision.to_string()),
    ..robot(id, None, status, 1)
  }
}

/// The open pull request `number` of the virtual branch `branch` under the prefix `user`, into `master`, on the commit
/// `<branch>-tip`, without reviewers
pub fn pull_request(branch: &str, number: u32) -> PullRequest {
  PullRequest {
    number,
    title: format!("Pull request of {branch}"),
    url: format!("https://github.com/acme/monorepo/pull/{number}"),
    state: PullRequestState::Open,
    is_draft: false,
    review_decision: None,
    head_ref_name: format!("user/virtual/{branch}"),
    base_ref_name: "master".to_string(),
    head_ref_oid: format!("{branch}-tip"),
    checks: None,
    updated_at: 1_704_139_200,
    reviewers: Vec::new(),
    author: Some("user".to_string()),
  }
}
