use crate::baseline::resolve_baseline_upstream;
use crate::repo_facts::{RepoFacts, read_baseline_listing, read_repo_facts};
use branch_integration::common::{ListedBranch, list_branches};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::baseline::{NoBaseline, detect_baseline_branch};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use test_utils::git_test_utils::{TestRepo, git_command};

/// The facts of a sync of `test` on the checked out branch, and how many git processes they cost
fn facts(repo: &TestRepo) -> (anyhow::Result<RepoFacts>, u32) {
  let git = GitCommandExecutor::new().with_process_stats("test");
  let branch = repo.current_branch().unwrap();
  let facts = read_repo_facts(&git, repo.path_str(), &branch, "test");
  let processes = git.process_stats().unwrap().total();
  (facts, processes)
}

/// The facts are what the separate readers say: the baseline of `detect_baseline_branch`, the upstream of
/// `resolve_baseline_upstream`, the commits of HEAD and the baseline, the branches of `list_branches` and the remote
/// virtual branches; returns the baseline
#[track_caller]
fn assert_facts(repo: &TestRepo, processes: u32) -> String {
  let (read, count) = facts(repo);
  let facts = read.unwrap();
  let git = repo.git_executor();
  let baseline = detect_baseline_branch(git, repo.path_str()).unwrap();
  assert_eq!(facts.baseline, baseline);
  assert_eq!(facts.upstream, resolve_baseline_upstream(git, repo.path_str(), &baseline).unwrap());
  assert_eq!(facts.baseline_commit, repo.rev_parse(&format!("{baseline}^{{commit}}")).unwrap());
  assert_eq!(facts.head, repo.head());
  assert_eq!(facts.branches, list_branches(git, repo.path_str(), "test").unwrap());
  let remote: HashMap<String, String> = repo
    .git(&["for-each-ref", "--format=%(refname:lstrip=3) %(objectname)", "refs/remotes/origin/test/"])
    .lines()
    .filter_map(|line| line.split_once(' ').map(|(name, tip)| (name.to_string(), tip.to_string())))
    .collect();
  assert_eq!(facts.remote_tips, remote);
  assert_eq!(count, processes, "git processes");
  baseline
}

#[test]
fn a_local_baseline() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.checkout_new_branch("feature").unwrap();
  repo.create_commit("(auth) Add login", "login.txt", "login");
  assert_eq!(assert_facts(&repo, 3), "master");
}

#[test]
fn the_master_of_the_first_remote_with_the_branches_under_the_prefix() {
  let repo = TestRepo::new();
  let initial = repo.create_commit("Initial commit", "README.md", "# Test");
  let head = repo.create_commit("(auth) Add login", "login.txt", "login");
  repo.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/master", &initial]);
  repo.git(&["update-ref", "refs/heads/test/virtual/auth", &head]);
  repo.git(&["update-ref", "refs/heads/test/archived/2026-10-01/old", &initial]);
  repo.git(&["update-ref", "refs/remotes/origin/test/virtual/auth", &initial]);
  // another prefix and other remote branches are not listed
  repo.git(&["update-ref", "refs/heads/other/virtual/auth", &head]);
  repo.git(&["update-ref", "refs/remotes/origin/feature", &head]);
  assert_eq!(assert_facts(&repo, 3), "origin/master");
}

/// master with a commit ahead of `origin/master` and `origin/develop`, tracking `origin/develop`
fn tracking_develop() -> TestRepo {
  let repo = TestRepo::new();
  let initial = repo.create_commit("Initial commit", "README.md", "# Test");
  repo.create_commit("(auth) Add login", "login.txt", "login");
  repo.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/master", &initial]);
  repo.git(&["update-ref", "refs/remotes/origin/develop", &initial]);
  repo.git(&["config", "branch.master.remote", "origin"]);
  repo.git(&["config", "branch.master.merge", "refs/heads/develop"]);
  repo
}

#[test]
fn the_upstream_of_the_checked_out_branch() {
  let repo = tracking_develop();
  // the upstream is no candidate: one `rev-parse` more tells that it exists
  assert_eq!(assert_facts(&repo, 4), "origin/develop");

  // the upstream is a candidate: the listing has it
  repo.git(&["config", "branch.master.merge", "refs/heads/master"]);
  assert_eq!(assert_facts(&repo, 3), "origin/master");
}

#[test]
fn a_detached_head_tracks_nothing() {
  let repo = tracking_develop();
  repo.git(&["checkout", "-q", "--detach"]);

  let git = GitCommandExecutor::new().with_process_stats("test");
  let listing = read_baseline_listing(&git, repo.path_str(), None, "test", false).unwrap();
  // the upstream of master is not HEAD's: the first candidate, as `detect_baseline_branch` says
  assert_eq!(listing.baseline, detect_baseline_branch(repo.git_executor(), repo.path_str()).unwrap());
  assert_eq!((listing.baseline.as_str(), listing.baseline_ref.as_str()), ("origin/master", "refs/remotes/origin/master"));
  assert_eq!(listing.upstream, resolve_baseline_upstream(repo.git_executor(), repo.path_str(), "origin/master").unwrap());
  let processes = git.process_stats().unwrap().total();
  assert_eq!(processes, 2, "remote and for-each-ref");
}

#[test]
fn an_upstream_not_fetched_falls_back_to_the_candidates() {
  let repo = TestRepo::new();
  let initial = repo.create_commit("Initial commit", "README.md", "# Test");
  repo.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/main", &initial]);
  repo.git(&["config", "branch.master.remote", "origin"]);
  repo.git(&["config", "branch.master.merge", "refs/heads/develop"]);
  assert_eq!(assert_facts(&repo, 4), "origin/main");
}

#[test]
fn no_baseline_and_no_commits() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.git(&["remote", "add", "origin", "."]);
  let (facts, _) = facts(&repo);
  let error = facts.unwrap_err();
  assert_eq!(
    error.downcast_ref::<NoBaseline>(),
    Some(&NoBaseline::NoRemoteBranch {
      tried: vec!["origin/master".to_string(), "origin/main".to_string()],
    })
  );

  let empty = TestRepo::new_empty();
  empty.git(&["init", "-q", "-b", "master"]);
  let git = GitCommandExecutor::new();
  let error = read_repo_facts(&git, empty.path_str(), "master", "test").unwrap_err();
  assert_eq!(error.downcast_ref::<NoBaseline>(), Some(&NoBaseline::NoCommits));
  // a remote baseline that exists does not hide the missing commits
  empty.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/master", &repo.head()]);
  empty.git(&["fetch", "-q", repo.path_str(), "+refs/heads/master:refs/remotes/origin/master"]);
  let error = read_repo_facts(&git, empty.path_str(), "master", "test").unwrap_err();
  assert_eq!(error.downcast_ref::<NoBaseline>(), Some(&NoBaseline::NoCommits));
}

/// The listing reads refs, no commit: a `for-each-ref` of their subjects or by their dates (`--sort=-committerdate`)
/// reads the commit of every listed ref (on the monorepo 452, 1 to 2 s from a cold pack before the first rows of a sync)
/// and fails on one whose commit is gone; the listing of a sync lists it with its tip
#[test]
fn the_listing_reads_no_commit_of_the_branches_it_lists() {
  let repo = TestRepo::new();
  let initial = repo.create_commit("Initial commit", "README.md", "# Test");
  repo.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/master", &initial]);
  let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
  let gone = repo.git(&["commit-tree", "-m", "(old) Gone", tree.trim()]).trim().to_string();
  repo.git(&["update-ref", "refs/heads/test/archived/2026-10-01/old", &gone]);
  repo.git(&["update-ref", "refs/remotes/origin/test/virtual/old", &gone]);
  std::fs::remove_file(repo.path().join(".git/objects").join(&gone[..2]).join(&gone[2..])).unwrap();
  let by_date = git_command()
    .args(["for-each-ref", "--sort=-committerdate", "refs/heads/test/"])
    .current_dir(repo.path())
    .output()
    .unwrap();
  assert!(!by_date.status.success(), "the commit is gone");

  let (facts, processes) = facts(&repo);
  let facts = facts.unwrap();
  let archived = ListedBranch {
    name: "test/archived/2026-10-01/old".to_string(),
    tip: gone.clone(),
    checked_out: false,
  };
  assert_eq!(facts.branches, [archived]);
  assert_eq!(facts.remote_tips, HashMap::from([("test/virtual/old".to_string(), gone)]));
  assert_eq!(processes, 3, "git processes");
}
