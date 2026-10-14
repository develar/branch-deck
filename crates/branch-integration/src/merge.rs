use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_types::branch_integration::{BranchIntegrationStatus, IntegrationConfidence};
use tracing::info;

/// Find the merge commit that integrated a branch into baseline
/// Returns the commit hash and timestamp of the merge commit that brought this branch in
pub fn find_integration_commit(git_executor: &GitCommandExecutor, repo_path: &str, branch_name: &str, baseline_branch: &str) -> Option<(String, u32)> {
  let lines = git_executor
    .execute_command_lines(
      &["log", "--merges", "--ancestry-path", "--format=%H %ct", &format!("{branch_name}..{baseline_branch}")],
      repo_path,
    )
    .ok()?;
  lines.last().and_then(|line| {
    let mut parts = line.split_whitespace();
    let hash = parts.next()?.to_string();
    let timestamp = parts.next()?.parse::<u32>().ok()?;
    Some((hash, timestamp))
  })
}

/// Detect integration via merge commit detection
/// The caller must already know that `branch_name` is merged into `baseline`. `stack_base` is the commit a stacked
/// branch was built on: the commits below it are its parent's, not counted.
pub fn detect_merge_status(git: &GitCommandExecutor, repo: &str, branch_name: &str, baseline: &str, stack_base: Option<&str>) -> Result<Option<BranchIntegrationStatus>> {
  let range = format!("{baseline}...{branch_name}");
  let exclusion = stack_base.map(|base| format!("^{base}"));
  let mut args = vec!["rev-list", "--count", &range];
  args.extend(exclusion.as_deref());
  let commit_count = git.execute_command(&args, repo).ok().and_then(|output| output.trim().parse::<u32>().ok()).unwrap_or(0);

  let integrated_at = find_integration_commit(git, repo, branch_name, baseline).map(|(_, timestamp)| timestamp);
  info!(name = %branch_name, method = "git branch --merged", "Branch fully integrated");
  Ok(Some(BranchIntegrationStatus::Integrated {
    integrated_at,
    confidence: IntegrationConfidence::Exact,
    commit_count,
  }))
}
