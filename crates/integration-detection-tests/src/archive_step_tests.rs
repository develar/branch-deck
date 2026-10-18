//! Tests for the archive step of a sync: which branches it moves into the archive and how it fails

use crate::test_helpers::{setup_test_repos, sync_user_branches};
use branch_integration::common::get_all_branch_data;
use branch_integration::detector::archive_and_clean_up;
use branch_integration::strategy::DetectionStrategy;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use sync_utils::timestamp::utc_today;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, git_command};

/// A clone with one commit and the virtual branch `user/virtual/auth` at it: (upstream, clone, git, commit)
fn repo_with_virtual_auth() -> (TestRepo, TestRepo, GitCommandExecutor, String) {
  let (upstream_repo, local_repo, git_executor) = setup_test_repos();
  let commit = local_repo.create_commit("(auth) Add auth", "auth.js", "// auth");
  local_repo.create_branch_at("user/virtual/auth", &commit).unwrap();
  (upstream_repo, local_repo, git_executor, commit)
}

fn sorted(names: impl IntoIterator<Item = String>) -> Vec<String> {
  let mut names: Vec<String> = names.into_iter().collect();
  names.sort_unstable();
  names
}

/// A group name may contain `virtual/` or `archived/`: a branch is virtual or archived by its namespace only, and an
/// unapplied branch is neither, so it is never archived nor cleaned up. A stack branch is not even listed: its tip,
/// which may be the tip of an archived branch too, must not name that branch's detection note
#[test]
fn branches_are_classified_by_their_namespace() {
  let (_upstream_repo, local_repo, git_executor) = setup_test_repos();
  let repo_path = local_repo.path().to_str().unwrap();
  let commit = local_repo.create_commit("(auth) Add auth", "auth.js", "// auth");
  let archived = "user/archived/2026-10-01/a/virtual/b";
  for branch in [
    "user/virtual/c/archived/d",
    archived,
    "user/unapplied/x/archived/y",
    "user/unapplied/x/virtual/y",
    "user/stack/auth",
  ] {
    local_repo.create_branch_at(branch, &commit).unwrap();
  }
  // a tag of the same name makes `%(refname:short)` `heads/<name>`
  let status = git_command().args(["tag", archived, &commit]).current_dir(local_repo.path()).status().unwrap();
  assert!(status.success());

  let data = get_all_branch_data(&git_executor, repo_path, "user").unwrap();
  assert_eq!(sorted(data.virtual_commits.into_keys()), ["user/virtual/c/archived/d"]);
  assert_eq!(data.archived_all, [archived]);
  assert!(!data.all_branch_commits.contains_key("user/stack/auth"), "{:?}", data.all_branch_commits.keys());
  assert!(data.all_branch_commits.contains_key("user/unapplied/x/virtual/y"));
}

/// A virtual branch checked out in another worktree stays: deleting it would leave that worktree on an unborn branch
#[test]
fn a_branch_checked_out_in_a_worktree_is_not_archived() {
  let (_upstream_repo, local_repo, git_executor, commit) = repo_with_virtual_auth();
  let repo_path = local_repo.path().to_str().unwrap();
  local_repo.create_branch_at("user/virtual/docs", &commit).unwrap();
  let other = TestRepo::new_empty();
  let worktree = other.path().join("worktree");
  let status = git_command()
    .args(["worktree", "add", worktree.to_str().unwrap(), "user/virtual/auth"])
    .current_dir(local_repo.path())
    .status()
    .unwrap();
  assert!(status.success());

  let progress = TestReporter::new();
  archive_and_clean_up(&git_executor, repo_path, "user", &HashSet::new(), 7, &progress).unwrap();

  assert!(local_repo.branch_exists("user/virtual/auth"), "the checked out branch stays");
  assert!(!local_repo.branch_exists("user/virtual/docs"));
  assert!(local_repo.branch_exists(&format!("user/archived/{}/docs", utc_today())));
}

/// The archive step is best effort: when the archive transaction fails, the virtual branch stays for a later sync and
/// the archived branches are still reported
#[test]
fn a_failed_archive_still_reports_the_archived_branches() {
  let (_upstream_repo, local_repo, git_executor, commit) = repo_with_virtual_auth();
  let repo_path = local_repo.path().to_str().unwrap();
  let today = utc_today();
  // a ref at the level of the day: no archive of today can be created below it
  local_repo.create_branch_at(&format!("user/archived/{today}"), &commit).unwrap();
  local_repo.create_branch_at("user/archived/2026-10-01/docs", &commit).unwrap();

  let progress = TestReporter::new();
  archive_and_clean_up(&git_executor, repo_path, "user", &HashSet::new(), 7, &progress).unwrap();

  assert!(local_repo.branch_exists("user/virtual/auth"), "nothing is archived");
  let found: Vec<(Vec<String>, Vec<String>)> = progress
    .get_events()
    .into_iter()
    .filter_map(|event| match event {
      SyncEvent::ArchivedBranchesFound { branch_names, newly_archived } => Some((sorted(branch_names), newly_archived)),
      _ => None,
    })
    .collect();
  assert_eq!(found, [(sorted([format!("user/archived/{today}"), "user/archived/2026-10-01/docs".to_string()]), vec![])]);
}

/// A group renamed from `(foo)` to `(foo/bar)`: its virtual branch is archived before `foo/bar` is built, which git's
/// directory/file rule forbids beside `foo`
#[test(tokio::test)]
async fn a_group_renamed_below_its_old_name_syncs() {
  let (_upstream_repo, local_repo, git_executor) = setup_test_repos();
  local_repo.create_commit("(foo) Add foo", "foo.js", "// foo");
  sync_user_branches(&git_executor, &local_repo, DetectionStrategy::Rebase).await.unwrap();
  assert!(local_repo.branch_exists("user/virtual/foo"));

  local_repo.reset_hard("HEAD~1").unwrap();
  local_repo.create_commit("(foo/bar) Add foo", "foo.js", "// foo");
  sync_user_branches(&git_executor, &local_repo, DetectionStrategy::Rebase).await.unwrap();

  assert!(local_repo.branch_exists("user/virtual/foo/bar"));
  assert!(!local_repo.branch_exists("user/virtual/foo"));
  assert!(local_repo.branch_exists(&format!("user/archived/{}/foo", utc_today())));

  // and back: `foo/bar` is archived before `foo` is built
  local_repo.reset_hard("HEAD~1").unwrap();
  local_repo.create_commit("(foo) Add foo again", "foo.js", "// foo again");
  sync_user_branches(&git_executor, &local_repo, DetectionStrategy::Rebase).await.unwrap();
  assert!(local_repo.branch_exists("user/virtual/foo"));
  assert!(!local_repo.branch_exists("user/virtual/foo/bar"));
}
