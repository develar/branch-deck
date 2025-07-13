use git2::Error;
use tracing::{debug, instrument};

const MAX_COMMITS_TO_SCAN: usize = 50;

#[instrument(skip(repo))]
pub fn get_commit_list<'a>(repo: &'a git2::Repository, main_branch_name: &'a str) -> Result<Vec<git2::Commit<'a>>, Error> {
  // #[instrument] handles logging the function call

  let mut rev_walk = repo.revwalk()?;
  let head = repo.head()?;
  let head_oid = head.target().ok_or_else(|| Error::from_str("HEAD has no target"))?;
  rev_walk.push(head_oid)?;

  // Check if current branch has upstream
  let has_upstream = check_has_upstream(repo)?;

  // Try to find the baseline branch (remote or local)
  let baseline_oid = find_baseline_branch(repo, main_branch_name)?;

  match baseline_oid {
    Some((oid, is_remote)) => {
      if oid == head_oid && !is_remote && !has_upstream {
        // Special case: local branch same as HEAD with no upstream
        // Return commits with branch prefixes for organization
        debug!("No upstream configured, looking for commits with branch prefixes");
        return get_prefixed_commits(repo, &mut rev_walk);
      } else if oid == head_oid {
        // HEAD equals baseline with upstream or remote exists
        debug!("HEAD equals baseline branch, returning empty list");
        return Ok(Vec::new());
      } else {
        // Hide commits reachable from baseline
        rev_walk.hide(oid)?;
      }
    }
    None => {
      debug!("No baseline branch {main_branch_name} found, returning all commits from HEAD");
      // Continue with all commits from HEAD
    }
  }

  // Collect commits ahead of baseline
  let commits = collect_commits_from_revwalk(repo, rev_walk)?;
  debug!(commits_count = commits.len(), branch = %main_branch_name, "found commits ahead of baseline");
  Ok(commits)
}

/// Check if the current branch has an upstream configured
#[instrument(skip(repo))]
fn check_has_upstream(repo: &git2::Repository) -> Result<bool, Error> {
  let current_branch = repo.head()?.shorthand().unwrap_or("HEAD").to_string();

  if current_branch == "HEAD" {
    return Ok(false);
  }

  Ok(
    repo
      .find_branch(&current_branch, git2::BranchType::Local)
      .ok()
      .and_then(|branch| branch.upstream().ok())
      .is_some(),
  )
}

/// Find the baseline branch (remote or local) and return its OID
/// Returns Some((oid, is_remote)) or None if not found
#[instrument(skip(repo))]
fn find_baseline_branch(repo: &git2::Repository, branch_name: &str) -> Result<Option<(git2::Oid, bool)>, Error> {
  // Try remote branch first
  let remote_branch_name = format!("origin/{branch_name}");
  if let Ok(remote_obj) = repo.revparse_single(&remote_branch_name) {
    debug!("Found remote branch: {remote_branch_name}");
    return Ok(Some((remote_obj.id(), true)));
  }

  // Try local branch
  debug!("No remote branch {remote_branch_name} found, trying local branch");
  if let Ok(local_obj) = repo.revparse_single(branch_name) {
    debug!("Found local branch: {branch_name}");
    return Ok(Some((local_obj.id(), false)));
  }

  Ok(None)
}

/// Get commits with branch prefix patterns
#[instrument(skip(repo, rev_walk))]
fn get_prefixed_commits<'a>(repo: &'a git2::Repository, rev_walk: &mut git2::Revwalk<'a>) -> Result<Vec<git2::Commit<'a>>, Error> {
  let mut prefixed_commits = Vec::new();

  // Collect recent commits up to the limit
  for (count, oid) in rev_walk.enumerate() {
    if count >= MAX_COMMITS_TO_SCAN {
      break;
    }

    let commit = repo.find_commit(oid?)?;
    if has_branch_prefix(commit.message()) {
      prefixed_commits.push(commit);
    }
  }

  prefixed_commits.reverse();
  debug!(commits_count = prefixed_commits.len(), "found commits with branch prefixes (no upstream configured)");
  Ok(prefixed_commits)
}

/// Check if a commit message has a branch prefix pattern
#[instrument]
fn has_branch_prefix(message: Option<&str>) -> bool {
  if let Some(msg) = message {
    if msg.starts_with('(') {
      if let Some(close_paren) = msg.find(')') {
        return close_paren > 1; // Ensure content between parentheses
      }
    }
  }
  false
}

/// Collect all commits from the revwalk iterator
#[instrument(skip(repo, rev_walk))]
fn collect_commits_from_revwalk<'a>(repo: &'a git2::Repository, rev_walk: git2::Revwalk<'a>) -> Result<Vec<git2::Commit<'a>>, Error> {
  let mut commits = Vec::new();
  for oid in rev_walk {
    commits.push(repo.find_commit(oid?)?);
  }
  commits.reverse();
  Ok(commits)
}

#[cfg(test)]
#[path = "commit_list_test.rs"]
mod tests;
