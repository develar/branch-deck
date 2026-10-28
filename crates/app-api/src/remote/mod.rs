//! The reads of the remote services and of the app's accounts that a window makes beside the facts of its session
//! (`remote_facts::scheduler`): a command of a repository reaches GitHub, Patronus and YouTrack through the services of
//! its session (`ServiceHost`, by the key its frame resolved), a command of the app through `AppHost`.
//!
//! - `pull_requests`: the text of a new pull request of a branch or of a whole stack, the reviewers a review can ask.
//! - `patronus`: the Safe Push statistics, why a robot failed, the commits a running robot covers.
//! - `branch_review`: the parts of a branch for its review window, and their read ahead.
//! - `suggest`: the branch names of Claude Code.
//! - `accounts`: the sign-ins of a window without a session, the issue of a popover.

pub mod accounts;
pub mod branch_review;
pub mod patronus;
pub mod pull_requests;
pub mod suggest;

use crate::remote::patronus::RobotStatsReads;
use crate::remote::suggest::BranchNameRequests;
use sync_core::branch_review::BranchReviewMemo;

/// What the commands of the remote family keep between their calls, one of each per app (per test repository in the
/// e2e server)
#[derive(Default)]
pub struct RemoteMemory {
  /// The reviews of branches the windows read, by their tips (`get_branch_review`, `prefetch_branch_review`)
  pub branch_reviews: BranchReviewMemo,
  /// The latest request for branch names (`suggest_branch_names`)
  pub branch_names: BranchNameRequests,
  /// The reads of the Safe Push statistics of each repository, one asking Patronus at a time (`get_robot_stats`)
  pub robot_stats: RobotStatsReads,
}
