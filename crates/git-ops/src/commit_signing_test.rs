use super::{Signing, sign_range};
use crate::commit_writer::{NewCommit, NewParent, read_raw_commits, write_commits};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

#[cfg(any(unix, windows))]
#[test]
fn a_replayed_range_is_signed_after_its_signed_prefix_and_all_signed_ranges_are_reused() {
  #[cfg(windows)]
  if !super::ssh_keygen_available() {
    return;
  }

  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  repo.enable_ssh_signing();
  let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
  let ident = "Test User <test@example.com> 1700000000 +0000";
  let signed_prefix = write_commits(
    repo.git_executor(),
    repo.path_str(),
    &[NewCommit {
      tree: &tree,
      parent: Some(NewParent::Existing(&base)),
      author: ident,
      committer: ident,
      encoding: None,
      message: b"signed prefix\n",
    }],
    Signing::On,
  )
  .unwrap();
  let unsigned_replay = write_commits(
    repo.git_executor(),
    repo.path_str(),
    &[NewCommit {
      tree: &tree,
      parent: Some(NewParent::Existing(&signed_prefix.ids[0])),
      author: ident,
      committer: ident,
      encoding: Some("ISO-8859-1"),
      message: b"replayed\n",
    }],
    Signing::Off,
  )
  .unwrap();
  let unsigned_id = &unsigned_replay.ids[0];

  let signed = sign_range(repo.git_executor(), repo.path_str(), Some(&base), unsigned_id, Signing::On).unwrap().unwrap();
  assert_eq!(signed.ids.len(), 1);
  repo.git(&["verify-commit", &signed.ids[0]]);
  let raw = read_raw_commits(repo.git_executor(), repo.path_str(), &[unsigned_id, &signed.ids[0]]).unwrap();
  let before = &raw[unsigned_id];
  let after = &raw[&signed.ids[0]];
  assert!(!before.signed);
  assert!(after.signed);
  assert_eq!(before.tree, after.tree);
  assert_eq!(before.parents, after.parents);
  assert_eq!(before.author, after.author);
  assert_eq!(before.committer, after.committer);
  assert_eq!(before.encoding, after.encoding);
  assert_eq!(before.message, after.message);
  assert_eq!(
    sign_range(repo.git_executor(), repo.path_str(), Some(&base), &signed.ids[0], Signing::On)
      .unwrap()
      .is_none(),
    true
  );

  signed.discard(repo.git_executor(), repo.path_str());
  unsigned_replay.discard(repo.git_executor(), repo.path_str());
  signed_prefix.discard(repo.git_executor(), repo.path_str());
}

#[cfg(any(unix, windows))]
#[test]
fn sign_range_supports_a_root_commit() {
  #[cfg(windows)]
  if !super::ssh_keygen_available() {
    return;
  }

  let repo = TestRepo::new();
  let root = repo.create_commit("root", "a.txt", "a\n");
  repo.enable_ssh_signing();

  let signed = sign_range(repo.git_executor(), repo.path_str(), None, &root, Signing::On).unwrap().unwrap();
  repo.git(&["verify-commit", &signed.ids[0]]);
  let raw = read_raw_commits(repo.git_executor(), repo.path_str(), &[&signed.ids[0]]).unwrap();
  assert_eq!(raw[&signed.ids[0]].parents, Vec::<String>::new());
  assert!(raw[&signed.ids[0]].signed);
  signed.discard(repo.git_executor(), repo.path_str());
}
