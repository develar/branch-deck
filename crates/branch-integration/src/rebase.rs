use crate::cache::Recheck;
use crate::common::{UPSTREAM_COMMIT_FORMAT, parse_upstream_commit};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashSet;
use sync_types::branch_integration::{BranchIntegrationStatus, IntegrationConfidence};
use sync_types::upstream_commit::UpstreamCommit;
use tracing::info;

/// `rev-list` arguments for the symmetric difference of `baseline` and the branch, without the commits below the stack
/// base of a stacked branch (its parent's) and, for a recheck, without the baseline commits up to `since`: the branch
/// side stays the same, the baseline side shrinks to the commits since the last check
fn symmetric_range(baseline: &str, branch_name: &str, stack_base: Option<&str>, since: Option<&str>) -> Vec<String> {
  let mut range = vec![format!("{baseline}...{branch_name}")];
  range.extend(stack_base.map(|base| format!("^{base}")));
  range.extend(since.map(|since| format!("^{since}")));
  range
}

/// The commits of the branch side, by their cherry-mark
#[derive(Debug, Clone)]
pub struct RightSideMarks {
  pub total: u32,
  /// Without an equivalent in the baseline, newest first
  pub orphaned: Vec<String>,
  /// With an equivalent in the baseline
  pub integrated: u32,
}

/// Scan right side (branch) with cherry-mark to derive counts in a single pass; with `since`, the branch commits are
/// compared only with the baseline commits after it
pub fn scan_right_side_marks(git: &GitCommandExecutor, repo: &str, baseline: &str, branch_name: &str, stack_base: Option<&str>, since: Option<&str>) -> Result<RightSideMarks> {
  let range = symmetric_range(baseline, branch_name, stack_base, since);
  let mut args = vec!["rev-list", "--right-only", "--cherry-mark", "--no-merges", "--format=%m %H", "--no-commit-header"];
  args.extend(range.iter().map(String::as_str));
  let lines = git.execute_command_lines(&args, repo)?;

  let mut total: u32 = 0;
  let mut orphaned = Vec::new();
  for line in lines {
    match line.split_once(' ') {
      Some((">", id)) => {
        total += 1;
        orphaned.push(id.to_string());
      }
      Some(("=", _)) => total += 1,
      _ => {}
    }
  }

  let integrated = total.saturating_sub(u32::try_from(orphaned.len()).unwrap_or(u32::MAX));
  Ok(RightSideMarks { total, orphaned, integrated })
}

/// The newest baseline commit equivalent to a commit of the branch (cherry-pick detection)
fn find_integration_commit(git: &GitCommandExecutor, repo: &str, baseline: &str, branch_name: &str, stack_base: Option<&str>, since: Option<&str>) -> Option<UpstreamCommit> {
  let range = symmetric_range(baseline, branch_name, stack_base, since);
  let format = format!("--pretty=format:%m{UPSTREAM_COMMIT_FORMAT}");
  let mut args = vec!["rev-list", "--left-right", "--left-only", "--cherry-mark", "--no-merges", &format];
  args.extend(range.iter().map(String::as_str));
  let lines = git.execute_command_lines(&args, repo).ok()?;
  // newest first, each after its `commit <id>` line
  lines.iter().find_map(|line| line.strip_prefix('=').and_then(parse_upstream_commit))
}

/// Detect integration via rebase/cherry-pick using single-pass marker scan counts
pub fn detect_rebase_status_with_marks(
  git: &GitCommandExecutor,
  repo: &str,
  branch_name: &str,
  baseline: &str,
  stack_base: Option<&str>,
  marks: &RightSideMarks,
) -> Result<BranchIntegrationStatus> {
  let total_right = marks.total;
  let orphaned_right = u32::try_from(marks.orphaned.len()).unwrap_or(u32::MAX);
  let integrated_right = marks.integrated;
  if total_right == 0 || (orphaned_right == 0 && integrated_right > 0) {
    let commit_count = total_right;
    let commit = if integrated_right > 0 {
      find_integration_commit(git, repo, baseline, branch_name, stack_base, None)
    } else {
      None
    };
    info!(name = %branch_name, method = "cherry-pick", "Branch fully integrated");
    return Ok(BranchIntegrationStatus::Integrated {
      integrated_at: commit.as_ref().map(|commit| commit.time),
      confidence: IntegrationConfidence::High,
      commit_count,
      commit,
    });
  }

  let total_count = total_right;
  let orphaned_count = orphaned_right;
  let integrated_count = integrated_right;

  if orphaned_count > 0 || total_count > 0 {
    let integrated_at = if integrated_count > 0 {
      find_integration_commit(git, repo, baseline, branch_name, stack_base, None).map(|commit| commit.time)
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

/// Checks a `NotIntegrated` result again, against the baseline commits since `check.baseline` only (an ancestor of
/// `baseline`): a commit still orphaned is one of `check.orphaned` (all of the branch side when `None`) without an
/// equivalent among them. Returns the status and the commits still orphaned, newest first.
pub fn recheck_rebase_status(
  git: &GitCommandExecutor,
  repo: &str,
  branch_name: &str,
  baseline: &str,
  stack_base: Option<&str>,
  previous: &BranchIntegrationStatus,
  check: &Recheck,
) -> Result<(BranchIntegrationStatus, Vec<String>)> {
  let (previous_total, previous_orphaned, previous_integrated_at) = match previous {
    BranchIntegrationStatus::NotIntegrated {
      total_commit_count,
      orphaned_count,
      integrated_at,
      ..
    } => (*total_commit_count, *orphaned_count, *integrated_at),
    _ => return Err(anyhow::anyhow!("only a NotIntegrated result is rechecked")),
  };
  let since = Some(check.baseline.as_str());
  let marks = scan_right_side_marks(git, repo, baseline, branch_name, stack_base, since)?;
  // a commit that left the branch side is reachable from the baseline now (merged): integrated
  let orphaned: Vec<String> = match &check.orphaned {
    None => marks.orphaned,
    Some(before) => {
      let unmatched: HashSet<&str> = marks.orphaned.iter().map(String::as_str).collect();
      before.iter().filter(|id| unmatched.contains(id.as_str())).cloned().collect()
    }
  };
  let orphaned_count = u32::try_from(orphaned.len()).unwrap_or(u32::MAX);
  let total = previous_total.max(orphaned_count);
  let newly_integrated = orphaned_count < previous_orphaned;
  let commit = if newly_integrated {
    find_integration_commit(git, repo, baseline, branch_name, stack_base, since)
  } else {
    None
  };

  if orphaned.is_empty() {
    info!(name = %branch_name, method = "cherry-pick", "Branch fully integrated since the last check");
    let status = BranchIntegrationStatus::Integrated {
      integrated_at: commit.as_ref().map(|commit| commit.time).or(previous_integrated_at),
      confidence: IntegrationConfidence::High,
      commit_count: total,
      commit,
    };
    return Ok((status, orphaned));
  }

  let integrated_at = match (previous_integrated_at, commit.map(|commit| commit.time)) {
    (Some(before), Some(now)) => Some(before.max(now)),
    (before, now) => before.or(now),
  };
  info!(name = %branch_name, total, orphaned = orphaned_count, "Branch still partially orphaned since the last check");
  let status = BranchIntegrationStatus::NotIntegrated {
    total_commit_count: total,
    integrated_count: total - orphaned_count,
    orphaned_count,
    integrated_at,
  };
  Ok((status, orphaned))
}
