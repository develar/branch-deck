//! Builders of the types for the tests of other crates, behind the feature `fixtures` that only their dev-dependencies
//! enable

use crate::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use crate::pull_request::{PullRequest, PullRequestState};
use crate::review_request::ReviewRequest;

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

/// The request of `number` (alice's, asked by bob) to the user, made at `requested_at` and waiting since then (Unix
/// seconds)
pub fn review_request(number: u32, requested_at: u32) -> ReviewRequest {
  ReviewRequest {
    number,
    title: format!("Change {number}"),
    url: format!("https://github.com/acme/monorepo/pull/{number}"),
    body_text: String::new(),
    author: Some("alice".to_string()),
    requested_by: Some("bob".to_string()),
    created_at: requested_at,
    updated_at: requested_at,
    requested_at,
    waiting_since: requested_at,
    back_to_you: None,
    additions: 1,
    deletions: 0,
    changed_files: 1,
    checks_state: None,
    review_decision: None,
    re_requested: false,
    reviewed_commit: None,
    head_ref_name: format!("alice/change-{number}"),
    head_ref_oid: String::new(),
    base_ref_name: "master".to_string(),
    base_ref_oid: String::new(),
    is_cross_repository: false,
    base_is_default: true,
  }
}

/// The robot `robot_id` of the branch `branch`, as a fact of the view names it
pub fn robot_of(branch: &str, robot_id: &str) -> crate::repository_view::RobotOf {
  crate::repository_view::RobotOf {
    branch: branch.to_string(),
    robot_id: robot_id.to_string(),
    whole_stack: false,
  }
}

/// The robot `robot_id` of the whole stack of `root`, as a fact of the view names it
pub fn whole_stack_robot_of(root: &str, robot_id: &str) -> crate::repository_view::RobotOf {
  crate::repository_view::RobotOf {
    whole_stack: true,
    ..robot_of(root, robot_id)
  }
}

/// The commits `commits` of a branch a running robot's commit has, none of them amended since
pub fn carried(commits: &[&str]) -> crate::repository_view::Carried {
  crate::repository_view::Carried {
    commits: commits.iter().map(|id| (*id).to_string()).collect(),
    amended: Vec::new(),
  }
}

/// The commit `id` of `branch` with `subject`, as a refusal of the landing gate names it
pub fn stack_commit(id: &str, subject: &str, branch: &str) -> crate::landing::StackCommit {
  crate::landing::StackCommit {
    id: id.to_string(),
    subject: subject.to_string(),
    branch: branch.to_string(),
  }
}

/// The readiness of a change whose group `group` hasn't approved, `needed` more approvals counting
pub fn missing_approvals(group: &str, needed: Option<u32>) -> crate::patronus::ReviewReadiness {
  crate::patronus::ReviewReadiness::Missing {
    groups: vec![crate::patronus::MissingReviewGroup {
      name: group.to_owned(),
      needed,
      approved_by: Vec::new(),
      requested: Vec::new(),
      suggested: Vec::new(),
      changes_requested_by: Vec::new(),
    }],
    not_counted: Vec::new(),
    reason: crate::patronus::MissingApprovalsReason::ApprovalsMissing,
  }
}

/// The remote status of `branch` pushed as it is: on the remote at "remote", nothing to push
pub fn remote_status(branch: &str) -> crate::RemoteStatusUpdate {
  crate::RemoteStatusUpdate {
    branch_name: branch.to_string(),
    remote_exists: true,
    remote_tip: Some("remote".to_string()),
    unpushed_commits: Vec::new(),
    commits_behind: 0,
    others_behind: 0,
    dropped_commits: Vec::new(),
    commits_amended: 0,
    commits_restacked: 0,
    commits_landed: 0,
    landed_commits: Vec::new(),
    commits_reworded: 0,
    ancestor_commits_ahead: 0,
    my_unpushed_count: 0,
    fast_forward: true,
    last_push_time: 0,
    action: crate::PushAction::UpToDate,
  }
}
