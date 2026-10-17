use crate::common::{UPSTREAM_COMMIT_FORMAT, parse_upstream_commit};
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use sync_types::branch_integration::{BranchIntegrationStatus, IntegrationConfidence};
use tracing::info;

/// Detect integration via merge commit detection
/// The caller must already know that `tip`, the tip of `branch_name`, is merged into `baseline`. `stack_base` is the
/// commit a stacked branch was built on: the commits below it are its parent's, not counted. `None` when no merge
/// brought the tip in (a fast-forward): the cherry-mark scan names the tip then.
pub fn detect_merge_status(
  git: &GitCommandExecutor,
  repo: &str,
  branch_name: &str,
  tip: &str,
  baseline: &str,
  stack_base: Option<&str>,
) -> Result<Option<BranchIntegrationStatus>> {
  // the merge that brought the branch in: the oldest merge on the ancestry path, with its first parent
  let format = format!("--format={UPSTREAM_COMMIT_FORMAT}%x00%P");
  let ancestry = format!("{tip}..{baseline}");
  let lines = git.execute_command_lines(&["log", &format, "--merges", "--ancestry-path", "--end-of-options", &ancestry], repo)?;
  let Some((commit, first_parent)) = lines
    .last()
    .and_then(|line| Some((parse_upstream_commit(line)?, line.rsplit('\0').next()?.split(' ').next()?)))
  else {
    return Ok(None);
  };

  // the branch's own commits are the ones the merge brought in: not the baseline's before or after it
  let mainline = format!("^{first_parent}");
  let exclusion = stack_base.map(|base| format!("^{base}"));
  let mut args = vec!["rev-list", "--count", "--end-of-options", tip, &mainline];
  args.extend(exclusion.as_deref());
  let output = git.execute_command(&args, repo)?;
  let commit_count = output.trim().parse::<u32>().with_context(|| format!("Cannot parse the commit count {output:?}"))?;
  if commit_count == 0 {
    // the first parent has the tip already: a later merge, the branch was fast-forwarded
    return Ok(None);
  }

  info!(name = %branch_name, method = "git branch --merged", "Branch fully integrated");
  Ok(Some(BranchIntegrationStatus::Integrated {
    integrated_at: Some(commit.time),
    confidence: IntegrationConfidence::Exact,
    commit_count,
    commit: Some(commit),
  }))
}
