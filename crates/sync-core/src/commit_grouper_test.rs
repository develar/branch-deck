use crate::commit_grouper::CommitGrouper;
use git_ops::commit_list::Commit;
use test_log::test;

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
    let (grouped, unassigned, _branch_emails) = grouper.finish();

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

  let (grouped, unassigned, _branch_emails) = grouper.finish();

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

  let (grouped, _unassigned, _branch_emails) = grouper.finish();

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

  let (grouped, unassigned, _branch_emails) = grouper.finish();

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

  let (grouped, unassigned, _branch_emails) = grouper.finish();

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

  let (grouped, _unassigned, _branch_emails) = grouper.finish();

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

  let (grouped, _unassigned, _branch_emails) = grouper.finish();

  assert_eq!(grouped.len(), 3, "Should have 3 groups");
  assert!(grouped.contains_key("feature"));
  assert!(grouped.contains_key("JIRA-123"));
  assert!(grouped.contains_key("ABC-456"));
}

#[test]
fn test_branch_name_sanitization() {
  let mut grouper = CommitGrouper::new();

  // Add commits with prefixes that need sanitization
  grouper.add_commit(create_test_commit("1", "(mockito 5.19) Fix test framework"));
  grouper.add_commit(create_test_commit("2", "(ui dispatcher) Update UI handler"));
  grouper.add_commit(create_test_commit("3", "(test~branch) Add tests"));
  grouper.add_commit(create_test_commit("4", "(feature/sub*path) New feature"));
  grouper.add_commit(create_test_commit("5", "(---test---) Edge case"));
  grouper.add_commit(create_test_commit("6", "(.dots.) Another edge case"));

  let (grouped, unassigned, _branch_emails) = grouper.finish();

  // Verify that branch names are sanitized
  assert!(grouped.contains_key("mockito-5.19"), "'mockito 5.19' should be sanitized to 'mockito-5.19'");
  assert!(grouped.contains_key("ui-dispatcher"), "'ui dispatcher' should be sanitized to 'ui-dispatcher'");
  assert!(grouped.contains_key("test-branch"), "'test~branch' should be sanitized to 'test-branch'");
  assert!(grouped.contains_key("feature/sub-path"), "'feature/sub*path' should be sanitized to 'feature/sub-path'");
  assert!(grouped.contains_key("test"), "'---test---' should be sanitized to 'test'");
  assert!(grouped.contains_key("dots"), "'.dots.' should be sanitized to 'dots'");

  // Verify the commits are properly grouped under sanitized names
  assert_eq!(grouped.get("mockito-5.19").unwrap().len(), 1);
  assert_eq!(grouped.get("ui-dispatcher").unwrap().len(), 1);
  assert_eq!(unassigned.len(), 0, "Should have 0 unassigned - all should be grouped under sanitized names");
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

  let (grouped, unassigned, _branch_emails) = grouper.finish();

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

  let (grouped, unassigned, _branch_emails) = grouper.finish();

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
  let (grouped, unassigned, _branch_emails) = grouper.finish();
  assert_eq!(grouped.len(), 0);
  assert_eq!(unassigned.len(), 0);

  // Test with commit that has empty message
  let mut grouper2 = CommitGrouper::new();
  grouper2.add_commit(create_test_commit("1", ""));
  let (grouped2, unassigned2, _branch_emails2) = grouper2.finish();
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

#[test]
fn test_autosquash_commits_with_parentheses_prefix() {
  let mut grouper = CommitGrouper::new();

  // Regular commits with parentheses prefix
  grouper.add_commit(create_test_commit("1", "(feature-auth) Add login functionality"));
  grouper.add_commit(create_test_commit("2", "(bugfix) Fix memory leak"));

  // Fixup commits that should be grouped with their base commits
  grouper.add_commit(create_test_commit("3", "fixup! (feature-auth) Add login functionality"));
  grouper.add_commit(create_test_commit("4", "squash! (bugfix) Fix memory leak"));
  grouper.add_commit(create_test_commit("5", "amend! (feature-auth) Add login functionality"));

  // Additional regular commits
  grouper.add_commit(create_test_commit("6", "(feature-auth) Add password reset"));

  let (grouped, unassigned, _branch_emails) = grouper.finish();

  // Should have 2 groups: feature-auth and bugfix
  assert_eq!(grouped.len(), 2, "Should have 2 groups");
  assert_eq!(unassigned.len(), 0, "Should have no unassigned commits");

  // Check feature-auth group - should have 4 commits (1, 3, 5, 6)
  assert!(grouped.contains_key("feature-auth"));
  let auth_commits = grouped.get("feature-auth").unwrap();
  assert_eq!(auth_commits.len(), 4, "feature-auth should have 4 commits (including fixup and amend)");
  assert_eq!(auth_commits[0].id, "1");
  assert_eq!(auth_commits[1].id, "3"); // fixup commit
  assert_eq!(auth_commits[2].id, "5"); // amend commit
  assert_eq!(auth_commits[3].id, "6");

  // Check bugfix group - should have 2 commits (2, 4)
  assert!(grouped.contains_key("bugfix"));
  let bugfix_commits = grouped.get("bugfix").unwrap();
  assert_eq!(bugfix_commits.len(), 2, "bugfix should have 2 commits (including squash)");
  assert_eq!(bugfix_commits[0].id, "2");
  assert_eq!(bugfix_commits[1].id, "4"); // squash commit
}

#[test]
fn test_autosquash_commits_with_issue_numbers() {
  let mut grouper = CommitGrouper::new();

  // Regular commits with issue numbers
  grouper.add_commit(create_test_commit("1", "JIRA-123: Fix authentication bug"));
  grouper.add_commit(create_test_commit("2", "ABC-456: Update documentation"));

  // Fixup commits with issue numbers
  grouper.add_commit(create_test_commit("3", "fixup! JIRA-123: Fix authentication bug"));
  grouper.add_commit(create_test_commit("4", "squash! ABC-456: Update documentation"));

  let (grouped, unassigned, _branch_emails) = grouper.finish();

  assert_eq!(grouped.len(), 2, "Should have 2 groups");
  assert_eq!(unassigned.len(), 0, "Should have no unassigned commits");

  // Check JIRA-123 group
  assert!(grouped.contains_key("JIRA-123"));
  let jira_commits = grouped.get("JIRA-123").unwrap();
  assert_eq!(jira_commits.len(), 2, "JIRA-123 should have 2 commits (including fixup)");

  // Check ABC-456 group
  assert!(grouped.contains_key("ABC-456"));
  let abc_commits = grouped.get("ABC-456").unwrap();
  assert_eq!(abc_commits.len(), 2, "ABC-456 should have 2 commits (including squash)");
}

#[test]
fn test_autosquash_commits_with_square_brackets_and_issue() {
  let mut grouper = CommitGrouper::new();

  // Regular commits with square brackets and issue numbers
  grouper.add_commit(create_test_commit("1", "[subsystem] ISSUE-123: Do stuff"));
  grouper.add_commit(create_test_commit("2", "[database] XYZ-789: Update schema"));

  // Fixup commits - should be grouped by issue number even with square brackets
  grouper.add_commit(create_test_commit("3", "fixup! [subsystem] ISSUE-123: Do stuff"));
  grouper.add_commit(create_test_commit("4", "squash! [database] XYZ-789: Update schema"));
  grouper.add_commit(create_test_commit("5", "amend! [subsystem] ISSUE-123: Do stuff"));

  let (grouped, unassigned, _branch_emails) = grouper.finish();

  assert_eq!(grouped.len(), 2, "Should have 2 groups");
  assert_eq!(unassigned.len(), 0, "Should have no unassigned commits");

  // Check ISSUE-123 group
  assert!(grouped.contains_key("ISSUE-123"));
  let issue_commits = grouped.get("ISSUE-123").unwrap();
  assert_eq!(issue_commits.len(), 3, "ISSUE-123 should have 3 commits (including fixup and amend)");

  // Check XYZ-789 group
  assert!(grouped.contains_key("XYZ-789"));
  let xyz_commits = grouped.get("XYZ-789").unwrap();
  assert_eq!(xyz_commits.len(), 2, "XYZ-789 should have 2 commits (including squash)");
}

#[test]
fn test_autosquash_commits_edge_cases() {
  let mut grouper = CommitGrouper::new();

  // Test various spacing scenarios
  grouper.add_commit(create_test_commit("1", "(feature) Add feature"));
  grouper.add_commit(create_test_commit("2", "fixup!(feature) Add feature")); // No space after prefix
  grouper.add_commit(create_test_commit("3", "fixup!  (feature) Add feature")); // Multiple spaces after prefix

  // Test autosquash commits without any recognizable prefix in the base message
  grouper.add_commit(create_test_commit("4", "fixup! Regular commit message"));

  let (grouped, unassigned, _branch_emails) = grouper.finish();

  // Commits 1, 2, 3 should be grouped under "feature"
  assert!(grouped.contains_key("feature"));
  let feature_commits = grouped.get("feature").unwrap();
  assert_eq!(feature_commits.len(), 3, "feature should have 3 commits");

  // Commit 4 should be unassigned (no prefix in base message)
  assert_eq!(unassigned.len(), 1, "Should have 1 unassigned commit");
  assert_eq!(unassigned[0].id, "4");
}

#[test]
fn test_autosquash_commits_preserve_original_subject() {
  let mut grouper = CommitGrouper::new();

  // Add a fixup commit
  let fixup_commit = create_test_commit("1", "fixup! (feature) Add login");
  let original_subject = fixup_commit.subject.clone();
  grouper.add_commit(fixup_commit);

  let (grouped, _unassigned, _branch_emails) = grouper.finish();

  // The commit should be grouped correctly
  assert!(grouped.contains_key("feature"));
  let commits = grouped.get("feature").unwrap();
  assert_eq!(commits.len(), 1);

  // The original subject should be preserved
  assert_eq!(commits[0].subject, original_subject);

  // The stripped_subject should only contain the message without prefix
  assert_eq!(commits[0].stripped_subject, "Add login");
}
