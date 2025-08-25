//! Tests specifically for merge-based integration detection

use super::integration_tests::test_integration_workflow_helper;
use super::test_helpers::{IntegrationMethod, UpdateMethod, test_partial_integration_detection_with_strategy};
use branch_integration::strategy::DetectionStrategy;
use test_log::test;

#[test]
fn test_full_integration_workflow_with_push_and_merge() {
  // Test full integration workflow: create commits, sync to virtual branches, push, merge upstream with --no-ff, pull, detect integration
  // This test verifies NO-FF MERGE detection with proper timestamp verification
  test_integration_workflow_helper(
    UpdateMethod::Rebase,
    IntegrationMethod::Merge,
    vec![
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
    vec![(
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
