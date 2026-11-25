use crate::rebuilt_branch::{MovedBranch, rebuilt_with_the_same_commits};
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{assert_processes, counting};

/// A repository whose branch `pushed` has two commits on the first commit of `master`, and whose `master` moved since:
/// the pushed commits, their ids
fn pushed_then_moved() -> (TestRepo, [String; 2]) {
  let repo = TestRepo::new();
  repo.create_commit_with_timestamp("Initial commit", "base.txt", "base\n", Some(1_700_000_000));
  repo.checkout_new_branch("pushed").unwrap();
  let login = repo.create_commit_with_timestamp("(auth) Add login", "login.txt", "login\n", Some(1_700_000_100));
  let logout = repo.create_commit_with_timestamp("(auth) Add logout", "logout.txt", "logout\n", Some(1_700_000_200));
  repo.checkout("master").unwrap();
  repo.create_commit_with_timestamp("Upstream change", "upstream.txt", "upstream\n", Some(1_700_000_300));
  (repo, [login, logout])
}

/// The commits `ids` copied onto `master` as the branch `now` (a cherry-pick keeps the author and the message): the copy
/// of each
fn rebuilt(repo: &TestRepo, ids: &[&str]) -> Vec<String> {
  repo.git(&["checkout", "-q", "-B", "now", "master"]);
  ids
    .iter()
    .map(|id| {
      repo.git(&["cherry-pick", id]);
      repo.head()
    })
    .collect()
}

fn moved(pushed: &str, now: &str) -> MovedBranch {
  MovedBranch {
    pushed: pushed.to_owned(),
    now: now.to_owned(),
  }
}

fn same(repo: &TestRepo, branches: &[MovedBranch]) -> bool {
  rebuilt_with_the_same_commits(repo.git_executor(), repo.path_str(), "master", branches).unwrap()
}

/// The pushed commits copied onto the moved baseline, each with its author, time and message, are the same commits,
/// told by one `rev-list`
#[test]
fn the_pushed_commits_copied_onto_another_base_are_the_same_commits() {
  let (repo, [login, logout]) = pushed_then_moved();
  let copies = rebuilt(&repo, &[&login, &logout]);
  let git = counting();
  assert!(rebuilt_with_the_same_commits(&git, repo.path_str(), "master", &[moved(&logout, &copies[1])]).unwrap());
  assert_processes(&git, &[("rev-list", 1)]);
}

/// A copy with another message or another author is another version, which the remote status counts as reworded: no
/// rebuild of the same commits
#[test]
fn a_copy_with_another_message_or_author_is_no_rebuild() {
  let (repo, [login, logout]) = pushed_then_moved();
  rebuilt(&repo, &[&login, &logout]);
  repo.git(&["commit", "-q", "--amend", "-m", "(auth) Add logout, reworded"]);
  assert!(!same(&repo, &[moved(&logout, &repo.head())]));

  rebuilt(&repo, &[&login, &logout]);
  repo.git(&["commit", "-q", "--amend", "--no-edit", "--author", "Ann <ann@example.com>"]);
  assert!(!same(&repo, &[moved(&logout, &repo.head())]));
}

/// Another change, a commit less (one that landed in the baseline) or a commit more is no rebuild of the same commits
#[test]
fn another_change_or_another_number_of_commits_is_no_rebuild() {
  let (repo, [login, logout]) = pushed_then_moved();
  rebuilt(&repo, &[&login]);
  repo.create_commit_with_timestamp("(auth) Add logout", "logout.txt", "logout, changed\n", Some(1_700_000_200));
  assert!(!same(&repo, &[moved(&logout, &repo.head())]), "another change");

  let copies = rebuilt(&repo, &[&logout]);
  assert!(!same(&repo, &[moved(&logout, &copies[0])]), "a commit less");

  rebuilt(&repo, &[&login, &logout]);
  repo.create_commit("(auth) More", "more.txt", "more\n");
  assert!(!same(&repo, &[moved(&logout, &repo.head())]), "a commit more");
}

/// A stack pushed parents first: the branch it is built on is the same rebuild where it is at the same place of both
/// chains of its leaf, and none where the parent now points elsewhere
#[test]
fn a_parent_of_the_leaf_is_rebuilt_at_the_same_place() {
  let (repo, [login, logout]) = pushed_then_moved();
  let copies = rebuilt(&repo, &[&login, &logout]);
  let leaf = moved(&logout, &copies[1]);
  assert!(same(&repo, &[moved(&login, &copies[0]), leaf.clone()]));
  assert!(!same(&repo, &[moved(&login, &copies[1]), leaf]), "the parent at another place");
}
