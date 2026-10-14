use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_types::branch_integration::{BranchIntegrationStatus, IntegrationConfidence};
use tracing::info;

fn diff_is_clean(git: &GitCommandExecutor, repo: &str, base: &str, other: &str) -> bool {
  git.execute_command(&["diff", "--quiet", &format!("{base}...{other}")], repo).is_ok()
}

fn get_tree_id(git: &GitCommandExecutor, repo: &str, reference: &str) -> Result<String> {
  let tree_ref = format!("{reference}^{{tree}}");
  let out = git.execute_command(&["rev-parse", &tree_ref], repo)?;
  Ok(out.trim().to_string())
}

fn find_commit_time_by_subject(git: &GitCommandExecutor, repo: &str, baseline: &str, subject: &str) -> Option<u32> {
  git
    .execute_command(&["log", "--format=%ct", "-F", "--grep", subject.trim(), "-n", "1", baseline], repo)
    .ok()
    .and_then(|s| s.trim().parse::<u32>().ok())
}

fn find_squash_timestamp(git_executor: &GitCommandExecutor, repo_path: &str, branch_name: &str, baseline_branch: &str) -> Option<u32> {
  let subject = git_executor.execute_command(&["log", "-1", "--format=%s", branch_name], repo_path).ok()?;
  find_commit_time_by_subject(git_executor, repo_path, baseline_branch, subject.trim())
}

/// `stack_base` is the commit a stacked branch was built on: only the changes after it are the branch's own
pub fn detect_squash_status(
  git: &GitCommandExecutor,
  repo: &str,
  branch_name: &str,
  baseline: &str,
  stack_base: Option<&str>,
  right_count: usize,
) -> Result<Option<BranchIntegrationStatus>> {
  let diff_clean = match stack_base {
    Some(base) => git.execute_command(&["diff", "--quiet", base, branch_name], repo).is_ok(),
    None => diff_is_clean(git, repo, baseline, branch_name),
  };
  if diff_clean {
    let integrated_at = find_squash_timestamp(git, repo, branch_name, baseline);
    info!(name = %branch_name, method = "diff-clean", "Branch fully integrated");
    return Ok(Some(BranchIntegrationStatus::Integrated {
      integrated_at,
      confidence: IntegrationConfidence::High,
      commit_count: u32::try_from(right_count).unwrap_or(u32::MAX),
    }));
  }

  let merge_base_result = match stack_base {
    Some(base) => Ok(base.to_string()),
    None => git.execute_command(&["merge-base", baseline, branch_name], repo),
  };
  if let Ok(merge_base_output) = merge_base_result {
    let merge_base = merge_base_output.trim();
    if !merge_base.is_empty()
      && let Ok(merge_tree_output) = git.execute_command_accepting(&["merge-tree", "--write-tree", &format!("--merge-base={merge_base}"), baseline, branch_name], repo, &[1])
    {
      let merge_tree_hash = merge_tree_output.trim();
      if let Ok(baseline_tree) = get_tree_id(git, repo, baseline)
        && merge_tree_hash == baseline_tree
      {
        let integrated_at = find_squash_timestamp(git, repo, branch_name, baseline);
        info!(name = %branch_name, method = "merge-tree", "Branch fully integrated");
        return Ok(Some(BranchIntegrationStatus::Integrated {
          integrated_at,
          confidence: IntegrationConfidence::High,
          commit_count: u32::try_from(right_count).unwrap_or(u32::MAX),
        }));
      }
    }
  }

  Ok(None)
}
