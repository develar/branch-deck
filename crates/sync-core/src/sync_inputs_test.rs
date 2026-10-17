use crate::sync::{SyncOptions, sync_branches};
use crate::sync_inputs::sync_inputs_fingerprint;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_test_utils::TestReporter;
use test_utils::git_test_utils::TestRepo;

fn fingerprint(git: &GitCommandExecutor, repo: &TestRepo) -> String {
  sync_inputs_fingerprint(git, repo.path().to_str().unwrap(), "test").unwrap()
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

  // `bd describe` writes the config; the stack parents are a ref of their own
  git(&executor, &repo, &["config", "branch.test/virtual/auth.description", "Login and logout"]);
  let described = fingerprint(&executor, &repo);
  assert_ne!(described, pushed);
  git(&executor, &repo, &["update-ref", "refs/branch-deck/stack-parents", &head]);
  assert_ne!(fingerprint(&executor, &repo), described);

  // another prefix reads other branches
  assert_ne!(sync_inputs_fingerprint(&executor, repo.path().to_str().unwrap(), "other")?, fingerprint(&executor, &repo));
  Ok(())
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
    counting.process_stats().unwrap().by_command().values().map(|stats| stats.count).sum::<u32>()
  };
  assert_eq!(processes(&repo), 3);
  git(&executor, &repo, &["config", "branch.master.merge", "refs/heads/master"]);
  git(&executor, &repo, &["update-ref", "refs/remotes/origin/master", &initial]);
  assert_eq!(processes(&repo), 2);
}
