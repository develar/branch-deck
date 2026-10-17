//! The git processes of the `bd` commands that add work around their core, by subcommand, as
//! `sync_core::process_budget_test` counts the cores: a change that runs fewer processes lowers the numbers here.
//! A command runs on the counting executor of its `RepoContext`, so its own reads count with those of the core.

use crate::check_command::{CheckArgs, check};
use crate::context::{RepoContext, RepoOptions};
use crate::fixture_test::{PREFIX, commit_subjects, environment, repo_with_baseline};
use crate::fold_command::{FoldArgs, fold};
use crate::push_command::{PushArgs, push};
use crate::push_test::repo_with_remote;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_utils::git_test_utils::TestRepo;

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

/// `bd check` of `build` with `build_paths`, an empty plan file standing for no plan
fn run_check(repo: &TestRepo, build: Option<&str>, build_paths: &[&str]) -> RepoContext {
  let plan = tempfile::NamedTempFile::new().unwrap();
  let context = context(repo);
  let args = CheckArgs {
    plan: Some(plan.path().to_path_buf()),
    build: build.map(str::to_string),
    build_paths: build_paths.iter().map(|path| (*path).to_string()).collect(),
    fix: false,
  };
  let output = check(&context, &args).unwrap();
  assert!(output.ok, "{output:?}");
  context
}

/// The processes of `bd check` of the current grouping of `repo_with_branches`, with an empty object cache
const CHECK: [(&str, u32); 9] = [
  ("cat-file", 1),
  ("config", 2),
  ("fast-import", 2),
  ("for-each-ref", 2),
  ("log", 4),
  ("merge-tree", 2),
  ("remote", 1),
  ("rev-parse", 5),
  ("symbolic-ref", 1),
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
  let context = run_check(&repo, Some("true"), &["f0.txt", "f1.txt"]);
  assert_processes(&context, &changed(&CHECK, &[("archive", 2), ("cat-file", 2)]));
}

/// `bd push --dry-run auth` to a local bare remote: the sync, then the push it asks the remote about
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
      ("cat-file", 3),
      ("config", 5),
      ("fast-import", 3),
      ("for-each-ref", 5),
      ("log", 5),
      ("merge-base", 1),
      ("merge-tree", 1),
      ("notes", 1),
      ("push", 1),
      ("remote", 4),
      ("rev-list", 2),
      ("rev-parse", 6),
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
