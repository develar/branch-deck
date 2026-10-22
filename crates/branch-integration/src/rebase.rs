use crate::common::{UPSTREAM_COMMIT_FORMAT, oldest_logged_commit, parse_upstream_commit};
use crate::detection_notes::Recheck;
use anyhow::Result;
use domain_types::branch_integration::{BranchIntegrationStatus, IntegrationConfidence};
use domain_types::upstream_commit::UpstreamCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashSet;
use tracing::info;

/// The revision arguments of the symmetric difference of `baseline` and the branch tip, without the commits below the
/// stack base of a stacked branch (its parent's) and, for a recheck, without the baseline commits up to `since`: the
/// branch side stays the same, the baseline side shrinks to the commits since the last check. They start with
/// `--end-of-options`: `since` comes from a note.
fn symmetric_range(baseline: &str, tip: &str, stack_base: Option<&str>, since: Option<&str>) -> Vec<String> {
  let mut range = vec!["--end-of-options".to_string(), format!("{baseline}...{tip}")];
  range.extend(stack_base.map(|base| format!("^{base}")));
  range.extend(since.map(|since| format!("^{since}")));
  range
}

/// The commits of the branch side, by their cherry-mark
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RightSideMarks {
  pub total: u32,
  /// Without an equivalent in the baseline, newest first
  pub orphaned: Vec<String>,
  /// With an equivalent in the baseline
  pub integrated: u32,
}

/// Both sides of the symmetric difference by their cherry-mark
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CherryMarks {
  /// The branch side
  pub right: RightSideMarks,
  /// The newest baseline commit equivalent to a commit of the branch (cherry-pick detection)
  pub integration_commit: Option<UpstreamCommit>,
}

/// Marks both sides with one cherry-mark walk, which computes the patch ids of both sides anyway: the counts of the
/// branch side and the newest equivalent on the baseline side. `%m` is `=` for an equivalent commit of either side, so
/// the side of one comes from `%S`, the revision it was reached from (`log --source`, which `rev-list` rejects): the tip
/// for the branch side. With `since`, the branch commits are compared only with the baseline commits after it. The
/// format has no placeholder that `log.*`, `notes.*` or `.mailmap` change.
pub fn scan_cherry_marks(git: &GitCommandExecutor, repo: &str, baseline: &str, tip: &str, stack_base: Option<&str>, since: Option<&str>) -> Result<CherryMarks> {
  let range = symmetric_range(baseline, tip, stack_base, since);
  let format = format!("--format=%m%x00%S%x00{UPSTREAM_COMMIT_FORMAT}");
  let mut args = vec!["log", "--no-merges", "--left-right", "--cherry-mark", "--source", &format];
  args.extend(range.iter().map(String::as_str));
  let lines = git.execute_command_lines(&args, repo)?;

  let mut total: u32 = 0;
  let mut orphaned = Vec::new();
  let mut integration_commit = None;
  for line in &lines {
    let mut fields = line.splitn(3, '\0');
    let (Some(mark), Some(source), Some(commit)) = (fields.next(), fields.next(), fields.next()) else {
      continue;
    };
    match mark {
      ">" => {
        total += 1;
        orphaned.extend(commit.split('\0').next().map(str::to_string));
      }
      "=" if source == tip => total += 1,
      // newest first
      "=" if integration_commit.is_none() => integration_commit = parse_upstream_commit(commit),
      _ => {}
    }
  }

  let integrated = total.saturating_sub(u32::try_from(orphaned.len()).unwrap_or(u32::MAX));
  Ok(CherryMarks {
    right: RightSideMarks { total, orphaned, integrated },
    integration_commit,
  })
}

/// The tip of a branch whose branch side is empty: the tip itself is reachable from the baseline (the branch was
/// fast-forwarded or merged in), so it is the baseline commit that brought the branch in
fn tip_commit(git: &GitCommandExecutor, repo: &str, tip: &str) -> Option<UpstreamCommit> {
  oldest_logged_commit(git, repo, &["-1", "--end-of-options", tip])
}

/// Detect integration via rebase/cherry-pick from the marks of one walk (`scan_cherry_marks`); `tip` is the tip of
/// `branch_name`
pub fn detect_rebase_status_with_marks(git: &GitCommandExecutor, repo: &str, branch_name: &str, tip: &str, marks: &CherryMarks) -> Result<BranchIntegrationStatus> {
  let total_right = marks.right.total;
  let orphaned_right = u32::try_from(marks.right.orphaned.len()).unwrap_or(u32::MAX);
  let integrated_right = marks.right.integrated;
  if total_right == 0 || (orphaned_right == 0 && integrated_right > 0) {
    let commit_count = total_right;
    let commit = if integrated_right > 0 {
      marks.integration_commit.clone()
    } else {
      tip_commit(git, repo, tip)
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
      marks.integration_commit.as_ref().map(|commit| commit.time)
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
/// `baseline`): a commit still orphaned is one of `check.orphaned` without an equivalent among them. Other commits of
/// the branch side (integrated before, or a parent's once the stack base is gone) are left alone. Returns the status
/// and the commits still orphaned, newest first. `tip` is the tip of `branch_name`.
#[expect(clippy::too_many_arguments, reason = "the inputs of one recheck")]
pub fn recheck_rebase_status(
  git: &GitCommandExecutor,
  repo: &str,
  branch_name: &str,
  tip: &str,
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
  let marks = scan_cherry_marks(git, repo, baseline, tip, stack_base, since)?;
  // a commit that left the branch side is reachable from the baseline now (merged): integrated
  let unmatched: HashSet<&str> = marks.right.orphaned.iter().map(String::as_str).collect();
  let orphaned: Vec<String> = check.orphaned.iter().filter(|id| unmatched.contains(id.as_str())).cloned().collect();
  let orphaned_count = u32::try_from(orphaned.len()).unwrap_or(u32::MAX);
  let total = previous_total.max(orphaned_count);
  let newly_integrated = orphaned_count < previous_orphaned;
  // the newest equivalent since the last check; without one, the commits left the branch side: the tip is in the baseline
  let commit = if newly_integrated {
    marks.integration_commit.or_else(|| orphaned.is_empty().then(|| tip_commit(git, repo, tip)).flatten())
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
