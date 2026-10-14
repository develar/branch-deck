use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_types::RemoteStatusUpdate;
use tracing::instrument;

/// Get last push time from reflog if available
#[inline]
fn get_last_push_time(git_executor: &GitCommandExecutor, repository_path: &str, remote_ref: &str) -> u32 {
  // Only check reflog if we need the push time
  if let Ok(lines) = git_executor.execute_command_lines(&["reflog", "show", "--date=unix", remote_ref], repository_path) {
    for line in lines {
      if line.contains("update by push") {
        // Extract unix timestamp from format: "hash refs/remotes/origin/branch@{unix_timestamp}: update by push"
        if let Some(timestamp_start) = line.find('{')
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

/// The commits of a `rev-list` walk, oldest first, and how many of them the user authored
struct WalkedCommits {
  commits: Vec<String>,
  mine: u32,
}

fn walk(git_executor: &GitCommandExecutor, repository_path: &str, args: &[&str], my_email: Option<&str>) -> Result<WalkedCommits> {
  let mut command = vec!["rev-list", "--reverse", "--no-commit-header", "--format=%H %ae"];
  command.extend_from_slice(args);
  let mut walked = WalkedCommits { commits: Vec::new(), mine: 0 };
  for line in git_executor.execute_command_lines(&command, repository_path)? {
    let (hash, email) = line.split_once(' ').unwrap_or((&line, ""));
    if my_email.is_some_and(|my_email| email.eq_ignore_ascii_case(my_email)) {
      walked.mine += 1;
    }
    walked.commits.push(hash.to_string());
  }
  Ok(walked)
}

/// Compute remote status for a single local virtual branch.
/// `local_ref` must be in the form "{prefix}/virtual/{name}" (no refs/heads/ prefix). `parent_ref` is the virtual
/// branch a stacked branch is built on: its commits are the parent's, not unpushed commits of this branch.
///
/// The sync rebuilds a virtual branch on the latest baseline, so its hashes change while its work stays the same.
/// The status compares content, not hashes: the baseline commits are on the remote already, and a rebased copy of a
/// pushed commit is patch-equivalent to it. Only the commits whose content the remote lacks are unpushed.
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
  baseline_branch: &str,
  parent_ref: Option<&str>,
) -> Result<RemoteStatusUpdate> {
  let remote_ref = format!("origin/{local_ref}");
  let baseline_exclusion = format!("^{baseline_branch}");
  let parent_exclusion = parent_ref.map(|parent_ref| format!("^{parent_ref}"));

  // one process for both: whether the remote branch exists, and its tip
  let (remote_tip, exit_code) = git_executor.execute_command_with_status(
    &["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("refs/remotes/{remote_ref}")],
    repository_path,
  )?;
  if exit_code != 0 {
    let mut args = vec![baseline_exclusion.as_str()];
    args.extend(parent_exclusion.as_deref());
    args.push(local_ref);
    let walked = walk(git_executor, repository_path, &args, my_email)?;
    return Ok(RemoteStatusUpdate {
      branch_name: branch_name.to_string(),
      remote_exists: false,
      remote_tip: None,
      my_unpushed_count: walked.mine,
      unpushed_commits: walked.commits,
      commits_behind: 0,
      fast_forward: true,
      last_push_time: 0,
    });
  }

  // the local commits whose content the remote lacks: patch-equivalent (rebased) copies and the baseline are left out
  let range = format!("{remote_ref}...{local_ref}");
  let mut args = vec!["--cherry-pick", "--right-only", baseline_exclusion.as_str()];
  args.extend(parent_exclusion.as_deref());
  args.push(&range);
  let unpushed = walk(git_executor, repository_path, &args, my_email)?;

  // the remote commits whose content the branch lacks: a push drops them. The baseline is not excluded here,
  // so a remote commit that reached the baseline as a copy (cherry-pick, squash merge) is matched by that copy.
  let behind_output = git_executor.execute_command(&["rev-list", "--cherry-pick", "--left-only", "--count", &range], repository_path)?;
  let behind: u32 = behind_output.trim().parse().unwrap_or(0);

  // a rebased branch is not a fast-forward of its remote even when nothing is lost: the push replaces the remote
  // commits with their rebased copies
  let fast_forward = behind == 0
    && git_executor
      .execute_command_with_status(&["merge-base", "--is-ancestor", &remote_ref, local_ref], repository_path)?
      .1
      == 0;

  let last_push_time = get_last_push_time(git_executor, repository_path, &remote_ref);

  Ok(RemoteStatusUpdate {
    branch_name: branch_name.to_string(),
    remote_exists: true,
    remote_tip: Some(remote_tip.trim().to_string()),
    my_unpushed_count: unpushed.mine,
    unpushed_commits: unpushed.commits,
    commits_behind: behind,
    fast_forward,
    last_push_time,
  })
}
