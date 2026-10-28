#[cfg(unix)]
use crate::commit_writer::NotesOfWritten;
use crate::commit_writer::{NewCommit, NewParent, NoIdentity, StoreCommit, complete_line, current_committer, write_commits_with_notes, write_store_commits};
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

#[test]
fn a_temporary_ref_is_stale_after_its_age_or_without_a_time() {
  use crate::commit_writer::{STALE_TEMP_REF_AGE, is_stale_temp_ref, unique_temp_ref};
  assert!(!is_stale_temp_ref(&unique_temp_ref(), STALE_TEMP_REF_AGE));
  assert!(is_stale_temp_ref("refs/branch-deck/tmp/42-1000-0", STALE_TEMP_REF_AGE));
  assert!(is_stale_temp_ref("refs/branch-deck/tmp/42-1000-0-notes", STALE_TEMP_REF_AGE));
  // the shared ref of the commit chains up to 0.21
  assert!(is_stale_temp_ref("refs/branch-deck/tmp/chain", STALE_TEMP_REF_AGE));
  assert!(!is_stale_temp_ref("refs/branch-deck/conflict/abc", STALE_TEMP_REF_AGE));
}

/// A rewrite that carries mapping notes to its new commits loses the notes ref to another writer in the middle of its
/// `fast-import` (a sync that writes its notes): the write goes once more from the tip the other writer left, so the
/// rewrite does not fail, and both notes are kept
#[cfg(unix)]
#[test]
fn a_write_of_commits_that_loses_the_notes_ref_to_another_writer_goes_once_more_and_keeps_both_notes() {
  let repo = TestRepo::new();
  let initial = repo.create_commit("Initial", "README.md", "# Test\n");
  let head = repo.create_commit("Add a", "a.txt", "a\n");
  let path = repo.path_str();
  repo.git(&["notes", "add", "-m", "first", &head]);
  let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
  let ident = "Test User <test@example.com> 1700000000 +0000";
  let copy = NewCommit {
    tree: &tree,
    parent: Some(NewParent::Existing(&initial)),
    author: ident,
    committer: ident,
    encoding: None,
    message: b"Add a again\n",
  };
  let other = format!("[ -f '{{dir}}/moved' ] || {{ touch '{{dir}}/moved'; git -C '{path}' notes add -f -m 'other writer' {initial}; }}");
  let wrapped = test_utils::wrapped_git::WrappedGit::new(&format!(
    "*\" fast-import --quiet --done --date-format=raw\"*) {};;",
    test_utils::wrapped_git::WrappedGit::during_fast_import(&other)
  ));
  let notes = NotesOfWritten {
    notes_ref: "refs/notes/commits",
    notes: vec![(0, "v-commit-v1:copy".to_string())],
  };
  let written = write_commits_with_notes(wrapped.executor(), path, &[copy], Some(&notes), &[], Some(ident)).unwrap();
  assert!(wrapped.marked("moved"));
  assert_eq!(repo.git(&["notes", "show", &initial]).trim(), "other writer");
  assert_eq!(repo.git(&["notes", "show", &written.ids[0]]).trim(), "v-commit-v1:copy", "the rewrite's own note");
}

/// A committer git cannot form (here an empty name; for a user, no `user.name` or `user.email` git can guess) is
/// `NoIdentity`, for every write of notes and commits: the code the page shows the two `git config` commands for
#[test]
fn a_committer_git_cannot_form_is_no_identity() {
  let repo = TestRepo::new();
  let git = repo.git_executor().with_env("GIT_COMMITTER_NAME", "");
  let error = current_committer(&git, repo.path_str()).unwrap_err();
  assert!(error.downcast_ref::<NoIdentity>().is_some(), "{error:#}");
  assert!(current_committer(repo.git_executor(), repo.path_str()).unwrap().contains(" <test@example.com> "));
}
