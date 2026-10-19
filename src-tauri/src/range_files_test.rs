use crate::range_files::RangeFilesCache;
use git_ops::commit_files::{DiffSubject, read_diff_files};

/// The files of a range are read once and stay in memory, weighed by their paths and an overhead per file
#[test]
fn the_files_of_a_range_stay_in_memory() {
  let repo = test_utils::git_test_utils::TestRepo::new();
  let base = repo.create_commit("One", "a.txt", "a\nb\n");
  let head = repo.create_commit_with_files("Two", &[("a.txt", "a\nc\nd\n"), ("b.txt", "b\n")]);
  let path = repo.path_str().to_string();
  let subject = DiffSubject::Range { base, head };
  let git = repo.git_executor().clone();
  let cache = RangeFilesCache::default();

  let files = cache.read(&git, &path, &subject).unwrap();
  assert_eq!(*files, read_diff_files(&git, &path, &subject).unwrap());
  assert_eq!(cache.bytes(), 2 * (5 + 64));

  // from memory, no git: the repository is gone
  drop(repo);
  assert_eq!(cache.read(&git, &path, &subject).unwrap(), files);
}
