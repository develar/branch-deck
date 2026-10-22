//! A sync that resolves a conflict of git by words (`branchdeck.resolveByWords`): the copy, its note and its event,
//! the reuse by the next sync, the setting, all or nothing, a stack of resolved commits, and the same commit on two
//! machines.

use crate::check::{CheckOutcome, check_plan};
use crate::sync::{SyncOptions, sync_branches};
use domain_types::conflict::{ResolvedFileSummary, WordResolutionSummary};
use domain_types::{BranchError, BranchSyncStatus, CommitSyncStatus, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::{CacheConfig, ObjectCache};
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

const PREFIX: &str = "test";
/// The time of the commits: two repositories made alike have the same commit ids
const TIME: i64 = 1_700_000_000;

/// The baseline (`origin/master`) has `files`; alpha changes them by `alpha`, then beta by `beta`. Beta is copied onto
/// the baseline without alpha, so git conflicts wherever beta changes a line alpha changed
fn repo_with(files: &[(&str, &str)], alpha: &[(&str, &str)], beta: &[(&str, &str)]) -> TestRepo {
  let repo = TestRepo::new();
  for (index, (path, content)) in files.iter().enumerate() {
    repo.create_commit_with_timestamp(&format!("Add {path}"), path, content, Some(TIME + i64::try_from(index).unwrap()));
  }
  repo.git(&["remote", "add", "origin", "."]);
  let initial = repo.head();
  repo.git(&["update-ref", "refs/remotes/origin/master", &initial]);
  commit_files(&repo, "(alpha) Change the call", alpha, TIME + 100);
  commit_files(&repo, "(beta) Change the call again", beta, TIME + 200);
  repo
}

fn commit_files(repo: &TestRepo, message: &str, files: &[(&str, &str)], time: i64) -> String {
  for (path, content) in files {
    std::fs::write(repo.path().join(path), content).unwrap();
  }
  let date = format!("{time} +0000");
  repo
    .git_executor()
    .cmd(&["commit", "-qam", message], repo.path_str())
    .env(&[("GIT_AUTHOR_DATE", date.as_str()), ("GIT_COMMITTER_DATE", date.as_str())])
    .text()
    .unwrap();
  repo.head()
}

/// alpha changes the first argument, beta the third: one conflict of git that the words resolve
fn repo_with_a_word_conflict() -> TestRepo {
  repo_with(
    &[("Call.kt", "call(a, b, c)\nend\n")],
    &[("Call.kt", "call(A, b, c)\nend\n")],
    &[("Call.kt", "call(A, b, C)\nend\n")],
  )
}

async fn sync_events_with(git: &GitCommandExecutor, repo: &TestRepo, object_cache: ObjectCache) -> Vec<SyncEvent> {
  let progress = TestReporter::new();
  let options = SyncOptions {
    object_cache,
    ..SyncOptions::default()
  };
  sync_branches(git, repo.path_str(), PREFIX, progress.clone(), options).await.unwrap();
  progress.get_events()
}

async fn sync_events(repo: &TestRepo) -> Vec<SyncEvent> {
  sync_events_with(repo.git_executor(), repo, ObjectCache::disabled()).await
}

/// `(status, resolved)` of the copies of beta
fn beta_copies(events: &[SyncEvent]) -> Vec<(CommitSyncStatus, Option<WordResolutionSummary>)> {
  events
    .iter()
    .filter_map(|event| match event {
      SyncEvent::CommitSynced {
        branch_name, status, resolved, ..
      } if branch_name == "beta" => Some((*status, resolved.clone())),
      _ => None,
    })
    .collect()
}

fn has_conflict(events: &[SyncEvent]) -> bool {
  events.iter().any(|event| {
    matches!(
      event,
      SyncEvent::CommitError {
        error: BranchError::MergeConflict(_),
        ..
      }
    )
  })
}

fn file(repo: &TestRepo, rev: &str, path: &str) -> String {
  repo.git(&["show", &format!("{rev}:{path}")])
}

const BETA: &str = "refs/heads/test/virtual/beta";

/// The resolve of beta, the last commit, copied onto the baseline
fn resolved_call(repo: &TestRepo) -> WordResolutionSummary {
  WordResolutionSummary {
    parent: repo.rev_parse("HEAD~1").unwrap(),
    target: repo.rev_parse("refs/remotes/origin/master").unwrap(),
    conflicts: 1,
    file_count: 1,
    files: vec![ResolvedFileSummary {
      path: "Call.kt".to_string(),
      conflicts: 1,
    }],
  }
}

#[tokio::test]
async fn a_conflict_that_its_words_resolve_is_copied_with_the_author_and_the_message_of_the_commit() {
  let repo = repo_with_a_word_conflict();
  let events = sync_events(&repo).await;
  assert!(!has_conflict(&events));
  assert_eq!(beta_copies(&events), vec![(CommitSyncStatus::Created, Some(resolved_call(&repo)))]);
  // the change of beta on the baseline, as IntelliJ resolves it: the first argument of the baseline, the third of beta
  assert_eq!(file(&repo, BETA, "Call.kt"), "call(a, b, C)\nend");
  let original = repo.head();
  let format = "%an <%ae> %at%n%B";
  assert_eq!(
    repo.git(&["log", "-1", &format!("--format={format}"), BETA]),
    repo.git(&["log", "-1", &format!("--format={format}"), &original]).replace("(beta) ", "")
  );
  // the fact is in the mapping note of the original, not in the commit
  let note = repo.git(&["notes", "show", &original]);
  assert!(note.contains("v-commit-resolved-v1:1 1 1"), "{note}");
}

#[tokio::test]
async fn the_next_sync_reuses_the_resolved_copy_and_says_so_from_its_note() {
  let repo = repo_with_a_word_conflict();
  sync_events(&repo).await;
  let tip = repo.rev_parse(BETA).unwrap();
  let events = sync_events(&repo).await;
  assert_eq!(repo.rev_parse(BETA).unwrap(), tip);
  // the note keeps the counts, not the files: the view merges the commit again for them
  let from_note = WordResolutionSummary {
    files: Vec::new(),
    ..resolved_call(&repo)
  };
  assert_eq!(beta_copies(&events), vec![(CommitSyncStatus::Unchanged, Some(from_note))]);
}

#[tokio::test]
async fn the_setting_off_gives_the_conflict_of_git_and_on_again_the_resolved_copy() {
  let repo = repo_with_a_word_conflict();
  repo.set_config("branchdeck.resolveByWords", "false").unwrap();
  assert!(has_conflict(&sync_events(&repo).await));
  repo.set_config("branchdeck.resolveByWords", "true").unwrap();
  let events = sync_events(&repo).await;
  assert_eq!(beta_copies(&events), vec![(CommitSyncStatus::Created, Some(resolved_call(&repo)))]);
  // off again: the reused copy would keep a resolve the user turned off, so the commit is copied again and conflicts
  repo.set_config("branchdeck.resolveByWords", "false").unwrap();
  assert!(has_conflict(&sync_events(&repo).await));
}

#[tokio::test]
async fn a_file_with_a_conflict_that_its_words_leave_keeps_the_commit_a_conflict() {
  let repo = repo_with(
    &[("Call.kt", "call(a, b, c)\nend\n"), ("Name.kt", "name = x\n")],
    &[("Call.kt", "call(A, b, c)\nend\n"), ("Name.kt", "name = y\n")],
    &[("Call.kt", "call(A, b, C)\nend\n"), ("Name.kt", "name = z\n")],
  );
  let events = sync_events(&repo).await;
  assert!(has_conflict(&events));
  assert_eq!(beta_copies(&events), vec![]);
}

#[tokio::test]
async fn crlf_lines_keep_their_ends() {
  let repo = repo_with(
    &[("Win.kt", "call(a, b, c)\r\nend\r\n")],
    &[("Win.kt", "call(A, b, c)\r\nend\r\n")],
    &[("Win.kt", "call(A, b, C)\r\nend\r\n")],
  );
  assert!(!has_conflict(&sync_events(&repo).await));
  assert_eq!(
    repo.git_executor().cmd(&["cat-file", "blob", &format!("{BETA}:Win.kt")], repo.path_str()).bytes().unwrap(),
    b"call(a, b, C)\r\nend\r\n"
  );
}

/// Beta's second commit conflicts with the resolved copy of its first, and resolves too; the third changes another
/// file and applies cleanly onto it: the chain merges onto a tree the resolve wrote in the same branch
#[tokio::test]
async fn a_stack_of_commits_builds_on_the_resolved_copy() {
  let repo = repo_with(
    &[("Call.kt", "call(a, b, c)\nend\n"), ("Other.kt", "other\n")],
    &[("Call.kt", "call(A, b, c)\nend\n")],
    &[("Call.kt", "call(A, b, C)\nend\n")],
  );
  commit_files(&repo, "(beta) Change the second argument", &[("Call.kt", "call(A, B, C)\nend\n")], TIME + 300);
  commit_files(&repo, "(beta) Change the other file", &[("Other.kt", "other changed\n")], TIME + 400);
  let events = sync_events(&repo).await;
  assert!(!has_conflict(&events));
  let copies = beta_copies(&events);
  assert_eq!(copies.iter().map(|(_, resolved)| resolved.is_some()).collect::<Vec<_>>(), [true, true, false]);
  assert_eq!(file(&repo, BETA, "Call.kt"), "call(a, B, C)\nend");
  assert_eq!(file(&repo, BETA, "Other.kt"), "other changed");
}

/// Two machines with the same commits write the same copy: no deadline, the same committer date
#[tokio::test]
async fn the_same_commits_give_the_same_copy_in_another_repository() {
  let (first, second) = (repo_with_a_word_conflict(), repo_with_a_word_conflict());
  assert_eq!(first.head(), second.head());
  sync_events(&first).await;
  let dir = tempfile::tempdir().unwrap();
  sync_events_with(second.git_executor(), &second, ObjectCache::open_dir(dir.path(), &CacheConfig::default())).await;
  assert_eq!(first.rev_parse(BETA).unwrap(), second.rev_parse(BETA).unwrap());
}

/// A resolved conflict costs the merge with its messages, one `check-attr` and one `cat-file`, and no analysis
#[tokio::test]
async fn a_resolved_conflict_costs_no_analysis() {
  let repo = repo_with_a_word_conflict();
  let git = GitCommandExecutor::new().with_process_stats("sync");
  let events = sync_events_with(&git, &repo, ObjectCache::disabled()).await;
  let analysed = events.iter().any(|event| {
    matches!(
      event,
      SyncEvent::BranchStatusUpdate {
        status: BranchSyncStatus::AnalyzingConflict,
        ..
      }
    )
  });
  assert!(!analysed);
  let counts = git.process_stats().unwrap().counts();
  let count = |command: &str| counts.iter().find(|(name, _)| name == command).map_or(0, |(_, count)| *count);
  // the chain's merge and the merge with the messages
  assert_eq!((count("check-attr"), count("merge-tree")), (1, 2), "{counts:?}");
}

/// The head a check of beta reports, `None` when it does not build
fn checked_beta(repo: &TestRepo, cache: &ObjectCache) -> Option<String> {
  let report = check_plan(repo.git_executor(), repo.path_str(), PREFIX, None, cache).unwrap();
  report.branches.into_iter().find(|check| check.name == "beta").and_then(|check| match check.outcome {
    CheckOutcome::Ok { head } => Some(head),
    _ => None,
  })
}

/// `bd check` and the trials of a fix build as the sync does: the commit a check writes is the one the sync writes, also
/// from the resolve that the object cache remembers
#[tokio::test]
async fn a_check_resolves_as_the_sync_does() {
  let repo = repo_with_a_word_conflict();
  let dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(dir.path(), &CacheConfig::default());
  let head = checked_beta(&repo, &cache).expect("beta builds");
  assert_eq!(checked_beta(&repo, &cache), Some(head.clone()), "the remembered resolve gives the same commit");
  sync_events(&repo).await;
  assert_eq!(repo.rev_parse(BETA).unwrap(), head);
  repo.set_config("branchdeck.resolveByWords", "false").unwrap();
  assert_eq!(checked_beta(&repo, &cache), None, "with the resolve off, beta conflicts");
}

#[tokio::test]
async fn a_resolved_copy_leaves_no_temporary_ref_for_the_next_sync() {
  let repo = repo_with_a_word_conflict();
  sync_events(&repo).await;
  // the checkpoint of the resolve wrote the ref of the writer to disk; the end of the writer deletes it
  assert_eq!(repo.git(&["for-each-ref", "refs/branch-deck/tmp/"]), "");
}

#[tokio::test]
async fn the_shared_chain_ref_of_an_interrupted_older_sync_blocks_no_sync_and_is_deleted() {
  let repo = repo_with_a_word_conflict();
  // Branch Deck 0.21 wrote every chain on this one ref and left it behind after a resolve by words
  let unrelated = repo.rev_parse("refs/remotes/origin/master").unwrap();
  repo.git(&["update-ref", "refs/branch-deck/tmp/chain", &unrelated]);
  let young = format!(
    "refs/branch-deck/tmp/1-{}-0",
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
  );
  repo.git(&["update-ref", &young, &unrelated]);
  let events = sync_events(&repo).await;
  assert!(!has_conflict(&events));
  assert_eq!(beta_copies(&events), vec![(CommitSyncStatus::Created, Some(resolved_call(&repo)))]);
  // the ref of a running writer of another process stays
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/tmp/"]), young);
}
