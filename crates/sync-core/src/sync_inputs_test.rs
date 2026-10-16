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
