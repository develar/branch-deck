use super::carried_review::{CarryIdentity, carry_reviewed};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const JANE: CarryIdentity<'static> = CarryIdentity {
  name: "Jane",
  email: "jane@example.com",
  time: 1_700_000_000,
};

/// A repository whose `master` has a readme, and the reviewed commits on it: `login.rs` and then `logout.rs`; the base
/// and the tip of the review
fn reviewed() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add readme", "README.md", "readme\n");
  repo.git(&["switch", "-q", "-c", "feature"]);
  repo.create_commit("Add login", "login.rs", "fn login() {}\n");
  let tip = repo.create_commit("Add logout", "logout.rs", "fn logout() {}\n");
  (repo, base, tip)
}

fn carry(repo: &TestRepo, reviewed_base: &str, reviewed_tip: &str, base: &str) -> Option<String> {
  carry_reviewed(repo.git_executor(), repo.path_str(), reviewed_base, reviewed_tip, base, JANE).unwrap()
}

/// The paths and the texts of the tree of `commit`
fn files(repo: &TestRepo, commit: &str) -> Vec<(String, String)> {
  repo
    .git(&["ls-tree", "-r", "--name-only", commit])
    .lines()
    .map(|path| (path.to_string(), repo.git(&["show", &format!("{commit}:{path}")])))
    .collect()
}

/// A base that did not move carries the reviewed tip itself, without a process
#[test]
fn the_reviewed_tip_is_its_own_carry_on_the_same_base() {
  let (repo, base, tip) = reviewed();
  assert_eq!(carry(&repo, &base, &tip, &base), Some(tip));
}

/// A moved base gets the reviewed change merged onto it, in a commit whose parent is the base; the same carry writes
/// the same commit
#[test]
fn the_reviewed_change_is_merged_onto_a_moved_base_as_a_commit_of_it() {
  let (repo, reviewed_base, tip) = reviewed();
  repo.git(&["switch", "-q", "master"]);
  let base = repo.create_commit("Change readme", "README.md", "readme, changed\n");

  let carried = carry(&repo, &reviewed_base, &tip, &base).unwrap();
  assert_eq!(repo.git(&["rev-parse", &format!("{carried}^")]), base);
  assert_eq!(
    files(&repo, &carried),
    [
      ("README.md".to_string(), "readme, changed".to_string()),
      ("login.rs".to_string(), "fn login() {}".to_string()),
      ("logout.rs".to_string(), "fn logout() {}".to_string()),
    ]
  );
  assert_eq!(carry(&repo, &reviewed_base, &tip, &base).as_deref(), Some(carried.as_str()));
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", "--contains", &carried]), "");
}

/// A base that changed the lines the reviewed change changes carries nothing
#[test]
fn a_reviewed_change_that_conflicts_with_the_base_carries_nothing() {
  let (repo, reviewed_base, tip) = reviewed();
  repo.git(&["switch", "-q", "master"]);
  let base = repo.create_commit("Add another login", "login.rs", "fn login(user: &str) {}\n");
  assert_eq!(carry(&repo, &reviewed_base, &tip, &base), None);
}
