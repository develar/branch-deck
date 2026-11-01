//! The reads of the remote services and of the app's accounts that a window makes beside the facts of its session
//! (`remote_facts::scheduler`): a command of a repository reaches GitHub, Patronus and YouTrack through the services of
//! its session (`ServiceHost`, by the key its frame resolved), a command of the app through `AppHost`.
//!
//! - `pull_requests`: the text of a new pull request of a branch or of a whole stack, the reviewers a review can ask.
//! - `patronus`: the Safe Push statistics, why a robot failed, the commits a running robot covers.
//! - `review`: what the review window shows of its target, and its read ahead.
//! - `suggest`: the branch names of Claude Code.
//! - `accounts`: the sign-ins of a window without a session, the issue of a popover.

pub mod accounts;
pub mod patronus;
pub mod pull_requests;
pub mod review;
pub mod suggest;

use crate::remote::suggest::BranchNameRequests;
use remote_facts::robot_stats::RobotStatsReads;
use sync_core::review_target::ReviewMemo;

/// What the commands of the remote family keep between their calls, one of each per app (per test repository in the
/// e2e server)
#[derive(Default)]
pub struct RemoteMemory {
  /// The reviews the windows read, by the tips of their targets (`get_review`, `prefetch_review`)
  pub reviews: ReviewMemo,
  /// The latest request for branch names (`suggest_branch_names`)
  pub branch_names: BranchNameRequests,
  /// The reads of the Safe Push statistics that ask Patronus, one per repository and target (`get_robot_stats`)
  pub robot_stats: RobotStatsReads,
}
