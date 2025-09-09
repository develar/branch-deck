use super::test_helpers::{setup_test_repos, sync_branches_core_with_strategy};
use branch_integration::strategy::DetectionStrategy;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use sync_core::remote_status::compute_remote_status_for_branch;
use sync_test_utils::TestReporter;
use sync_types::SyncEvent;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// Result of extracting remote status from sync events
#[derive(Debug)]
struct RemoteStatusResult {
  pub my_commits_count: u32,
  pub total_commits_count: u32,
  pub commits_behind: u32,
  pub remote_exists: bool,
}

/// Helper function to extract remote status for a specific branch from sync events
fn extract_remote_status_for_branch(events: &[SyncEvent], branch_name: &str) -> Option<RemoteStatusResult> {
  for event in events {
    if let SyncEvent::RemoteStatusUpdate(status_update) = event {
      if status_update.branch_name == branch_name {
        return Some(RemoteStatusResult {
          my_commits_count: status_update.my_unpushed_count,
          total_commits_count: status_update.unpushed_commits.len() as u32,
          commits_behind: status_update.commits_behind,
          remote_exists: status_update.remote_exists,
        });
      }
    }
  }
  None
}

/// Test scenario builder for remote status tests
struct TestScenario {
  upstream_repo: TestRepo,
  local_repo: TestRepo,
  git_executor: GitCommandExecutor,
  user_email: String,
  user_name: String,
}

impl TestScenario {
  fn new() -> Self {
    let (upstream_repo, local_repo, git_executor) = setup_test_repos();
    Self {
      upstream_repo,
      local_repo,
      git_executor,
      user_email: "user@example.com".to_string(),
      user_name: "Test User".to_string(),
    }
  }

  fn with_author(mut self, email: &str, name: &str) -> Self {
    self.user_email = email.to_string();
    self.user_name = name.to_string();
    self
  }

  fn setup_development_branch(&self) -> &Self {
    self.local_repo.checkout_new_branch("development").unwrap();
    self.configure_author();
    self
  }

  fn configure_author(&self) {
    self.local_repo.set_config("user.email", &self.user_email).unwrap();
    self.local_repo.set_config("user.name", &self.user_name).unwrap();
  }

  fn create_commits(&self, commits: &[(String, &str, &str)]) -> Vec<String> {
    commits
      .iter()
      .map(|(subject, file, content)| self.local_repo.create_commit(subject, file, content))
      .collect()
  }

  fn simulate_commit_merge_to_master(&self, commit_hash: &str) {
    self.local_repo.checkout("main").unwrap();
    self.local_repo.cherry_pick(commit_hash).unwrap();
    self.local_repo.push("origin", "main").unwrap();
  }

  async fn run_sync(&self, branch_prefix: &str) -> TestReporter {
    let progress_reporter = TestReporter::new();
    let result = sync_branches_core_with_strategy(
      &self.git_executor,
      self.local_repo.path().to_str().unwrap(),
      branch_prefix,
      progress_reporter.clone(),
      DetectionStrategy::Rebase,
    )
    .await;
    assert!(result.is_ok(), "Sync should succeed");
    progress_reporter
  }
}

/// Helper function to configure test author for a repository
fn configure_test_author(repo: &TestRepo, email: &str, name: &str) {
  repo.set_config("user.email", email).unwrap();
  repo.set_config("user.name", name).unwrap();
}

/// Helper function to print remote status for debugging
fn print_remote_status(branch_name: &str, status: &RemoteStatusResult) {
  println!("Remote status for {}:", branch_name);
  println!("  My commits: {}", status.my_commits_count);
  println!("  Total commits: {}", status.total_commits_count);
  println!("  Commits behind: {}", status.commits_behind);
  println!("  Remote exists: {}", status.remote_exists);
}

/// Helper function to assert remote status with detailed messaging
fn assert_remote_status(events: &[SyncEvent], branch_name: &str, expected_my_commits: u32, expected_total_commits: u32, expected_commits_behind: u32, test_description: &str) {
  let status = extract_remote_status_for_branch(events, branch_name).expect(&format!("Should have received remote status update for {}", branch_name));

  print_remote_status(branch_name, &status);
  println!("Test: {}", test_description);

  assert_eq!(
    status.my_commits_count, expected_my_commits,
    "Should have {} my commits for {}: {}",
    expected_my_commits, branch_name, test_description
  );
  assert_eq!(
    status.total_commits_count, expected_total_commits,
    "Should have {} total commits for {}: {}",
    expected_total_commits, branch_name, test_description
  );
  assert_eq!(
    status.commits_behind, expected_commits_behind,
    "Should have {} commits behind for {}: {}",
    expected_commits_behind, branch_name, test_description
  );
}

/// Test that reproduces the "my commits" count bug where commits already in master
/// are incorrectly counted as "my commits to push" in virtual branches.
#[test(tokio::test)]
async fn test_my_commits_count_excludes_baseline_commits() {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();

  // Create two feature commits
  let commits = scenario.create_commits(&[
    ("(feature-a) Implement feature A".to_string(), "feature_a.rs", "// Feature A implementation"),
    ("(other-feature) Fix important bug".to_string(), "bug_fix.rs", "// Important bug fix"),
  ]);

  // Push feature branch to remote first
  scenario.local_repo.checkout("main").unwrap();
  scenario.local_repo.create_branch_at("user/virtual/feature-a", &commits[0]).unwrap();
  scenario.local_repo.push("origin", "user/virtual/feature-a").unwrap();

  // Simulate that the bug fix (commit B) was merged to master
  scenario.simulate_commit_merge_to_master(&commits[1]);

  // Get old master hash (before bug fix was merged)
  let old_master = scenario
    .git_executor
    .execute_command(&["rev-parse", "HEAD~1"], scenario.local_repo.path().to_str().unwrap())
    .unwrap()
    .trim()
    .to_string();

  // Setup remote branch based on OLD master with the feature-a commit
  scenario.upstream_repo.checkout("user/virtual/feature-a").unwrap();
  scenario.upstream_repo.reset_hard(&old_master).unwrap();
  scenario.upstream_repo.cherry_pick(&commits[0]).unwrap();

  // Switch back to development branch before adding local commit
  scenario.local_repo.checkout("development").unwrap();

  // Add one more local commit
  let _commit_c = scenario
    .local_repo
    .create_commit("(feature-a) Add more to feature A", "feature_a_more.rs", "// More feature A stuff");

  scenario.local_repo.fetch_prune("origin").unwrap();

  // Run sync and verify
  let progress_reporter = scenario.run_sync("user").await;
  let events = progress_reporter.get_events();

  assert_remote_status(
    &events,
    "feature-a",
    1,
    2,
    0,
    "commit_a is patch-equivalent to remote, commit_c is new and needs to be pushed, commit_b is in baseline",
  );
}

/// Test that reproduces the exact "my commits" count bug from the io-mockk-update scenario.
#[test(tokio::test)]
async fn test_real_io_mockk_update_bug_scenario() {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();

  // Create commits on the feature branch (simulating io-mockk-update)
  let commits = scenario.create_commits(&[
    (
      "(io-mockk-update) update MockK to version 1.14.5".to_string(),
      "mockk_update.kt",
      "// MockK version 1.14.5 update",
    ),
    ("(io-mockk-update) fix test compatibility".to_string(), "test_fix.kt", "// Fix tests for new MockK version"),
  ]);

  // Create and push the virtual branch
  scenario.local_repo.checkout("main").unwrap();
  let main_base = scenario.local_repo.head();
  scenario.local_repo.create_branch_at("user/virtual/io-mockk-update", &commits[1]).unwrap();
  scenario.local_repo.push("origin", "user/virtual/io-mockk-update").unwrap();

  // Simulate that commit_a gets merged to master
  scenario.simulate_commit_merge_to_master(&commits[0]);

  // Setup remote branch based on OLD master with both commits
  scenario.upstream_repo.checkout("user/virtual/io-mockk-update").unwrap();
  scenario.upstream_repo.reset_hard(&main_base).unwrap();
  scenario.upstream_repo.cherry_pick(&commits[0]).unwrap();
  scenario.upstream_repo.cherry_pick(&commits[1]).unwrap();

  scenario.local_repo.fetch_prune("origin").unwrap();
  scenario.local_repo.checkout("development").unwrap();

  // Run sync and verify
  let progress_reporter = scenario.run_sync("user").await;
  let events = progress_reporter.get_events();

  assert_remote_status(
    &events,
    "io-mockk-update",
    0,
    2,
    0,
    "Both commits are patch-equivalent to remote after rebase onto master with commit_a",
  );
}

/// Direct test of compute_remote_status_for_branch function
#[test(tokio::test)]
async fn test_compute_remote_status_directly() {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();

  // Create and push a test commit
  scenario.create_commits(&[("(test) Test commit".to_string(), "test.rs", "// Test")]);
  let latest_commit = scenario.local_repo.head();
  scenario.local_repo.create_branch_at("user/virtual/test", &latest_commit).unwrap();
  scenario.local_repo.push("origin", "user/virtual/test").unwrap();

  // Test direct function call
  let result = compute_remote_status_for_branch(
    &scenario.git_executor,
    scenario.local_repo.path().to_str().unwrap(),
    "user/virtual/test",
    "test",
    Some(&scenario.user_email),
    1,             // total_commits_in_branch
    "origin/main", // baseline_branch
  )
  .unwrap();

  // Since we just pushed, should be up to date
  assert_eq!(result.unpushed_commits.len(), 0, "Should be up to date after push");
  assert_eq!(result.my_unpushed_count, 0, "Should be up to date after push");
  assert_eq!(result.commits_behind, 0, "Should be up to date after push");
}

/// Test scenario where a NEW commit is added after baseline merge.
#[test(tokio::test)]
async fn test_new_commit_after_baseline_merge() {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();

  // Create commits on the feature branch
  let commits = scenario.create_commits(&[
    ("(feature) Add feature A".to_string(), "feature_a.kt", "// Feature A implementation"),
    ("(feature) Add feature B".to_string(), "feature_b.kt", "// Feature B implementation"),
  ]);

  // Push feature branch to remote
  scenario.local_repo.checkout("main").unwrap();
  let main_base = scenario.local_repo.head();
  scenario.local_repo.create_branch_at("user/virtual/feature", &commits[1]).unwrap();
  scenario.local_repo.push("origin", "user/virtual/feature").unwrap();

  // Simulate that commit_a gets merged to master
  scenario.simulate_commit_merge_to_master(&commits[0]);

  // Setup remote branch with both commits based on OLD master
  scenario.upstream_repo.checkout("user/virtual/feature").unwrap();
  scenario.upstream_repo.reset_hard(&main_base).unwrap();
  scenario.upstream_repo.cherry_pick(&commits[0]).unwrap();
  scenario.upstream_repo.cherry_pick(&commits[1]).unwrap();

  scenario.local_repo.fetch_prune("origin").unwrap();

  // Add a new commit locally (creates new unpushed commit)
  scenario.local_repo.checkout("development").unwrap();
  scenario
    .local_repo
    .create_commit("(feature) Add feature B with improvements", "feature_b.kt", "// Feature B implementation with improvements");

  // Run sync and verify
  let progress_reporter = scenario.run_sync("user").await;
  let events = progress_reporter.get_events();

  assert_remote_status(
    &events,
    "feature",
    1,
    3,
    0,
    "commit_a is in baseline, commit_b is patch-equivalent, new improved commit should be counted",
  );
}

/// Test the exact grpc-1.7.5 scenario: rebased commits with different SHAs but same content
#[test(tokio::test)]
async fn test_rebased_branch_shows_zero_unpushed() {
  let scenario = TestScenario::new().with_author("user@example.com", "Test User");
  scenario.setup_development_branch();

  // Create and push the grpc commit
  let grpc_commit = scenario
    .local_repo
    .create_commit("(grpc) update grpc from 1.73.0 to 1.75.0", "grpc_update.txt", "grpc 1.75.0");

  scenario.local_repo.checkout("main").unwrap();
  scenario.local_repo.create_branch_at("user/virtual/grpc", &grpc_commit).unwrap();
  scenario.local_repo.push("origin", "user/virtual/grpc").unwrap();

  // Simulate master getting updated by another user
  configure_test_author(&scenario.local_repo, "other@example.com", "Other User");
  scenario.local_repo.create_commit("Some other change", "other.txt", "other content");
  scenario.local_repo.push("origin", "main").unwrap();

  // Rebase the virtual branch onto new master (changes SHAs but keeps content)
  scenario.configure_author(); // Restore original user
  scenario.local_repo.checkout("user/virtual/grpc").unwrap();
  scenario.local_repo.rebase("origin/main").unwrap();

  // Test direct function call to verify content equivalence detection
  let result = compute_remote_status_for_branch(
    &scenario.git_executor,
    scenario.local_repo.path().to_str().unwrap(),
    "user/virtual/grpc",
    "grpc",
    Some(&scenario.user_email),
    1,
    "origin/main",
  )
  .unwrap();

  // After rebase, content should be recognized as equivalent despite different SHAs
  assert_eq!(result.my_unpushed_count, 0, "After rebase, should show 0 unpushed commits since content is equivalent");
  assert_eq!(result.unpushed_commits.len(), 2, "Should have 2 commits ahead (master update + rebased grpc commit)");
  assert_eq!(result.commits_behind, 0, "Should be 0 behind - remote grpc commit is patch-equivalent after rebase");
}

/// Test unrelated commits from same author are not counted as "my commits"
#[test(tokio::test)]
async fn test_unrelated_commits_from_same_author_not_counted() {
  let scenario = TestScenario::new().with_author("developer@example.com", "Developer");

  // Create BAZEL commits on one branch
  scenario.local_repo.checkout_new_branch("bazel-work").unwrap();
  scenario.configure_author();
  let bazel_commits = scenario.create_commits(&[
    ("(BAZEL-2453) introduce RunConfigurationProducerSuppressor".to_string(), "suppressor.rs", "// suppressor"),
    ("(BAZEL-2453) convert RunConfigurationProducersDisabler to kotlin".to_string(), "disabler.kt", "// kotlin"),
    ("(BAZEL-2453) Rename .java to .kt".to_string(), "rename.kt", "// renamed"),
    (
      "(BAZEL-2453) mark RunConfigurationProducersDisabler as internal API".to_string(),
      "internal.kt",
      "// internal",
    ),
  ]);

  // Create parallel-load-state commit on another branch
  scenario.local_repo.checkout("main").unwrap();
  scenario.local_repo.checkout_new_branch("parallel-work").unwrap();
  let _parallel_commit = scenario.local_repo.create_commit(
    "(parallel-load-state) IJPL-191229 part 8 - introduce NonCancelableInvocator",
    "invocator.rs",
    "// invocator",
  );

  // Merge BAZEL commits to master
  scenario.local_repo.checkout("main").unwrap();
  for commit in &bazel_commits {
    scenario.local_repo.cherry_pick(commit).unwrap();
  }
  scenario.local_repo.push("origin", "main").unwrap();

  // Setup remote virtual branch for parallel-load-state (before BAZEL merge)
  scenario.upstream_repo.checkout("main").unwrap();
  scenario.upstream_repo.reset_hard("HEAD~4").unwrap(); // Before BAZEL commits
  let old_master = scenario.upstream_repo.head();
  scenario.upstream_repo.create_branch_at("developer/virtual/parallel-load-state", &old_master).unwrap();
  scenario.upstream_repo.checkout("developer/virtual/parallel-load-state").unwrap();
  scenario.upstream_repo.create_commit(
    "(parallel-load-state) IJPL-191229 part 8 - introduce NonCancelableInvocator",
    "invocator.rs",
    "// invocator",
  );

  // Update upstream main with BAZEL commits
  scenario.upstream_repo.checkout("main").unwrap();
  for (subject, file, content) in [
    ("(BAZEL-2453) introduce RunConfigurationProducerSuppressor", "suppressor.rs", "// suppressor"),
    ("(BAZEL-2453) convert RunConfigurationProducersDisabler to kotlin", "disabler.kt", "// kotlin"),
    ("(BAZEL-2453) Rename .java to .kt", "rename.kt", "// renamed"),
    ("(BAZEL-2453) mark RunConfigurationProducersDisabler as internal API", "internal.kt", "// internal"),
  ] {
    scenario.upstream_repo.create_commit(subject, file, content);
  }

  // Prepare local branch with all commits for sync
  scenario.local_repo.fetch_prune("origin").unwrap();
  scenario.local_repo.checkout("parallel-work").unwrap();
  for commit in &bazel_commits {
    scenario.local_repo.cherry_pick(commit).unwrap();
  }

  // Run sync and verify BAZEL commits are NOT counted for parallel-load-state
  let progress_reporter = scenario.run_sync("developer").await;
  let events = progress_reporter.get_events();

  assert_remote_status(
    &events,
    "parallel-load-state",
    0,
    1,
    0,
    "BAZEL commits are in baseline, parallel commit is patch-equivalent - should count 0",
  );
}
