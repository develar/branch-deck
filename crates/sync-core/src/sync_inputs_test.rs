use crate::sync::{SyncOptions, sync_branches};
use crate::sync_inputs::{SyncInputs, read_sync_inputs};
use git_executor::git_command_executor::GitCommandExecutor;
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

fn fingerprint(git: &GitCommandExecutor, repo: &TestRepo) -> SyncInputs {
  read_sync_inputs(git, repo.path().to_str().unwrap(), "test").unwrap()
}

fn git(git: &GitCommandExecutor, repo: &TestRepo, args: &[&str]) -> String {
  git.execute_command(args, repo.path().to_str().unwrap()).unwrap()
}

#[tokio::test]
async fn changes_with_what_a_sync_reads_and_not_with_what_it_writes() -> anyhow::Result<()> {
  let repo = TestRepo::new();
  let executor = GitCommandExecutor::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.create_commit("(auth) Add login", "login.txt", "login");
  let before = fingerprint(&executor, &repo);
  assert_eq!(fingerprint(&executor, &repo), before, "the same inputs, the same fingerprint");

  // the sync writes the virtual branches and their notes: not inputs
  sync_branches(&executor, repo.path().to_str().unwrap(), "test", TestReporter::new(), SyncOptions::default()).await?;
  assert!(repo.branch_exists("test/virtual/auth"));
  assert_eq!(fingerprint(&executor, &repo), before, "a sync changes no input");

  // a review fetch writes its own refs: not an input either
  let head = git(&executor, &repo, &["rev-parse", "HEAD"]);
  git(&executor, &repo, &["update-ref", "refs/branch-deck/review/7/head", &head]);
  assert_eq!(fingerprint(&executor, &repo), before);

  // a commit in the IDE
  repo.create_commit("(auth) Add logout", "logout.txt", "logout");
  let committed = fingerprint(&executor, &repo);
  assert_ne!(committed, before);

  // a fetch elsewhere moves the baseline; a push elsewhere the remote virtual branch
  git(&executor, &repo, &["update-ref", "refs/remotes/origin/master", &head]);
  let fetched = fingerprint(&executor, &repo);
  assert_ne!(fetched, committed);
  git(&executor, &repo, &["update-ref", "refs/remotes/origin/test/virtual/auth", &head]);
  let pushed = fingerprint(&executor, &repo);
  assert_ne!(pushed, fetched);
  // the remote branches of the prefix are listed apart: what a push of this clone moved is replaced without git
  assert_eq!(pushed.rest, fetched.rest);
  let mut known = fetched;
  known.set_remote_ref("refs/remotes/origin/test/virtual/auth".to_string(), Some(head.clone()));
  assert_eq!(known, pushed);
  // `bd safe-push` of a whole stack pushes only its stack branch, `bd push --unapplied` an unapplied branch
  git(&executor, &repo, &["update-ref", "refs/remotes/origin/test/stack/auth", &head]);
  let stack_pushed = fingerprint(&executor, &repo);
  assert_ne!(stack_pushed, pushed);
  git(&executor, &repo, &["update-ref", "refs/remotes/origin/test/unapplied/ui", &head]);
  let unapplied_pushed = fingerprint(&executor, &repo);
  assert_ne!(unapplied_pushed, stack_pushed);
  // another prefix is no input
  git(&executor, &repo, &["update-ref", "refs/remotes/origin/testing/virtual/auth", &head]);
  assert_eq!(fingerprint(&executor, &repo), unapplied_pushed);
  let pushed = unapplied_pushed;

  // `bd describe` writes the config; the stack parents and the parked branches are refs of their own
  git(&executor, &repo, &["config", "branch.test/virtual/auth.description", "Login and logout"]);
  let described = fingerprint(&executor, &repo);
  assert_ne!(described, pushed);
  git(&executor, &repo, &["update-ref", "refs/branch-deck/stack-parents", &head]);
  let stacked = fingerprint(&executor, &repo);
  assert_ne!(stacked, described);
  // a park in `bd` syncs the window on its next focus
  git(&executor, &repo, &["update-ref", "refs/branch-deck/parked", &head]);
  assert_ne!(fingerprint(&executor, &repo), stacked);

  // another prefix reads other branches
  assert_ne!(read_sync_inputs(&executor, repo.path().to_str().unwrap(), "other")?, fingerprint(&executor, &repo));
  Ok(())
}

/// The app resolves the prefix per command (`BranchPrefixes`): a changed prefix (the app's, or the repository's own) is
/// a changed fingerprint even where neither prefix names a branch yet, so the next focus syncs under the new one
#[test]
fn changes_with_the_prefix_alone() {
  let repo = TestRepo::new();
  let executor = GitCommandExecutor::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  let path = repo.path().to_str().unwrap();
  let jane = read_sync_inputs(&executor, path, "jane").unwrap();
  assert_eq!(read_sync_inputs(&executor, path, "jane").unwrap(), jane);
  assert_ne!(read_sync_inputs(&executor, path, "john").unwrap(), jane);
}

/// The baseline of a branch that tracks another remote branch than `master` or `main` is that upstream (`@{u}`): a
/// fetch of it changes what a sync reads
#[test]
fn changes_with_the_upstream_of_the_checked_out_branch() {
  let repo = TestRepo::new();
  let executor = GitCommandExecutor::new();
  let initial = repo.create_commit("Initial commit", "README.md", "# Test");
  let head = repo.create_commit("(auth) Add login", "login.txt", "login");
  git(&executor, &repo, &["remote", "add", "origin", "."]);
  git(&executor, &repo, &["update-ref", "refs/remotes/origin/develop", &initial]);
  git(&executor, &repo, &["config", "branch.master.remote", "origin"]);
  git(&executor, &repo, &["config", "branch.master.merge", "refs/heads/develop"]);
  assert_eq!(git(&executor, &repo, &["rev-parse", "--abbrev-ref", "@{u}"]), "origin/develop");
  let before = fingerprint(&executor, &repo);

  // a fetch elsewhere
  git(&executor, &repo, &["update-ref", "refs/remotes/origin/develop", &head]);
  assert_ne!(fingerprint(&executor, &repo), before);

  // one process more for that upstream only: the `master` and `main` of a remote are listed already
  let processes = |repo: &TestRepo| {
    let counting = GitCommandExecutor::new().with_process_stats("test");
    fingerprint(&counting, repo);
    counting.process_stats().unwrap().total()
  };
  assert_eq!(processes(&repo), 3);
  git(&executor, &repo, &["config", "branch.master.merge", "refs/heads/master"]);
  git(&executor, &repo, &["update-ref", "refs/remotes/origin/master", &initial]);
  assert_eq!(processes(&repo), 2);
}

/// A sync git deferred (a rebase in progress, a detached HEAD) syncs once git finished: the end of an operation is a
/// change also when it moves no ref (an abort), and so is a branch checked out again
#[test]
fn changes_when_a_git_operation_starts_or_ends() {
  let repo = TestRepo::new();
  let executor = GitCommandExecutor::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  let head = repo.create_commit("(auth) Add login", "login.txt", "login");
  let before = fingerprint(&executor, &repo);

  let rebase = repo.path().join(".git/rebase-merge");
  std::fs::create_dir_all(&rebase).unwrap();
  let rebasing = fingerprint(&executor, &repo);
  assert_ne!(rebasing, before, "a rebase started");
  sync_utils::fs_retry::remove_dir(&rebase).unwrap();
  assert_eq!(fingerprint(&executor, &repo), before, "the rebase aborted: as before it");

  git(&executor, &repo, &["update-ref", "--no-deref", "HEAD", &head]);
  assert_ne!(fingerprint(&executor, &repo), before, "HEAD detached");
  git(&executor, &repo, &["symbolic-ref", "HEAD", "refs/heads/master"]);
  assert_eq!(fingerprint(&executor, &repo), before, "the branch checked out again");
}
