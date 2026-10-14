use crate::notes::PREFIX;
use crate::reword_commits::{RewordCommitParams, reword_commits_batch};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn reword(test_repo: &TestRepo, baseline_branch: &str, rewrites: &[(&str, &str, bool)]) -> anyhow::Result<HashMap<String, String>> {
  let rewrites = rewrites
    .iter()
    .map(|(commit_id, message, keeps_virtual_commit)| RewordCommitParams {
      commit_id: (*commit_id).to_string(),
      new_message: (*message).to_string(),
      keeps_virtual_commit: *keeps_virtual_commit,
    })
    .collect();
  reword_commits_batch(test_repo.git_executor(), test_repo.path().to_str().unwrap(), baseline_branch, rewrites)
}

#[test]
fn refuses_a_commit_that_is_not_ahead_of_the_baseline() {
  let test_repo = TestRepo::new();
  let initial = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.checkout_new_branch("feature").unwrap();
  let feature = test_repo.create_commit("(auth) Add login", "login.rs", "// login");

  let error = reword(&test_repo, "master", &[(&initial, "Initial commit, reworded", false)]).unwrap_err();
  assert!(error.to_string().contains("ahead of master"), "{error}");
  assert_eq!(test_repo.head(), feature);

  let new_ids = reword(&test_repo, "master", &[(&feature, "(auth) Add the login form", false)]).unwrap();
  assert_eq!(test_repo.head(), new_ids[&feature]);
  assert_eq!(test_repo.rev_parse("HEAD~1").unwrap(), initial);
}

#[test]
fn moves_the_mapping_notes_to_the_new_commits() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.checkout_new_branch("feature").unwrap();
  let login = test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  let logout = test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");
  let cache = test_repo.create_commit("(cache) Add cache", "cache.rs", "// cache");
  for (commit, virtual_commit) in [(&login, "1111"), (&logout, "2222"), (&cache, "3333")] {
    test_repo.add_note(commit, &format!("{PREFIX}{virtual_commit}")).unwrap();
  }

  // the subject of `login` changes, `logout` only moves to another branch, `cache` is recreated on top
  let new_ids = reword(
    &test_repo,
    "master",
    &[(&login, "(auth) Add the login form", false), (&logout, "(session) Add logout", true)],
  )
  .unwrap();

  assert!(test_repo.show_note(&new_ids[&login]).is_err(), "the virtual commit of a reworded subject is stale");
  assert_eq!(test_repo.show_note(&new_ids[&logout]).unwrap(), format!("{PREFIX}2222"));
  assert_eq!(test_repo.show_note(&new_ids[&cache]).unwrap(), format!("{PREFIX}3333"));
  // the old commits keep theirs
  assert_eq!(test_repo.show_note(&login).unwrap(), format!("{PREFIX}1111"));
  assert_eq!(test_repo.get_commit_messages(3), ["(cache) Add cache", "(session) Add logout", "(auth) Add the login form"]);
}
