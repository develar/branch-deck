//! What the scheduler of the facts (`scheduler`) reads the remote services with: one method per read, each for the
//! session it names (its worktree, and the branch prefix the robots are matched under). The hosts implement it over
//! their services: the app over the Patronus service and the GitHub mirror of each repository it keeps (src-tauri,
//! `AppFactSources`), the E2E server over the fakes of each test repository, counting the reads as its tests check them
//! (`TestFactSources`). A list and a poll never fail: what went wrong is the answer's (the availability of Patronus, of
//! GitHub for the pull requests); the reads of what the session asks about its robots (their details too), the rules of
//! a base, the merge profiles and the review readiness answer their failure, which the facts keep.

use domain_types::error::ApiError;
use domain_types::patronus::{BranchReviewReadiness, KnownRobotStatus, PatronusProblem, PatronusRobotDetails, PatronusRobotsResult, PatronusStartOptions, ReviewReadinessBranch};
use domain_types::pull_request::{BranchRules, PullRequestsResult};
use repository_model::session::SessionKey;
use std::future::Future;
use std::pin::Pin;

/// A read in flight
pub type Read<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait FactSources: Send + Sync + 'static {
  /// The user's robots of the repository of `key` started since `since` (Unix seconds), else of the service's whole
  /// window, with the GitHub repository its branches are mirrored to; `background`: not asked by the user, so skipped
  /// while Patronus backs off after a failure
  fn robots<'a>(&'a self, key: &'a SessionKey, since: Option<u32>, background: bool) -> Read<'a, PatronusRobotsResult>;

  /// The robots of `known` whose status changed since; the others are not in the answer
  fn poll<'a>(&'a self, key: &'a SessionKey, known: Vec<KnownRobotStatus>) -> Read<'a, PatronusRobotsResult>;

  /// Of the running robots `ids`, the ones a failed required check dooms (Patronus' warning `robot_is_failing`)
  fn failing<'a>(&'a self, key: &'a SessionKey, ids: Vec<String>) -> Read<'a, Result<Vec<String>, ApiError>>;

  /// The error problems of the finished robot `id`
  fn problems<'a>(&'a self, key: &'a SessionKey, id: String) -> Read<'a, Result<Vec<PatronusProblem>, ApiError>>;

  /// The robot `id` with its merge profile, CI checks, error problems and whether it will fail, of a running one the
  /// estimate of the CI for its running checks (`PatronusService::robot_details`, with the new API of the settings when
  /// they turn it on)
  fn robot_details<'a>(&'a self, key: &'a SessionKey, id: String) -> Read<'a, Result<PatronusRobotDetails, ApiError>>;

  /// The GitHub pull requests of the branches and whole stacks of `key` (`pull_requests::list_pull_requests`, one
  /// search), in the GitHub repository of its worktree; `NotGitHub` without one
  fn pull_requests<'a>(&'a self, key: &'a SessionKey) -> Read<'a, PullRequestsResult>;

  /// What the rules of the base `branch` do to the pull requests into it (`branch_rules::read_branch_rules`)
  fn branch_rules<'a>(&'a self, key: &'a SessionKey, branch: String) -> Read<'a, Result<BranchRules, ApiError>>;

  /// The merge profiles of the target `baseline` pushes to, without the default of a last robot
  /// (`PatronusService::start_options`)
  fn profiles<'a>(&'a self, key: &'a SessionKey, baseline: String) -> Read<'a, Result<PatronusStartOptions, ApiError>>;

  /// The code-owner review of `asks` at the tips they name, against `baseline` (`review_readiness::branch_review_readiness`):
  /// the commits of all of them in one read, a pre-check of Patronus for each whose kept answer doesn't hold; `recheck`
  /// asks Patronus again, never the kept answer, in the foreground. The answers in the order of `asks`
  fn readiness<'a>(&'a self, key: &'a SessionKey, baseline: String, asks: Vec<ReviewReadinessBranch>, recheck: bool) -> Read<'a, Result<Vec<BranchReviewReadiness>, ApiError>>;

  /// Of `asks`, in their order, the answers the object cache keeps that still hold (`review_readiness::kept_review_readiness`),
  /// read without Patronus; `None` for each it doesn't answer
  fn cached_readiness<'a>(&'a self, key: &'a SessionKey, baseline: String, asks: Vec<ReviewReadinessBranch>) -> Read<'a, Vec<Option<BranchReviewReadiness>>>;
}
