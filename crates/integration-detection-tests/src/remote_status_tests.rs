use super::test_helpers::{options_with_strategy, setup_test_repos};
use branch_integration::strategy::DetectionStrategy;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use sync_core::remote_status::{AncestorCommits, DROPPED_COMMITS_LIMIT, PushTimes, StackParent, compute_remote_status_for_branch};
use sync_core::sync::sync_branches;
use sync_test_utils::TestReporter;
use sync_types::{DroppedCommit, LandedRemoteCommit, SyncEvent};
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
    if let SyncEvent::RemoteStatusUpdate(status_update) = event
      && status_update.branch_name == branch_name
    {
      return Some(RemoteStatusResult {
        my_commits_count: status_update.my_unpushed_count,
        total_commits_count: u32::try_from(status_update.unpushed_commits.len()).unwrap_or(u32::MAX),
        commits_behind: status_update.commits_behind,
        remote_exists: status_update.remote_exists,
      });
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

  /// Resets the remote `branch` to `base`, cherry-picks `commits` onto it and fetches the result
  fn rebuild_remote_branch(&self, branch: &str, base: &str, commits: &[String]) {
    self.upstream_repo.checkout(branch).unwrap();
    self.upstream_repo.reset_hard(base).unwrap();
    for commit in commits {
      self.upstream_repo.cherry_pick(commit).unwrap();
    }
    self.local_repo.fetch_prune("origin").unwrap();
  }

  async fn run_sync(&self, branch_prefix: &str) -> TestReporter {
    let progress_reporter = TestReporter::new();
    let result = sync_branches(
      &self.git_executor,
      self.local_repo.path().to_str().unwrap(),
      branch_prefix,
      progress_reporter.clone(),
      options_with_strategy(DetectionStrategy::Rebase),
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
  println!("Remote status for {branch_name}:");
  println!("  My commits: {}", status.my_commits_count);
  println!("  Total commits: {}", status.total_commits_count);
  println!("  Commits behind: {}", status.commits_behind);
  println!("  Remote exists: {}", status.remote_exists);
}

/// Helper function to assert remote status with detailed messaging
fn assert_remote_status(events: &[SyncEvent], branch_name: &str, expected_my_commits: u32, expected_total_commits: u32, expected_commits_behind: u32, test_description: &str) {
  let status = extract_remote_status_for_branch(events, branch_name).unwrap_or_else(|| panic!("Should have received remote status update for {branch_name}"));

  print_remote_status(branch_name, &status);
  println!("Test: {test_description}");

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
    1,
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
  scenario.rebuild_remote_branch("user/virtual/io-mockk-update", &main_base, &commits);
  scenario.local_repo.checkout("development").unwrap();

  // Run sync and verify
  let progress_reporter = scenario.run_sync("user").await;
  let events = progress_reporter.get_events();

  assert_remote_status(
    &events,
    "io-mockk-update",
    0,
    0,
    0,
    "Both commits are on the remote by content: the remote has nothing to receive",
  );
}

/// Direct test of `compute_remote_status_for_branch` function
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
    "origin",
    "user/virtual/test",
    "test",
    Some(&scenario.user_email),
    "origin/main", // baseline_branch
    None,
  )
  .unwrap();

  // Since we just pushed, should be up to date
  assert_eq!(result.unpushed_commits.len(), 0, "Should be up to date after push");
  assert_eq!(result.my_unpushed_count, 0, "Should be up to date after push");
  assert_eq!(result.commits_behind, 0, "Should be up to date after push");
  assert_eq!(result.dropped_commits, []);
  assert_eq!(result.remote_tip, Some(latest_commit), "the status names the remote commit it compared with");
  assert!(result.fast_forward);
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
  scenario.rebuild_remote_branch("user/virtual/feature", &main_base, &commits);

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
    1,
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
    "origin",
    "user/virtual/grpc",
    "grpc",
    Some(&scenario.user_email),
    "origin/main",
    None,
  )
  .unwrap();

  // After rebase, content should be recognized as equivalent despite different SHAs
  assert_eq!(result.my_unpushed_count, 0, "After rebase, should show 0 unpushed commits since content is equivalent");
  // the master update is on the remote already, the rebased grpc commit is on it by content: nothing to push
  assert_eq!(result.unpushed_commits, Vec::<String>::new(), "The remote has every commit by content");
  assert_eq!(result.commits_behind, 0, "Should be 0 behind - remote grpc commit is patch-equivalent after rebase");
  assert!(!result.fast_forward, "The rebased branch replaces the remote commits");
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
    0,
    0,
    "BAZEL commits are in baseline, parallel commit is patch-equivalent - should count 0",
  );
}

/// The remote status of `user/virtual/{name}` against `origin/main`, for the user of the scenario
fn branch_status(git_executor: &GitCommandExecutor, scenario: &TestScenario, name: &str) -> sync_types::RemoteStatusUpdate {
  compute_remote_status_for_branch(
    git_executor,
    scenario.local_repo.path().to_str().unwrap(),
    "origin",
    &format!("user/virtual/{name}"),
    name,
    Some(&scenario.user_email),
    "origin/main",
    None,
  )
  .unwrap()
}

/// The remote status of `user/virtual/feature`
fn feature_status(scenario: &TestScenario) -> sync_types::RemoteStatusUpdate {
  branch_status(&scenario.git_executor, scenario, "feature")
}

/// The author time of `First` of `pushed_feature`, which a landed copy keeps
const FIRST_TIME: i64 = 1_700_000_000;

/// The pushed `user/virtual/feature` of one commit of the user, `First` (at `FIRST_TIME`), on `main`; HEAD is
/// `development`
fn pushed_feature() -> (TestScenario, String) {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();
  let first = scenario.local_repo.create_commit_with_timestamp("(feature) First", "first.txt", "first", Some(FIRST_TIME));
  scenario.local_repo.create_branch_at("user/virtual/feature", &first).unwrap();
  scenario.local_repo.push("origin", "user/virtual/feature").unwrap();
  (scenario, first)
}

/// Pushes a commit of another author to `origin/main`, from the checked-out `main`
fn push_baseline_change(scenario: &TestScenario) {
  scenario.local_repo.checkout("main").unwrap();
  configure_test_author(&scenario.local_repo, "other@example.com", "Other User");
  scenario.local_repo.create_commit("Baseline change", "baseline.txt", "baseline");
  scenario.local_repo.push("origin", "main").unwrap();
  scenario.configure_author();
}

/// A branch not on the remote sends its commits, the baseline excluded; mine counts only the user's commits
#[test(tokio::test)]
async fn test_new_branch_counts_commits_by_author() {
  let scenario = TestScenario::new().with_author("user@example.com", "Test User");
  scenario.setup_development_branch();

  scenario.local_repo.create_commit("(feature) Mine", "mine.txt", "mine");
  configure_test_author(&scenario.local_repo, "other@example.com", "Other User");
  let tip = scenario.local_repo.create_commit("(feature) Theirs", "theirs.txt", "theirs");
  scenario.local_repo.create_branch_at("user/virtual/feature", &tip).unwrap();

  let result = feature_status(&scenario);

  assert!(!result.remote_exists);
  assert_eq!(result.remote_tip, None);
  assert_eq!(result.unpushed_commits.len(), 2);
  assert_eq!(result.unpushed_commits.last(), Some(&tip), "oldest first");
  assert_eq!(result.my_unpushed_count, 1, "the other author's commit is not mine");
}

/// A pushed branch rebased onto a newer baseline with a new commit: the push sends only that commit and replaces the
/// remote commits with their rebased copies
#[test(tokio::test)]
async fn test_rebased_branch_with_new_commit_is_not_fast_forward() {
  let (scenario, first) = pushed_feature();
  push_baseline_change(&scenario);
  scenario.local_repo.checkout("user/virtual/feature").unwrap();
  scenario.local_repo.rebase("origin/main").unwrap();
  let second = scenario.local_repo.create_commit("(feature) Second", "second.txt", "second");

  let result = feature_status(&scenario);

  assert_eq!(result.unpushed_commits, vec![second]);
  assert_eq!(result.my_unpushed_count, 1);
  assert_eq!(result.commits_behind, 0);
  assert_eq!(result.commits_reworded, 0, "a rebased copy keeps its message and author");
  assert!(!result.fast_forward);
  assert_eq!(result.remote_tip.as_deref(), Some(first.as_str()), "the remote still has the commit before the rebase");
}

/// The pushed feature branch with `count` commits someone else added to its remote, oldest first, fetched
fn feature_with_their_fixes(count: i64) -> (TestScenario, Vec<String>) {
  let (scenario, _) = pushed_feature();
  scenario.upstream_repo.checkout("user/virtual/feature").unwrap();
  configure_test_author(&scenario.upstream_repo, "other@example.com", "Other User");
  let theirs: Vec<String> = (0..count)
    .map(|index| {
      scenario
        .upstream_repo
        .create_commit_with_timestamp(&format!("Their fix {index}"), &format!("fix{index}.txt"), "fix", Some(1_700_000_000 + index * 60))
    })
    .collect();
  scenario.local_repo.fetch_prune("origin").unwrap();
  (scenario, theirs)
}

/// Someone else added commits to the pushed branch: a push drops them. They are listed newest first, with their author
/// and time, up to the limit; the count covers them all, and the status names the remote head it compared with
#[test(tokio::test)]
async fn test_dropped_commits_are_listed_newest_first_up_to_the_limit() {
  let (scenario, theirs) = feature_with_their_fixes(12);

  let result = feature_status(&scenario);

  assert_eq!(result.remote_tip.as_ref(), theirs.last());
  assert_eq!(result.commits_behind, 12);
  let listed: Vec<&str> = result.dropped_commits.iter().map(|commit| commit.id.as_str()).collect();
  let newest: Vec<&str> = theirs.iter().rev().take(DROPPED_COMMITS_LIMIT).map(String::as_str).collect();
  assert_eq!(listed, newest);
  assert_eq!(
    result.dropped_commits[0],
    DroppedCommit {
      id: theirs[11].clone(),
      subject: "Their fix 11".to_string(),
      author: "Other User".to_string(),
      author_time: 1_700_000_660,
    }
  );
  assert!(!result.fast_forward);
}

/// One dropped commit more than the limit is listed too: the push never says "and 1 more" (`list_cut::kept`)
#[test(tokio::test)]
async fn test_dropped_commits_one_more_than_the_limit_are_listed_whole() {
  let count = i64::try_from(DROPPED_COMMITS_LIMIT).unwrap() + 1;
  let (scenario, theirs) = feature_with_their_fixes(count);

  let result = feature_status(&scenario);

  assert_eq!(result.commits_behind, u32::try_from(count).unwrap());
  let listed: Vec<&str> = result.dropped_commits.iter().map(|commit| commit.id.as_str()).collect();
  let newest: Vec<&str> = theirs.iter().rev().map(String::as_str).collect();
  assert_eq!(listed, newest);
}

/// The remote status of `user/virtual/{branch}` in the events of a sync
fn synced_status(events: &[SyncEvent], branch: &str) -> sync_types::RemoteStatusUpdate {
  events
    .iter()
    .find_map(|event| match event {
      SyncEvent::RemoteStatusUpdate(update) if update.branch_name == branch => Some(update.clone()),
      _ => None,
    })
    .unwrap_or_else(|| panic!("{branch} has a remote status"))
}

/// The subjects of the dropped commits of a status, newest first
fn dropped_subjects(status: &sync_types::RemoteStatusUpdate) -> Vec<&str> {
  status.dropped_commits.iter().map(|commit| commit.subject.as_str()).collect()
}

/// The user amended a pushed commit: the remote has its older version, with the same author, author time and subject.
/// The push replaces it; nothing of anyone's work is dropped
#[test(tokio::test)]
async fn an_amended_commit_is_amended_not_dropped() {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();
  scenario
    .local_repo
    .create_commit_with_timestamp("(feature) First", "first.txt", "first", Some(1_700_000_000));
  scenario
    .local_repo
    .create_commit_with_timestamp("(feature) Second", "second.txt", "second", Some(1_700_000_100));
  scenario.run_sync("user").await;
  scenario.local_repo.push("origin", "user/virtual/feature").unwrap();

  // the second commit amended: new content, the same author, author time and subject
  scenario.local_repo.reset_hard("HEAD~1").unwrap();
  scenario
    .local_repo
    .create_commit_with_timestamp("(feature) Second", "second.txt", "second, amended", Some(1_700_000_100));
  let events = scenario.run_sync("user").await.get_events();

  let status = synced_status(&events, "feature");
  assert_eq!(status.unpushed_commits.len(), 1, "the amended commit is unpushed");
  assert_eq!((status.commits_amended, status.commits_restacked, status.commits_behind), (1, 0, 0));
  assert_eq!(status.dropped_commits, []);
  assert!(!status.fast_forward, "the push replaces the older version");
}

/// The stack `login` on `auth`, synced and pushed with `auth_subject` (`// auth` its change), then made again with
/// the auth commit as `auth_again` (subject, change) at the same author time and login's commit as it was, and synced:
/// the events of that sync
async fn rebuilt_parent(scenario: &TestScenario, auth_subject: &str, auth_again: (&str, &str)) -> Vec<SyncEvent> {
  scenario.setup_development_branch();
  scenario.local_repo.set_config("branchdeck.stacks", "true").unwrap();
  let base = scenario.local_repo.head();
  let commit_both = |auth: (&str, &str)| {
    scenario.local_repo.create_commit_with_timestamp(auth.0, "auth.rs", auth.1, Some(1_700_000_000));
    scenario
      .local_repo
      .create_commit_with_timestamp("(auth/login) Add form", "login.rs", "// login", Some(1_700_000_100));
  };
  commit_both((auth_subject, "// auth"));
  scenario.run_sync("user").await;
  scenario.local_repo.push("origin", "user/virtual/auth").unwrap();
  scenario.local_repo.push("origin", "user/virtual/login").unwrap();

  scenario.local_repo.reset_hard(&base).unwrap();
  commit_both(auth_again);
  scenario.run_sync("user").await.get_events()
}

/// The status of `login` as a push computes it, without the sync's commits: the parent's virtual branch is read
fn login_status_outside_the_sync(scenario: &TestScenario) -> sync_types::RemoteStatusUpdate {
  compute_remote_status_for_branch(
    &scenario.git_executor,
    scenario.local_repo.path().to_str().unwrap(),
    "origin",
    "user/virtual/login",
    "login",
    Some(&scenario.user_email),
    "origin/main",
    Some(StackParent {
      base: "user/virtual/auth",
      ancestors: AncestorCommits::ReadWhenNeeded,
    }),
  )
  .unwrap()
}

/// A stack whose parent commit was amended: the child's remote branch has the parent's commit as it was. Restacked,
/// in the sync (the ancestors' commits known) and outside it (read from the parent's virtual branch), never dropped
#[test(tokio::test)]
async fn a_child_of_an_amended_parent_is_restacked() {
  let scenario = TestScenario::new();
  // the auth commit amended; login's own commit stays as it is
  let events = rebuilt_parent(&scenario, "(auth) Add auth", ("(auth) Add auth", "// auth, amended")).await;

  let auth = synced_status(&events, "auth");
  assert_eq!((auth.commits_amended, auth.commits_restacked, auth.commits_behind), (1, 0, 0));
  let login = synced_status(&events, "login");
  assert_eq!(login.unpushed_commits, Vec::<String>::new(), "login's own commit is on the remote by content");
  assert_eq!((login.commits_amended, login.commits_restacked, login.commits_behind), (0, 1, 0));
  assert_eq!(login.ancestor_commits_ahead, 0, "the newer version of a remote commit is restacked, not counted twice");
  assert!(!login.fast_forward);

  let pushed = login_status_outside_the_sync(&scenario);
  assert_eq!((pushed.commits_amended, pushed.commits_restacked, pushed.commits_behind), (0, 1, 0));
  assert_eq!(pushed.ancestor_commits_ahead, 0);
}

/// A stack whose parent got a new commit after the child was pushed: the child's remote branch lacks it, so a Safe
/// Push of that remote branch would land an older piece. The commit is the parent's, not unpushed work of the child,
/// in the sync and outside it; once the child is pushed, nothing is ahead
#[test(tokio::test)]
async fn a_child_whose_parent_got_a_new_commit_lacks_it_on_its_remote() {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();
  scenario.local_repo.set_config("branchdeck.stacks", "true").unwrap();
  scenario
    .local_repo
    .create_commit_with_timestamp("(auth) Add auth", "auth.rs", "// auth", Some(1_700_000_000));
  scenario
    .local_repo
    .create_commit_with_timestamp("(auth/login) Add form", "login.rs", "// login", Some(1_700_000_100));
  scenario.run_sync("user").await;
  scenario.local_repo.push("origin", "user/virtual/auth").unwrap();
  scenario.local_repo.push("origin", "user/virtual/login").unwrap();

  scenario
    .local_repo
    .create_commit_with_timestamp("(auth) Fix auth", "auth.rs", "// auth, fixed", Some(1_700_000_200));
  let events = scenario.run_sync("user").await.get_events();

  let auth = synced_status(&events, "auth");
  assert_eq!((auth.unpushed_commits.len(), auth.ancestor_commits_ahead), (1, 0));
  let login = synced_status(&events, "login");
  assert_eq!(login.unpushed_commits, Vec::<String>::new(), "the new commit is auth's");
  assert_eq!((login.commits_restacked, login.commits_behind, login.ancestor_commits_ahead), (0, 0, 1));

  let pushed = login_status_outside_the_sync(&scenario);
  assert_eq!((pushed.unpushed_commits.len(), pushed.commits_restacked, pushed.ancestor_commits_ahead), (0, 0, 1));

  scenario.local_repo.push("origin", "+user/virtual/login").unwrap();
  scenario.local_repo.fetch_prune("origin").unwrap();
  assert_eq!(login_status_outside_the_sync(&scenario).ancestor_commits_ahead, 0);
}

/// A stack whose parent commit was reworded (`bd reword`: the same changes, another subject): the child's remote branch
/// has the parent's commit with its old message, which a push replaces. Restacked, in the sync and outside it; a parent
/// only rebuilt (the same message) gives the child nothing to push
#[test(tokio::test)]
async fn a_child_of_a_reworded_parent_is_restacked() {
  let scenario = TestScenario::new();
  // the auth commit reworded: the same changes, the same author and author time, a shorter subject
  let events = rebuilt_parent(&scenario, "(auth) Add auth with a subject that is far too long", ("(auth) Add auth", "// auth")).await;

  let auth = synced_status(&events, "auth");
  assert_eq!((auth.commits_reworded, auth.commits_restacked, auth.commits_behind), (1, 0, 0));
  let login = synced_status(&events, "login");
  assert_eq!(login.unpushed_commits, Vec::<String>::new(), "login's own commit is on the remote as it is");
  assert_eq!(
    (login.commits_reworded, login.commits_amended, login.commits_restacked, login.commits_behind),
    (0, 0, 1, 0),
    "the remote has auth's commit with the old subject"
  );

  assert_eq!(login.ancestor_commits_ahead, 0, "the remote has auth's commit by content");

  let pushed = login_status_outside_the_sync(&scenario);
  assert_eq!((pushed.commits_reworded, pushed.commits_restacked, pushed.commits_behind), (0, 1, 0));

  // pushed again, then both rebuilt on a newer baseline: the same messages, nothing for login to push
  scenario.local_repo.push("origin", "+user/virtual/auth").unwrap();
  scenario.local_repo.push("origin", "+user/virtual/login").unwrap();
  scenario.local_repo.checkout("main").unwrap();
  scenario.local_repo.create_commit("Upstream change", "upstream.txt", "upstream");
  scenario.local_repo.push("origin", "main").unwrap();
  scenario.local_repo.checkout("development").unwrap();
  scenario.local_repo.rebase("origin/main").unwrap();
  let events = scenario.run_sync("user").await.get_events();
  let login = synced_status(&events, "login");
  assert_eq!(
    (login.commits_reworded, login.commits_restacked, login.commits_behind, login.ancestor_commits_ahead),
    (0, 0, 0, 0)
  );
}

/// Someone else's commit with the subject and the author time of a commit of the branch is someone else's work: the
/// author email is part of the identity, a push drops it
#[test(tokio::test)]
async fn another_authors_commit_stays_dropped() {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();
  let first = scenario.local_repo.create_commit_with_timestamp("(feature) Fix", "fix.txt", "mine", Some(1_700_000_000));
  scenario.local_repo.create_branch_at("user/virtual/feature", &first).unwrap();
  scenario.local_repo.push("origin", "user/virtual/feature").unwrap();

  scenario.upstream_repo.checkout("user/virtual/feature").unwrap();
  scenario.upstream_repo.reset_hard("main").unwrap();
  configure_test_author(&scenario.upstream_repo, "other@example.com", "Other User");
  scenario.upstream_repo.create_commit_with_timestamp("Fix", "fix.txt", "theirs", Some(1_700_000_000));
  scenario.local_repo.fetch_prune("origin").unwrap();

  let result = feature_status(&scenario);

  assert_eq!((result.commits_amended, result.commits_restacked, result.commits_behind), (0, 0, 1));
  assert_eq!(dropped_subjects(&result), ["Fix"]);
  assert_eq!(result.dropped_commits[0].author, "Other User");
}

/// The remote has the older version of a commit amended here and someone else's commit: the push replaces the one
/// and drops the other, which alone counts as behind and is listed
#[test(tokio::test)]
async fn an_amended_commit_and_a_dropped_one_are_told_apart() {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();
  let base = scenario.local_repo.head();
  scenario
    .local_repo
    .create_commit_with_timestamp("(feature) First", "first.txt", "first", Some(1_700_000_000));
  let second = scenario
    .local_repo
    .create_commit_with_timestamp("(feature) Second", "second.txt", "second", Some(1_700_000_100));
  scenario.local_repo.create_branch_at("user/virtual/feature", &second).unwrap();
  scenario.local_repo.push("origin", "user/virtual/feature").unwrap();

  scenario.upstream_repo.checkout("user/virtual/feature").unwrap();
  configure_test_author(&scenario.upstream_repo, "other@example.com", "Other User");
  scenario
    .upstream_repo
    .create_commit_with_timestamp("Their fix", "theirs.txt", "theirs", Some(1_700_000_200));
  scenario.local_repo.fetch_prune("origin").unwrap();

  // the second commit amended
  scenario.local_repo.reset_hard(&base).unwrap();
  scenario
    .local_repo
    .create_commit_with_timestamp("(feature) First", "first.txt", "first", Some(1_700_000_000));
  let amended = scenario
    .local_repo
    .create_commit_with_timestamp("(feature) Second", "second.txt", "second, amended", Some(1_700_000_100));
  scenario
    .local_repo
    .git_executor()
    .execute_command(&["branch", "-f", "user/virtual/feature", &amended], scenario.local_repo.path().to_str().unwrap())
    .unwrap();

  let result = feature_status(&scenario);

  assert_eq!(result.unpushed_commits, [amended]);
  assert_eq!((result.commits_amended, result.commits_restacked, result.commits_behind), (1, 0, 1));
  assert_eq!(dropped_subjects(&result), ["Their fix"]);
  assert!(!result.fast_forward);
}

/// A pushed commit rebuilt on a baseline that changed the lines around its change: its patch differs, so
/// `--cherry-pick` can't match the remote copy, but it makes the same changes. A rebased copy, neither amended nor
/// dropped: the push replaces it as any rebased commit
#[test(tokio::test)]
async fn a_rebased_copy_whose_context_moved_is_not_amended() {
  let scenario = TestScenario::new();
  let lines = |changed: &[(usize, &str)]| {
    (1..=10)
      .map(|line| {
        changed
          .iter()
          .find(|(at, _)| *at == line)
          .map_or_else(|| format!("line {line}\n"), |(_, text)| format!("{text}\n"))
      })
      .collect::<String>()
  };
  scenario.local_repo.checkout("main").unwrap();
  scenario.configure_author();
  scenario.local_repo.create_commit("Add lines", "lines.txt", &lines(&[]));
  scenario.local_repo.push("origin", "main").unwrap();
  scenario.local_repo.checkout_new_branch("development").unwrap();
  let pushed = scenario
    .local_repo
    .create_commit_with_timestamp("(feature) Change line 5", "lines.txt", &lines(&[(5, "five")]), Some(1_700_000_000));
  scenario.local_repo.create_branch_at("user/virtual/feature", &pushed).unwrap();
  scenario.local_repo.push("origin", "user/virtual/feature").unwrap();

  // the baseline changes line 2, in the context of the change; the branch is rebuilt on it
  scenario.local_repo.checkout("main").unwrap();
  scenario.local_repo.create_commit("Change line 2", "lines.txt", &lines(&[(2, "two")]));
  scenario.local_repo.push("origin", "main").unwrap();
  scenario.local_repo.checkout("user/virtual/feature").unwrap();
  scenario.local_repo.rebase("origin/main").unwrap();
  let rebased = scenario.local_repo.head();

  let result = feature_status(&scenario);

  assert_eq!(result.unpushed_commits, [rebased], "its patch differs from the pushed one");
  assert_eq!((result.commits_amended, result.commits_restacked, result.commits_behind), (0, 0, 0));
  assert!(!result.fast_forward, "the push replaces the remote copy");
}

/// A Safe Push landed a pushed commit rebuilt on a newer target, where a change below it changed its lines: the baseline
/// has a commit with its author, author time and subject but another patch, so `--cherry-mark` misses it. The remote's
/// copy is an older version of a landed commit, which a push replaces without --force; someone else's commit on the
/// remote branch stays dropped
#[test(tokio::test)]
async fn a_remote_commit_that_landed_in_the_baseline_with_other_changes_is_landed_not_dropped() {
  let (scenario, theirs) = feature_with_their_fixes(1);
  let first = scenario.local_repo.git(&["rev-parse", &format!("{}~1", theirs[0])]);

  // the landed copy: the subject without the prefix, the author and the author time kept, the content changed
  scenario.local_repo.checkout("main").unwrap();
  scenario.configure_author();
  let landed = scenario
    .local_repo
    .create_commit_with_timestamp("First", "first.txt", "first, rebuilt on a changed target", Some(FIRST_TIME));
  scenario.local_repo.push("origin", "main").unwrap();
  // the branch rebuilt on the new baseline, with one new commit
  scenario.local_repo.git(&["branch", "-f", "user/virtual/feature", "origin/main"]);
  scenario.local_repo.checkout("user/virtual/feature").unwrap();
  let second = scenario.local_repo.create_commit("(feature) Second", "second.txt", "second");

  let result = feature_status(&scenario);

  assert_eq!(result.unpushed_commits, [second]);
  assert_eq!(
    (result.commits_amended, result.commits_restacked, result.commits_landed, result.commits_behind),
    (0, 0, 1, 1)
  );
  assert_eq!(
    result.landed_commits,
    [LandedRemoteCommit {
      id: first,
      subject: "(feature) First".to_string(),
      baseline_commit: landed,
    }]
  );
  assert_eq!(dropped_subjects(&result), ["Their fix 0"], "only someone else's commit is dropped");
}

/// A reword keeps the changes, so the remote has the commit by content, yet in another version: its message. The
/// remote branch is not up to date, a robot would land the old message; a new author is another version too, and a copy
/// with the message and the author as pushed only rebuilt
#[test(tokio::test)]
async fn a_reworded_commit_is_not_on_the_remote_as_it_is() {
  let (scenario, _) = pushed_feature();
  let amend = |args: &[&str]| {
    scenario.local_repo.git(&[&["commit", "--amend", "--no-verify"], args].concat());
    scenario.local_repo.git(&["branch", "-f", "user/virtual/feature", "HEAD"]);
    let status = feature_status(&scenario);
    assert_eq!(status.unpushed_commits, Vec::<String>::new(), "the remote has the changes");
    assert_eq!((status.commits_amended, status.commits_landed, status.commits_behind), (0, 0, 0));
    assert!(!status.fast_forward);
    status.commits_reworded
  };

  assert_eq!(amend(&["-m", "(feature) First, reworded"]), 1, "reworded");
  assert_eq!(amend(&["-m", "(feature) First", "--author", "Other User <other@example.com>"]), 1, "another author");
  assert_eq!(amend(&["-m", "(feature) First", "--author", "Test User <user@example.com>"]), 0, "as pushed, only rebuilt");
}

/// What a status says of the remote side, and the git processes it ran by subcommand
#[derive(Debug, PartialEq, Eq)]
struct RemoteSide {
  unpushed: Vec<String>,
  behind: u32,
  fast_forward: bool,
  processes: BTreeMap<String, u32>,
}

/// The `RemoteSide` of `user/virtual/feature`, computed by an executor that counts its processes
fn counted_remote_side(scenario: &TestScenario) -> RemoteSide {
  let git_executor = GitCommandExecutor::new().with_process_stats("test");
  let status = branch_status(&git_executor, scenario, "feature");
  let processes = git_executor.process_stats().unwrap().by_command();
  RemoteSide {
    unpushed: status.unpushed_commits,
    behind: status.commits_behind,
    fast_forward: status.fast_forward,
    processes: processes.into_iter().map(|(command, stats)| (command, stats.count)).collect(),
  }
}

/// The `RemoteSide` of a status that walks: its processes read the tips of both branches, walk the unpushed commits
/// and the remote ones (behind and the fast-forward), and read the push time
fn walked(unpushed: Vec<String>, behind: u32, fast_forward: bool) -> RemoteSide {
  RemoteSide {
    unpushed,
    behind,
    fast_forward,
    processes: BTreeMap::from([("for-each-ref".to_string(), 1), ("reflog".to_string(), 1), ("rev-list".to_string(), 2)]),
  }
}

/// The walk of the remote commits tells what is behind and whether the push is a fast-forward: a patch-equivalent copy
/// of a local commit, also one in the baseline, is not behind, yet the remote tip is no ancestor of the branch. A
/// remote branch at the branch's tip walks nothing
#[test]
fn one_walk_tells_what_is_behind_and_the_fast_forward() {
  let (equal, _) = pushed_feature();
  let only_push_time = RemoteSide {
    processes: BTreeMap::from([("for-each-ref".to_string(), 1), ("reflog".to_string(), 1)]),
    ..walked(Vec::new(), 0, true)
  };
  assert_eq!(counted_remote_side(&equal), only_push_time, "the remote branch is the branch");

  let (ancestor, _) = pushed_feature();
  let second = ancestor.local_repo.create_commit("(feature) Second", "second.txt", "second");
  ancestor.local_repo.git(&["branch", "-f", "user/virtual/feature", &second]);
  assert_eq!(
    counted_remote_side(&ancestor),
    walked(vec![second], 0, true),
    "the remote branch is an ancestor of the branch"
  );

  let (rebased, _) = pushed_feature();
  push_baseline_change(&rebased);
  rebased.local_repo.checkout("user/virtual/feature").unwrap();
  rebased.local_repo.rebase("origin/main").unwrap();
  assert_eq!(
    counted_remote_side(&rebased),
    walked(Vec::new(), 0, false),
    "the remote has the branch's commits as they were before a rebase"
  );

  let (theirs, _) = pushed_feature();
  theirs.upstream_repo.checkout("user/virtual/feature").unwrap();
  configure_test_author(&theirs.upstream_repo, "other@example.com", "Other User");
  theirs.upstream_repo.create_commit("Their fix", "theirs.txt", "theirs");
  theirs.local_repo.fetch_prune("origin").unwrap();
  // a commit behind is looked for in the baseline by its identity (landed): one more `rev-list`, only then
  let dropped = RemoteSide {
    processes: BTreeMap::from([("for-each-ref".to_string(), 1), ("reflog".to_string(), 1), ("rev-list".to_string(), 3)]),
    ..walked(Vec::new(), 1, false)
  };
  assert_eq!(counted_remote_side(&theirs), dropped, "the remote has a commit of another author");

  // First reached the baseline as a copy, and the branch is rebuilt on it with a new commit
  let (landed, first) = pushed_feature();
  push_baseline_change(&landed);
  landed.local_repo.checkout("main").unwrap();
  landed.local_repo.cherry_pick(&first).unwrap();
  landed.local_repo.push("origin", "main").unwrap();
  landed.local_repo.checkout("user/virtual/feature").unwrap();
  landed.local_repo.reset_hard("origin/main").unwrap();
  let second = landed.local_repo.create_commit("(feature) Second", "second.txt", "second");
  assert_eq!(
    counted_remote_side(&landed),
    walked(vec![second], 0, false),
    "the remote commit is in the baseline as a copy"
  );
}

/// Pushes `user/virtual/{name}` with the reflog entry at `time`, in a translated locale: git writes the reflog message
/// of a push untranslated
fn push_at(scenario: &TestScenario, name: &str, time: u32) {
  let date = format!("@{time} +0000");
  scenario
    .local_repo
    .git_executor()
    .cmd(&["push", "origin", &format!("user/virtual/{name}")], scenario.local_repo.path_str())
    .env(&[("GIT_COMMITTER_DATE", &date), ("LC_ALL", "de_DE.UTF-8"), ("LANGUAGE", "de")])
    .text()
    .unwrap();
}

/// The push time is the time of the newest push of the remote branch from this clone, not of a fetch that moved it
/// later, also for a branch whose name holds a `{`, as the reflog selector does
#[test]
fn the_push_time_is_the_newest_push_not_a_later_fetch() {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();
  let name = "fix{1}";
  let first = scenario.local_repo.create_commit("(fix) First", "first.txt", "first");
  scenario.local_repo.create_branch_at(&format!("user/virtual/{name}"), &first).unwrap();
  push_at(&scenario, name, 1_700_000_000);
  let second = scenario.local_repo.create_commit("(fix) Second", "second.txt", "second");
  scenario.local_repo.git(&["branch", "-f", &format!("user/virtual/{name}"), &second]);
  push_at(&scenario, name, 1_700_000_100);

  scenario.upstream_repo.checkout(&format!("user/virtual/{name}")).unwrap();
  scenario.upstream_repo.create_commit("Their fix", "theirs.txt", "theirs");
  scenario.local_repo.fetch_prune("origin").unwrap();

  let status = branch_status(&scenario.git_executor, &scenario, name);
  assert_eq!(status.commits_behind, 1, "the fetch moved the remote branch");
  assert_eq!(status.last_push_time, 1_700_000_100);
}

/// The push times of the branches of an operation, read by an executor that counts its processes: `(times, processes)`
fn counted_push_times(scenario: &TestScenario, names: &[&str]) -> (Vec<u32>, BTreeMap<String, u32>) {
  let git_executor = GitCommandExecutor::new().with_process_stats("test");
  let refs: Vec<String> = names.iter().map(|name| format!("refs/remotes/origin/user/virtual/{name}")).collect();
  let push_times = PushTimes::new(refs.clone());
  let times = refs
    .iter()
    .map(|remote_ref| push_times.of(&git_executor, scenario.local_repo.path_str(), remote_ref))
    .collect();
  let processes = git_executor.process_stats().unwrap().by_command();
  (times, processes.into_iter().map(|(command, stats)| (command, stats.count)).collect())
}

/// The push times of all branches of an operation are one `reflog`, the newest push of each, a fetch not counted. A
/// remote branch deleted since its tip was read fails that read: then each branch reads its own, and the deleted one
/// has none
#[test]
fn the_push_times_of_several_branches_are_one_read() {
  let scenario = TestScenario::new();
  scenario.setup_development_branch();
  for (name, time) in [("pushed", 1_700_000_000), ("fetched", 1_700_000_100), ("deleted", 1_700_000_200)] {
    let commit = scenario.local_repo.create_commit(&format!("({name}) First"), &format!("{name}.txt"), name);
    scenario.local_repo.create_branch_at(&format!("user/virtual/{name}"), &commit).unwrap();
    push_at(&scenario, name, time);
  }
  // `fetched` was pushed by someone else, the clone only fetched it
  let fetched = scenario.local_repo.rev_parse("user/virtual/fetched").unwrap();
  scenario.local_repo.git(&["update-ref", "-d", "refs/remotes/origin/user/virtual/fetched"]);
  scenario
    .local_repo
    .git(&["update-ref", "-m", "fetch", "refs/remotes/origin/user/virtual/fetched", &fetched]);

  let (times, processes) = counted_push_times(&scenario, &["pushed", "fetched", "deleted"]);
  assert_eq!(times, [1_700_000_000, 0, 1_700_000_200]);
  assert_eq!(processes, BTreeMap::from([("reflog".to_string(), 1)]));

  scenario.local_repo.git(&["update-ref", "-d", "refs/remotes/origin/user/virtual/deleted"]);
  let (times, processes) = counted_push_times(&scenario, &["pushed", "fetched", "deleted"]);
  assert_eq!(times, [1_700_000_000, 0, 0]);
  assert_eq!(processes, BTreeMap::from([("reflog".to_string(), 4)]), "the failed read, then one per branch");
}
