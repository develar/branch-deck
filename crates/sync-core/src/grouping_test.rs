use crate::baseline::resolve_baseline_upstream;
use crate::grouping::group_commits_ahead_of_baseline;
use branch_integration::common::list_branches;
use domain_types::rewrite::AutosquashScope;
use history_rewrite::autosquash::{SquashFixupsParams, squash_fixup_commits_core};
use history_rewrite::baseline::NoBaseline;
use history_rewrite::repository_lock::RewriteOptions;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::history::HistoryCommit;

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

/// The branches come newest first by where each begins in the history, not by a time: a squash of the fixup of the
/// oldest commit gives every commit above it a new committer time, and moves no branch; a commit added to a branch
/// moves none either, and a new branch comes first
#[test]
fn the_branches_come_newest_first_by_their_oldest_commit_and_no_rewrite_of_their_commits_moves_one() {
  let (_upstream, repo) = TestRepo::upstream_and_clone(&[("README.md", "readme")]);
  repo.commit_history(
    "refs/heads/master",
    &[
      HistoryCommit::new("(auth) IJAI-101 Validate the password", &[("auth.txt", "validate\n")]).at(1_700_000_100),
      HistoryCommit::new("(docs) Write the guide", &[("docs.txt", "guide\n")]).at(1_700_000_200),
      HistoryCommit::new("(search) Index the names", &[("search.txt", "index\n")]).at(1_700_000_300),
      HistoryCommit::new("fixup! (auth) IJAI-101 Validate the password", &[("auth.txt", "validate, fixed\n")]).at(1_700_000_400),
    ],
  );
  let order = |repo: &TestRepo| -> Vec<String> {
    let grouping = group_commits_ahead_of_baseline(repo.git_executor(), repo.path_str(), "test").unwrap();
    grouping.branches.into_iter().map(|branch| branch.name).collect()
  };
  assert_eq!(
    order(&repo),
    ["search", "docs", "auth"],
    "auth began first, so it comes last though its fixup is the newest commit"
  );

  let squashed = squash_fixup_commits_core(
    repo.git_executor(),
    &SquashFixupsParams {
      repository_path: repo.path_str().to_string(),
      scope: AutosquashScope::All,
    },
    &RewriteOptions::default(),
  )
  .unwrap();
  assert_eq!(squashed.squashed_count, 1);
  assert_eq!(
    order(&repo),
    ["search", "docs", "auth"],
    "docs and search got a new committer time, auth lost its newest commit"
  );

  repo.commit_history(
    "refs/heads/master",
    &[
      HistoryCommit::new("(auth) Lock the account", &[("lock.txt", "lock\n")]),
      HistoryCommit::new("(api) Add the endpoint", &[("api.txt", "endpoint\n")]),
    ],
  );
  assert_eq!(order(&repo), ["api", "search", "docs", "auth"]);
}
