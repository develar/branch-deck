use crate::baseline::resolve_baseline_upstream;
use crate::grouping::group_commits_ahead_of_baseline;
use branch_integration::common::list_branches;
use history_rewrite::baseline::NoBaseline;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The grouping says what the separate readers say: HEAD and its branch, the upstream of `resolve_baseline_upstream`,
/// the branches of `list_branches` and the remote ones under the prefix
#[test]
fn the_grouping_names_head_the_upstream_and_the_branches_under_the_prefix() {
  let (_upstream, repo) = TestRepo::upstream_and_clone(&[("README.md", "readme")]);
  let base = repo.head();
  let login = repo.create_commit("(auth) Add login", "login.txt", "login");
  repo.git(&["update-ref", "refs/heads/test/virtual/auth", &login]);
  repo.git(&["update-ref", "refs/remotes/origin/test/virtual/auth", &base]);

  let grouping = group_commits_ahead_of_baseline(repo.git_executor(), repo.path_str(), "test").unwrap();
  assert_eq!((grouping.head.as_str(), grouping.head_branch.as_deref()), (login.as_str(), Some("master")));
  assert_eq!(grouping.baseline_branch, "origin/master");
  assert_eq!(grouping.upstream, resolve_baseline_upstream(repo.git_executor(), repo.path_str(), "origin/master").unwrap());
  assert_eq!(grouping.listed, list_branches(repo.git_executor(), repo.path_str(), "test").unwrap());
  assert_eq!(grouping.remote_tips, HashMap::from([("test/virtual/auth".to_string(), base)]));
  assert_eq!(grouping.branches.iter().map(|branch| branch.name.as_str()).collect::<Vec<_>>(), ["auth"]);

  // a detached HEAD has no branch, and still its baseline
  repo.git(&["checkout", "-q", "--detach"]);
  let grouping = group_commits_ahead_of_baseline(repo.git_executor(), repo.path_str(), "test").unwrap();
  assert_eq!((grouping.head.as_str(), grouping.head_branch), (login.as_str(), None));
  assert_eq!(grouping.baseline_branch, "origin/master");
}

#[test]
fn a_branch_without_commits_has_no_baseline() {
  let repo = TestRepo::new_empty();
  repo.git(&["init", "-q", "-b", "master"]);
  let error = group_commits_ahead_of_baseline(repo.git_executor(), repo.path_str(), "test").err().unwrap();
  assert_eq!(error.downcast_ref::<NoBaseline>(), Some(&NoBaseline::NoCommits), "{error:#}");
}
