use crate::baseline::{NoBaseline, detect_baseline_branch};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn detect(repo: &TestRepo, preferred_branch: &str) -> anyhow::Result<String> {
  detect_baseline_branch(repo.git_executor(), repo.path().to_str().unwrap(), preferred_branch)
}

/// The error, which must be a `NoBaseline`, and its message
fn no_baseline(repo: &TestRepo, preferred_branch: &str) -> (NoBaseline, String) {
  let error = detect(repo, preferred_branch).unwrap_err();
  let reason = error.downcast_ref::<NoBaseline>().unwrap_or_else(|| panic!("not a NoBaseline: {error:#}")).clone();
  (reason, format!("{error:#}"))
}

#[test]
fn the_upstream_of_a_clone_is_the_baseline() {
  let (_upstream, clone) = TestRepo::upstream_and_clone(&[("README.md", "readme")]);
  assert_eq!(detect(&clone, "master").unwrap(), "origin/master");
}

#[test]
fn a_local_master_is_the_baseline_without_a_remote() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "README.md", "readme");
  assert_eq!(detect(&repo, "master").unwrap(), "master");
  // the preferred branch comes first
  repo.create_branch("develop").unwrap();
  assert_eq!(detect(&repo, "develop").unwrap(), "develop");
}

#[test]
fn a_repository_without_commits_says_so() {
  let repo = TestRepo::new();
  assert_eq!(
    no_baseline(&repo, "master"),
    (NoBaseline::NoCommits, "The repository has no commits yet: commit to the main branch, then sync".to_string())
  );

  // a clone of an empty repository has a remote, but no commits either
  let clone = TestRepo::new_empty();
  clone.clone_from(repo.path()).unwrap();
  assert_eq!(no_baseline(&clone, "master").0, NoBaseline::NoCommits);
}

#[test]
fn no_local_baseline_names_each_branch_once() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "README.md", "readme");
  repo.rename_branch("master", "trunk").unwrap();
  assert_eq!(
    no_baseline(&repo, "master"),
    (
      NoBaseline::NoLocalBranch {
        tried: vec!["master".to_string(), "main".to_string()]
      },
      "Cannot find the baseline branch: the repository has no remote and no master or main branch".to_string()
    )
  );
  assert_eq!(
    no_baseline(&repo, "develop").1,
    "Cannot find the baseline branch: the repository has no remote and no develop, master or main branch"
  );
}

#[test]
fn no_remote_baseline_names_each_branch_once() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "README.md", "readme");
  // a remote that was never fetched: no upstream, no remote branches
  repo.add_remote("origin", "https://example.com/repo.git").unwrap();
  assert_eq!(
    no_baseline(&repo, "main"),
    (
      NoBaseline::NoRemoteBranch {
        tried: vec!["origin/main".to_string(), "origin/master".to_string()]
      },
      "Cannot find the baseline branch: no origin/main or origin/master, and no upstream".to_string()
    )
  );
}
