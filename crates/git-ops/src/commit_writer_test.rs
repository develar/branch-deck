use crate::commit_writer::{NewCommit, NewParent, StoreCommit, complete_line, write_commits_with_notes, write_store_commits};
use crate::ref_transaction::RefTransaction;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The bytes `commit-tree -m <message>` stores are the bytes the writers give `fast-import`, an empty message included
#[test]
fn a_message_is_completed_as_commit_tree_stores_it() {
  let repo = TestRepo::new();
  // the empty tree, of the empty index
  let tree = repo.git(&["write-tree"]);
  for message in ["", "Add login", "Add login\n", "Add login\n\nBody\n\n"] {
    let commit = repo.git(&["commit-tree", &tree, "-m", message]);
    let raw = repo.git_executor().cmd(&["cat-file", "commit", &commit], repo.path_str()).bytes().unwrap();
    let stored = &raw[raw.windows(2).position(|pair| pair == b"\n\n").unwrap() + 2..];
    assert_eq!(complete_line(message), stored, "{message:?}");
  }
}

/// Each temporary ref is deleted only while it points at the commit written on it: a transaction that finds one moved
/// fails as a whole and deletes none
#[test]
fn temporary_refs_are_deleted_with_compare_and_swap() {
  let repo = TestRepo::new();
  let (git, path) = (repo.git_executor(), repo.path_str());
  let head = repo.create_commit("Add a", "a.txt", "a\n");
  let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
  let ident = "Test User <test@example.com> 1700000000 +0000";
  let copy = NewCommit {
    tree: &tree,
    parent: Some(NewParent::Existing(&head)),
    author: ident,
    committer: ident,
    encoding: None,
    message: b"Copy\n",
  };
  let written = write_commits_with_notes(git, path, &[copy], None, &[StoreCommit::file("a", b"a", None, "a")], Some(ident)).unwrap();
  let store = write_store_commits(git, path, &[StoreCommit::file("b", b"b", None, "b")], None, Some(ident)).unwrap();
  let temp_refs = || repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/tmp/"]).lines().count();
  assert_eq!(temp_refs(), 3);

  repo.git(&["update-ref", &written.temp_ref, &head]);
  let mut transaction = RefTransaction::new();
  written.delete_temp_refs(&mut transaction);
  store.delete_temp_refs(&mut transaction);
  transaction.commit(git, path, "test").unwrap_err();
  assert_eq!(temp_refs(), 3);

  repo.git(&["update-ref", &written.temp_ref, &written.ids[0]]);
  let mut transaction = RefTransaction::new();
  written.delete_temp_refs(&mut transaction);
  store.delete_temp_refs(&mut transaction);
  transaction.commit(git, path, "test").unwrap();
  assert_eq!(temp_refs(), 0);
}
