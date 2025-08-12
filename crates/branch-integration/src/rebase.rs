use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_types::branch_integration::{BranchIntegrationStatus, IntegrationConfidence};
use tracing::info;

/// Scan right side (branch) with cherry-mark to derive counts in a single pass
pub fn scan_right_side_marks(git: &GitCommandExecutor, repo: &str, baseline: &str, branch_name: &str) -> Result<(u32, u32, u32)> {
  let range = format!("{baseline}...{branch_name}");
  let lines = git.execute_command_lines(&["rev-list", "--right-only", "--cherry-mark", "--no-merges", "--pretty=format:%m", &range], repo)?;

  let mut total_right: u32 = 0;
  let mut orphaned_right: u32 = 0;

  for line in lines {
    if line.starts_with("commit ") || line.is_empty() {
      continue;
    }
    match line.as_bytes().first() {
      Some(b'>') => {
        total_right += 1;
        orphaned_right += 1;
      }
      Some(b'=') => {
        total_right += 1;
      }
      _ => {}
    }
  }

  let integrated_right = total_right.saturating_sub(orphaned_right);
  Ok((total_right, orphaned_right, integrated_right))
}

/// Get the timestamp of the most recent integrated commit using cherry-pick detection
fn get_integration_timestamp(git: &GitCommandExecutor, repo: &str, baseline: &str, branch_name: &str) -> Option<u32> {
  let range = format!("{baseline}...{branch_name}");
  let lines = git
    .execute_command_lines(
      &["rev-list", "--left-right", "--left-only", "--cherry-mark", "--no-merges", "--pretty=format:%m %ct", &range],
      repo,
    )
    .ok()?;
  for line in lines {
    if line.starts_with("commit ") {
      continue;
    }
    if let Some(rest) = line.strip_prefix("= ")
      && let Ok(ts) = rest.parse::<u32>()
    {
      return Some(ts);
    }
  }
  None
}

/// Detect integration via rebase/cherry-pick using single-pass marker scan counts
pub fn detect_rebase_status_with_marks(
  git: &GitCommandExecutor,
  repo: &str,
  branch_name: &str,
  baseline: &str,
  total_right: u32,
  orphaned_right: u32,
  integrated_right: u32,
) -> Result<BranchIntegrationStatus> {
  if total_right == 0 || (orphaned_right == 0 && integrated_right > 0) {
    let commit_count = total_right;
    let integrated_at = if integrated_right > 0 {
      get_integration_timestamp(git, repo, baseline, branch_name)
    } else {
      None
    };
    info!(name = %branch_name, method = "cherry-pick", "Branch fully integrated");
    return Ok(BranchIntegrationStatus::Integrated {
      integrated_at,
      confidence: IntegrationConfidence::High,
      commit_count,
    });
  }

  let total_count = total_right;
  let orphaned_count = orphaned_right;
  let integrated_count = integrated_right;

  if orphaned_count > 0 || total_count > 0 {
    let integrated_at = if integrated_count > 0 {
      get_integration_timestamp(git, repo, baseline, branch_name)
    } else {
      None
    };
    info!(name = %branch_name, total = total_count, integrated = integrated_count, orphaned = orphaned_count, "Branch partially orphaned - some commits integrated, some not");
    return Ok(BranchIntegrationStatus::NotIntegrated {
      total_commit_count: total_count,
      integrated_count,
      orphaned_count,
      integrated_at,
    });
  }

  // This should never be reached given the logic above
  unreachable!("All cases should be handled by the conditions above")
}
