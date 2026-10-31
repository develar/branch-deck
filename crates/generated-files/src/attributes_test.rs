use crate::attributes::{attribute_generated_paths, generated_paths};
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use test_utils::git_test_utils::TestRepo;

fn set(paths: &[&str]) -> HashSet<String> {
  paths.iter().map(|path| (*path).to_string()).collect()
}

/// `linguist-generated` and `-diff` (which `binary` implies) of the revision, not of the working tree; the lock files by
/// their name
#[test]
fn the_attributes_of_the_revision_mark_generated_files_and_a_lock_file_is_one_by_its_name() {
  let repo = TestRepo::new();
  repo.create_commit(
    "attributes",
    ".gitattributes",
    "gen/** linguist-generated\n*.bin binary\nkept.json linguist-generated=false\n",
  );
  let first = repo.head();
  repo.create_commit("more", ".gitattributes", "gen/** linguist-generated\n*.bin binary\n*.json linguist-generated\n");
  let (git, path) = (repo.git_executor(), repo.path_str());
  let paths = ["gen/a.rs", "x.bin", "kept.json", "src/a.rs", "Cargo.lock"];
  assert_eq!(attribute_generated_paths(git, path, &first, paths).unwrap(), set(&["gen/a.rs", "x.bin"]));
  assert_eq!(attribute_generated_paths(git, path, "HEAD", paths).unwrap(), set(&["gen/a.rs", "x.bin", "kept.json"]));
  assert_eq!(generated_paths(git, path, &first, paths).unwrap(), set(&["gen/a.rs", "x.bin", "Cargo.lock"]));
  assert_eq!(generated_paths(git, path, "HEAD", ["Cargo.lock"]).unwrap(), set(&["Cargo.lock"]));
}
