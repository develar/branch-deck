//! A sync that resolves a conflict of git by words (`branchdeck.resolveByWords`): the copy, its note and its event,
//! the reuse by the next sync, the setting, all or nothing, a stack of resolved commits, and the same commit on two
//! machines.

use crate::check::{CheckOutcome, check_plan};
use domain_types::conflict::{ResolvedFileSummary, WordResolutionSummary};
use domain_types::{BranchError, BranchSyncStatus, CommitSyncStatus, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::word_resolve::RESOLVE_VERSION;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use sync_core::sync::{SyncOptions, sync_branches};
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::{TestReporter, WORD_CONFLICT_TIME as TIME, commit_files, repo_with_a_word_conflict, repo_with_word_changes as repo_with, with_resolve_version};

const PREFIX: &str = "test";
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
  assert!(note.contains("v-commit-resolved-v1:4 1 1"), "{note}");
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
  sync_events_with(second.git_executor(), &second, test_utils::process_budget::cache_in(dir.path())).await;
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

/// `brd check` and the trials of a fix build as the sync does: the commit a check writes is the one the sync writes, also
/// from the resolve that the object cache remembers
#[tokio::test]
async fn a_check_resolves_as_the_sync_does() {
  let repo = repo_with_a_word_conflict();
  let dir = tempfile::tempdir().unwrap();
  let cache = test_utils::process_budget::cache_in(dir.path());
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

/// A copy that the resolve by words of `version` wrote of the commit `original`: `copy` on the branch of beta, its
/// mapping note in the words of that resolve
fn older_copy(repo: &TestRepo, original: &str, copy: &str, version: u32) {
  repo.git(&["update-ref", BETA, copy]);
  with_resolve_version(repo, original, version, Some(copy));
}

/// A commit with the parent, the message and the author of the copy of beta and `path` holding `content`: a copy that
/// another resolve wrote otherwise
fn copy_with(repo: &TestRepo, path: &str, content: &str) -> String {
  let branch = repo.git(&["symbolic-ref", "--short", "HEAD"]);
  repo.git(&["checkout", "-q", "--detach", "refs/remotes/origin/master"]);
  let copy = commit_files(repo, "Change the call again", &[(path, content)], TIME + 200);
  repo.git(&["checkout", "-q", &branch]);
  copy
}

/// The resolve version of the mapping note of `original`, and the copy it names
fn noted(repo: &TestRepo, original: &str) -> (u32, String) {
  let note = repo.git(&["notes", "show", original]);
  let version = git_ops::notes::mapped_resolution(&note).expect("a resolve line").version;
  (version, git_ops::notes::mapped_commit_id(&note).expect("a mapping line").to_string())
}

/// A copy that an older resolve wrote with the bytes this resolve gives keeps its commit, and its note takes this resolve,
/// whichever older resolve it was: the next sync checks it no more
/// (`process_budget_test::a_copy_of_an_older_resolve_is_checked_once` of sync-core)
#[tokio::test]
async fn a_copy_of_an_older_resolve_with_the_same_bytes_keeps_its_commit_and_takes_this_resolve() {
  let repo = repo_with_a_word_conflict();
  sync_events(&repo).await;
  let (original, tip) = (repo.head(), repo.rev_parse(BETA).unwrap());
  for version in 1..RESOLVE_VERSION {
    older_copy(&repo, &original, &tip, version);
    let events = sync_events(&repo).await;
    assert_eq!(repo.rev_parse(BETA).unwrap(), tip, "the copy of version {version}");
    assert_eq!(beta_copies(&events), vec![(CommitSyncStatus::Unchanged, Some(resolved_call(&repo)))], "version {version}");
    assert_eq!(noted(&repo, &original), (RESOLVE_VERSION, tip.clone()), "version {version}");
  }
}

/// A copy that an older resolve wrote with other bytes is copied again, as a copy whose commit changed; the same copy
/// noted by this resolve is taken as it is
#[tokio::test]
async fn a_copy_of_an_older_resolve_with_other_bytes_is_copied_again() {
  let repo = repo_with_a_word_conflict();
  sync_events(&repo).await;
  let (original, tip) = (repo.head(), repo.rev_parse(BETA).unwrap());
  let other = copy_with(&repo, "Call.kt", "call(A, B, C)\nend\n");
  older_copy(&repo, &original, &other, RESOLVE_VERSION);
  sync_events(&repo).await;
  assert_eq!(repo.rev_parse(BETA).unwrap(), other, "a copy of this resolve is taken as it is");

  older_copy(&repo, &original, &other, 1);
  let events = sync_events(&repo).await;
  assert_eq!(repo.rev_parse(BETA).unwrap(), tip, "the copy this resolve writes");
  assert_eq!(file(&repo, BETA, "Call.kt"), "call(a, b, C)\nend");
  assert_eq!(beta_copies(&events), vec![(CommitSyncStatus::Created, Some(resolved_call(&repo)))]);
  assert_eq!(noted(&repo, &original), (RESOLVE_VERSION, tip));
}

/// `copy` on the branch of beta as the copy of the commit `original` that the resolve of `version` wrote of a conflict:
/// its mapping note with the source of `original` and one resolved conflict
fn resolved_copy(repo: &TestRepo, original: &str, copy: &str, version: u32) {
  let tree = |rev: &str| repo.rev_parse(&format!("{rev}^{{tree}}")).unwrap();
  let source = format!("{} {}", tree(&format!("{original}~1")), tree(original));
  repo.git(&["update-ref", BETA, copy]);
  let note = format!("v-commit-v1:{copy}\nv-commit-source-v1:{source}\nv-commit-resolved-v1:{version} 1 1");
  repo.git(&["notes", "add", "-f", "-m", &note, original]);
}

/// The status the last sync of `events` gave beta
fn beta_status(events: &[SyncEvent]) -> Option<BranchSyncStatus> {
  events.iter().rev().find_map(|event| match event {
    SyncEvent::BranchStatusUpdate { branch_name, status, .. } if branch_name == "beta" => Some(*status),
    _ => None,
  })
}

/// A copy that an older resolve wrote of a conflict that this resolve leaves is the conflict of git: a blank line at the
/// end of the baseline and a line that beta adds there, which an older merge took for two insertions
/// (`word_resolve_test::two_insertions_at_the_end_of_a_file_stay_the_conflict_of_git` of git-ops); the same copy noted
/// by this resolve is taken as it is
#[tokio::test]
async fn a_copy_of_an_older_resolve_that_this_resolve_leaves_is_the_conflict_of_git() {
  let repo = repo_with(&[("end.txt", "x\n\n")], &[("end.txt", "x\n")], &[("end.txt", "x\ny\n")]);
  let original = repo.head();
  assert!(has_conflict(&sync_events(&repo).await), "this resolve leaves the conflict to git");
  let copy = copy_with(&repo, "end.txt", "x\ny\n\n");
  resolved_copy(&repo, &original, &copy, RESOLVE_VERSION);
  let events = sync_events(&repo).await;
  assert!(!has_conflict(&events), "a copy of this resolve is taken as it is");
  assert_eq!(repo.rev_parse(BETA).unwrap(), copy);

  resolved_copy(&repo, &original, &copy, 1);
  assert!(has_conflict(&sync_events(&repo).await));
}

/// A copy of resolve 3 whose bytes its step that ignored whitespace wrote, a line of code joined
/// (`releaseOnFailure = true;try {`, a shape of a merge of real history), is not kept by this resolve, which takes the
/// default policy alone: the sync gives the conflict of git, and the branch is not built, so no copy of the sync holds
/// the joined line. Its ref keeps the last build, as the ref of every conflicting branch does: nothing pushes it, and
/// the view takes no tip of it ("A conflicting branch keeps its copies before the conflict" of
/// docs/design/git-operations.md). The same copy noted by this resolve is taken as it is
#[tokio::test]
async fn a_copy_of_resolve_3_with_a_line_its_whitespace_step_joined_is_the_conflict_of_git() {
  let (baseline, alpha, beta) = ("try {\n  connect();", "try {\n", "releaseOnFailure = true; try {\n\n");
  let joined = "releaseOnFailure = true;try {\n  connect();";
  // what the resolve of version 3 wrote: the second step of IntelliJ's simple resolve, which this one no longer takes
  let [ours, base, theirs] = [baseline, alpha, beta].map(|text| text.encode_utf16().collect::<Vec<u16>>());
  let old = line_diff::merge_resolve_util::try_resolve(&ours, &base, &theirs, &line_diff::cancellation::Deadline::NONE);
  assert_eq!(old.map(|text| String::from_utf16_lossy(&text)).as_deref(), Some(joined));

  let repo = repo_with(&[("Release.java", baseline)], &[("Release.java", alpha)], &[("Release.java", beta)]);
  let original = repo.head();
  assert!(has_conflict(&sync_events(&repo).await), "this resolve leaves the conflict to git");
  let copy = copy_with(&repo, "Release.java", joined);
  resolved_copy(&repo, &original, &copy, RESOLVE_VERSION);
  assert!(!has_conflict(&sync_events(&repo).await), "a copy of this resolve is taken as it is");

  resolved_copy(&repo, &original, &copy, 3);
  let events = sync_events(&repo).await;
  assert!(has_conflict(&events));
  assert_eq!(beta_status(&events), Some(BranchSyncStatus::MergeConflict));
  assert_eq!(beta_copies(&events), Vec::new(), "no copy holds the joined line");
}
