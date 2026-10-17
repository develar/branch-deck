use crate::common::oldest_logged_commit;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_types::branch_integration::{BranchIntegrationStatus, IntegrationConfidence};
use tracing::info;

/// Detect integration via merge commit detection
/// The caller must already know that `tip`, the tip of `branch_name`, is merged into `baseline`. `stack_base` is the
/// commit a stacked branch was built on: the commits below it are its parent's, not counted.
pub fn detect_merge_status(
  git: &GitCommandExecutor,
  repo: &str,
  branch_name: &str,
  tip: &str,
  baseline: &str,
  stack_base: Option<&str>,
) -> Result<Option<BranchIntegrationStatus>> {
  let range = format!("{baseline}...{tip}");
  let exclusion = stack_base.map(|base| format!("^{base}"));
  let mut args = vec!["rev-list", "--count", "--end-of-options", &range];
  args.extend(exclusion.as_deref());
  let commit_count = git.execute_command(&args, repo).ok().and_then(|output| output.trim().parse::<u32>().ok()).unwrap_or(0);

  // the merge that brought the branch in: the oldest merge on the ancestry path
  let commit = oldest_logged_commit(git, repo, &["--merges", "--ancestry-path", "--end-of-options", &format!("{tip}..{baseline}")]);
  info!(name = %branch_name, method = "git branch --merged", "Branch fully integrated");
  Ok(Some(BranchIntegrationStatus::Integrated {
    integrated_at: commit.as_ref().map(|commit| commit.time),
    confidence: IntegrationConfidence::Exact,
    commit_count,
    commit,
  }))
}
