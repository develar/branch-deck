use crate::model::{DiffSource, GetFileDiffsParams};
use crate::range_files::RangeFilesCache;
use crate::service::FileDiffs;
use git_ops::commit_files::{DiffSubject, read_diff_files};
use object_cache::cache::ObjectCache;
use text_diff::policy::WhitespacePolicy;

/// A repository whose range `base..head` changes a.txt and adds b.txt
fn range_repo() -> (test_utils::git_test_utils::TestRepo, String, String) {
  let repo = test_utils::git_test_utils::TestRepo::new();
  let base = repo.create_commit("One", "a.txt", "a\nb\n");
  let head = repo.create_commit_with_files("Two", &[("a.txt", "a\nc\nd\n"), ("b.txt", "b\n")]);
  (repo, base, head)
}

/// The files of a range are read once and stay in memory, weighed by their paths and an overhead per file
#[test]
fn the_files_of_a_range_stay_in_memory() {
  let (repo, base, head) = range_repo();
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

/// Opening a range review: the window lists the files (`get_range_files`) while it streams their diffs
/// (`get_file_diffs`); both read the list of the range from one `diff-tree`, also when they ask at once, and a second
/// opening runs none
#[test]
fn the_list_and_the_diffs_of_a_range_run_one_diff_tree() {
  let (repo, base, head) = range_repo();
  let path = repo.path_str().to_string();
  let counting = repo.git_executor().with_process_stats("test");
  let diffs = FileDiffs::default();
  let subject = DiffSubject::Range {
    base: base.clone(),
    head: head.clone(),
  };
  let params = GetFileDiffsParams {
    repository_path: path.clone(),
    source: DiffSource::Range { base, head },
    policy: WhitespacePolicy::Default,
    priority_paths: Vec::new(),
  };
  for _ in 0..2 {
    std::thread::scope(|scope| {
      scope.spawn(|| diffs.range_files.read(&counting, &path, &subject).unwrap());
      scope.spawn(|| diffs.stream(&counting, &ObjectCache::disabled(), &params, |_| {}).unwrap());
    });
  }
  let diff_trees: u32 = counting
    .process_stats()
    .unwrap()
    .counts()
    .into_iter()
    .filter(|(name, _)| name == "diff-tree")
    .map(|(_, count)| count)
    .sum();
  assert_eq!(diff_trees, 1);
}
