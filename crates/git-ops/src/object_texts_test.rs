use crate::object_texts::{TreeEntry, read_blob_bytes, read_texts, read_trees};
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

fn git(repo: &TestRepo, args: &[&str]) -> String {
  repo.git_executor().execute_command(args, repo.path_str()).unwrap()
}

#[test]
fn blobs_by_id_or_by_path_and_only_text_ones() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("files", &[("a.txt", "one\n"), ("big.txt", "0123456789\n")]);
  std::fs::write(repo.path().join("bin.dat"), b"x\0y").unwrap();
  git(&repo, &["add", "bin.dat"]);
  git(&repo, &["commit", "-q", "-m", "binary"]);
  let id = git(&repo, &["rev-parse", "HEAD:a.txt"]);
  let names = [id, "HEAD:big.txt".to_string(), "HEAD:bin.dat".to_string(), "HEAD:none.txt".to_string(), "HEAD".to_string()];

  assert_eq!(
    read_blob_bytes(repo.git_executor(), repo.path_str(), &names, 8).unwrap(),
    [Some(b"one\n".to_vec()), None, None, None, None]
  );
  assert_eq!(read_texts(repo.git_executor(), repo.path_str(), &names[..1], 8).unwrap(), [Some("one\n".to_string())]);
}

#[test]
fn trees_list_their_entries_with_full_ids() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("files", &[("Cargo.toml", "[package]\n"), ("src/lib.rs", "pub fn f() {}\n"), ("a b.txt", "x\n")]);
  let names = ["HEAD:".to_string(), "HEAD:src".to_string(), "HEAD:Cargo.toml".to_string(), "HEAD:none".to_string()];

  let trees = read_trees(repo.git_executor(), repo.path_str(), &names).unwrap();
  let entry = |mode: &str, name: &str, path: &str| TreeEntry {
    mode: mode.to_string(),
    name: name.to_string(),
    id: git(&repo, &["rev-parse", &format!("HEAD:{path}")]),
  };
  assert_eq!(
    trees,
    [
      Some(vec![
        entry("100644", "Cargo.toml", "Cargo.toml"),
        entry("100644", "a b.txt", "a b.txt"),
        entry("40000", "src", "src")
      ]),
      Some(vec![entry("100644", "lib.rs", "src/lib.rs")]),
      None,
      None,
    ]
  );
}
