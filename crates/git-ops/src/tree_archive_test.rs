use crate::tree_archive::write_tree_archive;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The paths and contents of the files of a tar
fn files_of(tar: &[u8]) -> Vec<(String, String)> {
  let mut archive = tar::Archive::new(tar);
  let mut files = Vec::new();
  for entry in archive.entries().unwrap() {
    let mut entry = entry.unwrap();
    if entry.header().entry_type().is_file() {
      let path = entry.path().unwrap().to_string_lossy().into_owned();
      let mut content = String::new();
      std::io::Read::read_to_string(&mut entry, &mut content).unwrap();
      files.push((path, content));
    }
  }
  files
}

#[test]
fn the_tar_of_a_tree_or_of_some_of_its_paths() {
  let repo = TestRepo::new();
  let head = repo.create_commit_in_index("files", &[("a.txt", "a\n"), ("src/main.rs", "fn main() {}\n")]);

  let mut whole = Vec::new();
  write_tree_archive(repo.git_executor(), repo.path_str(), &head, None, &mut whole).unwrap();
  assert_eq!(
    files_of(&whole),
    [("a.txt".to_string(), "a\n".to_string()), ("src/main.rs".to_string(), "fn main() {}\n".to_string())]
  );

  let mut part = Vec::new();
  write_tree_archive(repo.git_executor(), repo.path_str(), &head, Some(&["src"]), &mut part).unwrap();
  assert_eq!(files_of(&part), [("src/main.rs".to_string(), "fn main() {}\n".to_string())]);
}

/// A path is a name, never a pattern: `a*.txt` takes no `ab.txt`, and `:colon.txt` is no pathspec magic
#[test]
fn a_path_is_a_name() {
  let repo = TestRepo::new();
  let head = repo.create_commit_in_index("files", &[("a*.txt", "star\n"), ("ab.txt", "b\n"), (":colon.txt", "colon\n")]);

  let mut part = Vec::new();
  write_tree_archive(repo.git_executor(), repo.path_str(), &head, Some(&["a*.txt", ":colon.txt"]), &mut part).unwrap();
  assert_eq!(
    files_of(&part),
    [(":colon.txt".to_string(), "colon\n".to_string()), ("a*.txt".to_string(), "star\n".to_string())]
  );
}
