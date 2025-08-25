use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_types::RemoteStatusUpdate;

/// Compute remote status for a single local virtual branch.
/// local_ref must be in the form "{prefix}/virtual/{name}" (no refs/heads/ prefix).
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

  // Check if remote exists
  let remote_exists = git_executor
    .execute_command(&["--no-pager", "rev-parse", "--verify", "--quiet", &remote_ref], repository_path)
    .is_ok();

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

    // Remote head oid
    let remote_head = git_executor
      .execute_command(&["--no-pager", "rev-parse", &remote_ref], repository_path)
      .ok()
      .map(|s| s.trim().to_string());

    Ok(RemoteStatusUpdate {
      branch_name: branch_name.to_string(),
      remote_exists: true,
      remote_head,
      unpushed_commits,
      commits_behind: behind,
      my_unpushed_count,
    })
  } else {
    // No remote: we don't need the list or counts; indicate absence only.
    Ok(RemoteStatusUpdate {
      branch_name: branch_name.to_string(),
      remote_exists: false,
      remote_head: None,
      unpushed_commits: Vec::new(),
      commits_behind: 0,
      my_unpushed_count: total_commits_in_branch,
    })
  }
}
