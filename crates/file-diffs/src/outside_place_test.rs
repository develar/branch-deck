use crate::fixture_test::{commit_all, process_counts, repo_path, unhurried, write};
use crate::model::{DiffSide, OutsidePlaceParams};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn params(repo: &TestRepo, revision: &str, path: &str, line: u32, name: Option<&str>) -> OutsidePlaceParams {
  OutsidePlaceParams {
    repository_path: repo_path(repo).to_string(),
    revision: revision.to_string(),
    path: path.to_string(),
    line,
    name: name.map(str::to_string),
  }
}

/// A use left outside the review opens as its file at the revision, at its line, the name selected; a revision without
/// the file or a line past its end is none. Two processes: the blob, then its text
#[test]
fn a_use_outside_the_review_opens_at_its_line_with_its_name_selected() {
  let repo = TestRepo::new();
  std::fs::create_dir_all(repo.path().join("src")).unwrap();
  write(&repo, "src/Base.kt", b"package a\n\nfun outsider(value: Int) = helper(value)\n");
  let head = commit_all(&repo, "Add the base");
  let diffs = unhurried();
  let counting = repo.git_executor().with_process_stats("test");

  let target = diffs.outside_place(&counting, &params(&repo, &head, "src/Base.kt", 3, Some("helper"))).unwrap().unwrap();
  assert_eq!(
    (target.path.as_str(), target.side, target.line, target.column, target.end_column),
    ("src/Base.kt", DiffSide::New, 2, 27, 33)
  );
  let outside = target.outside.unwrap();
  assert_eq!((outside.revision.as_deref(), outside.line_count), (Some(head.as_str()), 4));
  assert_eq!(Some(outside.blob_id), repo.rev_parse(&format!("{head}:src/Base.kt")).ok());
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 2)]);

  // no name: the start of the line; no file, or a line past the end: none
  let line = diffs.outside_place(repo.git_executor(), &params(&repo, &head, "src/Base.kt", 1, None)).unwrap().unwrap();
  assert_eq!((line.line, line.column, line.end_column), (0, 0, 0));
  assert_eq!(diffs.outside_place(repo.git_executor(), &params(&repo, &head, "src/Gone.kt", 1, None)).unwrap(), None);
  assert_eq!(diffs.outside_place(repo.git_executor(), &params(&repo, &head, "src/Base.kt", 9, None)).unwrap(), None);
}
