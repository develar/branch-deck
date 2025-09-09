use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_types::RemoteStatusUpdate;
use tracing::instrument;

/// Check if remote branch exists using show-ref (faster than reflog)
#[inline]
fn remote_branch_exists(git_executor: &GitCommandExecutor, repository_path: &str, remote_ref: &str) -> bool {
  git_executor
    .execute_command_with_status(&["--no-pager", "show-ref", "--verify", "--quiet", &format!("refs/remotes/{}", remote_ref)], repository_path)
    .map(|(_, exit_code)| exit_code == 0)
    .unwrap_or(false)
}

/// Get last push time from reflog if available
#[inline]
fn get_last_push_time(git_executor: &GitCommandExecutor, repository_path: &str, remote_ref: &str) -> u32 {
  // Only check reflog if we need the push time
  if let Ok(lines) = git_executor.execute_command_lines(&["--no-pager", "reflog", "show", "--date=unix", remote_ref], repository_path) {
    for line in lines {
      if line.contains("update by push") {
        // Extract unix timestamp from format: "hash refs/remotes/origin/branch@{unix_timestamp}: update by push"
        if let Some(timestamp_start) = line.find("{")
          && let Some(timestamp_end) = line.find("}: update by push")
          && let Ok(timestamp) = line[timestamp_start + 1..timestamp_end].parse::<u32>()
        {
          return timestamp;
        }
      }
    }
  }
  0
}

/// Compute remote status for a single local virtual branch.
/// local_ref must be in the form "{prefix}/virtual/{name}" (no refs/heads/ prefix).
#[instrument(
  skip(git_executor),
  fields(
    branch_name = %branch_name,
    local_ref = %local_ref
  )
)]
pub fn compute_remote_status_for_branch(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  local_ref: &str,
  branch_name: &str,
  my_email: Option<&str>,
  total_commits_in_branch: u32,
  baseline_branch: &str, // Used to exclude commits already in master
) -> Result<RemoteStatusUpdate> {
  let remote_ref = format!("origin/{}", local_ref);

  // Fast check if remote exists
  if !remote_branch_exists(git_executor, repository_path, &remote_ref) {
    return Ok(RemoteStatusUpdate {
      branch_name: branch_name.to_string(),
      remote_exists: false,
      unpushed_commits: Vec::new(),
      commits_behind: 0,
      my_unpushed_count: total_commits_in_branch,
      last_push_time: 0,
    });
  }

  // Get ahead/behind counts
  let range = format!("{}...{}", remote_ref, local_ref);
  let counts = git_executor.execute_command(&["--no-pager", "rev-list", "--left-right", "--count", &range], repository_path)?;
  let counts = counts.trim();
  let mut parts = counts.split_whitespace();
  let _raw_behind: u32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
  let ahead: u32 = parts.next().unwrap_or("0").parse().unwrap_or(0);

  // Calculate commits behind using cherry-pick to exclude patch-equivalent commits
  // This prevents showing rebased commits as "removed or changed locally"
  let behind_output = git_executor.execute_command(&["--no-pager", "rev-list", "--cherry-pick", "--left-only", "--count", &range], repository_path)?;
  let behind: u32 = behind_output.trim().parse().unwrap_or(0);

  // Early return if nothing ahead
  if ahead == 0 {
    let last_push_time = get_last_push_time(git_executor, repository_path, &remote_ref);
    return Ok(RemoteStatusUpdate {
      branch_name: branch_name.to_string(),
      remote_exists: true,
      unpushed_commits: Vec::new(),
      commits_behind: behind,
      my_unpushed_count: 0,
      last_push_time,
    });
  }

  // Get unpushed commits (all commits ahead, including patch-equivalent)
  let unpushed_range = format!("{}..{}", remote_ref, local_ref);
  let unpushed_commits = git_executor.execute_command_lines(&["--no-pager", "rev-list", "--reverse", &unpushed_range], repository_path)?;

  // Calculate my_unpushed_count only if we have an email to filter by
  let my_unpushed_count = if let Some(email) = my_email {
    // Get patch-unique commits by the specific author, excluding those already in baseline
    // Using git's --author filter and ^baseline to exclude commits already in master
    let author_filter = format!("--author={}", email);
    let baseline_exclusion = format!("^{}", baseline_branch);
    let output = git_executor.execute_command_lines(
      &["--no-pager", "rev-list", "--cherry-pick", "--right-only", &author_filter, &baseline_exclusion, &range],
      repository_path,
    )?;

    // Count patch-unique commits by the user that are not already in baseline
    output.iter().filter(|line| !line.is_empty()).count() as u32
  } else {
    0
  };

  let last_push_time = get_last_push_time(git_executor, repository_path, &remote_ref);

  Ok(RemoteStatusUpdate {
    branch_name: branch_name.to_string(),
    remote_exists: true,
    unpushed_commits,
    commits_behind: behind,
    my_unpushed_count,
    last_push_time,
  })
}
