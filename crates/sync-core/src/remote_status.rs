use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_types::RemoteStatusUpdate;
use tracing::instrument;

/// Get remote existence and last push time in a single git reflog call
/// Returns (remote_exists, last_push_time)
#[instrument(skip(git_executor), fields(remote_ref = %remote_ref))]
fn get_remote_status_and_push_time(git_executor: &GitCommandExecutor, repository_path: &str, remote_ref: &str) -> (bool, u32) {
  // Get reflog entries with unix timestamps for the remote branch
  let lines = git_executor
    .execute_command_lines(&["--no-pager", "reflog", "show", "--date=unix", remote_ref], repository_path)
    .ok();

  if let Some(lines) = lines {
    // Remote exists, look for push time
    for line in lines {
      if line.contains("update by push") {
        // Extract unix timestamp from format: "hash refs/remotes/origin/branch@{unix_timestamp}: update by push"
        if let Some(timestamp_start) = line.find("{")
          && let Some(timestamp_end) = line.find("}: update by push")
        {
          let timestamp_str = &line[timestamp_start + 1..timestamp_end];

          // Parse the unix timestamp directly
          if let Ok(timestamp) = timestamp_str.parse::<u32>() {
            return (true, timestamp);
          }
        }
      }
    }
    // Remote exists but no push found
    (true, 0)
  } else {
    // Remote doesn't exist
    (false, 0)
  }
}

/// Compute remote status for a single local virtual branch.
/// local_ref must be in the form "{prefix}/virtual/{name}" (no refs/heads/ prefix).
#[instrument(
  skip(git_executor),
  fields(
    branch_name = %branch_name,
    local_ref = %local_ref,
    baseline_branch = %baseline_branch
  )
)]
pub fn compute_remote_status_for_branch(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  local_ref: &str,
  branch_name: &str,
  my_email: Option<&str>,
  total_commits_in_branch: u32,
  baseline_branch: &str,
) -> Result<RemoteStatusUpdate> {
  let remote_ref = format!("origin/{}", local_ref);

  // Get remote existence and last push time in a single call
  let (remote_exists, last_push_time) = get_remote_status_and_push_time(git_executor, repository_path, &remote_ref);

  if remote_exists {
    // Ahead/behind counts in one call
    let counts = git_executor.execute_command(
      &["--no-pager", "rev-list", "--left-right", "--count", &format!("{}...{}", &remote_ref, local_ref)],
      repository_path,
    )?;
    let counts = counts.trim();
    let mut parts = counts.split_whitespace();
    let behind: u32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let ahead: u32 = parts.next().unwrap_or("0").parse().unwrap_or(0);

    // Unpushed commits (ahead set)
    let unpushed_commits: Vec<String> = if ahead > 0 {
      git_executor.execute_command_lines(&["--no-pager", "rev-list", "--reverse", &format!("{}..{}", &remote_ref, local_ref)], repository_path)?
    } else {
      Vec::new()
    };

    // My authored count within ahead set, excluding commits already in baseline
    let my_unpushed_count: u32 = if ahead > 0 {
      if let Some(email) = my_email {
        let out = git_executor.execute_command(
          &[
            "--no-pager",
            "rev-list",
            "--count",
            "-F",
            "--author",
            email,
            &format!("{}..{}", &remote_ref, local_ref),
            &format!("^{}", baseline_branch),
          ], // Exclude baseline commits
          repository_path,
        )?;
        out.trim().parse().unwrap_or(0)
      } else {
        0
      }
    } else {
      0
    };

    Ok(RemoteStatusUpdate {
      branch_name: branch_name.to_string(),
      remote_exists: true,
      unpushed_commits,
      commits_behind: behind,
      my_unpushed_count,
      last_push_time,
    })
  } else {
    // No remote: we don't need the list or counts; indicate absence only.
    Ok(RemoteStatusUpdate {
      branch_name: branch_name.to_string(),
      remote_exists: false,
      unpushed_commits: Vec::new(),
      commits_behind: 0,
      my_unpushed_count: total_commits_in_branch,
      last_push_time: 0, // Never pushed since remote doesn't exist
    })
  }
}
