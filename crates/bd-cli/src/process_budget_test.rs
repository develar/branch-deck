//! The git processes of the `bd` commands that add work around their core, by subcommand, as
//! `sync_core::process_budget_test` counts the cores: a change that runs fewer processes lowers the numbers here.
//! A command runs on the counting executor of its `RepoContext`, so its own reads count with those of the core.

use crate::check_command::{CheckArgs, check};
use crate::context::{RepoContext, RepoOptions};
use crate::fixture_test::{PREFIX, commit_subjects, context_with_the_config_prefix, environment, repo_with_baseline};
use crate::fold_command::{FoldArgs, fold};
use crate::pick_command::{PickArgs, pick};
use crate::push_command::{PushArgs, push};
use crate::push_test::repo_with_remote;
use crate::request_review_command::{RequestReviewArgs, request_review};
use crate::request_review_test::needs_code_owner;
use crate::rewrite::{GroupArgs, RewriteArgs, group};
use crate::robot_command::{RobotArgs, robot};
use crate::safe_push_command::{SafePushArgs, safe_push};
use crate::safe_push_test::Space;
use crate::status::{StatusArgs, status};
use crate::sync_command::sync;
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
  let counted: Vec<(String, u32)> = context
    .git
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

/// `bd check` of `build` with `build_paths`, an empty plan file standing for no plan
fn run_check(repo: &TestRepo, build: Option<&str>, build_paths: &[&str]) -> RepoContext {
  let plan = tempfile::NamedTempFile::new().unwrap();
  let context = context(repo);
  let args = CheckArgs {
    plan: Some(plan.path().to_path_buf()),
    build: build.map(str::to_string),
    build_paths: build_paths.iter().map(|path| (*path).to_string()).collect(),
    rebuild: false,
    fix: false,
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

/// The processes of `bd check` of the current grouping of `repo_with_branches`, with an empty object cache
const CHECK: [(&str, u32); 8] = [
  ("cat-file", 1),
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
  assert_processes(&context, &changed(&CHECK, &[("archive", 2), ("cat-file", 2)]));
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

/// `bd status` of the synced branches: HEAD, the grouping, whose one listing has the virtual tips too, and the stack
/// setting and the descriptions from the one `git config` of the command
#[test]
fn a_status() {
  let repo = repo_with_branches();
  sync(&context(&repo)).unwrap();
  let context = context(&repo);
  let output = status(&context, &StatusArgs::default()).unwrap();
  assert_eq!(output.branch_count, 2);
  assert_processes(
    &context,
    &[("cat-file", 1), ("config", 1), ("for-each-ref", 1), ("log", 1), ("remote", 1), ("rev-parse", 2)],
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
  let output = status(&context, &StatusArgs::default()).unwrap();
  assert_eq!(output.stacks.len(), 1);
  assert_processes(
    &context,
    &[("cat-file", 1), ("config", 1), ("for-each-ref", 1), ("log", 1), ("remote", 1), ("rev-parse", 2)],
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

/// `bd pick --from <worktree> --follow`: the lock, the marks of both worktrees, the baseline, the check (the range, one
/// `replay`, the copies, both dry runs), the two moves with their marks (`update-ref`, `read-tree`), and the grouping
/// of the picked commits
#[test]
fn a_pick() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let lane = TestWorktree::detached(&repo);
  lane.commit("(lane) Add a", "a.txt", "a\n");
  lane.commit("(lane) Add b", "b.txt", "b\n");
  commit_subjects(&repo, &["(own) Add c"]);
  let context = context(&repo);
  let args = PickArgs {
    from: Some(lane.path.into()),
    follow: true,
    ..PickArgs::default()
  };
  let output = pick(&context, &args, &environment(repo.path())).unwrap();
  assert_eq!((output.picked.len(), output.followed.as_ref().map(|followed| followed.state.as_str())), (2, Some("moved")));
  assert_processes(
    &context,
    &[
      ("cat-file", 1),
      ("config", 1),
      ("diff-tree", 2),
      ("for-each-ref", 2),
      ("log", 2),
      ("merge-base", 3),
      ("read-tree", 4),
      ("remote", 2),
      ("replay", 1),
      ("rev-list", 1),
      ("rev-parse", 18),
      ("symbolic-ref", 7),
      ("update-index", 6),
      ("update-ref", 4),
    ],
  );
}

/// `bd safe-push auth --check` on Space: the sync, then what the start is decided on, nothing pushed. The target, the
/// stack and the tips come from the sync's report, the Space and the GitHub repository from one push URL of `origin`
/// (`remote`) and the Space settings (`cat-file`); the dangling references take the grouping from the report too and
/// read the commits with their new mapping notes (`log`) and the branch tips (`for-each-ref`) for the trial; the review
/// (`log`, `diff-tree`) reads its own
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
  assert_processes(
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
  assert_processes(
    &context,
    &[
      ("cat-file", 2),
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
  assert_processes(
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
fn space_context(environment: &crate::context::Environment) -> RepoContext {
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
    plan: Some(plan.path().to_path_buf()),
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
