//! The landing gate as the app's start and popover ask it (`start_guard`): what refuses the start of a Safe Push or a
//! Dry Run before anything is pushed, by the rule of `bd safe-push` (`trial_check::landing_gate`, spec/branch/patronus.spec.md).
//! The model runs no check itself: its hosts (the app, the E2E server) answer it through the trial check of their
//! repository with its object cache, the check of the same inputs kept (`trial_check::landing_gate::KeptChecks`), so a
//! start right after the check of its popover reuses it.

use domain_types::landing::{LandingCheck, LandingGateParams};
use std::collections::HashSet;

/// The landing check of a start, which the hosts answer with the trial check
pub trait LandingGate: Send + Sync + 'static {
  /// The refusals that hold the start `params` back under `branch_prefix` after its overrides; `landed` are the commits
  /// (original ids) merged Safe Pushes landed as the Patronus view names them (`patronus_view::landed_commits`), whose
  /// names the target has. Blocking: call from the blocking pool
  ///
  /// # Errors
  /// What the check failed with: a repository it can't read, a grouping it can't build
  fn check(&self, branch_prefix: &str, params: &LandingGateParams, landed: &HashSet<String>) -> anyhow::Result<LandingCheck>;
}
