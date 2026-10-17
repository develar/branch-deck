//! Tests specifically for merge-based integration detection

use super::integration_tests::test_integration_workflow_helper;
use super::test_helpers::{IntegrationMethod, UpdateMethod, detected_branch_info, setup_test_repos, test_partial_integration_detection_with_strategy};
use branch_integration::cache::STACK_NOTES_REF;
use branch_integration::detector::{DetectConfig, detect_integrated_branches};
use branch_integration::strategy::DetectionStrategy;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use sync_test_utils::TestReporter;
use sync_types::branch_integration::{BranchIntegrationStatus, IntegrationConfidence};
use sync_utils::timestamp::utc_today;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

#[test]
fn test_full_integration_workflow_with_push_and_merge() {
  // Test full integration workflow: create commits, sync to virtual branches, push, merge upstream with --no-ff, pull, detect integration
  // This test verifies NO-FF MERGE detection with proper timestamp verification
  test_integration_workflow_helper(
    UpdateMethod::Rebase,
    IntegrationMethod::Merge,
    &[
      (
        "feature-auth",
        vec![
          ("(feature-auth) Add authentication module", "auth.js", "export function login() {}"),
          ("(feature-auth) Improve authentication", "auth.js", "export function login() { return true; }"),
        ],
      ),
      ("bugfix-login", vec![("(bugfix-login) Fix login timeout", "fix.js", "// fix code")]),
    ],
  );
}

#[test]
fn test_full_integration_workflow_with_merge_instead_of_rebase() {
  // Test full integration workflow with FF-ONLY MERGE update method (instead of rebase for pulling changes)
  // This still tests NO-FF MERGE detection upstream, but uses merge (not rebase) for local updates
  test_integration_workflow_helper(
    UpdateMethod::Merge,
    IntegrationMethod::Merge,
    &[(
      "feature-merge",
      vec![
        ("(feature-merge) Add feature function", "feature.js", "export function feature() {}"),
        ("(feature-merge) Improve feature", "feature.js", "export function feature() { return true; }"),
      ],
    )],
  );
}

#[test]
fn test_partial_integration_detection_with_merge_strategy() {
  // Test partial integration detection using merge strategy
  // Only some commits from a branch are cherry-picked, branch should be detected as orphaned
  test_partial_integration_detection_with_strategy(DetectionStrategy::Merge);
}

/// Detects the archived branches of `repo` against `origin/master` with the merge strategy and returns the status of
/// `archived`
fn detect_merged(repo: &TestRepo, archived: &str) -> BranchIntegrationStatus {
  let progress = TestReporter::new();
  let active_branches = HashSet::new();
  let config = DetectConfig {
    active_branches: &active_branches,
    progress: &progress,
    strategy: DetectionStrategy::Merge,
    retention_days: 7,
  };
  detect_integrated_branches(repo.git_executor(), repo.path().to_str().unwrap(), "user", "origin/master", config).unwrap();
  detected_branch_info(&progress.get_events(), archived).status.clone()
}

/// The baseline of `repo` is its `HEAD` now
fn publish_head(repo: &TestRepo) {
  repo
    .git_executor()
    .execute_command(&["update-ref", "refs/remotes/origin/master", "HEAD"], repo.path().to_str().unwrap())
    .unwrap();
}

/// A branch merged with a merge commit counts its own commits: not the baseline commits before or after the merge
#[test]
fn a_merged_branch_counts_the_commits_the_merge_brought_in() {
  let (_upstream_repo, local_repo, _) = setup_test_repos();
  let mainline = local_repo.current_branch().unwrap();
  local_repo.checkout_new_branch("feature").unwrap();
  local_repo.create_commit("Add one", "one.rs", "// one");
  let tip = local_repo.create_commit("Add two", "two.rs", "// two");
  let archived = format!("user/archived/{}/feature", utc_today());
  local_repo.create_branch_at(&archived, &tip).unwrap();
  local_repo.checkout(&mainline).unwrap();
  local_repo.create_commit("Add before", "before.rs", "// before");
  local_repo.merge_no_ff("feature", "Merge feature").unwrap();
  let merge = local_repo.head();
  local_repo.create_commit("Add after", "after.rs", "// after");
  local_repo.create_commit("Add later", "later.rs", "// later");
  publish_head(&local_repo);

  let status = detect_merged(&local_repo, &archived);

  let BranchIntegrationStatus::Integrated {
    commit_count,
    confidence: IntegrationConfidence::Exact,
    commit: Some(commit),
    ..
  } = &status
  else {
    panic!("{status:?}");
  };
  assert_eq!((*commit_count, commit.id.as_str()), (2, merge.as_str()));
}

/// Of a merged stacked branch only the commits after its stack base count, not its parent's
#[test]
fn a_merged_stacked_branch_counts_only_its_own_commits() {
  let (_upstream_repo, local_repo, _) = setup_test_repos();
  let mainline = local_repo.current_branch().unwrap();
  local_repo.checkout_new_branch("feature").unwrap();
  let parent = local_repo.create_commit("Add auth", "auth.rs", "// auth");
  let tip = local_repo.create_commit("Add form", "login.rs", "// login");
  let archived = format!("user/archived/{}/login", utc_today());
  local_repo.create_branch_at(&archived, &tip).unwrap();
  local_repo
    .git_executor()
    .execute_command(&["notes", "--ref", STACK_NOTES_REF, "add", "-m", &parent, &tip], local_repo.path().to_str().unwrap())
    .unwrap();
  local_repo.checkout(&mainline).unwrap();
  local_repo.merge_no_ff("feature", "Merge feature").unwrap();
  local_repo.create_commit("Add after", "after.rs", "// after");
  publish_head(&local_repo);

  let status = detect_merged(&local_repo, &archived);

  assert!(matches!(status, BranchIntegrationStatus::Integrated { commit_count: 1, .. }), "{status:?}");
}

/// No merge brought a fast-forwarded branch in, and a later merge on the baseline is not its merge: the cherry-mark scan
/// names the tip as the commit that brought it in
#[test]
fn a_fast_forwarded_branch_is_not_attributed_to_a_later_merge() {
  let (_upstream_repo, local_repo, _) = setup_test_repos();
  let mainline = local_repo.current_branch().unwrap();
  let tip = local_repo.create_commit("Add feature", "feature.rs", "// feature");
  let archived = format!("user/archived/{}/feature", utc_today());
  local_repo.create_branch_at(&archived, &tip).unwrap();
  local_repo.checkout_new_branch("other").unwrap();
  local_repo.create_commit("Add other", "other.rs", "// other");
  local_repo.checkout(&mainline).unwrap();
  local_repo.create_commit("Add after", "after.rs", "// after");
  local_repo.merge_no_ff("other", "Merge other").unwrap();
  publish_head(&local_repo);

  let status = detect_merged(&local_repo, &archived);

  let BranchIntegrationStatus::Integrated { commit: Some(commit), .. } = &status else {
    panic!("{status:?}");
  };
  assert_eq!(commit.id, tip);
}
