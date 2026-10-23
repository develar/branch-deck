use crate::patronus_review::{ReviewInput, read_review_inputs};
use object_cache::cache::ObjectCache;
use std::fmt::Write;
use sync_core::patronus::MAX_REVIEW_COMMITS;
use test_utils::git_test_utils::TestRepo;

/// A repository with a commit on the branch `baseline`
fn repo_with_baseline() -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  repo.git_executor().execute_command(&["branch", "baseline"], repo.path().to_str().unwrap()).unwrap();
  repo
}

fn read_inputs(repo: &TestRepo, baseline: &str, tips: &[&str]) -> Vec<Option<ReviewInput>> {
  let path = repo.path().to_str().unwrap();
  let tips: Vec<String> = tips.iter().map(|tip| repo.git_executor().execute_command(&["rev-parse", tip], path).unwrap()).collect();
  read_review_inputs(repo.git_executor(), path, &ObjectCache::disabled(), baseline, &tips).unwrap()
}

#[test]
fn reads_the_commits_of_the_branch_with_their_changed_paths() {
  let repo = repo_with_baseline();
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  repo.create_commit_with_files("IJPL-1 first\n\nThe body", &[("a.txt", "a"), ("dir/b c.txt", "b")]);
  git.execute_command(&["rm", "--quiet", "a.txt"], path).unwrap();
  repo.create_commit("IJPL-1 second", "c.txt", "c");
  git.execute_command(&["mv", "dir/b c.txt", "moved.txt"], path).unwrap();
  git.execute_command(&["commit", "--quiet", "-m", "IJPL-1 rename"], path).unwrap();

  let [Some(input)] = <[Option<ReviewInput>; 1]>::try_from(read_inputs(&repo, "baseline", &["HEAD"])).ok().unwrap() else {
    panic!("the branch is short")
  };
  let commits = &input.commits;
  assert_eq!(
    commits.iter().map(|commit| commit.message.as_str()).collect::<Vec<_>>(),
    ["IJPL-1 rename", "IJPL-1 second", "IJPL-1 first\n\nThe body"]
  );
  // a rename is a delete and an add, as `--no-renames` lists it: the owners of both review it
  assert_eq!(commits[0].changed_paths, ["dir/b c.txt", "moved.txt"]);
  // a deleted path counts: its owners review the deletion
  assert_eq!(commits[1].changed_paths, ["a.txt", "c.txt"]);
  assert_eq!(commits[2].changed_paths, ["a.txt", "dir/b c.txt"]);
  assert_eq!(input.paths, ["a.txt", "c.txt", "dir/b c.txt", "moved.txt"]);
  assert!(!commits[0].is_merge_commit);
  assert!(commits[0].author_date.ends_with('Z'), "{}", commits[0].author_date);
  assert_ne!(commits[0].author_email, "");
}

/// Stacked branches share the commits of their parents; one log reads all of them, each branch gets the commits it reaches
#[test]
fn reads_several_branches_in_one_log() {
  let repo = repo_with_baseline();
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  repo.create_commit("parent", "parent.txt", "p");
  git.execute_command(&["branch", "parent"], path).unwrap();
  repo.create_commit("child", "child.txt", "c");
  git.execute_command(&["branch", "child"], path).unwrap();
  git.execute_command(&["checkout", "--quiet", "-b", "side", "baseline"], path).unwrap();
  repo.create_commit("side", "side.txt", "s");

  let inputs = read_inputs(&repo, "baseline", &["parent", "child", "side", "baseline"]);
  let messages: Vec<Vec<&str>> = inputs
    .iter()
    .map(|input| input.as_ref().unwrap().commits.iter().map(|commit| commit.message.as_str()).collect())
    .collect();
  assert_eq!(messages, [vec!["parent"], vec!["child", "parent"], vec!["side"], vec![]]);
  assert_eq!(inputs[1].as_ref().unwrap().paths, ["child.txt", "parent.txt"]);
}

#[test]
fn does_not_read_a_branch_with_too_many_commits() {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  let base = git.execute_command(&["rev-parse", "HEAD"], path).unwrap();
  // MAX_REVIEW_COMMITS + 1 commits without a work tree: one fast-import
  let mut stream = String::new();
  for index in 0..=MAX_REVIEW_COMMITS {
    let from = if index == 0 { format!("from {base}\n") } else { String::new() };
    write!(
      stream,
      "commit refs/heads/long\ncommitter Me <me@example.com> 1700000000 +0000\ndata 1\nc\n{from}M 644 inline f.txt\ndata {}\n{index}\n",
      index.to_string().len()
    )
    .unwrap();
  }
  git.cmd(&["fast-import", "--quiet"], path).stdin(stream.as_bytes()).text().unwrap();
  let inputs = read_inputs(&repo, &base, &["refs/heads/long", "refs/heads/long~1"]);
  assert!(inputs[0].is_none());
  assert_eq!(inputs[1].as_ref().map(|input| input.commits.len()), Some(MAX_REVIEW_COMMITS));
  assert!(read_inputs(&repo, &base, &["refs/heads/long"])[0].is_none());
}
