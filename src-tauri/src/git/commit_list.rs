use git2::Error;

pub fn get_commit_list<'a>(repo: &'a git2::Repository, main_branch_name: &'a str) -> Result<Vec<git2::Commit<'a>>, Error> {
  let mut rev_walk = repo.revwalk()?;

  // push HEAD to start walking from
  let head = repo.head()?;
  let head_oid = head.target().ok_or_else(|| Error::from_str("HEAD has no target"))?;
  rev_walk.push(head_oid)?;

  // try to find the remote branch first
  let remote_branch_name = format!("origin/{main_branch_name}");
  match repo.revparse_single(&remote_branch_name) {
    Ok(remote_obj) => {
      // Remote branch exists, hide commits reachable from it
      rev_walk.hide(remote_obj.id())?;
    }
    Err(_) => {
      // No remote branch found, try local branch
      if let Ok(local_obj) = repo.revparse_single(main_branch_name) {
        // Check if local branch is the same as HEAD
        if local_obj.id() == head_oid {
          // Local branch and HEAD are the same, return empty list
          // This is equivalent to `git log master..HEAD` when both point to the same commit
          return Ok(Vec::new());
        }
        // Local branch exists and is different from HEAD, hide commits reachable from it
        rev_walk.hide(local_obj.id())?;
      } else {
        // Neither remote nor local branch found, return all commits from HEAD
        // This might be useful for repositories without the specified branch
      }
    }
  }

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
  use crate::test_utils::git_test_utils::{create_test_repo, create_commit};

  #[test]
  fn test_get_commit_list_with_no_commits_ahead() {
    let (_dir, repo) = create_test_repo();
    
    // Create only the initial commit
    create_commit(&repo, "Initial commit", "README.md", "# Test");
    
    // Since there's no remote branch and we're comparing against the same branch (HEAD == master),
    // we should get an empty list. This is equivalent to `git log master..HEAD` when both point 
    // to the same commit.
    let commits = get_commit_list(&repo, "master").unwrap();
    assert_eq!(commits.len(), 0, "Should return 0 commits when HEAD equals master");
  }

  #[test]
  fn test_get_commit_list_head_equals_local_branch() {
    let (_dir, repo) = create_test_repo();
    
    // Create multiple commits to simulate a real repository
    create_commit(&repo, "Initial commit", "README.md", "# Test");
    create_commit(&repo, "[feature-auth] Add authentication", "auth.js", "auth code");
    create_commit(&repo, "[bugfix-login] Fix login issue", "login.js", "login fix");
    create_commit(&repo, "Regular commit", "regular.txt", "regular content");
    
    // In this scenario:
    // - No origin/master exists
    // - HEAD and master point to the same commit (last commit)
    // - This mimics the /private/tmp/test-git-repo situation
    let commits = get_commit_list(&repo, "master").unwrap();
    assert_eq!(commits.len(), 0, "Should return 0 commits when HEAD equals local master branch");
    
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
    let id2 = create_commit(&repo, "[feature-auth] Add authentication", "auth.js", "auth code");
    let id3 = create_commit(&repo, "[feature-auth] Improve auth", "auth.js", "better auth code");
    
    // Get commits ahead of baseline
    let commits = get_commit_list(&repo, "baseline").unwrap();
    
    // Should return commits in chronological order (oldest first)
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].id(), id2);
    assert_eq!(commits[1].id(), id3);
    assert_eq!(commits[0].message().unwrap(), "[feature-auth] Add authentication");
    assert_eq!(commits[1].message().unwrap(), "[feature-auth] Improve auth");
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
    let id2 = create_commit(&repo, "[bugfix-login] Fix login issue", "login.js", "fixed login");
    let id3 = create_commit(&repo, "[ui-components] Add button", "button.vue", "<button></button>");
    
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
    create_commit(&repo, "[feature-test] Test feature", "test.js", "test code");
    
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
    let messages = ["[feature-auth] First auth commit",
      "[feature-auth] Second auth commit", 
      "[bugfix-login] Login fix",
      "[feature-auth] Third auth commit",
      "[ui-components] UI commit"];
    
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
