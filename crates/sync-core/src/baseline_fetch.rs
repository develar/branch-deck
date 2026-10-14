//! Fetch the baseline without updating: after a robot merged a branch, the header learns from the fetched baseline that
//! an update brings the branch in. Only the baseline's remote branch is fetched (as Update Project does); the branch,
//! the index and the working tree stay as they are.

use crate::baseline::detect_baseline_branch;
use crate::transfer_progress::TransferProgress;
use crate::update_project::{fetch_baseline, resolve_baseline_upstream};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use sync_utils::timestamp::unix_now;
use tracing::instrument;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FetchBaselineParams {
  pub repository_path: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FetchBaselineResult {
  /// The fetched baseline as it is shown, e.g. `origin/master`
  pub upstream: String,
  /// When the fetch ended (Unix seconds)
  pub fetched_at: u32,
}

/// Fetch only the baseline's remote branch into its tracking ref; `on_progress` gets how far the fetch is. A local
/// baseline is an error: there is nothing to fetch.
#[instrument(skip(git_executor, params, on_progress), fields(repository_path = %params.repository_path))]
pub fn fetch_baseline_core(git_executor: &GitCommandExecutor, params: &FetchBaselineParams, on_progress: &mut dyn FnMut(TransferProgress)) -> Result<FetchBaselineResult> {
  let repo_path = &params.repository_path;
  let baseline_branch = detect_baseline_branch(git_executor, repo_path, "master")?;
  let upstream =
    resolve_baseline_upstream(git_executor, repo_path, &baseline_branch)?.ok_or_else(|| anyhow!("The baseline {baseline_branch} is a local branch: there is nothing to fetch"))?;
  // a forced update needs no note: the next update check excludes the old tip by the fork point of the reflog
  fetch_baseline(git_executor, repo_path, &upstream, on_progress).map_err(|e| anyhow!("Failed to fetch {}: {e:#}", upstream.name))?;
  Ok(FetchBaselineResult {
    upstream: upstream.name,
    fetched_at: unix_now(),
  })
}

#[cfg(test)]
#[path = "baseline_fetch_test.rs"]
mod baseline_fetch_test;
