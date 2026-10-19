use crate::sync::{SyncOptions, sync_branches};
use git_ops::model::{BranchError, MergeConflictInfo};
use pretty_assertions::assert_eq;
use std::fmt::Write;
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use test_utils::git_test_utils::TestRepo;

/// A file of `lines` lines that each name their number
fn numbered_lines(lines: usize) -> String {
  (1..=lines).fold(String::new(), |mut content, line| {
    let _ = writeln!(content, "line {line} of the big file");
    content
  })
}

/// The baseline has `big.txt`; alpha changes its first line, beta changes it again: beta conflicts on the baseline,
/// with the commit of alpha missing
fn repo_with_a_conflict_in(content: &str) -> TestRepo {
  let repo = TestRepo::new();
  let initial = repo.create_commit("Initial commit", "big.txt", content);
  repo.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/master", &initial]);
  let (first, rest) = content.split_once('\n').unwrap();
  repo.create_commit("(alpha) Change the first line", "big.txt", &format!("{first} alpha\n{rest}"));
  repo.create_commit("(beta) Change the first line again", "big.txt", &format!("{first} beta\n{rest}"));
  repo
}

/// The events of a sync of `repo`
async fn sync_events(repo: &TestRepo) -> Vec<SyncEvent> {
  let progress = TestReporter::new();
  sync_branches(repo.git_executor(), repo.path_str(), "test", progress.clone(), SyncOptions::default())
    .await
    .unwrap();
  progress.get_events()
}

/// The events of the sync that report a conflict, as JSON, the way the app sends them to the page
fn conflict_events(events: &[SyncEvent]) -> Vec<String> {
  events
    .iter()
    .filter(|event| {
      matches!(
        event,
        SyncEvent::CommitError {
          error: BranchError::MergeConflict(_),
          ..
        } | SyncEvent::BranchStatusUpdate {
          error: Some(BranchError::MergeConflict(_)),
          ..
        }
      )
    })
    .map(|event| serde_json::to_string(event).unwrap())
    .collect()
}

fn conflict_of(events: &[SyncEvent]) -> MergeConflictInfo {
  events
    .iter()
    .find_map(|event| match event {
      SyncEvent::CommitError {
        error: BranchError::MergeConflict(info),
        ..
      } => Some((**info).clone()),
      _ => None,
    })
    .expect("a conflict")
}

/// The sync reports a conflict with its files and missing commits but no contents, a few kilobytes whatever the size of
/// the files: a conflict in a file of megabytes is not sent to the page at every sync. The views read the files with
/// `get_file_merges` and `get_file_diffs`
#[tokio::test]
async fn a_sync_reports_a_conflict_without_the_contents_of_its_files() {
  let small = sync_events(&repo_with_a_conflict_in(&numbered_lines(10))).await;
  let big_content = numbered_lines(200_000);
  assert!(big_content.len() > 5 << 20);
  let big_repo = repo_with_a_conflict_in(&big_content);
  let big = sync_events(&big_repo).await;

  let (small_events, big_events) = (conflict_events(&small), conflict_events(&big));
  assert_eq!(big_events.len(), 2, "the error of the commit and the status of the branch");
  for (small_event, big_event) in small_events.iter().zip(&big_events) {
    assert!(big_event.len() < 4096, "{} bytes: {big_event}", big_event.len());
    // the ids and times differ, their lengths do not
    assert_eq!(small_event.len(), big_event.len());
    assert!(!big_event.contains("of the big file"), "{big_event}");
  }

  let conflict = conflict_of(&big);
  let files: Vec<(&str, u32)> = conflict.conflicting_files.iter().map(|file| (file.file.as_str(), file.conflict_count)).collect();
  assert_eq!(files, [("big.txt", 1)]);
  let missing: Vec<&str> = conflict.conflict_analysis.missing_commits.iter().map(|commit| commit.subject.as_str()).collect();
  assert_eq!(missing, ["(alpha) Change the first line"]);
}
