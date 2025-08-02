use crate::commit_grouper::CommitGrouper;
use git_ops::commit_list::Commit;

/// Helper function to create a test commit with minimal required fields
fn create_test_commit(id: &str, subject: &str) -> Commit {
  Commit {
    id: id.to_string(),
    subject: subject.to_string(),
    stripped_subject: subject.to_string(),
    message: subject.to_string(),
    author_name: "Test Author".to_string(),
    author_email: "test@example.com".to_string(),
    author_timestamp: 1234567890,
    committer_timestamp: 1234567890,
    parent_id: None,
    tree_id: "tree123".to_string(),
    note: None,
    mapped_commit_id: None,
  }
}

#[test]
fn test_prefix_regex_patterns() {
  let test_cases = vec![
    ("(simple) message", Some("simple"), "message"),
    ("(with-dashes) message", Some("with-dashes"), "message"),
    ("(with_underscores) message", Some("with_underscores"), "message"),
    ("(CamelCase) message", Some("CamelCase"), "message"),
    ("(numbers123) message", Some("numbers123"), "message"),
    ("(feature/sub) message", Some("feature/sub"), "message"),
    ("((nested)) message", Some("(nested"), ") message"),
    ("prefix (middle) suffix", None, "prefix (middle) suffix"),
    ("(empty-message)", Some("empty-message"), ""),
    ("no parentheses", None, "no parentheses"),
    ("(no closing paren", None, "(no closing paren"),
    ("no opening paren)", None, "no opening paren)"),
    ("()", None, "()"),
    ("", None, ""),
  ];

  for (i, (subject, expected_prefix, expected_stripped)) in test_cases.into_iter().enumerate() {
    let mut grouper = CommitGrouper::new();
    let commit = create_test_commit(&format!("commit{i}"), subject);
    grouper.add_commit(commit);
    let (grouped, unassigned) = grouper.finish();

    if let Some(prefix) = expected_prefix {
      assert!(grouped.contains_key(prefix), "Subject '{subject}' should have prefix '{prefix}'");

      let commits = grouped.get(prefix).unwrap();
      assert_eq!(commits.len(), 1);
      assert_eq!(
        commits[0].stripped_subject, expected_stripped,
        "Subject '{subject}' should be stripped to '{expected_stripped}'"
      );
      assert_eq!(unassigned.len(), 0);
    } else {
      assert_eq!(grouped.len(), 0, "Subject '{subject}' should not have any prefix");
      assert_eq!(unassigned.len(), 1);
    }
  }
}

#[test]
fn test_group_commits_with_issue_numbers() {
  let mut grouper = CommitGrouper::new();

  // Add commits with issue numbers
  grouper.add_commit(create_test_commit("1", "IJPL-163558: Fix observability"));
  grouper.add_commit(create_test_commit("2", "XYZ-1001: Improve performance"));
  grouper.add_commit(create_test_commit("3", "IJPL-163558: Enhance logging"));

  let (grouped, unassigned) = grouper.finish();

  assert_eq!(grouped.len(), 2, "Should have 2 groups");
  assert_eq!(unassigned.len(), 0, "Should have no unassigned commits");

  // Check IJPL-163558 group
  assert!(grouped.contains_key("IJPL-163558"));
  let ijpl_commits = grouped.get("IJPL-163558").unwrap();
  assert_eq!(ijpl_commits.len(), 2, "IJPL-163558 should have 2 commits");

  // Check XYZ-1001 group
  assert!(grouped.contains_key("XYZ-1001"));
  let xyz_commits = grouped.get("XYZ-1001").unwrap();
  assert_eq!(xyz_commits.len(), 1, "XYZ-1001 should have 1 commit");
}

#[test]
fn test_issue_numbers_with_square_brackets() {
  let mut grouper = CommitGrouper::new();

  // Add commits with square brackets and issue numbers
  grouper.add_commit(create_test_commit("1", "[threading] IJPL-163558: Fix observability"));
  grouper.add_commit(create_test_commit("2", "[subsystem] ABC-456: Update documentation"));
  grouper.add_commit(create_test_commit("3", "[threading] IJPL-163558: Enhance logging"));
  grouper.add_commit(create_test_commit("4", "[database] IJPL-163558: Update schema"));

  let (grouped, _unassigned) = grouper.finish();

  // Issue numbers should be detected even with square bracket prefixes
  assert_eq!(grouped.len(), 2, "Should have 2 issue groups");
  assert!(grouped.contains_key("IJPL-163558"));
  assert!(grouped.contains_key("ABC-456"));

  let ijpl_commits = grouped.get("IJPL-163558").unwrap();
  assert_eq!(ijpl_commits.len(), 3, "IJPL-163558 should have 3 commits");
}

#[test]
fn test_issue_numbers_only_in_first_line() {
  let mut grouper = CommitGrouper::new();

  // Issue number in subject line - should be detected
  grouper.add_commit(create_test_commit("1", "ABC-123: Fix authentication"));

  // Issue number only in body - should NOT be detected (subject has no issue)
  let mut commit2 = create_test_commit("2", "Refactor authentication module");
  commit2.message = "Refactor authentication module\n\nThis fixes issue DEF-456 which was causing login failures".to_string();
  grouper.add_commit(commit2);

  // Issue number in subject with square brackets - should be detected
  grouper.add_commit(create_test_commit("3", "[auth] GHI-789: Update login flow"));

  // No issue number in subject - should NOT be grouped
  let mut commit4 = create_test_commit("4", "Update dependencies");
  commit4.message = "Update dependencies\n\nResolves: JKL-111".to_string();
  grouper.add_commit(commit4);

  let (grouped, unassigned) = grouper.finish();

  assert_eq!(grouped.len(), 2, "Should have 2 groups (ABC-123 and GHI-789)");
  assert!(grouped.contains_key("ABC-123"));
  assert!(grouped.contains_key("GHI-789"));
  assert_eq!(unassigned.len(), 2, "Should have 2 unassigned commits");
}

#[test]
fn test_group_commits_by_prefix() {
  let mut grouper = CommitGrouper::new();

  // Add commits with different prefixes
  grouper.add_commit(create_test_commit("1", "(feature-auth) Add login functionality"));
  grouper.add_commit(create_test_commit("2", "(feature-auth) Add password reset"));
  grouper.add_commit(create_test_commit("3", "(bugfix) Fix memory leak"));
  grouper.add_commit(create_test_commit("4", "No prefix commit"));
  grouper.add_commit(create_test_commit("5", "(feature-auth) Add two-factor auth"));

  let (grouped, unassigned) = grouper.finish();

  assert_eq!(grouped.len(), 2, "Should have 2 groups");
  assert_eq!(unassigned.len(), 1, "Should have 1 unassigned commit");

  // Check feature-auth group
  assert!(grouped.contains_key("feature-auth"));
  let auth_commits = grouped.get("feature-auth").unwrap();
  assert_eq!(auth_commits.len(), 3, "feature-auth should have 3 commits");

  // Check bugfix group
  assert!(grouped.contains_key("bugfix"));
  let bugfix_commits = grouped.get("bugfix").unwrap();
  assert_eq!(bugfix_commits.len(), 1, "bugfix should have 1 commit");
}

#[test]
fn test_group_commits_by_prefix_preserves_order() {
  let mut grouper = CommitGrouper::new();

  // Add commits in specific order
  grouper.add_commit(create_test_commit("1", "(feature) First feature commit"));
  grouper.add_commit(create_test_commit("2", "(feature) Second feature commit"));
  grouper.add_commit(create_test_commit("3", "(bugfix) Bugfix commit"));
  grouper.add_commit(create_test_commit("4", "(feature) Third feature commit"));

  let (grouped, _unassigned) = grouper.finish();

  let feature_commits = grouped.get("feature").unwrap();
  assert_eq!(feature_commits[0].id, "1");
  assert_eq!(feature_commits[1].id, "2");
  assert_eq!(feature_commits[2].id, "4");
}

#[test]
fn test_group_commits_mixed_patterns() {
  let mut grouper = CommitGrouper::new();

  // Mix of parentheses prefixes and issue numbers
  grouper.add_commit(create_test_commit("1", "(feature) Add new feature"));
  grouper.add_commit(create_test_commit("2", "JIRA-123: Fix bug"));
  grouper.add_commit(create_test_commit("3", "(feature) Enhance feature"));
  grouper.add_commit(create_test_commit("4", "ABC-456: Update docs"));
  grouper.add_commit(create_test_commit("5", "No prefix or issue"));

  let (grouped, unassigned) = grouper.finish();

  assert_eq!(grouped.len(), 3, "Should have 3 groups");
  assert!(grouped.contains_key("feature"));
  assert!(grouped.contains_key("JIRA-123"));
  assert!(grouped.contains_key("ABC-456"));
  assert_eq!(unassigned.len(), 1, "Should have 1 unassigned");
}

#[test]
fn test_group_commits_by_prefix_handles_malformed_messages() {
  let mut grouper = CommitGrouper::new();

  // Add commits with various edge cases
  grouper.add_commit(create_test_commit("1", "(valid) Normal commit"));
  grouper.add_commit(create_test_commit("2", "()")); // Empty parentheses
  grouper.add_commit(create_test_commit("3", "(")); // Unclosed
  grouper.add_commit(create_test_commit("4", ")")); // No opening
  grouper.add_commit(create_test_commit("5", "")); // Empty
  grouper.add_commit(create_test_commit("6", "(valid-2) Another valid"));

  let (grouped, unassigned) = grouper.finish();

  assert_eq!(grouped.len(), 2, "Should have 2 valid groups");
  assert!(grouped.contains_key("valid"));
  assert!(grouped.contains_key("valid-2"));
  assert_eq!(unassigned.len(), 4, "Should have 4 malformed as unassigned");
}

#[test]
fn test_parentheses_only_at_beginning() {
  let mut grouper = CommitGrouper::new();

  // Test the fix for IJPL-191229 bug
  let long_message = "IJPL-191229 prepare to fix parallel blocking access to PathMacros (part 4 - remove default impl of `componentStore` in ComponentManagerImpl - avoid calling blocking getService, extract TestMutableComponentManager)";
  grouper.add_commit(create_test_commit("1", long_message));
  grouper.add_commit(create_test_commit("2", "IJPL-191229 continue refactoring of PathMacros"));
  grouper.add_commit(create_test_commit("3", "(hotfix) Emergency fix for login (critical)"));
  grouper.add_commit(create_test_commit("4", "Fix critical bug (should not be grouped)"));

  let (grouped, unassigned) = grouper.finish();

  // Check that IJPL-191229 is correctly grouped by issue number
  assert!(grouped.contains_key("IJPL-191229"), "Should have 'IJPL-191229' group");
  let ijpl_group = grouped.get("IJPL-191229").unwrap();
  assert_eq!(ijpl_group.len(), 2, "IJPL-191229 should have 2 commits");

  // Check hotfix group (parentheses at beginning)
  assert!(grouped.contains_key("hotfix"), "Should have 'hotfix' group");
  let hotfix_group = grouped.get("hotfix").unwrap();
  assert_eq!(hotfix_group.len(), 1);
  assert_eq!(hotfix_group[0].stripped_subject, "Emergency fix for login (critical)");

  // Check unassigned - should only have the "Fix critical bug" commit
  assert_eq!(unassigned.len(), 1, "Should have exactly 1 unassigned commit");
  assert!(unassigned[0].message.contains("Fix critical bug"));
}

#[test]
fn test_group_commits_edge_cases() {
  // Test with empty commit list
  let empty_commits: Vec<Commit> = vec![];
  let mut grouper = CommitGrouper::new();
  for commit in empty_commits {
    grouper.add_commit(commit);
  }
  let (grouped, unassigned) = grouper.finish();
  assert_eq!(grouped.len(), 0);
  assert_eq!(unassigned.len(), 0);

  // Test with commit that has empty message
  let mut grouper2 = CommitGrouper::new();
  grouper2.add_commit(create_test_commit("1", ""));
  let (grouped2, unassigned2) = grouper2.finish();
  assert_eq!(grouped2.len(), 0);
  assert_eq!(unassigned2.len(), 1);
}

#[test]
fn test_commit_count_tracking() {
  let mut grouper = CommitGrouper::new();
  assert_eq!(grouper.commit_count, 0);

  grouper.add_commit(create_test_commit("1", "(feature) First"));
  assert_eq!(grouper.commit_count, 1);

  grouper.add_commit(create_test_commit("2", "(feature) Second"));
  assert_eq!(grouper.commit_count, 2);

  grouper.add_commit(create_test_commit("3", "No prefix"));
  assert_eq!(grouper.commit_count, 3);
}

#[test]
fn test_oldest_commit_tracking() {
  let mut grouper = CommitGrouper::new();
  assert!(grouper.oldest_commit.is_none());

  let first_commit = create_test_commit("first", "(feature) First commit");
  grouper.add_commit(first_commit.clone());
  assert_eq!(grouper.oldest_commit.as_ref().unwrap().id, "first");

  // Adding more commits shouldn't change the oldest
  grouper.add_commit(create_test_commit("second", "(feature) Second"));
  grouper.add_commit(create_test_commit("third", "(feature) Third"));
  assert_eq!(grouper.oldest_commit.as_ref().unwrap().id, "first");
}
