//! The measurement of the marks of a subject (`marks_measure`) on the `commit_marks` template: what a view gets, with
//! the texts, the proofs and the times, and the same subject without any mark.

use crate::fixture_test::unhurried;
use crate::marks_measure::{MeasuredFile, MeasuredSubject};
use crate::model::{DiffContent, DiffSource};
use crate::service::{FileDiffs, diff_options, disk_key, model_version};
use code_symbols::highlight::Grammar;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use test_log::test;
use text_diff::model::{BlockKind, RunKind, TextDiff};
use text_diff::policy::WhitespacePolicy::Default as Exact;

/// The repository of the `commit_marks` template, its path and its commit
fn template() -> (tempfile::TempDir, String, DiffSource) {
  let directory = tempfile::tempdir().unwrap();
  test_utils::repo_template::templates::commit_marks().build(directory.path()).unwrap();
  let path = directory.path().to_str().unwrap().to_string();
  let head = GitCommandExecutor::new().execute_command(&["rev-parse", "HEAD"], &path).unwrap();
  (directory, path, DiffSource::Commit { id: head })
}

fn file<'a>(subject: &'a MeasuredSubject, path: &str) -> (&'a MeasuredFile, &'a TextDiff) {
  let file = subject.files.iter().find(|file| file.file.path == path).unwrap_or_else(|| panic!("no file {path}"));
  match &file.file.content {
    DiffContent::Text(diff) => (file, diff),
    other => panic!("{path}: {other:?}"),
  }
}

/// A measured subject is the subject a view reads, file by file, and says beside each text what no view is told: the
/// two texts, whether the tokens were read, which runs are proven whatever their kind is, and the stops the file has
/// once every proven run is passed
#[test]
fn a_measured_subject_is_what_a_view_reads_with_its_texts_its_proofs_and_its_times() {
  let (_directory, path, source) = template();
  let git = GitCommandExecutor::new();
  let cache = ObjectCache::disabled();
  let measured = unhurried().measure(&git, &path, &cache, None, &source, Exact).unwrap();
  let shown = unhurried().read(&git, &path, &cache, &source, Exact).unwrap();
  assert_eq!(measured.files.iter().map(|file| &file.file).collect::<Vec<_>>(), shown.iter().collect::<Vec<_>>());
  // the first content came before the end of the read, and no index was asked
  assert!(measured.first_content.is_some_and(|first| first <= measured.end), "{measured:?}");
  assert_eq!(measured.index, None);

  let (ledger, diff) = file(&measured, "src/Ledger.kt");
  let text = ledger.text.as_ref().unwrap();
  assert_eq!((text.grammar, text.parsed, text.differs), (Some(Grammar::Kotlin), true, None));
  assert!(text.text(false).contains("var total = 0L") && text.text(true).contains("var sum = 0L"));
  // the wrapped call and the three lines of the renamed local are proven; the comment is not
  let kinds: Vec<(RunKind, bool)> = diff.marks.runs.iter().map(|run| run.kind).zip(text.proven.iter().copied()).collect();
  assert_eq!(
    kinds,
    [
      (RunKind::Formatting, true),
      (RunKind::Renamed, true),
      (RunKind::Renamed, true),
      (RunKind::Renamed, true),
      (RunKind::Comments, false)
    ]
  );
  assert_eq!(text.proven_renames, [0]);
  // without the three blocks of the renamed local: five stops of eleven blocks
  assert_eq!(text.proven_starts, [0, 1, 5, 2, 1, 18, 7, 1, 36, 8, 1, 41, 10, 0, 53]);
  // a run a view passes is a proven one, and the stops of a view are never fewer than the proven ones
  for (run, proven) in diff.marks.runs.iter().zip(&text.proven) {
    assert!(*proven || !run.quiet, "{run:?}");
  }
  assert!(diff.marks.starts.len() >= text.proven_starts.len());

  // the calls of a name another file declares are runs the subject added: no index proved them
  let (report, diff) = file(&measured, "src/Report.kt");
  let text = report.text.as_ref().unwrap();
  assert_eq!((text.differs, text.proven.as_slice()), (None, &[false, false][..]));
  assert_eq!(text.proven_starts, diff.marks.starts);
  let (store, _) = file(&measured, "src/Store.kt");
  assert!(store.text.as_ref().is_some_and(|text| text.parsed && text.differs.is_none() && text.proven.is_empty()));
}

/// The same subject without its marks is the comparison alone: every block a stop, no run, no move, no rename,
/// nothing of the subject, nothing a measurement reads beside it; and no store keeps such a diff
#[test]
fn a_subject_read_without_marks_has_a_stop_at_each_block_and_no_store_keeps_it() {
  let (directory, path, source) = template();
  let git = GitCommandExecutor::new();
  let cache = test_utils::process_budget::cache_in(directory.path());
  let measured = FileDiffs::without_marks().measure(&git, &path, &cache, None, &source, Exact).unwrap();
  assert_eq!(measured.files.len(), 3);
  for file in &measured.files {
    let DiffContent::Text(diff) = &file.file.content else {
      panic!("{}: {:?}", file.file.path, file.file.content)
    };
    assert!(file.text.is_none(), "{}", file.file.path);
    let marks = &diff.marks;
    assert!(
      marks.runs.is_empty() && marks.moves.is_empty() && marks.renames.is_empty() && marks.subject.is_none(),
      "{}: {marks:?}",
      file.file.path
    );
    let blocks = diff.blocks.iter().filter(|block| block.kind != BlockKind::Ignored).count();
    assert_eq!(marks.starts.len(), blocks * 3, "{}", file.file.path);
    let key = (
      diff_options(Exact, file.file.old_path.as_deref(), &file.file.path),
      file.file.old_id.clone().unwrap(),
      file.file.new_id.clone().unwrap(),
    );
    assert_eq!(cache.get_file_diffs::<TextDiff>(model_version(), &[disk_key(&key)]), [None], "{}", file.file.path);
  }
  // a view that reads the subject next, with the same store, gets its marks
  let shown = FileDiffs::default().read(&git, &path, &cache, &source, Exact).unwrap();
  let DiffContent::Text(ledger) = &shown[0].content else {
    panic!("{:?}", shown[0].content)
  };
  assert_eq!((ledger.marks.runs.len(), ledger.marks.moves.len(), ledger.marks.renames.len()), (5, 2, 1));
}
