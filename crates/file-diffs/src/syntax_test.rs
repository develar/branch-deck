//! The highlighting of blobs for the views: the layout of the answer, where the text comes from, and the git processes

use crate::fixture_test::{commit_all, process_counts, repo_path, write};
use crate::model::{BlobRequest, DiffSource, GetBlobSyntaxParams};
use crate::service::FileDiffs;
use crate::syntax::{SYNTAX_FORMAT, SyntaxStatus};
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use text_diff::policy::WhitespacePolicy;

const KOTLIN: &str = "class A {\n  val count = 1\n  fun get() = count\n}\n";

/// The `u32`s of an answer
fn words(bytes: &[u8]) -> Vec<u32> {
  bytes.as_chunks::<4>().0.iter().map(|it| u32::from_le_bytes(*it)).collect()
}

fn params(repo: &TestRepo, id: &str, file_path: &str, lines: (u32, u32), text: Option<&str>) -> GetBlobSyntaxParams {
  GetBlobSyntaxParams {
    repository_path: repo_path(repo).to_string(),
    blob: BlobRequest { id: id.to_string(), path: None },
    file_path: file_path.to_string(),
    from_line: lines.0,
    to_line: lines.1,
    text: text.map(str::to_string),
  }
}

fn committed() -> (TestRepo, String) {
  let repo = TestRepo::new();
  write(&repo, "A.kt", KOTLIN.as_bytes());
  write(&repo, "notes.txt", b"plain\n");
  let commit = commit_all(&repo, "Initial");
  let id = repo.rev_parse(&format!("{commit}:A.kt")).unwrap();
  (repo, id)
}

#[test]
fn answers_the_runs_of_the_lines_asked_for_with_the_line_starts_from_zero() {
  let (repo, id) = committed();
  let answer = words(&FileDiffs::default().blob_syntax(repo.git_executor(), &params(&repo, &id, "A.kt", (1, 3), None)).unwrap());
  let (header, rest) = answer.split_at(6);
  // the format, the status, 5 lines (the empty one after the final line end), the lines 1 and 2, then their runs
  assert_eq!(header[..5], [SYNTAX_FORMAT, SyntaxStatus::Runs as u32, 5, 1, 3]);
  let pairs = header[5] as usize;
  let (starts, runs) = rest.split_at(3);
  assert_eq!(starts[0], 0);
  assert_eq!(starts[2] as usize, pairs);
  assert_eq!(runs.len(), pairs * 2);
  // `  val count = 1`: the keyword at 2, the property at 6
  assert_eq!(runs[..4], [2, 1, 5, 0]);
  assert_eq!(runs[4..6], [6, 7]);
}

#[test]
fn parses_a_blob_once_from_the_blobs_a_diff_read_and_reads_a_cold_one_with_one_cat_file() {
  let (repo, id) = committed();
  write(&repo, "A.kt", KOTLIN.replace("= 1", "= 2").as_bytes());
  let second = commit_all(&repo, "Change");
  let diffs = FileDiffs::default();
  diffs
    .read(
      repo.git_executor(),
      repo_path(&repo),
      &ObjectCache::disabled(),
      &DiffSource::Commit { id: second },
      WhitespacePolicy::Default,
    )
    .unwrap();
  let counting = repo.git_executor().with_process_stats("test");
  let first = diffs.blob_syntax(&counting, &params(&repo, &id, "A.kt", (0, 5), None)).unwrap();
  assert_eq!(process_counts(&counting), []);
  // the second time from the highlightings it keeps
  assert_eq!(diffs.blob_syntax(&counting, &params(&repo, &id, "A.kt", (0, 5), None)).unwrap(), first);

  let fresh = FileDiffs::default();
  let counting = repo.git_executor().with_process_stats("test");
  assert_eq!(fresh.blob_syntax(&counting, &params(&repo, &id, "A.kt", (0, 5), None)).unwrap(), first);
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1)]);
}

#[test]
fn says_missing_for_a_blob_it_cannot_read_and_highlights_the_text_a_view_sends() {
  let (repo, _) = committed();
  let diffs = FileDiffs::default();
  let status = |bytes: Vec<u8>| words(&bytes)[1];
  // a text the view made: no object id, no git
  let counting = repo.git_executor().with_process_stats("test");
  assert_eq!(
    status(diffs.blob_syntax(&counting, &params(&repo, "merge-result-1", "A.kt", (0, 1), None)).unwrap()),
    SyntaxStatus::Missing as u32
  );
  assert_eq!(
    status(diffs.blob_syntax(&counting, &params(&repo, "merge-result-1", "A.kt", (0, 1), Some(KOTLIN))).unwrap()),
    SyntaxStatus::Runs as u32
  );
  assert_eq!(process_counts(&counting), []);
  let absent = "0123456789abcdef0123456789abcdef01234567";
  assert_eq!(
    status(diffs.blob_syntax(repo.git_executor(), &params(&repo, absent, "A.kt", (0, 1), None)).unwrap()),
    SyntaxStatus::Missing as u32
  );
}

#[test]
fn falls_back_to_the_worker_for_a_file_without_a_grammar() {
  let (repo, _) = committed();
  let id = repo.rev_parse("HEAD:notes.txt").unwrap();
  let answer = words(
    &FileDiffs::default()
      .blob_syntax(repo.git_executor(), &params(&repo, &id, "notes.txt", (0, 1), None))
      .unwrap(),
  );
  assert_eq!(answer, [SYNTAX_FORMAT, SyntaxStatus::Fallback as u32, 0, 0, 0, 0]);
}
