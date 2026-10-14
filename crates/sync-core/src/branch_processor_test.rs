use crate::branch_processor::{BranchJob, BranchSyncContext, process_single_branch};
use crate::commit_grouper::CommitGrouper;
use crate::sync::{SyncOptions, sync_branches};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::{Commit, get_commit_list};
use git_ops::model::CommitSyncStatus;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Mutex;
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use test_utils::git_test_utils::TestRepo;

fn repo_path(test_repo: &TestRepo) -> &str {
  test_repo.path().to_str().unwrap()
}

/// Syncs with the `test` branch prefix and returns the statuses of the copied commits, by branch, oldest first
async fn sync(test_repo: &TestRepo) -> BTreeMap<String, Vec<CommitSyncStatus>> {
  let progress = TestReporter::new();
  sync_branches(test_repo.git_executor(), repo_path(test_repo), "test", progress.clone(), SyncOptions::default())
    .await
    .unwrap();
  let mut statuses: BTreeMap<String, Vec<CommitSyncStatus>> = BTreeMap::new();
  for event in progress.get_events() {
    if let SyncEvent::CommitSynced { branch_name, status, .. } = event {
      statuses.entry(branch_name).or_default().push(status);
    }
  }
  statuses
}

/// The commits of the virtual branch, oldest first
fn virtual_commits(test_repo: &TestRepo, branch: &str) -> Vec<String> {
  test_repo
    .log(&["--reverse", "--format=%H", &format!("master..test/virtual/{branch}")])
    .unwrap()
    .lines()
    .map(str::to_string)
    .collect()
}

/// The commits of a branch as the sync groups them, oldest first
fn grouped_commits(test_repo: &TestRepo, branch: &str) -> Vec<Commit> {
  let mut grouper = CommitGrouper::new();
  for commit in get_commit_list(test_repo.git_executor(), repo_path(test_repo), "master").unwrap() {
    grouper.add_commit(commit);
  }
  grouper.finish().branches.swap_remove(branch).unwrap()
}

/// Rebuilds the virtual branch `branch` on `base`, as a sync does
fn rebuild_branch(test_repo: &TestRepo, branch: &str, base: &str) {
  let context = BranchSyncContext {
    repository_path: repo_path(test_repo).to_string(),
    branch_prefix: "test".to_string(),
    parent_commit_hash: base.to_string(),
    baseline_branch: "master".to_string(),
    total_branches: 1,
    progress: TestReporter::new(),
    git_executor: GitCommandExecutor::new(),
    tree_id_cache: TreeIdCache::new(),
    git_notes_mutex: Mutex::new(()),
  };
  let job = BranchJob {
    branch_name: branch.to_string(),
    commits: grouped_commits(test_repo, branch),
    my_email: None,
    branch_idx: 0,
  };
  process_single_branch(&context, job).unwrap();
}

#[test]
fn a_new_base_recopies_the_commits_whose_mapped_commits_still_exist() {
  let test_repo = TestRepo::new();
  let initial = test_repo.create_commit("Initial commit", "README.md", "# Test");
  let base = test_repo.create_commit("Update docs", "docs.md", "docs");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");

  rebuild_branch(&test_repo, "auth", &initial);
  let on_initial = virtual_commits(&test_repo, "auth");
  assert_eq!(test_repo.rev_parse("test/virtual/auth~2").unwrap(), initial);

  // the notes still map both commits to the copies on `initial`, which are still in the branch
  rebuild_branch(&test_repo, "auth", &base);

  assert_eq!(test_repo.rev_parse("test/virtual/auth~2").unwrap(), base);
  let on_base = virtual_commits(&test_repo, "auth");
  assert_eq!(on_base.len(), 2);
  assert!(on_base.iter().all(|id| !on_initial.contains(id)), "{on_base:?} reuses a commit of {on_initial:?}");
}

#[tokio::test]
async fn a_rebuild_without_notes_writes_the_same_commits() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  test_repo.create_commit("(cache) Add cache", "cache.rs", "// cache");
  test_repo.create_commit("(auth) Add logout\n\nClears the session.", "logout.rs", "// logout");

  sync(&test_repo).await;
  let auth = virtual_commits(&test_repo, "auth");
  let cache = virtual_commits(&test_repo, "cache");

  // a later second, so a committer date of "now" would change the ids
  std::thread::sleep(std::time::Duration::from_millis(1100));
  test_repo
    .git_executor()
    .execute_command(&["update-ref", "-d", "refs/notes/commits"], repo_path(&test_repo))
    .unwrap();
  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Created, CommitSyncStatus::Created]);
  assert_eq!(virtual_commits(&test_repo, "auth"), auth);
  assert_eq!(virtual_commits(&test_repo, "cache"), cache);

  // also from scratch
  test_repo.delete_branch("test/virtual/auth").unwrap();
  test_repo.delete_branch("test/virtual/cache").unwrap();
  test_repo
    .git_executor()
    .execute_command(&["update-ref", "-d", "refs/notes/commits"], repo_path(&test_repo))
    .unwrap();
  sync(&test_repo).await;
  assert_eq!(virtual_commits(&test_repo, "auth"), auth);
  assert_eq!(virtual_commits(&test_repo, "cache"), cache);
}
