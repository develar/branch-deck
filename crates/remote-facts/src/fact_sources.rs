//! What the scheduler of the facts (`scheduler`) reads the remote services with: one method per read, each for the
//! session it names (its worktree, and the branch prefix the robots are matched under). The hosts implement it over
//! their services: the app over the Patronus service and the GitHub mirror of each repository it keeps (src-tauri,
//! `AppFactSources`), the E2E server over the fakes of each test repository, counting the reads as its tests check them
//! (`TestFactSources`). A list and a poll never fail: what went wrong is the answer's (the availability of Patronus); the
//! reads of what the session asks about its robots answer their failure, which the facts keep.

use domain_types::error::ApiError;
use domain_types::patronus::{KnownRobotStatus, PatronusProblem, PatronusRobotsResult};
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
}
