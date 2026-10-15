use crate::branch_processor::{BranchJob, BranchSyncContext, process_single_branch};
use crate::commit_grouper::CommitGrouper;
use crate::move_commits::{MoveCommitsParams, move_commits_to_branch_core};
use crate::reword_commit::{CommitRewrite, RewordCommitsRequest, reword_commits_core};
use crate::sync::{SyncOptions, sync_branches};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::{Commit, get_commit_list};
use git_ops::model::CommitSyncStatus;
use git_ops::notes::write_commit_notes;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::HashMap;
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
    baseline_branch: "master".to_string(),
    total_branches: 1,
    progress: TestReporter::new(),
    git_executor: GitCommandExecutor::new(),
    tree_id_cache: TreeIdCache::new(),
    existing_tips: existing_tips(test_repo),
    ancestor_keys: HashMap::new(),
  };
  let job = BranchJob {
    branch_name: branch.to_string(),
    commits: grouped_commits(test_repo, branch),
    my_email: None,
    branch_idx: 0,
    base_commit: base.to_string(),
    parent_ref: None,
  };
  let notes = process_single_branch(&context, job).unwrap();
  write_commit_notes(test_repo.git_executor(), repo_path(test_repo), &notes).unwrap();
}

/// The virtual branches and their tips, as a sync lists them
fn existing_tips(test_repo: &TestRepo) -> HashMap<String, String> {
  let lines = test_repo
    .git_executor()
    .execute_command_lines(
      &["for-each-ref", "--format=%(refname:lstrip=2) %(objectname)", "refs/heads/test/virtual/"],
      repo_path(test_repo),
    )
    .unwrap();
  lines
    .iter()
    .filter_map(|line| line.split_once(' '))
    .map(|(name, tip)| (name.to_string(), tip.to_string()))
    .collect()
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

#[tokio::test]
async fn assigning_an_older_commit_reuses_every_virtual_commit() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  let unassigned = test_repo.create_commit("Fix typo", "typo.txt", "typo");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  test_repo.create_commit("(cache) Add cache", "cache.rs", "// cache");
  test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");

  sync(&test_repo).await;
  let auth = virtual_commits(&test_repo, "auth");
  let cache = virtual_commits(&test_repo, "cache");

  // recreates every commit after it
  let params = MoveCommitsParams::new(repo_path(&test_repo), vec![unassigned], Some("docs"));
  move_commits_to_branch_core(test_repo.git_executor(), &params).unwrap();

  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Unchanged, CommitSyncStatus::Unchanged]);
  assert_eq!(statuses["cache"], [CommitSyncStatus::Unchanged]);
  assert_eq!(statuses["docs"], [CommitSyncStatus::Created]);
  assert_eq!(virtual_commits(&test_repo, "auth"), auth);
  assert_eq!(virtual_commits(&test_repo, "cache"), cache);
}

#[tokio::test]
async fn moving_a_commit_to_another_branch_copies_it_there() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  test_repo.create_commit("(cache) Add cache", "cache.rs", "// cache");
  let logout = test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");

  sync(&test_repo).await;
  let auth = virtual_commits(&test_repo, "auth");
  let cache = virtual_commits(&test_repo, "cache");

  // only the prefix changes: the note moves with the commit, but its virtual commit is in another branch
  let params = MoveCommitsParams::new(repo_path(&test_repo), vec![logout], Some("cache"));
  move_commits_to_branch_core(test_repo.git_executor(), &params).unwrap();

  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Unchanged]);
  assert_eq!(statuses["cache"], [CommitSyncStatus::Unchanged, CommitSyncStatus::Created]);
  assert_eq!(virtual_commits(&test_repo, "auth"), auth[..1]);
  let moved = virtual_commits(&test_repo, "cache");
  assert_eq!(moved[0], cache[0]);
  assert_eq!(test_repo.log(&["-1", "--format=%s", &moved[1]]).unwrap(), "Add logout");
  assert!(test_repo.get_files_in_commit(&moved[1]).unwrap().contains(&"logout.rs".to_string()));
}

#[tokio::test]
async fn a_reworded_body_recopies_the_commit() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  let login = test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  test_repo.create_commit("(cache) Add cache", "cache.rs", "// cache");

  sync(&test_repo).await;
  let cache = virtual_commits(&test_repo, "cache");

  let request = RewordCommitsRequest {
    repository_path: repo_path(&test_repo).to_string(),
    rewrites: vec![CommitRewrite {
      commit_id: login,
      message: "(auth) Add login\n\nWith a remember-me box.".to_string(),
    }],
  };
  reword_commits_core(test_repo.git_executor(), &request).unwrap();

  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Created]);
  assert_eq!(statuses["cache"], [CommitSyncStatus::Unchanged]);
  assert_eq!(virtual_commits(&test_repo, "cache"), cache);
  assert_eq!(
    test_repo.log(&["-1", "--format=%B", "test/virtual/auth"]).unwrap().trim_end(),
    "Add login\n\nWith a remember-me box."
  );
}

#[tokio::test]
async fn a_copy_keeps_the_timezones_of_the_original() {
  let test_repo = TestRepo::new();
  // older than the original, so the copy's committer date is the original's
  test_repo.create_commit_with_timestamp("Initial commit", "README.md", "# Test", Some(1_600_000_000));
  std::fs::write(test_repo.path().join("login.rs"), "// login").unwrap();
  test_repo.git_executor().execute_command(&["add", "login.rs"], repo_path(&test_repo)).unwrap();
  let env = [("GIT_AUTHOR_DATE", "1700000000 +0530"), ("GIT_COMMITTER_DATE", "1700000100 -0700")];
  test_repo
    .git_executor()
    .execute_command_with_env(&["commit", "-m", "(auth) Add login"], repo_path(&test_repo), &env)
    .unwrap();

  sync(&test_repo).await;

  let dates = test_repo.log(&["-1", "--format=%ad|%cd", "--date=raw", "test/virtual/auth"]).unwrap();
  assert_eq!(dates.trim(), "1700000000 +0530|1700000100 -0700");
}

#[tokio::test]
async fn a_sync_writes_the_notes_of_all_branches_in_one_notes_commit() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  for branch in ["auth", "cache", "docs"] {
    test_repo.create_commit(&format!("({branch}) Add {branch}"), &format!("{branch}.rs"), branch);
  }

  sync(&test_repo).await;

  assert_eq!(test_repo.log(&["--format=%H", "refs/notes/commits"]).unwrap().lines().count(), 1);
  assert_eq!(test_repo.list_notes_with_ref("refs/notes/commits").unwrap().len(), 3);
}
