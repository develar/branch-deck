//! The git processes of the `bd` commands that add work around their core, by subcommand, as
//! `sync_core::process_budget_test` counts the cores: a change that runs fewer processes lowers the numbers here.
//! A command runs on the counting executor of its `RepoContext`, so its own reads count with those of the core.

use crate::fixture_test::{EMPTY_GLOBAL_CONFIG, PREFIX, canonical, commit_all, commit_subjects, context_with_the_config_prefix, environment, repo_with_baseline, run_in, write};
use crate::remote_fixture_test::{
  AUTH, STACK, Space, auth_remote_moved_back, forking_stack, needs_code_owner, push_from_elsewhere_to, repo_with_remote, robot as robot_json, stack_remote_moved_back,
};
use crate::{Cli, Command};
use bd_core::context::{RepoContext, RepoOptions, counted_git};
use bd_core::sync_command::sync;
use clap::Parser;
// jscpd:ignore-start: the budgets run the commands that lib.rs dispatches, so they import the same ones
use bd_lease::lease_command::{LeaseContext, LeaseGetArgs, LeaseReturnArgs, lease_get, lease_get_reporting, lease_return};
use bd_lease::workspace_command::workspace_with;
use bd_remote::push_command::{PushArgs, push};
use bd_remote::request_review_command::{RequestReviewArgs, request_review};
use bd_remote::robot_command::{RobotArgs, robot};
use bd_remote::robot_stats_command::{RobotStatsArgs, robot_stats};
use bd_remote::safe_push_command::{SafePushArgs, safe_push};
use bd_remote::status::{StatusArgs, status};
use bd_rewrite::check_command::{CheckArgs, check};
use bd_rewrite::commits::commits;
// jscpd:ignore-end
use bd_rewrite::fold_command::{FoldArgs, fold};
use bd_rewrite::move_hunk_command::{MoveHunkArgs, move_hunk};
use bd_rewrite::reword_command::{RewordArgs, reword};
use bd_rewrite::rewrite::{GroupArgs, RewriteArgs, group};
use bd_update::pick_command::{PickArgs, PickOutput, pick};
use domain_types::{ProgressReporter, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use lease_pool::host_slots::{HostSlotsParams, LendAnswer, LendRequest, ShrinkOutcome, SlotCheckout, checkout, fill, give_back, give_back_run, lend, shrink, snapshot_run};
use lease_pool::lease::{GetStep, Pool, canonical_path};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use sync_core::sync::{SyncOptions, sync_branches};
use test_utils::git_test_utils::{TestRepo, TestWorktree};

/// The context `bd --prefix test-user` resolves in `repo`; its executor counts every process of the command
fn context(repo: &TestRepo) -> RepoContext {
  let options = RepoOptions {
    repo: None,
    prefix: Some(PREFIX),
    full_ids: false,
    no_cache: false,
  };
  RepoContext::resolve(&environment(repo.path()), &options).unwrap()
}

/// The command of `context` counted exactly `expected`: `(subcommand, processes)`, sorted by subcommand
#[track_caller]
fn assert_processes(context: &RepoContext, expected: &[(&str, u32)]) {
  assert_git_processes(&context.git, expected);
}

/// `assert_processes` of the counting executor `git`
#[track_caller]
fn assert_git_processes(git: &GitCommandExecutor, expected: &[(&str, u32)]) {
  let counted: Vec<(String, u32)> = git
    .process_stats()
    .expect("a counting executor")
    .by_command()
    .into_iter()
    .map(|(subcommand, stats)| (subcommand, stats.count))
    .collect();
  let expected: Vec<(String, u32)> = expected.iter().map(|(subcommand, count)| ((*subcommand).to_string(), *count)).collect();
  assert_eq!(counted, expected);
}

/// Three commits of two branches, each adding its own file `f<n>.txt`
fn repo_with_branches() -> TestRepo {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commit_subjects(&repo, &["(auth) Add a", "(ui) Add b", "(auth) Add c"]);
  repo
}

/// A build that passes in the shell of `bd check --build` on every OS: `sh -c`, and `cmd` on Windows, which has no `true`
/// unless Git's `usr\bin` is on `PATH`
const PASSING_BUILD: &str = "exit 0";

/// `bd check` of the current grouping with `build` and `build_paths`
fn run_check(repo: &TestRepo, build: Option<&str>, build_paths: &[&str]) -> RepoContext {
  let context = context(repo);
  let args = CheckArgs {
    build: build.map(str::to_string),
    build_paths: build_paths.iter().map(|path| (*path).to_string()).collect(),
    ..CheckArgs::default()
  };
  let output = check(&context, &args).unwrap();
  assert!(output.ok, "{output:?}");
  context
}

/// The processes of `bd check` of the current grouping of `repo_with_branches` after a check wrote the object cache
const WARM_CHECK: [(&str, u32); 7] = [
  ("cat-file", 2),
  ("config", 1),
  ("fast-import", 2),
  ("for-each-ref", 1),
  ("log", 2),
  ("remote", 1),
  ("rev-parse", 2),
];

/// The processes of `bd check` of the current grouping of `repo_with_branches`, with an empty object cache: one
/// `cat-file` reads the blobs of the commits' blocks
const CHECK: [(&str, u32); 8] = [
  ("cat-file", 2),
  ("config", 1),
  ("fast-import", 2),
  ("for-each-ref", 1),
  ("log", 4),
  ("merge-tree", 2),
  ("remote", 1),
  ("rev-parse", 2),
];

/// `processes` with the counts of `changes` set, sorted by subcommand
fn changed<'a>(processes: &[(&'a str, u32)], changes: &[(&'a str, u32)]) -> Vec<(&'a str, u32)> {
  let mut all: BTreeMap<&str, u32> = processes.iter().copied().collect();
  all.extend(changes.iter().copied());
  all.into_iter().collect()
}

/// `bd check` of the current grouping, with an empty object cache
#[test]
fn a_check() {
  let repo = repo_with_branches();
  let context = run_check(&repo, None, &[]);
  assert_processes(&context, &CHECK);
}

/// `bd check` with the branch prefix of the repository's config (no `--prefix`): the one `git config` of the context
/// names the prefix, and the session takes the settings from it (`CheckSession::open_on`), so the check runs the
/// processes of `a_check`
#[test]
fn a_check_with_the_prefix_of_the_config() {
  let repo = repo_with_branches();
  let context = context_with_the_config_prefix(&repo);
  let output = check(&context, &CheckArgs::default()).unwrap();
  assert!(output.ok, "{output:?}");
  assert_processes(&context, &CHECK);
}

/// `bd commits` with the branch prefix of the repository's config: the one `git config` of the context names the
/// prefix, the stack setting and the symbol setting
#[test]
fn commits_with_the_prefix_of_the_config() {
  let repo = repo_with_branches();
  let context = context_with_the_config_prefix(&repo);
  // the arguments of `bd commits`, with the defaults of its flags
  let Command::Commits(args) = Cli::parse_from(["bd", "commits"]).command else {
    unreachable!("bd commits parses as Command::Commits")
  };
  let output = commits(&context, &args).unwrap();
  assert_eq!(output.commits.len(), 3);
  assert_processes(
    &context,
    &[
      ("cat-file", 2),
      ("check-attr", 1),
      ("config", 1),
      ("for-each-ref", 1),
      ("log", 2),
      ("remote", 1),
      ("rev-parse", 2),
    ],
  );
}

/// `bd check --build` of two paths, which the tree of each branch is extracted with: one `archive` per branch, one
/// `cat-file` for which paths each tree has
#[test]
fn a_check_with_a_build() {
  let repo = repo_with_branches();
  let context = run_check(&repo, Some(PASSING_BUILD), &["f0.txt", "f1.txt"]);
  assert_processes(&context, &changed(&CHECK, &[("archive", 2), ("cat-file", 3)]));
}

/// `bd check --build` again on the same trees: the passes are cached, so no `archive`; the one `cat-file` that reads the
/// tree ids for the key, over what a warm check without a build runs
#[test]
fn a_warm_check_with_a_build() {
  let repo = repo_with_branches();
  run_check(&repo, Some(PASSING_BUILD), &[]);
  let warm = run_check(&repo, None, &[]);
  assert_processes(&warm, &WARM_CHECK);
  let context = run_check(&repo, Some(PASSING_BUILD), &[]);
  assert_processes(&context, &changed(&WARM_CHECK, &[("cat-file", 3)]));
}

/// A reporter that drops the events
#[derive(Clone)]
struct Discard;

impl ProgressReporter for Discard {
  fn send(&self, _event: SyncEvent) -> anyhow::Result<()> {
    Ok(())
  }
}

/// `bd sync` with the branch prefix of the repository's config (no `--prefix`): the processes of the core
/// (`sync_branches` of the same repository), nothing else: the `rev-parse` of the context's directories takes the
/// place of the core's (`with_dirs_of`), in which the guards find the markers of an operation in progress and the
/// facts `FETCH_HEAD` and the landed state. The lock is taken in the common git directory the context resolved; the one `git config`
/// of the context reads the prefix, and the sync takes the settings and the descriptions from it
#[test]
fn a_sync_with_the_prefix_of_the_config() {
  // the global config of bd in a test, so that the core reads the settings bd reads
  let core = GitCommandExecutor::new().with_env("GIT_CONFIG_GLOBAL", EMPTY_GLOBAL_CONFIG).with_process_stats("core");
  let core_repo = repo_with_branches();
  let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
  runtime
    .block_on(sync_branches(&core, core_repo.path().to_str().unwrap(), PREFIX, Discard, SyncOptions::default()))
    .unwrap();
  let core: Vec<(String, u32)> = core
    .process_stats()
    .unwrap()
    .by_command()
    .into_iter()
    .map(|(subcommand, stats)| (subcommand, stats.count))
    .collect();
  let core: Vec<(&str, u32)> = core.iter().map(|(subcommand, count)| (subcommand.as_str(), *count)).collect();

  let repo = repo_with_branches();
  let context = context_with_the_config_prefix(&repo);
  let output = sync(&context).unwrap();
  assert_eq!(output.branches.len(), 2);
  assert_processes(&context, &core);
}

/// `bd status` of the synced branches: HEAD, the grouping, whose one listing has the virtual tips too, the stack
/// setting and the descriptions from the one `git config` of the command, and the push URL of `origin` (`remote
/// get-url`), which tells whether Patronus is asked what merged Safe Pushes landed (`landed_by`); not on Space here
#[test]
fn a_status() {
  let repo = repo_with_branches();
  sync(&context(&repo)).unwrap();
  let context = context(&repo);
  let output = status(&context, &StatusArgs::default(), &environment(repo.path())).unwrap();
  assert_eq!(output.branch_count, 2);
  assert_processes(
    &context,
    &[("cat-file", 1), ("config", 1), ("for-each-ref", 1), ("log", 1), ("remote", 2), ("rev-parse", 2)],
  );
}

/// `bd status` of a stack that forks: its stack branch is read from the listing and the object cache, without a git
/// process of its own
#[test]
fn a_status_of_a_forking_stack() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  commit_subjects(&repo, &["(auth) Add a", "(auth/login) Add b", "(auth/session) Add c"]);
  sync(&context(&repo)).unwrap();
  let context = context(&repo);
  let output = status(&context, &StatusArgs::default(), &environment(repo.path())).unwrap();
  assert_eq!(output.stacks.len(), 1);
  assert_processes(
    &context,
    &[("cat-file", 1), ("config", 1), ("for-each-ref", 1), ("log", 1), ("remote", 2), ("rev-parse", 2)],
  );
}

/// `bd push --dry-run auth` to a local bare remote: the sync, then the push it asks the remote about. The push takes the
/// stack, the tips, the remote tips, the remotes and the descriptions from the sync's report; besides the push only the
/// push URL of `origin` (`remote`, for the Space and the GitHub repository), the Space settings (`cat-file`, for the
/// GitHub mirror `--repo` names: the remote is no GitHub URL) and the size check of the files the push adds (`log` and
/// `cat-file`; the limit is the sync's `config`). A dry run moves nothing, so it reads no witness of its plan: the tips
/// and the leases are the ones the plan was decided on
#[test]
fn a_push() {
  let (repo, _remote) = repo_with_remote(&["(auth) Add login", "(ui) Add form"]);
  let context = context(&repo);
  let args = PushArgs {
    branches: vec!["auth".to_string()],
    all: false,
    yes: false,
    with_children: false,
    alone: false,
    force: false,
    expect_remote: None,
    remote: None,
    dry_run: true,
    restart_running: false,
    unapplied: false,
  };
  push(&context, &args, &environment(repo.path())).unwrap();
  assert_processes(
    &context,
    &[
      ("cat-file", 3),
      ("config", 1),
      ("fast-import", 3),
      ("for-each-ref", 1),
      ("log", 3),
      ("merge-base", 1),
      ("merge-tree", 1),
      ("notes", 1),
      ("push", 1),
      ("remote", 2),
      ("rev-list", 2),
      ("rev-parse", 1),
      ("show-ref", 1),
      ("symbolic-ref", 1),
      ("update-ref", 2),
      ("var", 1),
    ],
  );
}

/// `bd push auth` over a remote branch someone moved back to an older commit of the branch since the sync: the witness of
/// its plan, the tip of the branch read before the repository lock and again under it (one `for-each-ref` each, the
/// lock held for that read and the freeze only), then the refused push fetches it and computes what it has now, then
/// the push again over it, once, and nothing else (D4)
#[test]
fn a_push_over_a_moved_remote() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  auth_remote_moved_back(&repo, &remote);

  let context = context(&repo);
  let args = PushArgs {
    branches: vec!["auth".to_string()],
    ..PushArgs::default()
  };
  let output = push(&context, &args, &environment(repo.path())).unwrap();
  assert!(output.branches[0].pushed_over);
  // the sync, the witness twice, the refused push, its fetch and the status of what the remote has now, then the push
  // again; each push checks the size of what it adds first (`log`, `cat-file`)
  assert_processes(
    &context,
    &[
      ("cat-file", 4),
      ("config", 1),
      ("fast-import", 2),
      ("fetch", 1),
      ("for-each-ref", 4),
      ("log", 6),
      ("merge-base", 1),
      ("notes", 1),
      ("push", 2),
      ("reflog", 2),
      ("remote", 2),
      ("rev-list", 4),
      ("rev-parse", 1),
      ("show-ref", 1),
      ("symbolic-ref", 1),
      ("update-ref", 1),
      ("var", 1),
    ],
  );
}

/// `bd push --dry-run env --alone` on Space while the Safe Push of auth runs, started before a restack placed the new
/// root env below auth: the sync of branches it built already (no build), the push and its size check (`log`,
/// `cat-file`), and what the robot's commit lands of the branches as they are now, one `cat-file --batch-check` of its
/// commit and the virtual branch and the two walks against it (`robot_spans`); a robot at the tip the sync left needs
/// none of them
#[test]
fn a_push_beside_a_robot_started_before_a_restack() {
  let space = Space::new(&["(auth) Add auth"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  run_in(&space.repo, &["sync"]);
  space.set_robots(vec![robot_json("auth-robot", "RUNNING", "auth", &space.local_tip(AUTH))]);
  write(&space.repo, "env.txt", "env\n");
  commit_all(&space.repo, "(env) Add env", 1_700_000_900);
  for args in [&["restack", "auth", "env"][..], &["sync"]] {
    let (exit_code, output) = run_in(&space.repo, args);
    assert_eq!(exit_code, 0, "{output}");
  }

  let environment = space.environment();
  let context = space_context(&environment);
  let args = PushArgs {
    branches: vec!["env".to_string()],
    all: false,
    yes: false,
    with_children: false,
    alone: true,
    force: false,
    expect_remote: None,
    remote: None,
    dry_run: true,
    restart_running: false,
    unapplied: false,
  };
  push(&context, &args, &environment).unwrap();
  assert_processes(
    &context,
    &[
      ("cat-file", 4),
      ("config", 1),
      ("for-each-ref", 1),
      ("log", 5),
      ("merge-base", 1),
      ("push", 1),
      ("remote", 2),
      ("rev-list", 2),
      ("rev-parse", 1),
      ("symbolic-ref", 1),
    ],
  );
}

/// `bd fold`: the guards and the plan of the command, then the squash of the fixups
#[test]
fn a_fold() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commit_subjects(&repo, &["(auth) Add a", "(ui) Add b", "fixup! (auth) Add a"]);
  let context = context(&repo);
  let args = FoldArgs {
    branch: None,
    expect_head: None,
    dry_run: false,
  };
  let output = fold(&context, &args).unwrap();
  assert_eq!(output.folded.len(), 1);
  assert_processes(
    &context,
    &[
      ("cat-file", 3),
      ("config", 2),
      ("fast-import", 1),
      ("for-each-ref", 2),
      ("log", 3),
      ("merge-tree", 1),
      ("remote", 2),
      ("replay", 1),
      ("rev-list", 1),
      ("rev-parse", 7),
      ("symbolic-ref", 2),
      ("update-ref", 1),
      ("var", 1),
    ],
  );
}

/// `bd reword` of the oldest of three commits: the regroup's snapshot, then one `fast-import` of the recreated commits
/// and one ref transaction
#[test]
fn a_reword() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let ids = commit_subjects(&repo, &["(auth) Add a with a subject that is far too long", "(ui) Add b", "(auth) Add c"]);
  let context = context(&repo);
  let args = RewordArgs {
    commit: Some(ids[0].clone()),
    message: None,
    message_file: None,
    subject: Some("(auth) Add a".to_string()),
    plan: None,
    rewrite: RewriteArgs::default(),
  };
  let output = reword(&context, &args).unwrap();
  assert_eq!(output.rewritten, 1);
  assert_processes(
    &context,
    &[
      ("cat-file", 2),
      ("config", 1),
      ("fast-import", 1),
      ("for-each-ref", 2),
      ("log", 2),
      ("remote", 2),
      ("rev-parse", 5),
      ("symbolic-ref", 1),
      ("update-ref", 1),
    ],
  );
}

/// `bd move-hunk` of a file change from the newest commit into the oldest, over `between` commits that don't touch the
/// file: its context, which counted its processes
fn move_hunk_context(between: usize) -> RepoContext {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let first = commit_subjects(&repo, &["(auth) Add a"])[0].clone();
  for index in 0..between {
    commit_subjects(&repo, &[&format!("(other) Add o{index}")]);
  }
  // the commit keeps its other file
  std::fs::write(repo.path().join("f1.txt"), "changed\n").unwrap();
  std::fs::write(repo.path().join("ui.txt"), "ui\n").unwrap();
  repo.git(&["add", "f1.txt", "ui.txt"]);
  repo.git(&["commit", "-q", "-m", "(ui) Change a"]);
  let last = repo.head();
  let context = context(&repo);
  let args = MoveHunkArgs {
    from: last,
    to: first,
    path: "f1.txt".to_string(),
    ..MoveHunkArgs::default()
  };
  move_hunk(&context, &args).unwrap();
  context
}

/// `bd move-hunk`: a fixed number of processes, whatever the number of commits between the two. The baseline with its
/// commit and HEAD come from one listing (`read_listed_baseline`: `remote`, `for-each-ref`), the commits ahead from one
/// `log` between the two ids
#[test]
fn a_move_hunk() {
  for between in [1, 6] {
    assert_processes(&move_hunk_context(between), &MOVE_HUNK);
  }
}

/// The processes of `bd move-hunk` (`a_move_hunk`)
const MOVE_HUNK: [(&str, u32); 12] = [
  ("cat-file", 2),
  ("diff-tree", 2),
  ("fast-import", 2),
  ("for-each-ref", 1),
  ("log", 3),
  ("merge-tree", 1),
  ("patch-id", 1),
  ("remote", 1),
  ("rev-parse", 2),
  ("symbolic-ref", 2),
  ("update-ref", 1),
  ("var", 1),
];

/// `bd pick --from <lane>` with the options `options` sets, in a counting context
fn pick_from(repo: &TestRepo, lane: TestWorktree, options: impl FnOnce(&mut PickArgs)) -> (RepoContext, PickOutput) {
  let context = context(repo);
  let mut args = PickArgs {
    from: Some(lane.path.into()),
    ..PickArgs::default()
  };
  options(&mut args);
  let output = pick(&context, &args, &environment(repo.path())).unwrap();
  (context, output)
}

/// Two commits picked onto a branch one commit ahead and the worktree followed, with the processes of `a_pick`
#[track_caller]
fn assert_followed_pick(context: &RepoContext, output: &PickOutput) {
  assert_eq!(
    (output.picked.len(), output.followed.as_ref().map(|followed| followed.state)),
    (2, Some(bd_update::pick_command::FollowedState::Moved))
  );
  assert_processes(
    context,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("diff-tree", 2),
      ("for-each-ref", 1),
      ("log", 4),
      ("merge-base", 4),
      ("read-tree", 4),
      ("remote", 1),
      ("replay", 1),
      ("rev-list", 1),
      ("rev-parse", 17),
      ("symbolic-ref", 9),
      ("update-index", 6),
      ("update-ref", 4),
    ],
  );
}

/// `bd pick --from <worktree> --follow`: before the lock the baseline, the marks of both worktrees and the check (HEAD
/// with its committer date, the fork point and whether the tip has it, the range with the patch id marks, one `replay`, the copies, both dry runs); under it the
/// marks again, the witness (the guards and HEAD of both worktrees, the upstream; the local changes of added paths need
/// no git) and the two moves with their marks (`update-ref`, `read-tree`); after it the grouping of the picked commits
/// (the inputs and one `log` of the commits ahead: a new prefix needs no diffs). `the_check_of_a_pick_runs_before_the_lock`
/// pins which of them run under the lock
#[test]
fn a_pick() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let lane = TestWorktree::detached(&repo);
  lane.commit("(lane) Add a", "a.txt", "a\n");
  lane.commit("(lane) Add b", "b.txt", "b\n");
  commit_subjects(&repo, &["(own) Add c"]);
  let (context, output) = pick_from(&repo, lane, |args| args.follow = true);
  assert_followed_pick(&context, &output);
}

/// `bd pick --remote auth` of a commit someone else pushed: the sync of `bd sync` first, then the fetch of the branch
/// before the lock, the walks of its remote status (the commits it lacks), the grouping inputs (whose subject groups
/// elsewhere), the check (the listed commits, one `replay`, the copies, one `cat-file` and one `fast-import` of the
/// prefixed copies, whose temporary ref goes at once), then the move under the lock and the grouping of the picked commit
#[test]
fn a_remote_pick() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  push_from_elsewhere_to(&remote, AUTH, "Their fix");
  let context = context(&repo);
  let args = PickArgs {
    remote: Some("auth".to_string()),
    ..PickArgs::default()
  };
  let output = pick(&context, &args, &environment(repo.path())).unwrap();
  assert_eq!(output.picked.len(), 1);
  assert_processes(
    &context,
    &[
      ("cat-file", 5),
      ("config", 3),
      ("diff-tree", 1),
      ("fast-import", 1),
      ("fetch", 1),
      ("for-each-ref", 3),
      ("log", 9),
      ("merge-base", 1),
      ("read-tree", 2),
      ("reflog", 1),
      ("remote", 2),
      ("replay", 1),
      ("rev-list", 3),
      ("rev-parse", 6),
      ("symbolic-ref", 6),
      ("update-index", 3),
      ("update-ref", 3),
    ],
  );
}

/// The environment of bd in `repo` with the lease pool in a temporary home, which lives as long as the guard
fn lease_environment(repo: &TestRepo) -> (tempfile::TempDir, bd_core::context::Environment) {
  let home = tempfile::tempdir().unwrap();
  let mut environment = environment(repo.path());
  environment.home = Some(home.path().to_path_buf());
  (home, environment)
}

/// `bd lease get` of a free slot and its `bd lease return`: the repository, the source HEAD and the pool config, one
/// `status` and the HEAD of the slot, which is at the source HEAD already, so no move; the return reads whether a resolve session of the slot is open, the
/// unlanded commits, the untracked mode of the config, the `status` and HEAD. No `status` of another slot, no full checkout
#[test]
fn a_lease_get_and_return() {
  let repo = repo_with_branches();
  let (_home, environment) = lease_environment(&repo);
  let context = LeaseContext::resolve(&environment, None).unwrap();
  let added = lease_get(&context, &environment, &LeaseGetArgs::default()).unwrap();
  let slot = std::path::PathBuf::from(&added.path);
  lease_return(&LeaseContext::resolve(&environment, Some(&slot)).unwrap(), &LeaseReturnArgs::default()).unwrap();

  let context = LeaseContext::resolve(&environment, None).unwrap();
  let got = lease_get(&context, &environment, &LeaseGetArgs::default()).unwrap();
  assert!(got.reused, "{got:?}");
  assert_git_processes(&context.git, &[("config", 1), ("rev-parse", 3), ("status", 1)]);
  let context = LeaseContext::resolve(&environment, Some(&slot)).unwrap();
  lease_return(&context, &LeaseReturnArgs::default()).unwrap();
  assert_git_processes(&context.git, &[("config", 1), ("log", 1), ("rev-parse", 2), ("show-ref", 1), ("status", 1)]);
}

/// `bd lease return` with `branchdeck.leaseMaintenance`: the processes of a return and one `maintenance run --auto
/// --detach` in the common dir, once the slot is free
#[test]
fn a_lease_return_that_starts_the_maintenance() {
  let repo = repo_with_branches();
  repo
    .git_executor()
    .execute_command(&["config", "branchdeck.leaseMaintenance", "true"], repo.path().to_str().unwrap())
    .unwrap();
  let (_home, environment) = lease_environment(&repo);
  let context = LeaseContext::resolve(&environment, None).unwrap();
  let added = lease_get(&context, &environment, &LeaseGetArgs::default()).unwrap();
  let context = LeaseContext::resolve(&environment, Some(std::path::Path::new(&added.path))).unwrap();
  let returned = lease_return(&context, &LeaseReturnArgs::default()).unwrap();
  assert!(returned.maintenance_started, "{returned:?}");
  assert_git_processes(
    &context.git,
    &[("config", 1), ("log", 1), ("maintenance", 1), ("rev-parse", 2), ("show-ref", 1), ("status", 1)],
  );
}

/// The host slots of `bd host`, each step after the `rev-parse` of its context: a fill of a free slot reads the config of
/// the host slots, then gets the slot as `bd lease get` does; a lend runs no git; the checkout of a run reads the config
/// and HEAD and moves by the diff with one `reset`, no `status`; a give-back reads the config and one `status`, and only
/// a dirty slot adds `reset` and `clean`; a shrink reads the config, one `status` and HEAD, which is at the leased commit
/// again, then returns the slot as `bd lease return` does (with its `show-ref` of the slot's resolve session)
#[test]
fn the_host_slots_lend_give_back_and_shrink() {
  let repo = repo_with_branches();
  let path = repo.path().to_str().unwrap();
  repo.git_executor().execute_command(&["config", "branchdeck.hostSlots", "1"], path).unwrap();
  let (home, environment) = lease_environment(&repo);
  let context = LeaseContext::resolve(&environment, None).unwrap();
  let added = lease_get(&context, &environment, &LeaseGetArgs::default()).unwrap();
  lease_return(
    &LeaseContext::resolve(&environment, Some(std::path::Path::new(&added.path))).unwrap(),
    &LeaseReturnArgs::default(),
  )
  .unwrap();
  let host = |run: &dyn Fn(&GitCommandExecutor, &HostSlotsParams<'_>)| {
    let context = LeaseContext::resolve(&environment, None).unwrap();
    let params = HostSlotsParams {
      repository: &context.dirs,
      home: Some(home.path()),
      lock_timeout: context.lock_timeout,
      server_timeout: context.server_stop_timeout,
    };
    run(&context.git, &params);
    context.git
  };
  let pid = std::process::id();
  let request = LendRequest {
    run: "run-1",
    pid,
    process_started: sync_utils::process_scan::process_started(pid),
    affinity: None,
  };

  let git = host(&|git, params| assert!(fill(git, params, None).unwrap()[0].reused));
  assert_git_processes(&git, &[("config", 2), ("rev-parse", 3), ("status", 1)]);
  let git = host(&|_, params| assert!(matches!(lend(params, &request).unwrap(), LendAnswer::Lent(_))));
  assert_git_processes(&git, &[("rev-parse", 1)]);
  let commit = repo.create_commit("Run commit", "run.txt", "run\n");
  let git = host(&|git, params| assert_eq!(checkout(git, params, "run-1", &commit).unwrap().moved, SlotCheckout::Switched));
  assert_git_processes(&git, &[("config", 1), ("reset", 1), ("rev-parse", 2)]);
  let git = host(&|git, params| assert_eq!(give_back(git, params, 1, "run-1").unwrap().discarded, Vec::<String>::new()));
  assert_git_processes(&git, &[("config", 1), ("rev-parse", 1), ("status", 1)]);
  host(&|_, params| assert!(matches!(lend(params, &request).unwrap(), LendAnswer::Lent(_))));
  std::fs::write(std::path::Path::new(&added.path).join("untracked.txt"), "x\n").unwrap();
  let git = host(&|git, params| assert_eq!(give_back(git, params, 1, "run-1").unwrap().discarded, ["untracked.txt"]));
  assert_git_processes(&git, &[("clean", 1), ("config", 1), ("reset", 1), ("rev-parse", 1), ("status", 1)]);
  // with `branchdeck.leaseMaintenance` the give-back starts the maintenance, and so does one of a run without a slot
  repo.git_executor().execute_command(&["config", "branchdeck.leaseMaintenance", "true"], path).unwrap();
  host(&|_, params| assert!(matches!(lend(params, &request).unwrap(), LendAnswer::Lent(_))));
  let git = host(&|git, params| assert!(give_back(git, params, 1, "run-1").unwrap().maintenance_started));
  assert_git_processes(&git, &[("config", 1), ("maintenance", 1), ("rev-parse", 1), ("status", 1)]);
  let git = host(&|git, params| assert!(give_back_run(git, params, "run-2").unwrap().maintenance_started));
  assert_git_processes(&git, &[("config", 1), ("maintenance", 1), ("rev-parse", 1)]);
  repo.git_executor().execute_command(&["config", "--unset", "branchdeck.leaseMaintenance"], path).unwrap();
  // the scans of a slot: the config, HEAD and its tree (`log`) and one status; when something changed, the blobs, the
  // commit and the delete of its temporary ref
  host(&|_, params| assert!(matches!(lend(params, &request).unwrap(), LendAnswer::Lent(_))));
  let git = host(&|git, params| assert_eq!(snapshot_run(git, params, "run-1").unwrap().changed, 0));
  assert_git_processes(&git, &[("config", 1), ("log", 1), ("rev-parse", 1), ("status", 1)]);
  std::fs::write(std::path::Path::new(&added.path).join("made.txt"), "made\n").unwrap();
  let git = host(&|git, params| assert_eq!(snapshot_run(git, params, "run-1").unwrap().changed, 1));
  assert_git_processes(
    &git,
    &[
      ("config", 1),
      ("fast-import", 1),
      ("hash-object", 1),
      ("log", 1),
      ("rev-parse", 1),
      ("status", 1),
      ("update-ref", 1),
    ],
  );
  host(&|git, params| assert_eq!(give_back(git, params, 1, "run-1").unwrap().discarded, ["made.txt"]));
  let git = host(&|git, params| assert!(matches!(shrink(git, params, 0).unwrap()[0].outcome, ShrinkOutcome::Returned(_))));
  assert_git_processes(&git, &[("config", 2), ("log", 1), ("reset", 1), ("rev-parse", 3), ("show-ref", 1), ("status", 2)]);
}

/// `bd workspace` in the shared checkout: the repository, the `branchdeck.*` config, the pool config and one `status` of
/// the checkout; in a lease the same without the pool config, the `status` of the shared checkout
#[test]
fn a_workspace() {
  let repo = repo_with_branches();
  let (_home, environment) = lease_environment(&repo);
  // bd's git, with the empty global config of the test
  let git = counted_git(&environment).unwrap();
  let output = workspace_with(&git, &environment, None).unwrap();
  assert_eq!(output.next.len(), 1, "{output:?}");
  assert_git_processes(&git, &[("config", 2), ("rev-parse", 1), ("status", 1)]);

  let context = LeaseContext::resolve(&environment, None).unwrap();
  let got = lease_get(&context, &environment, &LeaseGetArgs::default()).unwrap();
  let git = counted_git(&environment).unwrap();
  let output = workspace_with(&git, &environment, Some(std::path::Path::new(&got.path))).unwrap();
  assert_eq!(output.lease_id.as_deref(), Some(got.lease_id.as_str()));
  assert_git_processes(&git, &[("config", 1), ("rev-parse", 2), ("status", 1)]);
}

/// A repository with a commit on master, the pool in a temporary home, which lives as long as its guard, and a lease of
/// it
fn leased_repo() -> (TestRepo, tempfile::TempDir, bd_core::context::Environment, bd_lease::lease_command::LeaseGetOutput) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let (home, environment) = lease_environment(&repo);
  let got = lease_get(&LeaseContext::resolve(&environment, None).unwrap(), &environment, &LeaseGetArgs::default()).unwrap();
  (repo, home, environment, got)
}

/// `bd lease pick` whose pick conflicts: the check of the pick in the home checkout (no lock, nothing moves), then the
/// update of the lease onto the home branch without a fetch: the check of the update (a binary search of `replay`) and
/// the start of a resolve session in the lease under the lock. The two commits have subjects of their own: with one
/// subject they share an identity (`CommitKey`: the author, the second and the subject without the prefix) whenever they
/// fall into the same second, and the update then proves the pair by its replay and its changed lines
#[test]
fn a_lease_pick_that_conflicts() {
  let (repo, _home, environment, got) = leased_repo();
  let lane = std::path::Path::new(&got.path);
  std::fs::write(lane.join("README.md"), "lane\n").unwrap();
  test_utils::git_test_utils::git_command()
    .args(["commit", "-q", "-a", "-m", "(lane) Change readme"])
    .current_dir(lane)
    .output()
    .unwrap();
  repo.create_commit("(own) Rewrite readme", "README.md", "own\n");
  let context = context(&repo);
  let output = crate::lease_pick_in(
    &context,
    &got.path,
    "master".to_string(),
    None,
    &environment,
    &bd_lease::lease_command::LeasePickArgs::default(),
  )
  .unwrap();
  assert_eq!(output.state, bd_update::pick_command::PickOutcome::Conflict);
  assert_processes(
    &context,
    &[
      ("for-each-ref", 1),
      ("log", 2),
      ("merge-base", 4),
      ("merge-tree", 1),
      ("remote", 1),
      ("replay", 1),
      ("rev-list", 1),
      ("rev-parse", 8),
      ("symbolic-ref", 3),
    ],
  );
  let lease = RepoContext::resolve(
    &environment,
    &RepoOptions {
      repo: Some(lane),
      prefix: Some(PREFIX),
      full_ids: false,
      no_cache: false,
    },
  )
  .unwrap();
  let update = bd_update::update_command::update_lease_onto(&lease, "master").unwrap();
  assert_eq!(update.state, bd_update::update_command::UpdateOutcome::Conflict);
  assert_processes(
    &lease,
    &[
      ("cat-file", 6),
      ("check-attr", 2),
      ("config", 1),
      ("diff-tree", 2),
      ("fast-import", 1),
      ("for-each-ref", 1),
      ("hash-object", 1),
      ("log", 6),
      ("merge-base", 3),
      ("merge-tree", 3),
      ("replay", 1),
      ("rev-list", 2),
      ("rev-parse", 4),
      ("show-ref", 1),
      ("symbolic-ref", 2),
      ("update-ref", 1),
      ("var", 2),
    ],
  );
}

/// The check of `bd lease pick` with `pick_requires_check` (beside the resolve of the lease, one `rev-parse`): one
/// `rev-parse` of HEAD and its tree, and for a tree that only an older record names one `diff-tree` that counts the files
/// changed since
#[test]
fn the_tree_check_of_a_lease_pick() {
  let (_repo, _home, environment, got) = leased_repo();
  let lane = std::path::Path::new(&got.path);
  let lease = LeaseContext::resolve(&environment, Some(lane)).unwrap();
  // the empty tree, which git knows without an object
  std::fs::write(
    std::path::Path::new(&lease.dirs.git_dir).join(lease_pool::check_record::CHECKS_FILE),
    "{\"schema\":1,\"tree\":\"4b825dc642cb6eb9a060e54bf8d69288fbee4904\",\"exit\":0}\n",
  )
  .unwrap();
  let check = bd_lease::lease_command::lease_tree_check(&lease).unwrap();
  assert_eq!(
    (check.state, check.changed_paths),
    (lease_pool::check_record::CheckState::Stale, Some(vec!["README.md".to_string()]))
  );
  assert_git_processes(&lease.git, &[("diff-tree", 1), ("rev-parse", 2)]);
}

/// `bd lease pick`: the lease from the state of the pool and whether its resolve session is open (`show-ref`); then the processes of `bd pick --from <lease>
/// --follow` in the checkout it was cut from (`a_pick`)
#[test]
fn a_lease_pick() {
  let (repo, _home, environment, got) = leased_repo();
  let lane = std::path::Path::new(&got.path);
  for (subject, file) in [("(lane) Add a", "a.txt"), ("(lane) Add b", "b.txt")] {
    std::fs::write(lane.join(file), "x\n").unwrap();
    test_utils::git_test_utils::git_command().args(["add", file]).current_dir(lane).output().unwrap();
    test_utils::git_test_utils::git_command()
      .args(["commit", "-q", "-m", subject])
      .current_dir(lane)
      .output()
      .unwrap();
  }
  commit_subjects(&repo, &["(own) Add c"]);
  let lease = LeaseContext::resolve(&environment, Some(lane)).unwrap();
  let (checkout, branch) = bd_lease::lease_command::lease_home(&lease).unwrap();
  assert_git_processes(&lease.git, &[("rev-parse", 1), ("show-ref", 1)]);
  let context = context(&repo);
  // the checkout as the pool keeps it, the repository as git names its top level (forward slashes on Windows)
  assert_eq!(canonical(std::path::Path::new(&checkout)), canonical(std::path::Path::new(&context.repository)));
  let output = crate::lease_pick_in(&context, &got.path, branch, None, &environment, &bd_lease::lease_command::LeasePickArgs::default()).unwrap();
  assert_followed_pick(&context, &output);
}

/// `bd lease get` of a new slot by an APFS clone of the source (macOS): the repository, the source HEAD and the pool
/// config, `worktree add --force --no-checkout --lock` and its `worktree unlock` once the clone is in place (no
/// `worktree prune`), the refresh of the copied index, one `checkout` of the diff, `clean -fd` and one `status`
#[cfg(target_os = "macos")]
#[test]
fn a_lease_get_of_a_new_slot_by_a_clone() {
  let repo = repo_with_branches();
  let (_home, environment) = lease_environment(&repo);
  let context = LeaseContext::resolve(&environment, None).unwrap();
  let got = lease_get(&context, &environment, &LeaseGetArgs::default()).unwrap();
  assert!(matches!(got.created_by, Some(bd_lease::lease_command::CreatedByOutput::Clone)), "{got:?}");
  assert_git_processes(
    &context.git,
    &[
      ("checkout", 1),
      ("clean", 1),
      ("config", 1),
      ("rev-parse", 2),
      ("status", 1),
      ("update-index", 1),
      ("worktree", 2),
    ],
  );
}

/// `bd lease get` of a new slot that waited for the clones lock, which the test held: the processes of a new slot (on
/// macOS a clone, elsewhere the registration and the full checkout) and one `config` more, the minimum free space read
/// again once its turn came
#[test]
fn a_lease_get_that_waited_for_the_clones_lock() {
  let repo = repo_with_branches();
  let (_home, environment) = lease_environment(&repo);
  let context = LeaseContext::resolve(&environment, None).unwrap();
  let pool = Pool::new(std::path::Path::new(&canonical_path(std::path::Path::new(&context.dirs.git_common_dir))));
  let waiting = std::sync::atomic::AtomicBool::new(false);
  let got = std::thread::scope(|scope| {
    let held = pool.lock_clones(std::time::Duration::from_secs(10), &mut |_| {}).unwrap();
    let get = scope.spawn(|| {
      lease_get_reporting(&context, &environment, &LeaseGetArgs::default(), &|step| {
        if matches!(step, GetStep::WaitForClone(_)) {
          waiting.store(true, std::sync::atomic::Ordering::Relaxed);
        }
      })
    });
    while !waiting.load(std::sync::atomic::Ordering::Relaxed) && !get.is_finished() {
      std::thread::sleep(std::time::Duration::from_millis(10));
    }
    drop(held);
    get.join().unwrap().unwrap()
  });
  assert!(got.waited_for_clone_ms.is_some(), "{got:?}");
  if cfg!(target_os = "macos") {
    assert_git_processes(
      &context.git,
      &[
        ("checkout", 1),
        ("clean", 1),
        ("config", 2),
        ("rev-parse", 2),
        ("status", 1),
        ("update-index", 1),
        ("worktree", 2),
      ],
    );
  } else {
    assert_git_processes(&context.git, &[("checkout", 1), ("config", 2), ("rev-parse", 2), ("worktree", 1)]);
  }
}

/// `bd pick --dry-run` of a commit whose prefix names a branch the target has, a fast-forward: the check without a
/// replay and without the follow, the grouping, and the diffs of the dependency check (`log --raw` of the commits ahead
/// and the copy, one `cat-file` of their blobs)
#[test]
fn a_pick_into_a_branch_the_target_has() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commit_subjects(&repo, &["(own) Add c"]);
  let lane = TestWorktree::detached(&repo);
  lane.commit("(own) Change c", "f0.txt", "changed\n");
  let (context, output) = pick_from(&repo, lane, |args| args.dry_run = true);
  assert_eq!((output.picked.len(), output.dependencies.len()), (1, 0));
  assert_processes(
    &context,
    &[
      ("cat-file", 2),
      ("config", 1),
      ("diff-tree", 1),
      ("for-each-ref", 1),
      ("log", 4),
      ("merge-base", 4),
      ("read-tree", 1),
      ("remote", 1),
      ("rev-list", 1),
      ("rev-parse", 4),
      ("symbolic-ref", 2),
      ("update-index", 1),
    ],
  );
}

/// The processes of the target conflict check of `bd safe-push`: before the lock, the fetch of the target reads the
/// baseline and its upstream once (`read_baseline`: `symbolic-ref`, `remote`, one `for-each-ref`), then fetches it
/// (`fetch`); after the sync, one `replay` of the branch onto the target tip that the sync read
const TARGET_CHECK: &[(&str, u32)] = &[("fetch", 1), ("for-each-ref", 1), ("remote", 1), ("replay", 1), ("symbolic-ref", 1)];

/// What `bd safe-push auth --check` runs besides the target conflict check (`a_safe_push_check`)
const SAFE_PUSH_CHECK: &[(&str, u32)] = &[
  ("cat-file", 3),
  ("config", 1),
  ("diff-tree", 1),
  ("fast-import", 3),
  ("for-each-ref", 2),
  ("log", 7),
  ("merge-base", 1),
  ("merge-tree", 1),
  ("notes", 1),
  ("remote", 2),
  ("rev-list", 2),
  ("rev-parse", 1),
  ("show-ref", 1),
  ("symbolic-ref", 1),
  ("update-ref", 2),
  ("var", 1),
];

/// `base` with the counts of `more` added, by subcommand
fn with_more(base: &[(&'static str, u32)], more: &[(&'static str, u32)]) -> Vec<(&'static str, u32)> {
  let mut all: BTreeMap<&str, u32> = base.iter().copied().collect();
  for (subcommand, count) in more {
    *all.entry(subcommand).or_default() += count;
  }
  all.into_iter().collect()
}

/// `assert_processes` of a `bd safe-push` that runs `expected` and the target conflict check (`TARGET_CHECK`)
#[track_caller]
fn assert_safe_push_processes(context: &RepoContext, expected: &[(&str, u32)]) {
  let mut all: BTreeMap<&str, u32> = expected.iter().copied().collect();
  for (subcommand, count) in TARGET_CHECK {
    *all.entry(subcommand).or_default() += count;
  }
  let all: Vec<(&str, u32)> = all.into_iter().collect();
  assert_processes(context, &all);
}

/// `bd safe-push auth --check` on Space: the sync, then what the start is decided on, nothing pushed. The target, the
/// stack and the tips come from the sync's report, the Space and the GitHub repository from one push URL of `origin`
/// (`remote`) and the Space settings (`cat-file`); the dangling references take the grouping from the report too and
/// read the commits with their new mapping notes (`log`) and the branch tips (`for-each-ref`) for the trial; the
/// landing gate reads the names the commits use and add for the missing declarations, in the same trial session: the
/// changes of the commits (`log`) and the summaries of their source blobs (`cat-file`); the session takes the commit
/// the sync built on and the baseline commit from the report (`SyncReport::base`, `baseline_commit`), not from git; the
/// review (`log`, `diff-tree`) reads its own; and the target conflict check (`TARGET_CHECK`)
#[test]
fn a_safe_push_check() {
  let space = Space::new(&["(auth) Add login", "(ui) Add form"]);
  let options = RepoOptions {
    repo: None,
    prefix: Some(PREFIX),
    full_ids: false,
    no_cache: false,
  };
  let environment = space.environment();
  let context = RepoContext::resolve(&environment, &options).unwrap();
  let args = SafePushArgs {
    branch: "auth".to_string(),
    check: true,
    ..SafePushArgs::default()
  };
  let output = safe_push(&context, &args, &environment).unwrap();
  assert!(output.push_needed && output.robot.is_none(), "{output:?}");
  assert_safe_push_processes(&context, SAFE_PUSH_CHECK);
}

/// `bd safe-push auth`, a start through the API: what `a_safe_push_check` decides on, then the run of the engine
/// (`operations::land_execute`): the cheap witness before the lock and the full one under it, each one `for-each-ref` of
/// the refs of the plan, the push of the commit they froze after the lock (`push`, with the size check of what it adds:
/// `log`, `cat-file`), and the late witness of the pushed branch before the start (`for-each-ref`); the start is HTTP
#[test]
fn a_safe_push_start() {
  let space = Space::new(&["(auth) Add login", "(ui) Add form"]);
  let environment = space.environment();
  let context = space_context(&environment);
  let args = SafePushArgs {
    branch: "auth".to_string(),
    ..SafePushArgs::default()
  };
  let output = safe_push(&context, &args, &environment).unwrap();
  assert!(output.robot.is_some() && output.pushed.len() == 1, "{output:?}");
  assert_safe_push_processes(&context, &with_more(SAFE_PUSH_CHECK, &[("cat-file", 1), ("for-each-ref", 3), ("log", 1), ("push", 1)]));
}

/// `bd safe-push form --check` of a branch stacked on auth: what `a_safe_push_check` runs, and the trial build of form
/// alone (`alone`) in the same trial session as the dangling references, so no second `log` or `for-each-ref`: its
/// build (`merge-tree`, `fast-import`) and the changes of the commits for the names they use (`log`). The sync of the
/// stack builds form on auth without a merge and looks for the notes of the stack (`show-ref`). The `stack_not_landed`
/// refusal before it opened a second trial session for the same build: 40 processes
#[test]
fn a_safe_push_check_of_a_stacked_branch() {
  let space = Space::new(&["(auth) Add login", "(auth/form) Add form"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  let environment = space.environment();
  let context = space_context(&environment);
  let args = SafePushArgs {
    branch: "form".to_string(),
    check: true,
    ..SafePushArgs::default()
  };
  let output = safe_push(&context, &args, &environment).unwrap();
  assert_eq!(output.lands, ["auth", "form"]);
  assert!(output.alone.as_ref().is_some_and(|alone| alone.result == "builds"), "{output:?}");
  // the alone trial reads the names the landing gate reads, in the same session
  assert_safe_push_processes(&context, &with_more(SAFE_PUSH_CHECK, &[("fast-import", 1), ("show-ref", 1)]));
}

/// `bd safe-push auth --whole-stack --check` of a stack that forks into form and side: what `a_safe_push_check` runs,
/// with the sync of three stacked branches and their stack branch (one more `fast-import` and `update-ref` each for
/// the second child and the stack branch, a remote status walk each, `rev-list`, and the notes of the stack,
/// `show-ref`), and the review of the stack branch's commits; the stack, its stack branch and its remote status come
/// from the sync's report, the text of its pull request from the descriptions the sync read: no trial build alone, no
/// process of its own; the landing gate reads the names of the commits as `a_safe_push_check` does
#[test]
fn a_safe_push_check_of_the_whole_stack() {
  let space = Space::new(&["(auth) Add login", "(auth/form) Add form", "(auth/side) Add side"]);
  space.repo.set_config("branchdeck.stacks", "true").unwrap();
  let environment = space.environment();
  let context = space_context(&environment);
  let args = SafePushArgs {
    branch: "auth".to_string(),
    check: true,
    whole_stack: true,
    ..SafePushArgs::default()
  };
  let output = safe_push(&context, &args, &environment).unwrap();
  assert_eq!(output.lands, ["auth", "form", "side"]);
  assert!(output.refusals.is_empty() && output.push_needed && output.alone.is_none(), "{output:?}");
  assert_safe_push_processes(
    &context,
    &[
      ("cat-file", 3),
      ("config", 1),
      ("diff-tree", 1),
      ("fast-import", 5),
      ("for-each-ref", 2),
      ("log", 7),
      ("merge-base", 1),
      ("merge-tree", 1),
      ("notes", 1),
      ("remote", 2),
      ("rev-list", 4),
      ("rev-parse", 1),
      ("show-ref", 2),
      ("symbolic-ref", 1),
      ("update-ref", 4),
      ("var", 1),
    ],
  );
}

/// `bd safe-push auth --whole-stack` over a stack branch someone moved back on the remote to an older build since the
/// sync (D4): the sync of a stack it built already (no build), what the start is decided on and the review of the stack
/// branch's commits, then the refused push of the stack branch, the fetch of what the remote has now and its remote
/// status, the push again over it, once, each push with the size check of what it adds (`log`, `cat-file`); the witness
/// of the engine (one `for-each-ref` before the lock, one under it, one of the pushed stack branch before the start);
/// and the target conflict check (`TARGET_CHECK`)
#[test]
fn a_safe_push_of_the_whole_stack_over_a_moved_remote() {
  let space = forking_stack();
  stack_remote_moved_back(&space);
  let environment = space.environment();
  let context = space_context(&environment);
  let args = SafePushArgs {
    branch: "auth".to_string(),
    whole_stack: true,
    ..SafePushArgs::default()
  };
  let output = safe_push(&context, &args, &environment).unwrap();
  assert!(output.pushed.len() == 1 && output.pushed[0].pushed_over && output.robot.is_some(), "{output:?}");
  assert_eq!(space.remote_tip(STACK), space.local_tip(STACK));
  assert_safe_push_processes(
    &context,
    &[
      ("cat-file", 5),
      ("config", 1),
      ("diff-tree", 1),
      ("fetch", 1),
      ("for-each-ref", 6),
      ("log", 11),
      ("merge-base", 1),
      ("push", 2),
      ("reflog", 1),
      ("remote", 2),
      ("rev-list", 2),
      ("rev-parse", 1),
      ("symbolic-ref", 1),
    ],
  );
}

/// The context of the environment of a Space fixture, after its sync
fn space_context(environment: &bd_core::context::Environment) -> RepoContext {
  let options = RepoOptions {
    repo: None,
    prefix: Some(PREFIX),
    full_ids: false,
    no_cache: false,
  };
  RepoContext::resolve(environment, &options).unwrap()
}

/// `bd robot auth` of a synced branch without a robot: the grouping (HEAD, then the baseline and the branches under the
/// prefix with their local and pushed tips in one listing, `log`, the stores) and the Space repository (`remote`,
/// `cat-file`)
#[test]
fn a_robot() {
  let space = Space::new(&["(auth) Add login"]);
  let environment = space.environment();
  sync(&space_context(&environment)).unwrap();
  let context = space_context(&environment);
  let output = robot(
    &context,
    &RobotArgs {
      branch: "auth".to_string(),
      ..RobotArgs::default()
    },
    &environment,
  )
  .unwrap();
  assert!(output.robot.is_none(), "{output:?}");
  assert_processes(
    &context,
    &[("cat-file", 1), ("config", 1), ("for-each-ref", 1), ("log", 1), ("remote", 2), ("rev-parse", 2)],
  );
}

/// `bd robot-stats`: the Space repository (`remote`), the baseline (`remote`, the `@{u}` probe, one `for-each-ref`) and
/// the branch of origin it tracks (`remote`, `symbolic-ref`, `rev-parse`, `for-each-ref`); no grouping, no sync
#[test]
fn a_robot_stats() {
  let space = Space::new(&["(auth) Add login"]);
  let environment = space.environment();
  let context = space_context(&environment);
  robot_stats(&context, &RobotStatsArgs::default(), &environment).unwrap();
  assert_processes(&context, &[("for-each-ref", 2), ("remote", 3), ("rev-parse", 2), ("symbolic-ref", 1)]);
}

/// `bd request-review auth --push --check`: the sync, which says the remote lacks auth, the plan of its push, and the
/// review of Patronus (`log`, `diff-tree`, the `OWNERSHIP` files)
#[test]
fn a_request_review_check() {
  let (_space, _github, environment) = needs_code_owner("NOT_ADDED");
  let context = space_context(&environment);
  let args = RequestReviewArgs {
    branch: "auth".to_string(),
    reviewers: Vec::new(),
    push: true,
    check: true,
  };
  let output = request_review(&context, &args, &environment).unwrap();
  assert_eq!(output.reviewers.len(), 1, "{output:?}");
  assert_processes(
    &context,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("diff-tree", 1),
      ("for-each-ref", 1),
      ("log", 5),
      ("merge-base", 1),
      ("remote", 2),
      ("rev-parse", 1),
      ("symbolic-ref", 1),
    ],
  );
}

/// `bd group --dry-run` of a plan with a description, with the prefix of the config: the current descriptions come from
/// the one `git config` of the command, the regroup's snapshot reads its own
#[test]
fn a_group_dry_run_with_a_description() {
  let repo = repo_with_branches();
  let context = context_with_the_config_prefix(&repo);
  let plan = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(plan.path(), r#"{"descriptions": {"auth": {"title": "Login"}}}"#).unwrap();
  let args = GroupArgs {
    plan: plan.path().to_path_buf(),
    rewrite: RewriteArgs {
      dry_run: true,
      ..RewriteArgs::default()
    },
  };
  let output = group(&context, &args).unwrap();
  assert_eq!(output.described, ["auth"]);
  assert_processes(
    &context,
    &[("cat-file", 1), ("config", 2), ("for-each-ref", 1), ("log", 1), ("remote", 1), ("rev-parse", 3)],
  );
}

/// `bd lease host have` is one `cat-file`, `ingest` of a small pack the config, `unpack-objects` and one ref
/// transaction, `show` one `cat-file`, and the give-back of a run that sent a pack one more ref transaction
#[test]
fn the_host_objects_have_ingest_and_show() {
  let repo = repo_with_branches();
  let path = repo.path().to_str().unwrap();
  let base = repo.head();
  let client = TestRepo::new_empty();
  client.clone_from(repo.path()).unwrap();
  let commit = client.create_commit("Client change", "client.txt", "client\n");
  let pack = client
    .git_executor()
    .cmd(&["pack-objects", "--revs", "--thin", "--stdout", "-q"], client.path_str())
    .stdin(format!("{commit}\n^{base}\n").as_bytes())
    .bytes()
    .unwrap();
  let (_home, environment) = lease_environment(&repo);
  let context = || LeaseContext::resolve(&environment, None).unwrap();

  let have = context();
  assert_eq!(git_ops::host_objects::present_commits(&have.git, path, &[base.clone(), commit.clone()]).unwrap(), [base]);
  assert_git_processes(&have.git, &[("cat-file", 1), ("rev-parse", 1)]);
  let ingest = context();
  let target = lease_pool::host_refs::HostRef::Run("run-1".to_string());
  lease_pool::host_refs::ingest(&ingest.git, &ingest.dirs, &target, &commit, &mut std::io::Cursor::new(pack), ingest.lock_timeout).unwrap();
  assert_git_processes(&ingest.git, &[("config", 1), ("rev-parse", 1), ("unpack-objects", 1), ("update-ref", 1)]);
  let show = context();
  git_ops::host_objects::show_commit(&show.git, path, &commit, Some("client.txt"), &["README.md".to_string()]).unwrap();
  assert_git_processes(&show.git, &[("cat-file", 1), ("rev-parse", 1)]);
  let deleted = context();
  assert!(
    lease_pool::host_refs::delete_run_ref(&deleted.git, &deleted.dirs, "run-1", deleted.lock_timeout)
      .unwrap()
      .is_some()
  );
  assert_git_processes(&deleted.git, &[("rev-parse", 1), ("update-ref", 1)]);
  let none = context();
  assert_eq!(lease_pool::host_refs::delete_run_ref(&none.git, &none.dirs, "run-1", none.lock_timeout).unwrap(), None);
  assert_git_processes(&none.git, &[("rev-parse", 1)]);
}

/// `bd host snapshot` of a clean checkout: HEAD, the config, one status and the candidates (one `rev-list`, one
/// `for-each-ref`, a `merge-base` per base ref: the fixture has `master` and `origin/master`); of a changed one with
/// --working-tree the blobs, the commit and the delete of its temporary ref, and the file of --blob from one
/// `cat-file`; `bd host pack` the pack and its count
#[test]
fn the_snapshot_and_the_pack_of_a_client() {
  use bd_lease::host_client_command::{HostPackArgs, HostSnapshotArgs, host_pack, host_snapshot};
  let repo = repo_with_branches();
  let base = repo.head();
  let (_home, environment) = lease_environment(&repo);
  let context = || LeaseContext::resolve(&environment, None).unwrap();

  let clean = context();
  let snapshot = host_snapshot(
    &clean,
    &HostSnapshotArgs {
      farthest: 16,
      ..HostSnapshotArgs::default()
    },
  )
  .unwrap();
  assert_eq!(snapshot.commit, base);
  assert_git_processes(
    &clean.git,
    &[
      ("config", 1),
      ("for-each-ref", 1),
      ("log", 1),
      ("merge-base", 2),
      ("rev-list", 1),
      ("rev-parse", 1),
      ("status", 1),
    ],
  );

  std::fs::write(repo.path().join("README.md"), "changed\n").unwrap();
  let changed = context();
  let args = HostSnapshotArgs {
    working_tree: true,
    blob: Some("README.md".to_string()),
    farthest: 16,
    ..HostSnapshotArgs::default()
  };
  let snapshot = host_snapshot(&changed, &args).unwrap();
  assert_ne!(snapshot.commit, base);
  assert_git_processes(
    &changed.git,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("fast-import", 1),
      ("for-each-ref", 1),
      ("hash-object", 1),
      ("log", 1),
      ("merge-base", 2),
      ("rev-list", 1),
      ("rev-parse", 1),
      ("status", 1),
      ("update-ref", 1),
    ],
  );

  let pack = context();
  host_pack(
    &pack,
    &HostPackArgs {
      commit: snapshot.commit,
      have: vec![base],
    },
  )
  .unwrap();
  assert_git_processes(&pack.git, &[("pack-objects", 1), ("rev-list", 1), ("rev-parse", 1)]);
}
