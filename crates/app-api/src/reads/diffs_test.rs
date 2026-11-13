//! The question of a window to the index about the names a subject renamed (`get_index_marks`): asked after the
//! diffs of the subject, killed once the window asks of another subject, closes or loads again, and never by another
//! window.

use crate::reads::diffs::IndexAsker;
use crate::reads::review_prefetch_test::{Index, Indexes};
use crate::session::windows::Windows;
use crate::transport::Call;
use file_diffs::model::{FileMarks, GetFileDiffsParams, GetIndexMarksParams};
use file_diffs::service::FileDiffs;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use test_utils::git_test_utils::TestRepo;
use tokio::sync::mpsc;

const STORE: &str = "package demo\n\nclass Store(val path: String)\n\nfun openStore(path: String): Store {\n  return Store(path)\n}\n";
const REPORT: &str = "package demo\n\nfun report(path: String): String {\n  val store = openStore(path)\n  return store.path\n}\n";

/// The proof of the name the report uses, in the answer of a question, as the page reads it
fn proof_of(answer: Option<&Vec<FileMarks>>) -> Option<String> {
  let report = serde_json::to_value(answer?.iter().find(|file| file.path == "src/Report.kt")?).ok()?;
  Some(report.pointer("/marks/subject/used/0/proof")?.as_str()?.to_string())
}

#[test]
fn the_question_of_a_window_to_the_index_ends_once_the_window_asks_another_closes_or_loads_again() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("src/Store.kt", STORE), ("src/Report.kt", REPORT)]);
  let renamed = |text: &str| text.replace("openStore", "connectStore");
  let second = repo.create_commit_with_files("Rename", &[("src/Store.kt", &renamed(STORE)), ("src/Report.kt", &renamed(REPORT))]);
  let (diffs, cache) = (FileDiffs::default(), ObjectCache::disabled());
  // the diffs of the subject first, as a window reads them, then the question, both as the page sends them
  let subject = serde_json::json!({
    "repositoryPath": repo.path().to_str().unwrap(),
    "source": {"kind": "commit", "id": second},
    "policy": "default",
    "priorityPaths": [],
  });
  let shown: GetFileDiffsParams = serde_json::from_value(subject.clone()).unwrap();
  diffs.stream(repo.git_executor(), &cache, true, &shown, |_| {}).unwrap();
  let params: GetIndexMarksParams = serde_json::from_value(subject).unwrap();

  // an index that holds its query until the search is canceled, or it is released
  let (started, starts) = mpsc::unbounded_channel();
  let held = Arc::new(Index::default());
  held.held.store(true, Ordering::SeqCst);
  *held.started.lock().unwrap_or_else(PoisonError::into_inner) = Some(started);
  let (index, starts) = (Indexes(Arc::clone(&held)), Mutex::new(starts));
  let windows = Windows::default();
  // the question of `asker` as the page has it: the files of the stages it was told, `None` when it was not told the
  // last one
  let answer_of = |asker: &IndexAsker<'_>| {
    let mut told: Vec<FileMarks> = Vec::new();
    let answered = asker.ask(&diffs, repo.git_executor(), &cache, &params, &mut |stage| told.extend(stage.files)).unwrap();
    answered.then_some(told)
  };
  // asks as the page of `call`, and does `then` once the query of the index runs
  let ask = |call: &Call, then: &dyn Fn()| {
    let request = windows.begin_index_marks(&call.window);
    let asker = IndexAsker {
      windows: &windows,
      indexes: &index,
      call,
      request: &request,
    };
    std::thread::scope(|scope| {
      let asked = scope.spawn(|| answer_of(&asker));
      starts.lock().unwrap_or_else(PoisonError::into_inner).blocking_recv().unwrap();
      then();
      asked.join().unwrap()
    })
  };
  let killed = || held.killed.load(Ordering::SeqCst);
  let asked = || held.asked.lock().unwrap_or_else(PoisonError::into_inner).len();
  let (review, main) = (windows.call("review-7"), windows.call("main"));

  // the window asks of another subject: the query of this one is killed, and it answers nothing
  assert_eq!(ask(&review, &|| drop(windows.begin_index_marks(&review.window))), None);
  assert_eq!(killed(), 1);
  // the window loads again: its page is another
  assert_eq!(ask(&review, &|| drop(windows.forget_page(&review.window))), None);
  assert_eq!(killed(), 2);
  // a page that is gone asks nothing at all
  let before = asked();
  let request = windows.begin_index_marks(&review.window);
  let gone = IndexAsker {
    windows: &windows,
    indexes: &index,
    call: &review,
    request: &request,
  };
  assert_eq!(answer_of(&gone), None);
  assert_eq!(asked(), before);
  // the window closes
  let review = windows.call("review-7");
  assert_eq!(ask(&review, &|| windows.forget_reads(&review.window)), None);
  assert_eq!(killed(), 3);

  // what another window does ends nothing: the question of the main window goes on until the index answers
  let answer = ask(&main, &|| {
    drop(windows.begin_index_marks(&review.window));
    windows.forget_reads(&review.window);
    drop(windows.forget_page(&review.window));
    held.held.store(false, Ordering::SeqCst);
  });
  assert_eq!((proof_of(answer.as_ref()).as_deref(), killed()), (Some("unavailable"), 3));
}
