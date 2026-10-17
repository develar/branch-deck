use crate::commit_writer::complete_line;
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
    let (raw, _) = repo
      .git_executor()
      .execute_command_raw_accepting(&["cat-file", "commit", &commit], repo.path_str(), &[])
      .unwrap();
    let stored = &raw[raw.windows(2).position(|pair| pair == b"\n\n").unwrap() + 2..];
    assert_eq!(complete_line(message), stored, "{message:?}");
  }
}
