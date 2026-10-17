use crate::conflict_contents::{GetConflictContentsParams, read_conflict_contents};
use crate::sync::{SyncOptions, sync_branches};
use git_ops::model::{BranchError, MergeConflictInfo};
use object_cache::cache::{CacheConfig, ObjectCache};
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
/// the files: a conflict in a file of megabytes is not sent to the page at every sync. The view reads the contents
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

  let contents = read_conflict_contents(big_repo.git_executor(), &ObjectCache::disabled(), &params_of(&big_repo, &conflict)).unwrap();
  let merged = &contents.conflicting_files[0].merged.content;
  assert!(merged.starts_with("<<<<<<< ") && merged.ends_with("line 200000 of the big file\n"), "{}", &merged[..200]);
  let missing_diffs = &contents.missing_commit_diffs[&conflict.conflict_analysis.missing_commits[0].hash];
  assert_eq!(missing_diffs[0].new_file.content.len(), big_content.len() + " alpha".len());
}

/// What `get_conflict_contents` gets for `conflict`
fn params_of(repo: &TestRepo, conflict: &MergeConflictInfo) -> GetConflictContentsParams {
  GetConflictContentsParams {
    repository_path: repo.path_str().to_string(),
    commit: conflict.commit.hash.clone(),
    parent: conflict.original_parent.hash.clone(),
    target: conflict.target_branch.hash.clone(),
  }
}

/// The contents are read only for the full ids of a cherry-pick, which go to `merge-tree` as arguments, and only of
/// commits that conflict
#[tokio::test]
async fn the_contents_are_read_for_the_full_ids_of_a_conflict() {
  let repo = repo_with_a_conflict_in(&numbered_lines(10));
  let conflict = conflict_of(&sync_events(&repo).await);
  let params = GetConflictContentsParams {
    commit: "--output=x".to_string(),
    ..params_of(&repo, &conflict)
  };
  let error = read_conflict_contents(repo.git_executor(), &ObjectCache::disabled(), &params).unwrap_err();
  assert_eq!(error.to_string(), "--output=x is not a full commit id");

  // the commit onto itself applies cleanly
  let params = GetConflictContentsParams {
    target: conflict.original_parent.hash.clone(),
    ..params_of(&repo, &conflict)
  };
  let error = read_conflict_contents(repo.git_executor(), &ObjectCache::disabled(), &params).unwrap_err();
  assert_eq!(error.to_string(), "the commit applies cleanly now, sync again");
}

/// The contents a sync remembered are the ones the analysis makes when a view opens a conflict the cache misses
#[tokio::test]
async fn the_remembered_contents_are_the_ones_the_analysis_makes() {
  let repo = repo_with_a_conflict_in(&numbered_lines(10));
  let dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(dir.path(), &CacheConfig::default());
  let progress = TestReporter::new();
  let options = SyncOptions {
    object_cache: cache.clone(),
    ..SyncOptions::default()
  };
  sync_branches(repo.git_executor(), repo.path_str(), "test", progress.clone(), options).await.unwrap();
  let params = params_of(&repo, &conflict_of(&progress.get_events()));

  let remembered = read_conflict_contents(repo.git_executor(), &cache, &params).unwrap();
  let analysed = read_conflict_contents(repo.git_executor(), &ObjectCache::disabled(), &params).unwrap();
  assert_eq!(serde_json::to_value(&remembered).unwrap(), serde_json::to_value(&analysed).unwrap());
  assert_eq!(remembered.conflicting_files.len(), 1);
  assert_eq!(remembered.missing_commit_diffs.len(), 1);
}
