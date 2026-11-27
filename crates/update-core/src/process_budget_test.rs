//! The git processes of the cores of this crate, by subcommand: the numbers behind the batching rule
//! (docs/design/git-operations.md, "Batch"), as `sync_core`'s `process_budget_test` pins those of its cores. A change
//! that runs fewer processes lowers the numbers here; one that runs more has to say why.

use crate::resolve_session::unapply_branch_suggestion;
use crate::update_project::update_project_test::{Setup, path, setup};
use crate::update_project::{BdUpdate, UpdateOptions, UpdateProjectParams, update_and_resolve, update_project_core};
use domain_types::update_project::{UpdateProjectResult, UpdateState};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::LockWait;
use sync_core::lease_state::LeaseGuard;
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{assert_processes, assert_signed_commits, counting, processes, with_added_processes};

/// The processes of Update Project after its fetch (`an_update`, `an_update_after_its_fetch`): one `update-index`, the
/// refresh of the user's index that the dry run of the local changes copies and the move takes as it is. The lost
/// bindings of the replay cost nothing: the upstream changes no file the local commit changes, which the `log` of the
/// local commits of the pairing names (`read_landed_with_paths`), so the rule is not asked
const UPDATE: &[(&str, u32)] = &[
  ("cat-file", 1),
  ("config", 2),
  ("diff-tree", 2),
  ("for-each-ref", 1),
  ("log", 3),
  ("merge-base", 2),
  ("read-tree", 2),
  ("remote", 1),
  ("replay", 1),
  ("rev-list", 3),
  ("rev-parse", 5),
  ("symbolic-ref", 4),
  ("update-index", 1),
  ("update-ref", 2),
];

/// `processes` and one `fetch`, sorted by subcommand
fn with_fetch(processes: &[(&'static str, u32)]) -> Vec<(&'static str, u32)> {
  let mut all = processes.to_vec();
  all.push(("fetch", 1));
  all.sort_unstable();
  all
}

/// Update Project of `repo` on `git`, with its fetch or without
fn app_update(git: &GitCommandExecutor, repo: &TestRepo, fetch: bool) -> UpdateProjectResult {
  update_project_core(
    git,
    &UpdateProjectParams {
      repository_path: path(repo).to_string(),
      fetch,
      lease: LeaseGuard::any(),
      keep: Vec::new(),
    },
    &|_| {},
    LockWait::default(),
  )
  .unwrap()
}

fn run_update(git: &GitCommandExecutor, repo: &TestRepo, fetch: bool) {
  let result = app_update(git, repo, fetch);
  assert!(result.applied.is_some(), "{result:?}");
}

/// Update Project: the fetch, the check and the rebase of the local commit onto the upstream, the files following. The
/// directories are read once at the start (one `rev-parse`): the guards, the lock, the pending moves and the index of
/// the dry run take them from there. Before its replay the check pairs the local commit with the upstream commits by
/// its author: no twin, and the sets of paths of both sides (`diff-tree --name-only`) differ, so no patch id. The check
/// runs before the lock; under it, its witness (the guards, one `rev-parse` of HEAD and the upstream) costs one
/// `rev-parse` and one `symbolic-ref` more. The record of what landed reads HEAD once more under the lock of the landed
/// state (`record_update`), as it runs after the repository lock: one `rev-parse`
#[test]
fn an_update() {
  let fixture = setup();
  fixture.clone.create_commit("(auth) Add login", "login.txt", "login\n");
  fixture.upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  let git = counting();
  run_update(&git, &fixture.clone, true);
  assert_processes(&git, &with_fetch(UPDATE));
}

/// The local commit and the upstream change of `an_update`, fetched
fn fetched_update() -> Setup {
  let fixture = setup();
  fixture.clone.create_commit("(auth) Add login", "login.txt", "login\n");
  fixture.upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  fixture.clone.git(&["fetch", "--quiet", "origin"]);
  fixture
}

/// Update Project of the app after its fetch (the `FetchBaseline` of its operation, `operations::update`): the same
/// update without the fetch, the network before it holds HEAD
#[test]
fn an_update_after_its_fetch() {
  let fixture = fetched_update();
  let git = counting();
  run_update(&git, &fixture.clone, false);
  // the processes of `an_update` without the fetch: the time of the last fetch is that of `FETCH_HEAD` in the
  // directories read at the start
  assert_processes(&git, UPDATE);
}

/// `brd update` of the rebase of `an_update_after_its_fetch` (`update_and_resolve`): the same update after the look for
/// a resolve session of the worktree (one `cat-file` of its ref, none), and after the move the generated files that the
/// local commit and the upstream both changed, for `verify.regenerate`: two `diff-tree`, the paths of each side since the
/// merge base, and none in common, so neither `check-attr` nor `cat-file` follows
#[test]
fn a_bd_update_reads_the_generated_files_both_sides_changed() {
  let fixture = fetched_update();
  let git = counting();
  let options = UpdateOptions {
    fetch: false,
    lock_wait: LockWait::default(),
    onto: None,
    lease: &LeaseGuard::any(),
    keep: &[],
  };
  let BdUpdate::Update { result, regenerate, .. } = update_and_resolve(&git, path(&fixture.clone), options, None).unwrap() else {
    panic!("a clean update started a resolve session");
  };
  assert!(result.applied.is_some() && regenerate.is_empty(), "{result:?} {regenerate:?}");
  assert_processes(&git, &with_added_processes(UPDATE, &[("cat-file", 1), ("diff-tree", 2)]));
}

#[cfg(any(unix, windows))]
#[test]
fn signing_an_update_adds_one_batch_read_and_write() {
  #[cfg(windows)]
  if sync_utils::child_process::windowless_command("ssh-keygen").arg("-?").output().is_err() {
    return;
  }

  let fixture = setup();
  fixture.clone.create_commit("Local change", "local.txt", "local\n");
  fixture.upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  fixture.clone.git(&["fetch", "--quiet", "origin"]);
  fixture.clone.enable_ssh_signing();
  let git = counting();

  run_update(&git, &fixture.clone, false);
  let tip = fixture.clone.head();
  assert_signed_commits(&fixture.clone, &fixture.clone.git(&["rev-parse", "refs/remotes/origin/master"]), &tip);
  let expected = with_added_processes(UPDATE, &[("cat-file", 1), ("rev-list", 1), ("fast-import", 1)]);
  assert_processes(&git, &expected);
}

/// Update Project of `upstream_and_clone_with_twins` after its fetch: the check pairs the local commits once. Before the
/// replay, the pairing reads the local commits (`log`) and the upstream commits by the local authors (`log`), compares
/// the patch ids of both pairs by identity (`diff-tree -p` with `patch-id --stable`, none equal), replays them
/// (`merge-tree --stdin`, one `rev-parse` of the upstream trees), and proves the one whose cherry-pick conflicts by its
/// changed lines (`diff-tree -p -U0` with `patch-id --verbatim`, `diff-tree --raw`). The commit left compares its set of
/// paths with the upstream commits no twin took (`diff-tree --name-only`); the upstream change of the same file has
/// another patch id (`diff-tree -p` with `patch-id --stable`). The replay without the twins conflicts, so the pairing
/// looks for the commits of the same patch by other authors (`rev-list --cherry-mark`, none here) and reads the changed
/// lines of the commit left and the upstream change of its file (`diff-tree -p -U0`), which sum to no candidate, and the
/// resolve by words stops at the real conflict (`var`, `merge-tree`, `check-attr`, `cat-file`): the first conflict,
/// which needs no search. It is described by its merge (`merge-tree`), the commit (`log`) and the upstream commits that
/// change its files (`log`); the outline of the whole branch is gone (one `log` less). A conflict moves nothing: no
/// lock, no witness
#[test]
fn a_conflicting_update_pairs_its_commits_once() {
  let (_upstream, clone, _, _) = TestRepo::upstream_and_clone_with_twins(false);
  clone.git(&["fetch", "--quiet", "origin"]);
  let git = counting();
  let result = app_update(&git, &clone, false);
  assert!(matches!(result.check.state, UpdateState::Conflict { .. }), "{result:?}");
  assert_processes(
    &git,
    &[
      ("cat-file", 2),
      ("check-attr", 1),
      ("config", 2),
      ("diff-tree", 6),
      ("for-each-ref", 1),
      ("log", 4),
      ("merge-base", 2),
      ("merge-tree", 3),
      ("patch-id", 3),
      ("remote", 1),
      ("replay", 1),
      ("rev-list", 2),
      ("rev-parse", 4),
      ("symbolic-ref", 2),
      ("var", 1),
    ],
  );
}

/// Update Project of branch-deck#265 after its fetch: a commit and its fixup that GitHub squashed under another
/// identity. The replay conflicts, so after the `rev-list --cherry-mark` of the same patch by any author the pairing
/// reads the changed lines of the two and of the squash once (`diff-tree -p -U0`): one candidate. The replay without it
/// is clean, and one `merge-tree` with one `rev-parse` tell that its change is already there; the move takes the
/// upstream
#[test]
fn a_conflicting_update_of_a_squashed_pull_request_reads_its_changed_lines_once() {
  let Setup { upstream, clone } = setup();
  upstream.create_commit("Add f", "f.txt", "a\nb\nc\n");
  clone.git(&["pull", "-q", "--ff-only"]);
  clone.create_commit_with_timestamp("(feature) feat: change b", "f.txt", "a\nB1\nc\n", Some(1_700_000_100));
  clone.create_commit_with_timestamp("(feature) fixup! feat: change b", "f.txt", "a\nB2\nc\n", Some(1_700_000_200));
  upstream.create_commit("feat: change b (#1)", "f.txt", "a\nB2\nc\n");
  clone.git(&["fetch", "--quiet", "origin"]);
  let git = counting();
  let result = app_update(&git, &clone, false);
  assert!(result.applied.is_some(), "{result:?}");
  assert_eq!(clone.head(), upstream.head());
  assert_processes(
    &git,
    &[
      ("cat-file", 1),
      ("config", 2),
      ("diff-tree", 4),
      ("for-each-ref", 1),
      ("log", 3),
      ("merge-base", 2),
      ("merge-tree", 1),
      ("patch-id", 1),
      ("remote", 1),
      ("replay", 1),
      ("rev-list", 4),
      ("rev-parse", 6),
      ("symbolic-ref", 4),
      ("update-ref", 1),
    ],
  );
}

/// Update Project of the incident (`TestRepo::upstream_and_clone_losing_bindings`) after its fetch: the check of
/// `an_update_after_its_fetch` up to its clean replay (the pairing compares the patch ids of the local commit and the
/// upstream one by the same author, which change the same file: one `diff-tree -p` with `patch-id`), whose `log` of
/// the copies and the local commits finds the file both changed; its summaries before and after the copy and the local
/// commit, and at the tip, from one `cat-file --batch` (no cache: the copy's blobs are merge results). The lost bindings
/// make it a conflict, described by the commit (`log --no-walk`) and the upstream commits that change its file (`log`).
/// A conflict moves nothing: no lock, no witness, no files
#[test]
fn an_update_whose_clean_replay_loses_bindings() {
  let (_upstream, clone) = TestRepo::upstream_and_clone_losing_bindings(false);
  clone.git(&["fetch", "--quiet", "origin"]);
  let git = counting();
  let result = app_update(&git, &clone, false);
  let UpdateState::Conflict { first_conflict } = &result.check.state else {
    panic!("not a conflict: {result:?}");
  };
  assert_eq!(first_conflict.lost_bindings.len(), 2);
  assert_processes(
    &git,
    &[
      ("cat-file", 2),
      ("config", 2),
      ("diff-tree", 3),
      ("for-each-ref", 1),
      ("log", 5),
      ("merge-base", 2),
      ("patch-id", 1),
      ("remote", 1),
      ("replay", 1),
      ("rev-list", 1),
      ("rev-parse", 3),
      ("symbolic-ref", 2),
    ],
  );
}

/// `brd update` of `repo` without a fetch on `git`: the round of the resolve session it starts
fn lost_bindings_round(git: &GitCommandExecutor, repo: &TestRepo) -> crate::resolve_session::SessionView {
  let options = UpdateOptions {
    fetch: false,
    lock_wait: LockWait::default(),
    onto: None,
    lease: &LeaseGuard::any(),
    keep: &[],
  };
  let BdUpdate::Session(outcome) = update_and_resolve(git, path(repo), options, None).unwrap() else {
    panic!("no session");
  };
  match *outcome {
    crate::resolve_session::SessionOutcome::Round(view) => view,
    other => panic!("no round: {other:?}"),
  }
}

/// `brd update` of the incident: the check of `an_update_whose_clean_replay_loses_bindings` after the look for a
/// resolve session of the worktree (`cat-file`), then the session starts at the lost-bindings round on the tip the check
/// replayed, with no replay of its own: after the sweep of stale temporary refs (`for-each-ref`) and the committer
/// (`var`), the ids and modes of the round's file at the tip, before and in the copy (`cat-file --batch-check`) and its
/// objects (`cat-file --batch`); under the lock the witness (`rev-parse`, `symbolic-ref`), the state commit and the
/// session ref (`fast-import`, `update-ref`) and the patch of the copies (`log`). The view after the lock reads the local
/// changes that will block the finish (two `diff-tree`, the sizes and the hashes of the files) and the upstream commits
/// of the file (`log`)
#[test]
fn a_bd_update_whose_clean_replay_loses_bindings_opens_its_round() {
  let (_upstream, clone) = TestRepo::upstream_and_clone_losing_bindings(false);
  clone.git(&["fetch", "--quiet", "origin"]);
  let git = counting();
  lost_bindings_round(&git, &clone);
  assert_processes(
    &git,
    &[
      ("cat-file", 6),
      ("config", 2),
      ("diff-tree", 5),
      ("fast-import", 1),
      ("for-each-ref", 2),
      ("hash-object", 1),
      ("log", 7),
      ("merge-base", 2),
      ("patch-id", 1),
      ("remote", 1),
      ("replay", 1),
      ("rev-list", 1),
      ("rev-parse", 4),
      ("symbolic-ref", 3),
      ("update-ref", 1),
      ("var", 1),
    ],
  );
}

/// `brd update --continue` of the lost-bindings round of `a_bd_update_whose_clean_replay_loses_bindings_opens_its_round`
/// with an edit that binds both names again: the session (`cat-file`), the config once, the objects of the round
/// (`cat-file --batch`), whose edit the rule reads in process; the edit as one `fixup!` commit on the tip
/// (`fast-import`); the replay is done, so before the finish the rule reads the copies and the local commits once more
/// (`log`): the tip binds the names, nothing is left, and the session finishes as every finish does
#[test]
fn the_fixup_of_a_lost_bindings_round_finishes_the_update() {
  let (_upstream, clone) = TestRepo::upstream_and_clone_losing_bindings(false);
  clone.git(&["fetch", "--quiet", "origin"]);
  let view = lost_bindings_round(clone.git_executor(), &clone);
  let edit = &view.round.as_ref().unwrap().files[0].edit;
  let content = std::fs::read_to_string(edit).unwrap();
  let fixed = content
    .replace("import b.keep\n", "import b.keep\nimport b.rethrow\n")
    .replace("  private val name: String,\n", "  private val name: String,\n  private val backend: () -> Int,\n");
  std::fs::write(edit, fixed).unwrap();

  let git = counting();
  let continued = crate::resolve_session::continue_resolve(
    &git,
    path(&clone),
    &crate::resolve_session::ContinueOptions {
      lease: LeaseGuard::any(),
      ..crate::resolve_session::ContinueOptions::default()
    },
    std::time::Duration::from_secs(10),
  )
  .unwrap();
  assert!(matches!(continued, crate::resolve_session::SessionOutcome::Applied { .. }), "{continued:?}");
  assert_eq!(clone.git(&["log", "-1", "--format=%s"]), "fixup! (launch) Drop the old path");
  assert_processes(
    &git,
    &[
      ("cat-file", 5),
      ("check-attr", 1),
      ("config", 2),
      ("diff-tree", 3),
      ("fast-import", 2),
      ("log", 4),
      ("read-tree", 2),
      ("rev-list", 1),
      ("rev-parse", 3),
      ("symbolic-ref", 2),
      ("update-index", 2),
      ("update-ref", 3),
    ],
  );
}

/// The check of a sync (and of `brd update --dry-run`) with the lost-binding rule (`preview_update`) of the clean rebase
/// of `fetched_update`: the processes of the same check without the rule (`an_update_check` of sync-core), none more,
/// as the upstream changes no file the local commit changes (the `log` of the local commits of its pairing names their
/// paths)
#[test]
fn the_check_of_a_sync_whose_upstream_changes_no_local_file_asks_no_rule() {
  let fixture = fetched_update();
  let without = counting();
  sync_core::baseline::check_baseline_update(&without, path(&fixture.clone), "origin/master")
    .unwrap()
    .unwrap();
  // a check of another state: the preview cache keeps one per repository
  let other = fetched_update();
  let with = counting();
  crate::update_project::preview_update(&with, path(&other.clone), "origin/master").unwrap().unwrap();
  assert_eq!(processes(&with), processes(&without));
}

/// The check of a sync of the incident: the replay of the preview, then the rule (one `log` of the copies and the local
/// commits, one `cat-file` of the summaries of the file both change) and the conflict it describes (`log --no-walk` of
/// the commit, `log` of the upstream commits of its file); a second check of the same state takes the replay and the
/// lost bindings from the preview cache: only the reads of the state and the description again. The local file is older
/// than the racy margin, so the hash of the first check answers for it in the second however long the first takes
#[test]
fn the_check_of_a_sync_reads_the_lost_bindings_once_per_state() {
  let (_upstream, clone) = TestRepo::upstream_and_clone_losing_bindings(false);
  clone.git(&["fetch", "--quiet", "origin"]);
  let local_file = std::fs::File::options().write(true).open(clone.path().join("src/Launch.kt")).unwrap();
  local_file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(5)).unwrap();
  let git = counting();
  crate::update_project::preview_update(&git, path(&clone), "origin/master").unwrap().unwrap();
  assert_processes(
    &git,
    &[
      ("cat-file", 3),
      ("config", 1),
      ("diff-tree", 3),
      ("for-each-ref", 1),
      ("hash-object", 1),
      ("log", 5),
      ("merge-base", 2),
      ("patch-id", 1),
      ("remote", 1),
      ("replay", 1),
      ("rev-list", 1),
      ("rev-parse", 4),
      ("symbolic-ref", 2),
    ],
  );
  let git = counting();
  let check = crate::update_project::preview_update(&git, path(&clone), "origin/master").unwrap().unwrap().check;
  assert!(matches!(check.state, UpdateState::Conflict { .. }), "{check:?}");
  assert_processes(
    &git,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("for-each-ref", 1),
      ("log", 2),
      ("merge-base", 2),
      ("remote", 1),
      ("rev-parse", 3),
      ("symbolic-ref", 2),
    ],
  );
}

/// A suggested unapply reads the unsupported commit once and checks the branch with the existing dry-run core
#[test]
fn an_unapply_suggestion_reads_its_commit_once() {
  let repo = TestRepo::new();
  let commit = repo.create_commit("(auth) Add login", "login.txt", "login\n");
  let git = counting();
  let suggestion = unapply_branch_suggestion(&git, repo.path_str(), "branch-deck", &commit, LockWait::default());
  assert_eq!(suggestion, None);
  assert_processes(
    &git,
    &[
      ("cat-file", 2),
      ("for-each-ref", 1),
      ("log", 2),
      ("remote", 1),
      ("rev-list", 1),
      ("rev-parse", 1),
      ("symbolic-ref", 1),
    ],
  );
}
