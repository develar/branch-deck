use crate::commit_signing::{Signing, SigningFailed, commit_signing};
#[cfg(any(unix, windows))]
use crate::commit_writer::NotesOfWritten;
use crate::commit_writer::{
  EditedCommit, NewCommit, NewParent, NoIdentity, StoreCommit, complete_line, current_committer, read_raw_commits, write_commits, write_commits_with_notes, write_edited_commit,
  write_store_commits,
};
use crate::ref_transaction::RefTransaction;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

#[cfg(any(unix, windows))]
fn signing_tests_supported() -> bool {
  #[cfg(windows)]
  {
    crate::commit_signing::ssh_keygen_available()
  }
  #[cfg(unix)]
  {
    true
  }
}

#[cfg(any(unix, windows))]
fn signing_repo() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let parent = repo.create_commit("base", "a.txt", "a\n");
  repo.enable_ssh_signing();
  let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
  (repo, parent, tree)
}

#[test]
fn signing_configuration_and_unsigned_writer_ids_are_unchanged() {
  let repo = TestRepo::new();
  let parent = repo.create_commit("base", "a.txt", "a\n");
  assert_eq!(commit_signing(repo.git_executor(), repo.path_str()).unwrap(), Signing::Off);
  let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
  let ident = "Test User <test@example.com> 1700000000 +0000";
  let expected = repo
    .git_executor()
    .cmd(&["commit-tree", &tree, "-p", &parent], repo.path_str())
    .env(&[
      ("GIT_AUTHOR_NAME", "Test User"),
      ("GIT_AUTHOR_EMAIL", "test@example.com"),
      ("GIT_AUTHOR_DATE", "1700000000 +0000"),
      ("GIT_COMMITTER_NAME", "Test User"),
      ("GIT_COMMITTER_EMAIL", "test@example.com"),
      ("GIT_COMMITTER_DATE", "1700000000 +0000"),
    ])
    .stdin(b"unchanged\n")
    .text()
    .unwrap();
  repo.enable_ssh_signing();
  assert_eq!(commit_signing(repo.git_executor(), repo.path_str()).unwrap(), Signing::On);
  let written = write_commits(
    repo.git_executor(),
    repo.path_str(),
    &[NewCommit {
      tree: &tree,
      parent: Some(NewParent::Existing(&parent)),
      author: ident,
      committer: ident,
      encoding: None,
      message: b"unchanged\n",
    }],
    Signing::Off,
  )
  .unwrap();

  assert_eq!(written.ids[0], expected.trim());
  assert_eq!(repo.git(&["log", "-1", "--format=%G?", &written.ids[0]]).trim(), "N");
  written.discard(repo.git_executor(), repo.path_str());
}

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
  let written = write_commits_with_notes(git, path, &[copy], None, &[StoreCommit::file("a", b"a", None, "a")], Some(ident), Signing::Off).unwrap();
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
  let written = write_commits_with_notes(wrapped.executor(), path, &[copy], Some(&notes), &[], Some(ident), Signing::Off).unwrap();
  assert!(wrapped.marked("moved"));
  assert_eq!(repo.git(&["notes", "show", &initial]).trim(), "other writer");
  assert_eq!(repo.git(&["notes", "show", &written.ids[0]]).trim(), "v-commit-v1:copy", "the rewrite's own note");
}

#[cfg(any(unix, windows))]
#[test]
fn signing_writes_verifiable_user_commits_but_leaves_store_and_notes_commits_unsigned() {
  if !signing_tests_supported() {
    return;
  }
  let (repo, parent, tree) = signing_repo();
  let ident = "Test User <test@example.com> 1700000000 +0000";
  let commit = NewCommit {
    tree: &tree,
    parent: Some(NewParent::Existing(&parent)),
    author: ident,
    committer: ident,
    encoding: Some("ISO-8859-1"),
    message: b"signed copy\n",
  };
  let unsigned = write_commits(repo.git_executor(), repo.path_str(), &[commit], Signing::Off).unwrap();
  let notes = NotesOfWritten {
    notes_ref: "refs/notes/commits",
    notes: vec![(0, "mapping note".to_string())],
  };
  let signed = write_commits_with_notes(
    repo.git_executor(),
    repo.path_str(),
    &[commit],
    Some(&notes),
    &[StoreCommit::file("store", b"value", None, "store")],
    Some(ident),
    Signing::On,
  )
  .unwrap();

  assert_ne!(unsigned.ids[0], signed.ids[0]);
  repo.git(&["verify-commit", &signed.ids[0]]);
  let unsigned_commit = read_raw_commits(repo.git_executor(), repo.path_str(), &[&unsigned.ids[0]]).unwrap();
  let signed_commit = read_raw_commits(repo.git_executor(), repo.path_str(), &[&signed.ids[0]]).unwrap();
  let (unsigned_commit, signed_commit) = (&unsigned_commit[&unsigned.ids[0]], &signed_commit[&signed.ids[0]]);
  assert!(!unsigned_commit.signed);
  assert!(signed_commit.signed);
  assert_eq!(unsigned_commit.tree, signed_commit.tree);
  assert_eq!(unsigned_commit.parents, signed_commit.parents);
  assert_eq!(unsigned_commit.author, signed_commit.author);
  assert_eq!(unsigned_commit.committer, signed_commit.committer);
  assert_eq!(unsigned_commit.encoding, Some("ISO-8859-1".to_string()));
  assert_eq!(unsigned_commit.encoding, signed_commit.encoding);
  assert_eq!(unsigned_commit.message, signed_commit.message);

  let notes_commit = repo.git(&["rev-parse", "refs/notes/commits"]);
  assert_eq!(repo.git(&["log", "-1", "--format=%G?", &signed.store_commits[0]]).trim(), "N");
  assert_eq!(repo.git(&["log", "-1", "--format=%G?", &notes_commit]).trim(), "N");
  unsigned.discard(repo.git_executor(), repo.path_str());
  signed.discard(repo.git_executor(), repo.path_str());
}

#[cfg(any(unix, windows))]
#[test]
fn an_edited_commit_is_signed_by_its_existing_fast_import() {
  if !signing_tests_supported() {
    return;
  }
  let (repo, parent, tree) = signing_repo();
  let ident = "Test User <test@example.com> 1700000000 +0000";
  let written = write_edited_commit(
    repo.git_executor(),
    repo.path_str(),
    &EditedCommit {
      tree: &tree,
      edits: &[],
      parent: &parent,
      author: ident,
      committer: ident,
      encoding: None,
      message: b"edited copy\n",
    },
    Signing::On,
  )
  .unwrap();

  repo.git(&["verify-commit", &written.commit]);
  assert_eq!(repo.git(&["log", "-1", "--format=%G?", &written.commit]).trim(), "G");
  assert_eq!(written.tree, tree);
  let mut transaction = RefTransaction::new();
  written.delete_temp_ref(&mut transaction);
  transaction.commit(repo.git_executor(), repo.path_str(), "test").unwrap();
}

#[cfg(any(unix, windows))]
#[test]
fn a_fast_import_signing_failure_is_typed_and_does_not_move_the_branch() {
  if !signing_tests_supported() {
    return;
  }
  let (repo, head, tree) = signing_repo();
  repo.git(&["config", "user.signingkey", "/missing/branch-deck-test-key"]);
  let ident = "Test User <test@example.com> 1700000000 +0000";
  let error = write_commits(
    repo.git_executor(),
    repo.path_str(),
    &[NewCommit {
      tree: &tree,
      parent: Some(NewParent::Existing(&head)),
      author: ident,
      committer: ident,
      encoding: None,
      message: b"cannot sign\n",
    }],
    Signing::On,
  )
  .unwrap_err();

  assert!(error.downcast_ref::<SigningFailed>().is_some(), "{error:#}");
  assert!(format!("{error:#}").contains("sign"), "{error:#}");
  assert_eq!(repo.head(), head);
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/tmp/"]), "");
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
