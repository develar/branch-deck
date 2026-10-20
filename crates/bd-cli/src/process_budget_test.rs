//! The git processes of the `bd` commands that add work around their core, by subcommand, as
//! `sync_core::process_budget_test` counts the cores: a change that runs fewer processes lowers the numbers here.
//! A command runs on the counting executor of its `RepoContext`, so its own reads count with those of the core.

use crate::fixture_test::{PREFIX, commit_subjects, context_with_the_config_prefix, environment, repo_with_baseline};
use crate::remote_fixture_test::{Space, needs_code_owner, repo_with_remote};
use bd_core::context::{RepoContext, RepoOptions};
use bd_core::sync_command::sync;
// jscpd:ignore-start: the budgets run the commands that lib.rs dispatches, so they import the same ones
use bd_lease::lease_command::{LeaseContext, LeaseGetArgs, LeaseReturnArgs, lease_get, lease_return};
use bd_remote::push_command::{PushArgs, push};
use bd_remote::request_review_command::{RequestReviewArgs, request_review};
use bd_remote::robot_command::{RobotArgs, robot};
use bd_remote::safe_push_command::{SafePushArgs, safe_push};
use bd_remote::status::{StatusArgs, status};
use bd_rewrite::check_command::{CheckArgs, check};
// jscpd:ignore-end
use bd_rewrite::fold_command::{FoldArgs, fold};
use bd_rewrite::move_hunk_command::{MoveHunkArgs, move_hunk};
use bd_rewrite::reword_command::{RewordArgs, reword};
use bd_rewrite::rewrite::{GroupArgs, RewriteArgs, group};
use bd_update::pick_command::{PickArgs, PickOutput, pick};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use sync_core::sync::{SyncOptions, sync_branches};
use sync_types::{ProgressReporter, SyncEvent};
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
  ("config", 2),
  ("fast-import", 2),
  ("for-each-ref", 2),
  ("log", 2),
  ("remote", 1),
  ("rev-parse", 5),
];

/// The processes of `bd check` of the current grouping of `repo_with_branches`, with an empty object cache: one
/// `cat-file` reads the blobs of the commits' blocks
const CHECK: [(&str, u32); 8] = [
  ("cat-file", 2),
  ("config", 2),
  ("fast-import", 2),
  ("for-each-ref", 2),
  ("log", 4),
  ("merge-tree", 2),
  ("remote", 1),
  ("rev-parse", 5),
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
/// (`sync_branches` of the same repository) and the `rev-parse` of the context, nothing else. The lock is taken in the
/// common git directory the context resolved; the one `git config` of the context reads the prefix, and the sync takes
/// the settings and the descriptions from it
#[test]
fn a_sync_with_the_prefix_of_the_config() {
  let core = GitCommandExecutor::new().with_process_stats("core");
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
  let core_rev_parse = core.iter().find(|(subcommand, _)| *subcommand == "rev-parse").map_or(0, |(_, count)| *count);

  let repo = repo_with_branches();
  let context = context_with_the_config_prefix(&repo);
  let output = sync(&context).unwrap();
  assert_eq!(output.branches.len(), 2);
  assert_processes(&context, &changed(&core, &[("rev-parse", core_rev_parse + 1)]));
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
/// push URL of `origin` (`remote`, for the Space and the GitHub repository) and the Space settings (`cat-file`, for the
/// GitHub mirror `--repo` names: the remote is no GitHub URL)
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
  };
  push(&context, &args, &environment(repo.path())).unwrap();
  assert_processes(
    &context,
    &[
      ("cat-file", 2),
      ("config", 1),
      ("fast-import", 3),
      ("for-each-ref", 1),
      ("log", 2),
      ("merge-base", 1),
      ("merge-tree", 1),
      ("notes", 1),
      ("push", 1),
      ("remote", 2),
      ("rev-list", 2),
      ("rev-parse", 3),
      ("show-ref", 1),
      ("symbolic-ref", 1),
      ("update-ref", 2),
      ("var", 1),
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
      ("rev-parse", 10),
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
      ("rev-parse", 7),
      ("symbolic-ref", 1),
      ("update-ref", 1),
    ],
  );
}

/// `bd move-hunk` of a file change from the newest commit into the oldest, over `between` commits that don't touch the
/// file: its processes by subcommand
fn move_hunk_processes(between: usize) -> Vec<(String, u32)> {
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
    .git
    .process_stats()
    .unwrap()
    .by_command()
    .into_iter()
    .map(|(subcommand, stats)| (subcommand, stats.count))
    .collect()
}

/// `bd move-hunk`: a fixed number of processes, whatever the number of commits between the two
#[test]
fn a_move_hunk() {
  assert_eq!(move_hunk_processes(1), move_hunk_processes(6));
}

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

/// `bd pick --from <worktree> --follow`: the lock, the marks of both worktrees, the baseline, the check (HEAD with its
/// committer date, the range with the patch id marks, one `replay`, the copies, both dry runs), the grouping of the
/// picked commits before the move (the inputs and one `log` of the commits ahead: a new prefix needs no diffs), and the
/// two moves with their marks (`update-ref`, `read-tree`)
#[test]
fn a_pick() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let lane = TestWorktree::detached(&repo);
  lane.commit("(lane) Add a", "a.txt", "a\n");
  lane.commit("(lane) Add b", "b.txt", "b\n");
  commit_subjects(&repo, &["(own) Add c"]);
  let (context, output) = pick_from(&repo, lane, |args| args.follow = true);
  assert_eq!(
    (output.picked.len(), output.followed.as_ref().map(|followed| followed.state)),
    (2, Some(bd_update::pick_command::FollowedState::Moved))
  );
  assert_processes(
    &context,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("diff-tree", 2),
      ("for-each-ref", 2),
      ("log", 4),
      ("merge-base", 3),
      ("read-tree", 4),
      ("remote", 2),
      ("replay", 1),
      ("rev-list", 1),
      ("rev-parse", 17),
      ("symbolic-ref", 7),
      ("update-index", 6),
      ("update-ref", 4),
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
/// `status` and one `checkout` of the slot; the return reads the unlanded commits, the untracked mode of the config,
/// the `status` and HEAD. No `status` of another slot, no full checkout
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
  assert_git_processes(&context.git, &[("checkout", 1), ("config", 1), ("rev-parse", 2), ("status", 1)]);
  let context = LeaseContext::resolve(&environment, Some(&slot)).unwrap();
  lease_return(&context, &LeaseReturnArgs::default()).unwrap();
  assert_git_processes(&context.git, &[("config", 1), ("log", 1), ("rev-parse", 2), ("status", 1)]);
}

/// `bd lease get` of a new slot by an APFS clone of the source (macOS): the repository, the source HEAD and the pool
/// config, `worktree prune` and `worktree add --no-checkout`, the refresh of the copied index, one `checkout` of the
/// diff, `clean -fd` and one `status`
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
      ("for-each-ref", 2),
      ("log", 4),
      ("merge-base", 3),
      ("read-tree", 1),
      ("remote", 2),
      ("rev-list", 1),
      ("rev-parse", 8),
      ("symbolic-ref", 2),
      ("update-index", 1),
    ],
  );
}

/// The processes of the target conflict check of `bd safe-push`: before the lock, the fetch of the target reads the
/// baseline and its upstream (`remote`, `for-each-ref`, `symbolic-ref`, `rev-parse`), then fetches it (`fetch`); after
/// the sync, one `replay` of the branch onto the target tip that the sync read
const TARGET_CHECK: &[(&str, u32)] = &[("fetch", 1), ("for-each-ref", 2), ("remote", 2), ("replay", 1), ("rev-parse", 1), ("symbolic-ref", 1)];

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
/// read the commits with their new mapping notes (`log`) and the branch tips (`for-each-ref`) for the trial; the review
/// (`log`, `diff-tree`) reads its own; and the target conflict check (`TARGET_CHECK`)
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
  assert_safe_push_processes(
    &context,
    &[
      ("cat-file", 2),
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
      ("rev-parse", 4),
      ("show-ref", 1),
      ("symbolic-ref", 1),
      ("update-ref", 2),
      ("var", 1),
    ],
  );
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
  assert_safe_push_processes(
    &context,
    &[
      ("cat-file", 3),
      ("config", 1),
      ("diff-tree", 1),
      ("fast-import", 4),
      ("for-each-ref", 2),
      ("log", 8),
      ("merge-base", 1),
      ("merge-tree", 1),
      ("notes", 1),
      ("remote", 2),
      ("rev-list", 2),
      ("rev-parse", 4),
      ("show-ref", 2),
      ("symbolic-ref", 1),
      ("update-ref", 2),
      ("var", 1),
    ],
  );
}

/// `bd safe-push auth --whole-stack --check` of a stack that forks into form and side: what `a_safe_push_check` runs,
/// with the sync of three stacked branches and their stack branch (one more `fast-import` and `update-ref` each for
/// the second child and the stack branch, a remote status walk each, `rev-list`, and the notes of the stack,
/// `show-ref`), and the review of the stack branch's commits; the stack, its stack branch and its remote status come
/// from the sync's report, the text of its pull request from the descriptions the sync read: no trial build alone, no
/// process of its own
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
      ("cat-file", 2),
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
      ("rev-parse", 4),
      ("show-ref", 2),
      ("symbolic-ref", 1),
      ("update-ref", 4),
      ("var", 1),
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

/// `bd request-review auth --check`: the grouping, whose listing names the upstream the target is, the tip, and the
/// review of Patronus (`log`, `diff-tree`, the `OWNERSHIP` files)
#[test]
fn a_request_review_check() {
  let (_space, _github, environment) = needs_code_owner("NOT_ADDED");
  let context = space_context(&environment);
  let args = RequestReviewArgs {
    branch: "auth".to_string(),
    reviewers: Vec::new(),
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
      ("log", 2),
      ("remote", 2),
      ("rev-parse", 3),
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
    &[("cat-file", 1), ("config", 2), ("for-each-ref", 1), ("log", 1), ("remote", 1), ("rev-parse", 4)],
  );
}
