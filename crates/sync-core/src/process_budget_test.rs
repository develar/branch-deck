//! The git processes of each operation the app runs, by subcommand: the numbers behind the batching rule
//! (docs/design/git-operations.md, "Batch"). A change that runs fewer processes lowers the numbers here, so the diff of an
//! assert shows which process went away; one that runs more has to say why. One test per operation, on a small fixed
//! repository, so the counts are the same on every run.

use crate::amend_to_branch::{AmendCommandResult, AmendUncommittedToBranchParams, amend_uncommitted_to_branch_core};
use crate::baseline::check_baseline_update;
use crate::moved_remote::{MovedBranch, fetch_moved_remotes};
use crate::push_fixture::{PushBranchesOutcome, PushBranchesParams, StackPushBranch, push_branches};
use crate::reapply_branch::{ReapplyBranchParams, ReapplyBranchResult, ReapplyUndoToken, reapply_branch_core, undo_reapply_core};
use crate::remote_status::{AncestorCommits, StackParent, compute_remote_status_for_branch, forget_statuses};
use crate::robot_landing::{robot_landings, robot_spans};
use crate::stack_branch::forget_builds;
use crate::sync::{SyncOptions, sync_branches};
use crate::target_conflict::{TargetConflictParams, TargetOutcome, check_branch_target};
use crate::unapply_branch::{UnapplyBranchParams, UnapplyBranchResult, UnapplyUndoToken, unapply_branch_core, undo_unapply_core};
use domain_types::robot_landing::RobotLandingRequest;
use domain_types::{BranchError, RemoteStatusUpdate, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::autosquash::{AutosquashScope, SquashFixupsParams, squash_fixup_commits_core};
use history_rewrite::parked::{ParkLine, park_core, unpark_core};
use history_rewrite::regroup::RegroupOptions;
use history_rewrite::repository_lock::{LockWait, RewriteOptions};
use history_rewrite::reword::{CommitReword, RewordRequest, RewordText, reword_request_core};
use history_rewrite::squash::{SquashCommitsParams, squash_commits_core};
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_utils::git_test_utils::{TestRepo, stack_ahead_of_bare_origin};
use test_utils::process_budget::{alpha_and_beta_on_origin, assert_processes, counting, empty_cache, repo_on_origin};
use test_utils::sync_fixtures::TestReporter;

const PREFIX: &str = "test";

fn path(repo: &TestRepo) -> String {
  repo.path().to_str().unwrap().to_string()
}

async fn sync(git: &GitCommandExecutor, repo: &TestRepo) {
  sync_branches(git, &path(repo), PREFIX, TestReporter::new(), SyncOptions::default()).await.unwrap();
}

#[tokio::test]
async fn a_first_sync() {
  let (repo, _) = alpha_and_beta_on_origin();
  let git = counting();
  sync(&git, &repo).await;
  assert_processes(
    &git,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("fast-import", 3),
      ("for-each-ref", 1),
      ("log", 2),
      ("merge-base", 1),
      ("merge-tree", 2),
      ("notes", 1),
      ("remote", 1),
      ("rev-list", 2),
      ("rev-parse", 1),
      ("show-ref", 1),
      ("symbolic-ref", 1),
      ("update-ref", 2),
      ("var", 1),
    ],
  );
}

#[tokio::test]
async fn an_unchanged_sync() {
  let (repo, _) = alpha_and_beta_on_origin();
  sync(&GitCommandExecutor::new(), &repo).await;
  let git = counting();
  sync(&git, &repo).await;
  assert_processes(&git, &UNCHANGED_SYNC);
}

/// The processes of a sync of `repo` that changes nothing; a conflicting branch whose view the object cache remembers
/// adds none
const UNCHANGED_SYNC: [(&str, u32); 8] = [
  ("cat-file", 1),
  ("config", 1),
  ("for-each-ref", 1),
  ("log", 4),
  ("merge-base", 1),
  ("remote", 1),
  ("rev-parse", 1),
  ("symbolic-ref", 1),
];

/// `repo` with a branch whose commit changes a file another branch adds: `delta` conflicts at every sync
fn repo_with_a_conflict() -> TestRepo {
  let (repo, _) = alpha_and_beta_on_origin();
  repo.create_commit("(gamma) Add file", "file.txt", "1\n");
  repo.create_commit("(delta) Change file", "file.txt", "2\n");
  repo
}

/// The events of a sync of `repo` with `cache` (disabled: without the object cache) on `git`
pub(crate) async fn sync_events(git: &GitCommandExecutor, repo: &TestRepo, cache: &ObjectCache) -> Vec<SyncEvent> {
  let progress = TestReporter::new();
  let options = SyncOptions {
    object_cache: cache.clone(),
    ..SyncOptions::default()
  };
  sync_branches(git, &path(repo), PREFIX, progress.clone(), options).await.unwrap();
  progress.get_events()
}

/// A sync with the object cache, whose conflict views it remembers, on `git`
async fn sync_with_cache(git: &GitCommandExecutor, repo: &TestRepo, cache: &ObjectCache) {
  sync_events(git, repo, cache).await;
}

/// The commit the first conflict of a sync is applied onto, from its `events`: a view merges onto it
fn conflict_target(events: Vec<SyncEvent>) -> String {
  let conflict = events
    .into_iter()
    .find_map(|event| match event {
      SyncEvent::CommitError {
        error: BranchError::MergeConflict(info),
        ..
      } => Some(info),
      _ => None,
    })
    .expect("a branch conflicts");
  conflict.target_branch.hash.clone()
}

/// The conflict of an unchanged branch costs its merge (in its own `merge-tree`) and the analysis (`rev-list`, two
/// `log`, `cat-file`) at every sync without the object cache, and none of them with it
#[tokio::test]
async fn an_unchanged_sync_of_a_conflicting_branch() {
  let repo = repo_with_a_conflict();
  sync(&GitCommandExecutor::new(), &repo).await;
  let git = counting();
  sync(&git, &repo).await;
  let mut analysed = vec![("cat-file", 2), ("log", 6), ("merge-tree", 2), ("rev-list", 1)];
  analysed.extend(UNCHANGED_SYNC.iter().filter(|(command, _)| !["cat-file", "log"].contains(command)));
  analysed.sort_unstable();
  assert_processes(&git, &analysed);

  let (_dir, cache) = empty_cache();
  assert_unchanged_with_cache(&repo, &cache).await;
}

/// A sync of `repo` with `cache`, then another one that changes nothing: it runs the processes of `UNCHANGED_SYNC`
async fn assert_unchanged_with_cache(repo: &TestRepo, cache: &ObjectCache) {
  sync_with_cache(&GitCommandExecutor::new(), repo, cache).await;
  let git = counting();
  sync_with_cache(&git, repo, cache).await;
  assert_processes(&git, &UNCHANGED_SYNC);
}

/// `origin/master` at the initial commit and, with stacked branches on, `alpha` with two children, `beta` and
/// `gamma`: a stack that forks, whose stack branch `test/stack/alpha` the sync builds too
fn forking_stack_repo() -> TestRepo {
  let repo = repo_on_origin();
  repo.set_config("branchdeck.stacks", "true").unwrap();
  for (index, prefix) in ["alpha", "alpha/beta", "alpha/gamma", "beta", "gamma", "alpha"].into_iter().enumerate() {
    repo.create_commit(&format!("({prefix}) Change {index}"), &format!("file-{index}.txt"), "text\n");
  }
  repo
}

/// The first sync of a stack that forks: its stack branch is one more build beside the three branches, one `fast-import`
/// (its commits are every commit ahead of the baseline here, so each copy keeps its tree and merges nothing; a
/// `merge-tree` otherwise), its ref one more `update-ref` and its remote status one more walk (`rev-list`)
#[tokio::test]
async fn a_first_sync_of_a_forking_stack() {
  let repo = forking_stack_repo();
  let git = counting();
  sync(&git, &repo).await;
  assert!(repo.branch_exists("test/stack/alpha"));
  assert_processes(
    &git,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("fast-import", 5),
      ("for-each-ref", 1),
      ("log", 2),
      ("merge-base", 1),
      ("merge-tree", 3),
      ("notes", 1),
      ("remote", 1),
      ("rev-list", 4),
      ("rev-parse", 1),
      ("show-ref", 2),
      ("symbolic-ref", 1),
      ("update-ref", 4),
      ("var", 1),
    ],
  );
}

/// An unchanged sync of a stack that forks runs no git for its stack branch: the build of the same commits is
/// remembered by the process, and in a new process by the object cache, and so is its remote status
#[tokio::test]
async fn an_unchanged_sync_of_a_forking_stack() {
  let repo = forking_stack_repo();
  let (_dir, cache) = empty_cache();
  assert_unchanged_with_cache(&repo, &cache).await;

  forget_statuses();
  forget_builds();
  let git = counting();
  sync_with_cache(&git, &repo, &cache).await;
  assert_processes(&git, &UNCHANGED_SYNC);
}

/// The time of the push of `alpha` in `pushed_repo`
const PUSH_TIME: u32 = 1_700_000_000;

/// `repo` synced, `alpha` pushed at `PUSH_TIME` (its remote branch at its tip) and `beta` fetched one commit behind
/// its tip; the statuses of that sync are forgotten, as by a new process
async fn pushed_repo(cache: &ObjectCache) -> TestRepo {
  let (repo, _) = alpha_and_beta_on_origin();
  sync_with_cache(&GitCommandExecutor::new(), &repo, cache).await;
  let alpha = repo.rev_parse("test/virtual/alpha").unwrap();
  GitCommandExecutor::new()
    .cmd(&["update-ref", "-m", "update by push", "refs/remotes/origin/test/virtual/alpha", &alpha], &path(&repo))
    .env(&[("GIT_COMMITTER_DATE", &format!("@{PUSH_TIME} +0000"))])
    .text()
    .unwrap();
  let behind = repo.rev_parse("test/virtual/beta~1").unwrap();
  repo.git(&["update-ref", "refs/remotes/origin/test/virtual/beta", &behind]);
  forget_statuses();
  repo
}

/// The remote statuses a sync of `repo` with `cache` sends, by branch, on `git`
async fn remote_statuses(git: &GitCommandExecutor, repo: &TestRepo, cache: &ObjectCache) -> BTreeMap<String, RemoteStatusUpdate> {
  sync_events(git, repo, cache)
    .await
    .into_iter()
    .filter_map(|event| match event {
      SyncEvent::RemoteStatusUpdate(status) => Some((status.branch_name.clone(), status)),
      _ => None,
    })
    .collect()
}

/// `bd sync` is a new process at every call: the statuses of commits that did not move come from the object cache, and
/// the push times of all branches from one `reflog`. The first such sync walks `beta`, whose remote branch is not at
/// its tip (2 `rev-list`); `alpha`'s remote branch is at its tip and needs no walk
#[tokio::test]
async fn an_unchanged_sync_of_pushed_branches_in_a_new_process() {
  let (_dir, cache) = empty_cache();
  let repo = pushed_repo(&cache).await;
  let with = |extra: &[(&'static str, u32)]| {
    let mut expected: Vec<(&str, u32)> = UNCHANGED_SYNC.iter().copied().chain(extra.iter().copied()).collect();
    expected.sort_unstable();
    expected
  };

  let git = counting();
  let first = remote_statuses(&git, &repo, &cache).await;
  assert_processes(&git, &with(&[("reflog", 1), ("rev-list", 2)]));
  assert_eq!(first["alpha"].last_push_time, PUSH_TIME);
  assert_eq!(first["beta"].last_push_time, 0, "only a fetch moved it");
  assert_eq!(first["beta"].unpushed_commits.len(), 1);

  forget_statuses();
  let git = counting();
  let second = remote_statuses(&git, &repo, &cache).await;
  assert_processes(&git, &with(&[("reflog", 1)]));
  assert_eq!(second, first);
}

/// A stacked branch whose parent got a new commit after both were pushed, its status as a push outside the sync computes
/// it: one `for-each-ref` of the tips, the walk of the branch with its ancestors' commits, one `rev-list` of the
/// ancestors' identities that tells them apart, the walk of the remote side and the push time
#[tokio::test]
async fn the_status_of_a_child_whose_parent_got_a_new_commit_outside_the_sync() {
  let repo = repo_on_origin();
  repo.git(&["config", "branchdeck.stacks", "true"]);
  repo.create_commit("(alpha) Alpha 1", "alpha-1.txt", "alpha\n");
  repo.create_commit("(alpha/beta) Beta 1", "beta-1.txt", "beta\n");
  sync(&GitCommandExecutor::new(), &repo).await;
  for name in ["alpha", "beta"] {
    let tip = repo.rev_parse(&format!("test/virtual/{name}")).unwrap();
    repo.git(&["update-ref", &format!("refs/remotes/origin/test/virtual/{name}"), &tip]);
  }
  repo.create_commit("(alpha) Alpha 2", "alpha-2.txt", "alpha\n");
  sync(&GitCommandExecutor::new(), &repo).await;
  forget_statuses();

  let git = counting();
  let status = compute_remote_status_for_branch(
    &git,
    &path(&repo),
    "origin",
    "test/virtual/beta",
    "beta",
    None,
    "origin/master",
    Some(StackParent {
      base: "test/virtual/alpha",
      ancestors: AncestorCommits::ReadWhenNeeded,
    }),
  )
  .unwrap();
  assert_eq!((status.unpushed_commits.len(), status.ancestor_commits_ahead), (0, 1));
  assert_processes(&git, &[("for-each-ref", 1), ("reflog", 1), ("rev-list", 3)]);
}

/// `repo` with a branch whose clean first commit is copied with a merge (its parent tree is not the base's) before its
/// second commit conflicts: `delta` conflicts at every sync onto the copy of its first commit
pub(crate) fn repo_with_a_conflict_after_a_clean_commit() -> TestRepo {
  let (repo, _) = alpha_and_beta_on_origin();
  repo.create_commit("(gamma) Add file", "file.txt", "1\n");
  repo.create_commit("(delta) Add notes", "notes.txt", "notes\n");
  repo.create_commit("(delta) Change file", "file.txt", "2\n");
  repo
}

/// The events of a sync of `repo` with `cache` on `git` that say what became of each commit and branch, sorted: the
/// branches finish in parallel
async fn commit_events(git: &GitCommandExecutor, repo: &TestRepo, cache: &ObjectCache) -> Vec<String> {
  let mut events: Vec<String> = sync_events(git, repo, cache)
    .await
    .into_iter()
    .filter(|event| {
      matches!(
        event,
        SyncEvent::CommitSynced { .. } | SyncEvent::CommitError { .. } | SyncEvent::CommitsBlocked { .. } | SyncEvent::BranchStatusUpdate { .. }
      )
    })
    .map(|event| format!("{event:?}"))
    .collect();
  events.sort_unstable();
  events
}

/// The copies of a conflicting branch before its conflict are remembered with the object cache and kept by
/// `refs/branch-deck/conflict/`, so an unchanged sync neither merges nor writes them again (its first sync writes that
/// ref, one `update-ref` more), and says the same about every commit
#[tokio::test]
async fn an_unchanged_sync_of_a_conflict_after_a_clean_commit() {
  let repo = repo_with_a_conflict_after_a_clean_commit();
  let (_dir, cache) = empty_cache();
  let git = counting();
  commit_events(&git, &repo, &cache).await;
  // the conflict analysed, and one `update-ref` for the conflict ref beside one per built branch
  assert_processes(
    &git,
    &[
      ("cat-file", 2),
      ("config", 1),
      ("fast-import", 5),
      ("for-each-ref", 1),
      ("log", 4),
      ("merge-base", 1),
      ("merge-tree", 5),
      ("notes", 1),
      ("remote", 1),
      ("rev-list", 4),
      ("rev-parse", 1),
      ("show-ref", 1),
      ("symbolic-ref", 1),
      ("update-ref", 4),
      ("var", 1),
    ],
  );

  let git = counting();
  let second = commit_events(&git, &repo, &cache).await;
  assert_processes(&git, &UNCHANGED_SYNC);
  let git = counting();
  assert_eq!(commit_events(&git, &repo, &cache).await, second);
  assert_processes(&git, &UNCHANGED_SYNC);
  assert!(second.iter().any(|event| event.starts_with("CommitSynced") && event.contains("delta")), "{second:#?}");

  // a commit added after the conflict is blocked by it, and the copies before it are still taken
  repo.create_commit("(delta) Add more", "more.txt", "more\n");
  let git = counting();
  commit_events(&git, &repo, &cache).await;
  assert_processes(&git, &UNCHANGED_SYNC);
}

/// The commit a conflict is applied onto, the copy of an earlier commit of the branch, survives `git gc`, also after a
/// sync without the object cache: `refs/branch-deck/conflict/` keeps that copy while the branch conflicts, so the view
/// can still merge onto it (`get_file_merges`)
#[tokio::test]
async fn the_target_of_a_conflict_survives_gc() {
  let repo = repo_with_a_conflict_after_a_clean_commit();
  let target = conflict_target(sync_events(&GitCommandExecutor::new(), &repo, &ObjectCache::disabled()).await);
  // the target is the copy of delta's first commit, on no branch
  assert_ne!(target, repo.rev_parse("test/virtual/delta").unwrap_or_default());
  repo.git(&["gc", "--prune=now", "--quiet"]);
  repo.git(&["cat-file", "-e", &format!("{target}^{{commit}}")]);
}

/// A change of the working tree amended into the oldest commit of alpha
#[test]
fn an_amend() {
  let (repo, commits) = alpha_and_beta_on_origin();
  std::fs::write(repo.path().join("alpha-1.txt"), "amended\n").unwrap();
  let git = counting();
  let result = amend_uncommitted_to_branch_core(
    &git,
    AmendUncommittedToBranchParams {
      repository_path: path(&repo),
      branch_name: "alpha".to_string(),
      original_commit_id: commits[0].clone(),
      files: vec!["alpha-1.txt".to_string()],
    },
    &RewriteOptions::default(),
  )
  .unwrap();
  assert!(matches!(result, AmendCommandResult::Ok(_)), "{result:?}");
  assert_processes(
    &git,
    &[
      ("cat-file", 1),
      ("fast-import", 1),
      ("log", 1),
      ("merge-tree", 1),
      ("read-tree", 1),
      ("replay", 1),
      ("rev-parse", 3), // the directories, read once for the lock and the guards: 1
      ("symbolic-ref", 1),
      ("update-index", 3),
      ("update-ref", 1),
      ("var", 1),
      ("write-tree", 1),
    ],
  );
}

/// The unapply of beta, whose commits are the second, fourth and sixth of `commits`
fn unapply_beta(git: &GitCommandExecutor, repo: &TestRepo, commits: &[String]) -> UnapplyBranchResult {
  unapply_branch_core(
    git,
    UnapplyBranchParams {
      repository_path: path(repo),
      branch_name: "beta".to_string(),
      original_commit_ids: Some(vec![commits[1].clone(), commits[3].clone(), commits[5].clone()]),
    },
    PREFIX,
    &RewriteOptions::default(),
  )
  .unwrap()
}

/// The commits of beta dropped from the checked out branch with their files, its virtual branch kept as an unapplied one:
/// the lock and the mark of pending files, the grouping inputs and the commits ahead, the build of the virtual branch
/// (its tip, its stack note, its own commits), the drop (`git replay` per kept run), the dry run of the files, the free
/// name, and the move with its mark (`update-ref`, `read-tree`)
#[tokio::test]
async fn a_drop() {
  let (repo, commits) = alpha_and_beta_on_origin();
  sync(&GitCommandExecutor::new(), &repo).await;
  let git = counting();
  unapply_beta(&git, &repo, &commits);
  assert_processes(
    &git,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("diff-tree", 1),
      ("for-each-ref", 3),
      ("log", 3),
      ("merge-base", 1),
      ("read-tree", 2),
      ("remote", 1),
      ("replay", 2),
      ("rev-list", 1),
      ("rev-parse", 5), // the directories, read once for the lock, the guards and the index: 1
      ("symbolic-ref", 4),
      ("update-index", 3),
      ("update-ref", 2),
    ],
  );
}

/// The undo of an unapply or of a reapply: the directories, read once for the lock, the guards and the marks
/// (`rev-parse`: 1), the mark of pending files, the guards and the last two moves of the branch in its reflog, the dry
/// run of the files, and the move back with its mark
const UNDO_OF_A_MOVE: &[(&str, u32)] = &[
  ("diff-tree", 1),
  ("log", 1),
  ("read-tree", 2),
  ("rev-parse", 7),
  ("symbolic-ref", 3),
  ("update-index", 3),
  ("update-ref", 2),
];

/// The undo of an unapply: the lock and the mark of pending files, the guards and the last two moves of the branch in
/// its reflog, the dry run of the files, and the move back with its mark, the virtual branch created again in it
#[tokio::test]
async fn an_undo_of_an_unapply() {
  let (repo, commits) = alpha_and_beta_on_origin();
  sync(&GitCommandExecutor::new(), &repo).await;
  let unapplied = unapply_beta(&GitCommandExecutor::new(), &repo, &commits);
  let git = counting();
  let token = UnapplyUndoToken::parse(&unapplied.undo_token).unwrap();
  undo_unapply_core(&git, &path(&repo), &token, LockWait::default()).unwrap();
  assert_processes(&git, UNDO_OF_A_MOVE);
}

/// The reapply of beta after its unapply: the lock and the mark of pending files, the unapplied branch with its worktree,
/// the baseline, the start of its own commits, the grouping inputs, the restored messages (one `fast-import`), the pick
/// (the range with the patch id marks, one `replay`, the copies, the dry run of the files) and the move with its mark
#[tokio::test]
async fn a_reapply() {
  let (repo, commits) = alpha_and_beta_on_origin();
  sync(&GitCommandExecutor::new(), &repo).await;
  unapply_beta(&GitCommandExecutor::new(), &repo, &commits);
  let git = counting();
  reapply_beta(&git, &repo);
  assert_processes(
    &git,
    &[
      ("cat-file", 2),
      ("config", 1),
      ("diff-tree", 1),
      ("fast-import", 1),
      ("for-each-ref", 2),
      ("log", 4),
      ("merge-base", 1),
      ("read-tree", 2),
      ("remote", 1),
      ("replay", 1),
      ("rev-list", 2),
      ("rev-parse", 5), // the directories, read once for the lock, the guards and the index: 1
      ("symbolic-ref", 4),
      ("update-index", 3),
      ("update-ref", 2),
    ],
  );
}

/// The undo of a reapply: as the undo of an unapply, the unapplied branch created again in the move back
#[tokio::test]
async fn an_undo_of_a_reapply() {
  let (repo, commits) = alpha_and_beta_on_origin();
  sync(&GitCommandExecutor::new(), &repo).await;
  unapply_beta(&GitCommandExecutor::new(), &repo, &commits);
  let reapplied = reapply_beta(&GitCommandExecutor::new(), &repo);
  let git = counting();
  let token = ReapplyUndoToken::parse(reapplied.undo_token.as_deref().unwrap()).unwrap();
  undo_reapply_core(&git, &path(&repo), &token, LockWait::default()).unwrap();
  assert_processes(&git, UNDO_OF_A_MOVE);
}

fn reapply_beta(git: &GitCommandExecutor, repo: &TestRepo) -> ReapplyBranchResult {
  let params = ReapplyBranchParams {
    repository_path: path(repo),
    branch_name: "beta".to_string(),
    dry_run: false,
  };
  reapply_branch_core(git, &params, PREFIX, &RewriteOptions::default()).unwrap()
}

/// Two commits of alpha squashed into the older one
#[test]
fn a_squash() {
  let (repo, commits) = alpha_and_beta_on_origin();
  let git = counting();
  squash_commits_core(
    &git,
    &SquashCommitsParams {
      repository_path: path(&repo),
      commit_ids: vec![commits[0].clone(), commits[2].clone()],
      message: "(alpha) Alpha 1 and 2".to_string(),
    },
    &RewriteOptions::default(),
  )
  .unwrap();
  assert_processes(
    &git,
    &[
      ("cat-file", 1),
      ("fast-import", 1),
      ("log", 1),
      ("merge-tree", 1),
      ("replay", 2),
      ("rev-list", 1),
      ("rev-parse", 3), // the directories, read once for the lock and the guards: 1
      ("symbolic-ref", 1),
      ("update-ref", 1),
      ("var", 1),
    ],
  );
}

/// The app's edit of the message of the oldest commit (the Tauri command `reword`): the regroup's snapshot under the
/// repository lock, then one `fast-import` of the recreated commits and one ref transaction
#[test]
fn the_app_reword() {
  let (repo, commits) = alpha_and_beta_on_origin();
  let git = counting();
  let request = RewordRequest {
    repository_path: path(&repo),
    rewords: vec![CommitReword {
      commit: commits[0].clone(),
      text: RewordText::Message("(alpha) Alpha 1\n\nWith a body".to_string()),
    }],
  };
  let result = reword_request_core(&git, &request, &RegroupOptions::default()).unwrap();
  assert!(result.undo_token.is_some());
  // jscpd:ignore-start: the same core as `bd reword` (crates/bd-cli/src/process_budget_test.rs, `a_reword`), so the same
  // processes: a change of one count shows in both
  assert_processes(
    &git,
    &[
      ("cat-file", 2),
      ("config", 1),
      ("fast-import", 1),
      ("for-each-ref", 2),
      ("log", 2),
      ("remote", 2),
      ("rev-parse", 7),
      ("symbolic-ref", 1),
      ("update-ref", 1),
    ],
  );
  // jscpd:ignore-end
}

/// A park of alpha and beta, then their unpark: no rewrite, one store write each, whatever the number of branches
#[test]
fn a_park_and_an_unpark() {
  let (repo, _) = alpha_and_beta_on_origin();
  let git = counting();
  park_core(
    &git,
    &path(&repo),
    &[ParkLine { branch: "alpha", note: "later" }, ParkLine { branch: "beta", note: "" }],
    &RewriteOptions::default(),
  )
  .unwrap();
  // the grouping of the commits ahead (`config`, `remote`, `for-each-ref`, `log`, one `cat-file` for the stores), then
  // the store write
  assert_processes(
    &git,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("fast-import", 1),
      ("for-each-ref", 1),
      ("log", 1),
      ("remote", 1),
      ("rev-parse", 2), // the directories, read once for the lock: 1
      ("update-ref", 1),
      ("var", 1),
    ],
  );
  let git = counting();
  unpark_core(&git, &path(&repo), &["alpha", "beta"], &RewriteOptions::default()).unwrap();
  assert_processes(&git, &[("cat-file", 1), ("fast-import", 1), ("rev-parse", 1), ("update-ref", 1), ("var", 1)]);
}

/// A `fixup!` of alpha and a `squash!` of beta folded into their targets
#[test]
fn a_squash_of_fixups() {
  let (repo, _) = alpha_and_beta_on_origin();
  repo.create_commit("fixup! (alpha) Alpha 1", "alpha-1.txt", "alpha fixed\n");
  let head = repo.create_commit("squash! (beta) Beta 2\n\nAlso this", "beta-2.txt", "beta squashed\n");
  let git = counting();
  squash_fixup_commits_core(
    &git,
    &SquashFixupsParams {
      repository_path: path(&repo),
      expected_head: head,
      scope: AutosquashScope::All,
    },
    &RewriteOptions::default(),
  )
  .unwrap();
  assert_processes(
    &git,
    &[
      ("cat-file", 3),
      ("config", 1),
      ("fast-import", 2),
      ("for-each-ref", 1),
      ("log", 2),
      ("merge-tree", 1),
      ("remote", 1),
      ("replay", 2),
      ("rev-list", 1),
      ("rev-parse", 6), // the directories, read once for the lock and the guards: 1
      ("symbolic-ref", 1),
      ("update-ref", 1),
      ("var", 1),
    ],
  );
}

/// A clone with a local commit whose upstream got another one on another file: a clean rebase
fn fetched_update() -> (TestRepo, TestRepo) {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", "one\ntwo\nthree\n"), ("other.txt", "other\n")]);
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  (upstream, clone)
}

/// The check of Update Project against the fetched upstream, as the app shows it before an update
#[test]
fn an_update_check() {
  let (_upstream, clone) = fetched_update();
  clone.git(&["fetch", "-q", "origin"]);
  let git = counting();
  check_baseline_update(&git, &path(&clone), "origin/master").unwrap().unwrap();
  assert_processes(
    &git,
    &[
      ("cat-file", 2),
      ("config", 1),
      ("diff-tree", 1),
      ("for-each-ref", 1),
      ("hash-object", 1),
      ("log", 3),
      ("merge-base", 2),
      ("remote", 1),
      ("replay", 1),
      ("rev-list", 1),
      ("rev-parse", 4),
      ("symbolic-ref", 2),
    ],
  );
}

/// The app's push of a stack (Push Stack, Push with Children): `alpha` and `beta` built on it, to a bare `origin` that
/// has neither, in one atomic push. The tips are one `for-each-ref`, the push size limit one `config`, the files the
/// push adds one `log` and one `cat-file` (the size check), the push one `push`, the remote statuses after it one
/// `for-each-ref` of the local and the remote tips of both and one `reflog` of their push times: no walk, each remote
/// branch is at its tip
#[test]
fn an_atomic_push_of_a_stack() {
  let repo = stack_ahead_of_bare_origin();
  let branch = |name: &str, parent: Option<&str>| StackPushBranch {
    branch_name: name.to_string(),
    parent_branch: parent.map(str::to_string),
    ..StackPushBranch::default()
  };
  let params = PushBranchesParams {
    repository_path: path(&repo),
    my_email: None,
    baseline_branch: "origin/master".to_string(),
    branches: vec![branch("alpha", None), branch("beta", Some("alpha"))],
  };

  let git = counting();
  let outcome = push_branches(&git, PREFIX, &params, &mut |_| {}).unwrap();
  assert!(
    matches!(outcome, PushBranchesOutcome::Pushed { ref remote_statuses, .. } if remote_statuses.len() == 2),
    "{outcome:?}"
  );
  assert_processes(&git, &[("cat-file", 1), ("config", 1), ("for-each-ref", 2), ("log", 1), ("push", 1), ("reflog", 1)]);
}

/// The fetch of a remote branch whose pull request shows a head someone else pushed (`moved_remote`): the tips one
/// `for-each-ref`, which heads are here one `cat-file`, the fetch one `fetch`, then the status against the fetched tip:
/// one `for-each-ref` of the tips, the two walks and, as a commit is left dropped, the walk of the baseline by its
/// author that finds the landed ones (three `rev-list`), and the push time (`reflog`)
#[test]
fn a_fetch_of_a_moved_remote_branch() {
  let repo = stack_ahead_of_bare_origin();
  repo.git(&["push", "-q", "origin", "test/virtual/alpha"]);
  let origin = repo.path().join(".git/origin.git");
  let other = TestRepo::new_empty();
  other.clone_from(&origin).unwrap();
  other.checkout("test/virtual/alpha").unwrap();
  let theirs = other.create_commit("Their fix", "fix.txt", "fix\n");
  other.push("origin", "test/virtual/alpha").unwrap();
  let branch = MovedBranch {
    full_name: "test/virtual/alpha",
    branch_name: "alpha",
    parent: None,
    head: &theirs,
    my_email: Some("test@example.com"),
  };

  let git = counting();
  let fetched = fetch_moved_remotes(&git, &path(&repo), "origin", "master", &[branch]).unwrap();
  assert!(fetched[0].settled);
  assert_processes(&git, &[("cat-file", 1), ("fetch", 1), ("for-each-ref", 2), ("reflog", 1), ("rev-list", 3)]);
}

/// The question about the robot of `beta`, which ran on `source` and may land `alpha` and `beta`
fn beta_robot(source: String) -> RobotLandingRequest {
  RobotLandingRequest {
    robot_id: "beta-robot".to_string(),
    branch_name: Some("beta".to_string()),
    stack_root: None,
    source_revision: source,
    branches: vec!["alpha".to_string(), "beta".to_string()],
    tip: None,
  }
}

/// What a merged robot of `beta` (built on `alpha`) landed after a new commit of `alpha` rebuilt `beta`: the commits of
/// both branches from HEAD as the sync groups them (the stack setting, one `cat-file --batch` of the stores, one
/// `log`), one `cat-file --batch-check` of the robot's commit and the ref, the two walks against the robot's commit,
/// and one `diff-tree` with one `patch-id` of the copy whose identity the robot had
#[tokio::test]
async fn the_landing_of_a_merged_robot_after_a_change_below_its_commit() {
  let repo = repo_on_origin();
  repo.git(&["config", "branchdeck.stacks", "true"]);
  let lines = |changed: &[(usize, &str)]| -> String {
    (1..=10)
      .map(|number| {
        changed
          .iter()
          .find(|(line, _)| *line == number)
          .map_or_else(|| format!("line {number}\n"), |(_, text)| format!("{text}\n"))
      })
      .collect()
  };
  repo.create_commit("(alpha) Alpha", "shared.txt", &lines(&[]));
  repo.create_commit("(alpha/beta) Beta", "shared.txt", &lines(&[(5, "beta")]));
  sync(&GitCommandExecutor::new(), &repo).await;
  let source = repo.git(&["rev-parse", "refs/heads/test/virtual/beta"]);
  // a change next to beta's moves the context of its copy: only the changes without context tell it is the same
  repo.create_commit("(alpha) Alpha again", "shared.txt", &lines(&[(3, "alpha"), (5, "beta")]));
  sync(&GitCommandExecutor::new(), &repo).await;

  let git = counting();
  let request = beta_robot(source);
  let landings = robot_landings(&git, &path(&repo), PREFIX, "origin/master", &[request]).unwrap();
  let after: Vec<usize> = landings[0].branches.iter().map(|branch| branch.after.len()).collect();
  assert_eq!(after, [1, 0]);
  assert_processes(&git, &[("cat-file", 2), ("config", 1), ("diff-tree", 1), ("log", 1), ("patch-id", 1), ("rev-list", 2)]);
}

/// What a running robot of `beta` lands after a restack placed a new root `alpha` below it: as the landing of a merged
/// robot, the commits of both branches from HEAD (the stack setting, one `cat-file --batch` of the stores, one `log`),
/// one `cat-file --batch-check` of the robot's commit and the ref and the two walks against it; the copy of beta on
/// alpha is patch-equivalent (`--cherry-mark`), so no `same_changes` runs
#[tokio::test]
async fn the_span_of_a_running_robot_after_a_restack() {
  let repo = repo_on_origin();
  repo.git(&["config", "branchdeck.stacks", "true"]);
  repo.create_commit_with_timestamp("(beta) Beta", "beta.txt", "beta", Some(1_700_000_100));
  sync(&GitCommandExecutor::new(), &repo).await;
  let source = repo.git(&["rev-parse", "refs/heads/test/virtual/beta"]);
  repo.git(&["reset", "--hard", "origin/master"]);
  repo.create_commit_with_timestamp("(alpha) Alpha", "alpha.txt", "alpha", Some(1_700_000_200));
  repo.create_commit_with_timestamp("(alpha/beta) Beta", "beta.txt", "beta", Some(1_700_000_100));
  sync(&GitCommandExecutor::new(), &repo).await;

  let git = counting();
  let request = beta_robot(source);
  let spans = robot_spans(&git, &path(&repo), PREFIX, "origin/master", &[request]).unwrap();
  let branches: Vec<&str> = spans[0].branches.iter().flatten().map(|branch| branch.branch.as_str()).collect();
  assert_eq!(branches, ["beta"]);
  assert_processes(&git, &[("cat-file", 2), ("config", 1), ("log", 1), ("rev-list", 2)]);
}

/// The target conflict check of a Safe Push start without its fetch: the tips of the baseline and the branch
/// (`rev-parse`) and one `replay` when the branch applies. On a conflict, the commits of the range (`log`), the binary
/// search over them (no more `replay` for one commit), the files of its merge (`merge-tree`), the base of the branch
/// (`merge-base`) and the commits of the target that change the files (`log`)
#[tokio::test]
async fn a_target_conflict_check() {
  let repo = repo_on_origin();
  let initial = repo.head();
  repo.create_commit("(alpha) Alpha", "README.md", "# Alpha\n");
  sync(&GitCommandExecutor::new(), &repo).await;
  let params = TargetConflictParams {
    repository_path: path(&repo),
    baseline_branch: "origin/master".to_string(),
    branch_name: "alpha".to_string(),
    stack: false,
  };
  let move_target = |file: &str| {
    repo.git(&["checkout", "--quiet", "--detach", &initial]);
    let target = repo.create_commit("Target change", file, "# Target\n");
    repo.git(&["update-ref", "refs/remotes/origin/master", &target]);
    repo.git(&["checkout", "--quiet", "master"]);
  };

  move_target("other.md");
  let git = counting();
  assert_eq!(check_branch_target(&git, PREFIX, &params, None).outcome, TargetOutcome::Clean);
  assert_processes(&git, &[("replay", 1), ("rev-parse", 1)]);

  move_target("README.md");
  let git = counting();
  assert!(matches!(check_branch_target(&git, PREFIX, &params, None).outcome, TargetOutcome::Conflict(_)));
  assert_processes(&git, &[("log", 2), ("merge-base", 1), ("merge-tree", 1), ("replay", 1), ("rev-parse", 1)]);
}
