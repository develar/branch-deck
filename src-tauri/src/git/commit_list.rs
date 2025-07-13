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
mod tests {
  use super::*;
  use crate::test_utils::git_test_utils::{create_commit, create_test_repo};

  #[test]
  fn test_get_commit_list_with_no_commits_ahead() {
    let (_dir, repo) = create_test_repo();

    // Create only the initial commit
    create_commit(&repo, "Initial commit", "README.md", "# Test");

    // Since there's no remote branch and no commits with prefixes,
    // we should get an empty list
    let commits = get_commit_list(&repo, "master").unwrap();
    assert_eq!(commits.len(), 0, "Should return 0 commits when no prefixed commits exist");
  }

  #[test]
  fn test_get_commit_list_head_equals_local_branch_no_upstream() {
    let (_dir, repo) = create_test_repo();

    // Create multiple commits to simulate a real repository
    create_commit(&repo, "Initial commit", "README.md", "# Test");
    create_commit(&repo, "(feature-auth) Add authentication", "auth.js", "auth code");
    create_commit(&repo, "(bugfix-login) Fix login issue", "login.js", "login fix");
    create_commit(&repo, "Regular commit", "regular.txt", "regular content");
    create_commit(&repo, "(ui-components) Add button", "button.js", "button code");

    // In this scenario:
    // - No origin/master exists (no upstream)
    // - HEAD and master point to the same commit (last commit)
    // - This mimics a local repository without upstream
    let commits = get_commit_list(&repo, "master").unwrap();

    // Should return commits with branch prefixes when no upstream is configured
    assert_eq!(commits.len(), 3, "Should return commits with branch prefixes when no upstream");

    // Verify the commits are the ones with prefixes (in chronological order)
    assert_eq!(commits[0].message().unwrap(), "(feature-auth) Add authentication");
    assert_eq!(commits[1].message().unwrap(), "(bugfix-login) Fix login issue");
    assert_eq!(commits[2].message().unwrap(), "(ui-components) Add button");

    // Verify that HEAD and master indeed point to the same commit
    let head_oid = repo.head().unwrap().target().unwrap();
    let master_oid = repo.revparse_single("master").unwrap().id();
    assert_eq!(head_oid, master_oid, "HEAD and master should point to the same commit");
  }

  #[test]
  fn test_get_commit_list_with_commits_ahead() {
    let (_dir, repo) = create_test_repo();

    // Create an initial commit
    let id1 = create_commit(&repo, "Initial commit", "README.md", "# Test");
    let initial_commit = repo.find_commit(id1).unwrap();

    // Create a baseline branch reference
    repo.branch("baseline", &initial_commit, false).unwrap();

    // Create additional commits ahead of baseline
    let id2 = create_commit(&repo, "(feature-auth) Add authentication", "auth.js", "auth code");
    let id3 = create_commit(&repo, "(feature-auth) Improve auth", "auth.js", "better auth code");

    // Get commits ahead of baseline
    let commits = get_commit_list(&repo, "baseline").unwrap();

    // Should return commits in chronological order (oldest first)
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].id(), id2);
    assert_eq!(commits[1].id(), id3);
    assert_eq!(commits[0].message().unwrap(), "(feature-auth) Add authentication");
    assert_eq!(commits[1].message().unwrap(), "(feature-auth) Improve auth");
  }

  #[test]
  fn test_get_commit_list_with_remote_branch() {
    let (_dir, repo) = create_test_repo();

    // Create an initial commit
    let initial_commit_id = create_commit(&repo, "Initial commit", "README.md", "# Test");

    // Create a remote branch reference to simulate origin/master
    let initial_commit = repo.find_commit(initial_commit_id).unwrap();
    repo.branch("origin/master", &initial_commit, false).unwrap();

    // Create commits ahead of origin/master
    let id2 = create_commit(&repo, "(bugfix-login) Fix login issue", "login.js", "fixed login");
    let id3 = create_commit(&repo, "(ui-components) Add button", "button.vue", "<button></button>");

    // Get commits ahead of origin/master
    let commits = get_commit_list(&repo, "master").unwrap();

    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].id(), id2);
    assert_eq!(commits[1].id(), id3);
  }

  #[test]
  fn test_get_commit_list_handles_missing_branch() {
    let (_dir, repo) = create_test_repo();

    // Create some commits
    create_commit(&repo, "Initial commit", "README.md", "# Test");
    create_commit(&repo, "(feature-test) Test feature", "test.js", "test code");

    // Try to get commits against a non-existent branch
    let result = get_commit_list(&repo, "nonexistent-branch");

    // Should succeed and return all commits from HEAD since neither remote nor local branch exists
    // This behavior allows the function to work even when the specified baseline branch doesn't exist
    assert!(result.is_ok());
    let commits = result.unwrap();
    assert_eq!(commits.len(), 2, "Should return all commits when baseline branch doesn't exist");
  }

  #[test]
  fn test_get_commit_list_preserves_commit_order() {
    let (_dir, repo) = create_test_repo();

    // Create an initial commit and branch it
    let initial_id = create_commit(&repo, "Initial commit", "README.md", "# Test");
    let initial_commit = repo.find_commit(initial_id).unwrap();
    repo.branch("baseline", &initial_commit, false).unwrap();

    // Create multiple commits in sequence
    let messages = [
      "(feature-auth) First auth commit",
      "(feature-auth) Second auth commit",
      "(bugfix-login) Login fix",
      "(feature-auth) Third auth commit",
      "(ui-components) UI commit",
    ];

    let mut commit_ids = Vec::new();
    for (i, message) in messages.iter().enumerate() {
      let id = create_commit(&repo, message, &format!("file{i}.txt"), &format!("content {i}"));
      commit_ids.push(id);
    }

    // Get commits ahead of baseline
    let commits = get_commit_list(&repo, "baseline").unwrap();

    // Verify all commits are present and in chronological order (oldest first)
    assert_eq!(commits.len(), 5);
    for (i, commit) in commits.iter().enumerate() {
      assert_eq!(commit.id(), commit_ids[i]);
      assert_eq!(commit.message().unwrap(), messages[i]);
    }
  }
}
