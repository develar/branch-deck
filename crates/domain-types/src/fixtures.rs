//! Builders of the types for the tests of other crates, behind the feature `fixtures` that only their dev-dependencies
//! enable

use crate::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};

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
