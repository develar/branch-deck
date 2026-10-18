//! The git processes of each operation the app runs, by subcommand: the numbers behind the batching rule
//! (docs/design/git-operations.md, "Batch"). A change that runs fewer processes lowers the numbers here, so the diff of an
//! assert shows which process went away; one that runs more has to say why. One test per operation, on a small fixed
//! repository, so the counts are the same on every run.

use crate::amend_to_branch::{AmendCommandResult, AmendUncommittedToBranchParams, amend_uncommitted_to_branch_core};
use crate::autosquash::{AutosquashScope, SquashFixupsParams, squash_fixup_commits_core};
use crate::check::{CheckSession, check_plan};
use crate::conflict_contents::{GetConflictContentsParams, read_conflict_contents};
use crate::pull_request_create::read_new_pull_request;
use crate::remote_status::forget_statuses;
use crate::repository_lock::RewriteOptions;
use crate::squash::{SquashCommitsParams, squash_commits_core};
use crate::sync::{SyncOptions, sync_branches};
use crate::unapply_branch::{UnapplyBranchParams, unapply_branch_core};
use crate::update_project::{UpdateProjectParams, check_baseline_update, update_project_core};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::BranchError;
use object_cache::cache::{CacheConfig, ObjectCache};
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashMap};
use sync_test_utils::TestReporter;
use sync_types::{RemoteStatusUpdate, SyncEvent};
use test_utils::git_test_utils::TestRepo;

const PREFIX: &str = "test";

/// The processes `git` counted, by subcommand, sorted by name
fn processes(git: &GitCommandExecutor) -> Vec<(String, u32)> {
  git
    .process_stats()
    .expect("a counting executor")
    .by_command()
    .into_iter()
    .map(|(subcommand, stats)| (subcommand, stats.count))
    .collect()
}

/// `git` counted exactly `expected`: `(subcommand, processes)`, sorted by subcommand
#[track_caller]
fn assert_processes(git: &GitCommandExecutor, expected: &[(&str, u32)]) {
  let expected: Vec<(String, u32)> = expected.iter().map(|(subcommand, count)| ((*subcommand).to_string(), *count)).collect();
  assert_eq!(processes(git), expected);
}

fn counting() -> GitCommandExecutor {
  GitCommandExecutor::new().with_process_stats("test")
}

fn path(repo: &TestRepo) -> String {
  repo.path().to_str().unwrap().to_string()
}

/// `origin/master` at the initial commit, then three commits of `alpha` and three of `beta` in turns; returns them
fn repo() -> (TestRepo, Vec<String>) {
  let repo = TestRepo::new();
  let initial = repo.create_commit("Initial commit", "README.md", "# Test Project\n");
  repo.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/master", &initial]);
  let mut commits = Vec::new();
  for index in 1..=3 {
    commits.push(repo.create_commit(&format!("(alpha) Alpha {index}"), &format!("alpha-{index}.txt"), "alpha\n"));
    commits.push(repo.create_commit(&format!("(beta) Beta {index}"), &format!("beta-{index}.txt"), "beta\n"));
  }
  (repo, commits)
}

async fn sync(git: &GitCommandExecutor, repo: &TestRepo) {
  sync_branches(git, &path(repo), PREFIX, TestReporter::new(), SyncOptions::default()).await.unwrap();
}

/// An empty object cache in a temporary directory, which the cache needs while it is open
fn empty_cache() -> (tempfile::TempDir, ObjectCache) {
  let dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(dir.path(), &CacheConfig::default());
  (dir, cache)
}

/// `bd check` of the current grouping of `repo` on a counting executor, which it returns
fn counted_check(repo: &TestRepo, cache: &ObjectCache) -> GitCommandExecutor {
  let git = counting();
  check_plan(&git, &path(repo), PREFIX, None, cache).unwrap();
  git
}

#[tokio::test]
async fn a_first_sync() {
  let (repo, _) = repo();
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
      ("rev-parse", 2),
      ("show-ref", 1),
      ("symbolic-ref", 1),
      ("update-ref", 2),
      ("var", 1),
    ],
  );
}

#[tokio::test]
async fn an_unchanged_sync() {
  let (repo, _) = repo();
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
  ("rev-parse", 2),
  ("symbolic-ref", 1),
];

/// `repo` with a branch whose commit changes a file another branch adds: `delta` conflicts at every sync
fn repo_with_a_conflict() -> TestRepo {
  let (repo, _) = repo();
  repo.create_commit("(gamma) Add file", "file.txt", "1\n");
  repo.create_commit("(delta) Change file", "file.txt", "2\n");
  repo
}

/// A sync with the object cache, whose conflict views it remembers, on `git`
async fn sync_with_cache(git: &GitCommandExecutor, repo: &TestRepo, cache: &ObjectCache) {
  let options = SyncOptions {
    object_cache: cache.clone(),
    ..SyncOptions::default()
  };
  sync_branches(git, &path(repo), PREFIX, TestReporter::new(), options).await.unwrap();
}

/// The conflict of an unchanged branch costs its merge (in its own `merge-tree`) and the analysis (`rev-list`, two
/// `log`, `cat-file`, `diff-tree`) at every sync without the object cache, and none of them with it
#[tokio::test]
async fn an_unchanged_sync_of_a_conflicting_branch() {
  let repo = repo_with_a_conflict();
  sync(&GitCommandExecutor::new(), &repo).await;
  let git = counting();
  sync(&git, &repo).await;
  let mut analysed = vec![("cat-file", 2), ("diff-tree", 1), ("log", 6), ("merge-tree", 2), ("rev-list", 1)];
  analysed.extend(UNCHANGED_SYNC.iter().filter(|(command, _)| !["cat-file", "log"].contains(command)));
  analysed.sort_unstable();
  assert_processes(&git, &analysed);

  let (_dir, cache) = empty_cache();
  sync_with_cache(&GitCommandExecutor::new(), &repo, &cache).await;
  let git = counting();
  sync_with_cache(&git, &repo, &cache).await;
  assert_processes(&git, &UNCHANGED_SYNC);
}

/// The time of the push of `alpha` in `pushed_repo`
const PUSH_TIME: u32 = 1_700_000_000;

/// `repo` synced, `alpha` pushed at `PUSH_TIME` (its remote branch at its tip) and `beta` fetched one commit behind
/// its tip; the statuses of that sync are forgotten, as by a new process
async fn pushed_repo(cache: &ObjectCache) -> TestRepo {
  let (repo, _) = repo();
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
  let progress = TestReporter::new();
  let options = SyncOptions {
    object_cache: cache.clone(),
    ..SyncOptions::default()
  };
  sync_branches(git, &path(repo), PREFIX, progress.clone(), options).await.unwrap();
  progress
    .get_events()
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

/// The contents of a conflict, when a view shows it (`get_conflict_contents`): none from the object cache a sync left
/// them in; else the merge with the commits and the analysis, once, after which the cache has them
#[tokio::test]
async fn opening_a_conflict() {
  let repo = repo_with_a_conflict();
  let (_dir, cache) = empty_cache();
  let progress = TestReporter::new();
  let options = SyncOptions {
    object_cache: cache.clone(),
    ..SyncOptions::default()
  };
  sync_branches(&GitCommandExecutor::new(), &path(&repo), PREFIX, progress.clone(), options).await.unwrap();
  let conflict = progress
    .get_events()
    .into_iter()
    .find_map(|event| match event {
      SyncEvent::CommitError {
        error: BranchError::MergeConflict(info),
        ..
      } => Some(info),
      _ => None,
    })
    .expect("delta conflicts");
  let params = GetConflictContentsParams {
    repository_path: path(&repo),
    commit: conflict.commit.hash.clone(),
    parent: conflict.original_parent.hash.clone(),
    target: conflict.target_branch.hash.clone(),
  };

  let git = counting();
  read_conflict_contents(&git, &cache, &params).unwrap();
  assert_eq!(processes(&git), []);

  let (_dir, empty) = empty_cache();
  let analysed = [("cat-file", 1), ("diff-tree", 1), ("log", 2), ("merge-tree", 1), ("rev-list", 1)];
  let git = counting();
  read_conflict_contents(&git, &empty, &params).unwrap();
  assert_processes(&git, &analysed);
  read_conflict_contents(&git, &empty, &params).unwrap();
  assert_processes(&git, &analysed);
}

/// `bd check` without a plan before the first sync, with an empty object cache and then with the one it filled
#[test]
fn a_check() {
  let (repo, _) = repo();
  let (_dir, cache) = empty_cache();
  assert_processes(
    &counted_check(&repo, &cache),
    &[
      ("cat-file", 1),
      ("config", 2),
      ("fast-import", 2),
      ("for-each-ref", 2),
      ("log", 3),
      ("merge-tree", 2),
      ("remote", 1),
      ("rev-parse", 4),
    ],
  );

  assert_processes(
    &counted_check(&repo, &cache),
    &[
      ("cat-file", 2),
      ("config", 2),
      ("fast-import", 2),
      ("for-each-ref", 2),
      ("log", 2),
      ("remote", 1),
      ("rev-parse", 4),
    ],
  );
}

/// `bd check` after a sync, with the object cache that sync left empty: the copies of alpha and beta exist, and their
/// commits are read for reuse
#[tokio::test]
async fn a_check_of_synced_branches() {
  let (repo, _) = repo();
  sync(&GitCommandExecutor::new(), &repo).await;
  let (_dir, cache) = empty_cache();
  assert_processes(
    &counted_check(&repo, &cache),
    &[("cat-file", 1), ("config", 2), ("for-each-ref", 2), ("log", 4), ("remote", 1), ("rev-parse", 4)],
  );
}

/// The processes of `run` on `git`, which may have counted others before
fn processes_of(git: &GitCommandExecutor, run: impl FnOnce()) -> Vec<(String, u32)> {
  let before: HashMap<String, u32> = processes(git).into_iter().collect();
  run();
  processes(git)
    .into_iter()
    .map(|(command, count)| (command.clone(), count - before.get(&command).copied().unwrap_or(0)))
    .filter(|(_, count)| *count > 0)
    .collect()
}

/// The checks of one session after a sync (the conflict fix tries many plans): the first reads the commits of the
/// existing copies, the next reuses them and the merges the first remembered
#[tokio::test]
async fn the_second_check_of_a_session() {
  let (repo, _) = repo();
  sync(&GitCommandExecutor::new(), &repo).await;
  let (_dir, cache) = empty_cache();
  let git = counting();
  let session = CheckSession::open(&git, &path(&repo), PREFIX, &cache).unwrap();
  let first = processes_of(&git, || {
    session.check(None).unwrap();
  });
  assert_eq!(first, [("log".to_string(), 3), ("rev-parse".to_string(), 1)]);
  let second = processes_of(&git, || {
    session.check(None).unwrap();
  });
  assert_eq!(second, []);
  // a session of a rewrite (`bd check --fix` tries one) shares them, and the baseline commit
  let rewritten = session.with_rewrite(&[], &[]);
  let third = processes_of(&git, || {
    rewritten.check(None).unwrap();
  });
  assert_eq!(third, []);
}

/// A change of the working tree amended into the oldest commit of alpha
#[test]
fn an_amend() {
  let (repo, commits) = repo();
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
      ("rev-parse", 4), // the repository lock: 1
      ("symbolic-ref", 1),
      ("update-index", 3),
      ("update-ref", 1),
      ("var", 1),
      ("write-tree", 1),
    ],
  );
}

/// The commits of beta dropped from the checked out branch, its virtual branch kept as an unapplied one
#[tokio::test]
async fn a_drop() {
  let (repo, commits) = repo();
  sync(&GitCommandExecutor::new(), &repo).await;
  let git = counting();
  unapply_branch_core(
    &git,
    UnapplyBranchParams {
      repository_path: path(&repo),
      branch_name: format!("{PREFIX}/virtual/beta"),
      branch_prefix: PREFIX.to_string(),
      original_commit_ids: vec![commits[1].clone(), commits[3].clone(), commits[5].clone()],
    },
    &RewriteOptions::default(),
  )
  .unwrap();
  assert_processes(
    &git,
    &[
      ("for-each-ref", 1),
      ("log", 1),
      ("read-tree", 2),
      ("replay", 2),
      ("rev-parse", 6), // the repository lock: 1
      ("show-ref", 1),
      ("symbolic-ref", 2), // the checked out branch is not the one unapplied: 1
      ("update-index", 1),
      ("update-ref", 1),
    ],
  );
}

/// Two commits of alpha squashed into the older one
#[test]
fn a_squash() {
  let (repo, commits) = repo();
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
      ("rev-parse", 4), // the repository lock: 1
      ("symbolic-ref", 1),
      ("update-ref", 1),
      ("var", 1),
    ],
  );
}

/// A `fixup!` of alpha and a `squash!` of beta folded into their targets
#[test]
fn a_squash_of_fixups() {
  let (repo, _) = repo();
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
      ("rev-parse", 7), // the repository lock: 1
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

/// Update Project: the fetch, the check and the rebase of the local commit onto the upstream, the files following
#[test]
fn an_update() {
  let (_upstream, clone) = fetched_update();
  let git = counting();
  let result = update_project_core(&git, &UpdateProjectParams { repository_path: path(&clone) }, &|_| {}).unwrap();
  assert!(result.applied.is_some(), "{result:?}");
  assert_processes(
    &git,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("diff-tree", 1),
      ("fetch", 1),
      ("for-each-ref", 1),
      ("log", 3),
      ("merge-base", 2),
      ("read-tree", 2),
      ("remote", 2),
      ("replay", 1),
      ("rev-list", 3),
      ("rev-parse", 10),
      ("symbolic-ref", 5),
      ("update-index", 3),
      ("update-ref", 2),
    ],
  );
}

/// The text of the "Create pull request" dialog of a branch without a description: its commits against the baseline
#[test]
fn the_text_of_a_new_pull_request() {
  let (repo, _) = repo();
  repo.git(&["branch", "test/virtual/alpha"]);
  let git = counting();
  let text = read_new_pull_request(&git, &path(&repo), "origin", "test/virtual/alpha", "master").unwrap();
  assert_eq!(text.title, "alpha");
  assert_processes(&git, &[("config", 1), ("log", 1)]);
}
