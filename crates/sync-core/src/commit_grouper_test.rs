use crate::commit_grouper::{CommitGrouper, GroupedCommits, branch_name_of, stack_branch_name_of};
use crate::stack::{STACKS_CONFIG_KEY, StackForest, leaf_of, stack_of, stack_path, stacks_enabled};
use git_ops::commit_list::Commit;
use indexmap::IndexMap;
use sync_types::stack::{DeclaredParent, ParentState, StackProblem};
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
    author_timezone: String::new(),
    committer_timezone: String::new(),
    parent_id: None,
    tree_id: "tree123".to_string(),
    note: None,
    mapped_commit_id: None,
    mapped_source: None,
    mapped_resolution: None,
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
    let GroupedCommits {
      branches: grouped, unassigned, ..
    } = grouper.finish();

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

/// Finishes grouping and checks that the commits fell into exactly two branches, none unassigned
fn finish_into_two_groups(grouper: CommitGrouper) -> IndexMap<String, Vec<Commit>> {
  let GroupedCommits {
    branches: grouped, unassigned, ..
  } = grouper.finish();

  assert_eq!(grouped.len(), 2, "Should have 2 groups");
  assert_eq!(unassigned.len(), 0, "Should have no unassigned commits");
  grouped
}

#[test]
fn test_group_commits_with_issue_numbers() {
  let mut grouper = CommitGrouper::new();

  // Add commits with issue numbers
  grouper.add_commit(create_test_commit("1", "IJPL-163558: Fix observability"));
  grouper.add_commit(create_test_commit("2", "XYZ-1001: Improve performance"));
  grouper.add_commit(create_test_commit("3", "IJPL-163558: Enhance logging"));

  let grouped = finish_into_two_groups(grouper);

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

  let GroupedCommits { branches: grouped, .. } = grouper.finish();

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

  let GroupedCommits {
    branches: grouped, unassigned, ..
  } = grouper.finish();

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

  let GroupedCommits {
    branches: grouped, unassigned, ..
  } = grouper.finish();

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

  let GroupedCommits { branches: grouped, .. } = grouper.finish();

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

  let GroupedCommits { branches: grouped, .. } = grouper.finish();

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

  let GroupedCommits {
    branches: grouped, unassigned, ..
  } = grouper.finish();

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

  let GroupedCommits {
    branches: grouped, unassigned, ..
  } = grouper.finish();

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

  let GroupedCommits {
    branches: grouped, unassigned, ..
  } = grouper.finish();

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
  let GroupedCommits {
    branches: grouped, unassigned, ..
  } = grouper.finish();
  assert_eq!(grouped.len(), 0);
  assert_eq!(unassigned.len(), 0);

  // Test with commit that has empty message
  let mut grouper2 = CommitGrouper::new();
  grouper2.add_commit(create_test_commit("1", ""));
  let GroupedCommits {
    branches: grouped2,
    unassigned: unassigned2,
    ..
  } = grouper2.finish();
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
  assert!(grouper.first_parent_id.is_none());

  let mut first_commit = create_test_commit("first", "(feature) First commit");
  first_commit.parent_id = Some("base".to_string());
  grouper.add_commit(first_commit);
  assert_eq!(grouper.first_parent_id.as_deref(), Some("base"));

  // Adding more commits shouldn't change the parent of the oldest
  let mut second_commit = create_test_commit("second", "(feature) Second");
  second_commit.parent_id = Some("first".to_string());
  grouper.add_commit(second_commit);
  grouper.add_commit(create_test_commit("third", "(feature) Third"));
  assert_eq!(grouper.first_parent_id.as_deref(), Some("base"));
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

  let GroupedCommits {
    branches: grouped, unassigned, ..
  } = grouper.finish();

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

  let grouped = finish_into_two_groups(grouper);

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

  let grouped = finish_into_two_groups(grouper);

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
  // git requires the space after the marker: "fixup!(feature)" is an ordinary subject without a prefix
  grouper.add_commit(create_test_commit("2", "fixup!(feature) Add feature"));
  grouper.add_commit(create_test_commit("3", "fixup!  (feature) Add feature")); // Multiple spaces after the marker
  grouper.add_commit(create_test_commit("4", "fixup! squash! (feature) Add feature")); // Repeated markers

  // Test autosquash commits without any recognizable prefix in the base message
  grouper.add_commit(create_test_commit("5", "fixup! Regular commit message"));

  let GroupedCommits {
    branches: grouped, unassigned, ..
  } = grouper.finish();

  let feature_ids: Vec<&str> = grouped.get("feature").unwrap().iter().map(|commit| commit.id.as_str()).collect();
  assert_eq!(feature_ids, ["1", "3", "4"]);

  // Commits 2 and 5 should be unassigned (no prefix in base message)
  let unassigned_ids: Vec<&str> = unassigned.iter().map(|commit| commit.id.as_str()).collect();
  assert_eq!(unassigned_ids, ["2", "5"]);
}

#[test]
fn test_autosquash_commits_preserve_original_subject() {
  let mut grouper = CommitGrouper::new();

  // Add a fixup commit
  let fixup_commit = create_test_commit("1", "fixup! (feature) Add login");
  let original_subject = fixup_commit.subject.clone();
  grouper.add_commit(fixup_commit);

  let GroupedCommits { branches: grouped, .. } = grouper.finish();

  // The commit should be grouped correctly
  assert!(grouped.contains_key("feature"));
  let commits = grouped.get("feature").unwrap();
  assert_eq!(commits.len(), 1);

  // The original subject should be preserved
  assert_eq!(commits[0].subject, original_subject);

  // The stripped_subject should only contain the message without prefix
  assert_eq!(commits[0].stripped_subject, "Add login");
}

// Stacked branches

fn names(list: &[&str]) -> Vec<String> {
  list.iter().map(|name| (*name).to_string()).collect()
}

/// Groups subjects with the ids `c0`, `c1`, … (oldest first), stacked branches on
fn group_stacked(subjects: &[&str]) -> (GroupedCommits, StackForest) {
  let mut grouper = CommitGrouper::new_with_stacks(true);
  for (index, subject) in subjects.iter().enumerate() {
    grouper.add_commit(create_test_commit(&format!("c{index}"), subject));
  }
  let (grouped, forest) = grouper.finish_with_stacks();
  (grouped, forest.expect("stacks are on"))
}

fn ids(grouped: &GroupedCommits, branch: &str) -> Vec<String> {
  grouped.branches[branch].iter().map(|commit| commit.id.clone()).collect()
}

#[test]
fn stack_prefixes_split_into_sanitized_levels() {
  assert_eq!(stack_of("auth/login"), Ok(("login".to_string(), Some("auth".to_string()))));
  assert_eq!(stack_of("login"), Ok(("login".to_string(), None)));
  assert_eq!(stack_path("auth/login/oauth"), Ok(names(&["auth", "login", "oauth"])));
  assert_eq!(stack_path(" my auth / log in "), Ok(names(&["my-auth", "log-in"])));
  // empty levels at the ends are dropped
  assert_eq!(stack_of("auth/"), Ok(("auth".to_string(), None)));
  assert_eq!(stack_of("/login"), Ok(("login".to_string(), None)));
  assert_eq!(leaf_of("auth/login"), Some("login".to_string()));
}

#[test]
fn an_empty_stack_level_is_a_syntax_error() {
  let error = stack_of("a//b").unwrap_err();
  assert_eq!(error.leaf.as_deref(), Some("b"));
  assert_eq!(error.to_string(), "(a//b) has an empty stack level");
  // a level that sanitizes to nothing is empty too
  assert_eq!(stack_path("a/~/b").unwrap_err().leaf.as_deref(), Some("b"));
  assert_eq!(stack_path("/").unwrap_err().leaf, None);
  assert_eq!(leaf_of("a//b"), None);
}

#[test]
fn without_stacks_a_slash_is_part_of_a_flat_name() {
  let mut grouper = CommitGrouper::new();
  grouper.add_commit(create_test_commit("c0", "(auth/login) Add form"));
  grouper.add_commit(create_test_commit("c1", "(a//b) Odd"));
  let (grouped, forest) = grouper.finish_with_stacks();
  assert!(forest.is_none());
  assert_eq!(grouped.branches.keys().collect::<Vec<_>>(), ["auth/login", "a//b"]);
  assert_eq!(grouped.branches["auth/login"][0].stripped_subject, "Add form");
}

#[test]
fn groups_by_leaf_and_records_the_declared_parents() {
  let (grouped, forest) = group_stacked(&[
    "(auth) Add auth",
    "(auth/login) Add form",
    "(docs) Document",
    // a bare prefix joins the branch wherever it is declared
    "(login) Validate form",
    "(auth/login/oauth) Add OAuth",
  ]);

  assert_eq!(grouped.branches.keys().collect::<Vec<_>>(), ["auth", "login", "docs", "oauth"]);
  assert_eq!(ids(&grouped, "login"), ["c1", "c3"]);
  assert_eq!(grouped.branches["oauth"][0].stripped_subject, "Add OAuth");
  assert_eq!(forest.declared_parent("auth"), None);
  assert_eq!(forest.declared_parent("login"), Some("auth"));
  assert_eq!(forest.declared_parent("oauth"), Some("login"));
  assert_eq!(forest.parent_state("login"), Some(ParentState::Active));
  assert_eq!(forest.parent_state("auth"), None);
  assert_eq!(forest.order, ["auth", "login", "docs", "oauth"]);
  assert!(forest.nodes.values().all(|node| node.problems.is_empty() && !node.child_precedes_parent));
}

#[test]
fn a_parent_without_commits_is_missing() {
  let (grouped, forest) = group_stacked(&["(auth/login/oauth) Add OAuth", "(login) Add form"]);

  assert_eq!(grouped.branches.keys().collect::<Vec<_>>(), ["oauth", "login"]);
  // the deeper level declares the parent of `login`
  assert_eq!(forest.declared_parent("login"), Some("auth"));
  assert_eq!(forest.parent_state("login"), Some(ParentState::Missing));
  assert_eq!(forest.parent_state("oauth"), Some(ParentState::Active));
  assert!(!forest.nodes["auth"].has_commits);
  assert_eq!(forest.nodes.keys().collect::<Vec<_>>(), ["auth", "login", "oauth"]);
  assert_eq!(forest.order, ["login", "oauth"]);
}

#[test]
fn orders_parents_first_and_flags_a_child_that_comes_first() {
  let (_, forest) = group_stacked(&["(x) X", "(a/b) B", "(x/y) Y", "(a) A"]);

  assert_eq!(forest.order, ["x", "a", "b", "y"]);
  assert!(forest.nodes["b"].child_precedes_parent);
  assert!(!forest.nodes["y"].child_precedes_parent);
}

#[test]
fn conflicting_parents_name_their_commits() {
  let (grouped, forest) = group_stacked(&["(auth/login) Add form", "(api/login) Call API", "(auth/login) Validate", "(docs) Document"]);

  assert_eq!(ids(&grouped, "login"), ["c0", "c1", "c2"]);
  assert_eq!(forest.declared_parent("login"), None);
  assert_eq!(
    forest.problems("login"),
    [StackProblem::ConflictingParents(vec![
      DeclaredParent {
        parent: "auth".to_string(),
        commits: names(&["c0", "c2"]),
      },
      DeclaredParent {
        parent: "api".to_string(),
        commits: names(&["c1"]),
      },
    ])]
  );
  // the other branches are not affected
  assert_eq!(forest.problems("docs"), []);
  assert!(forest.order.contains(&"login".to_string()) && forest.order.contains(&"docs".to_string()));
}

#[test]
fn a_branch_can_not_be_its_own_parent() {
  let (grouped, forest) = group_stacked(&["(a/a) Loop"]);

  assert_eq!(ids(&grouped, "a"), ["c0"]);
  assert_eq!(forest.declared_parent("a"), None);
  assert_eq!(forest.problems("a"), [StackProblem::SelfParent(names(&["c0"]))]);
  assert_eq!(forest.order, ["a"]);
}

#[test]
fn a_cycle_marks_each_of_its_branches() {
  let (_, forest) = group_stacked(&["(a/b) B", "(b/c) C", "(c/a) A", "(c/d) D", "(e) E"]);

  assert_eq!(forest.problems("a"), [StackProblem::Cycle(names(&["a", "c", "b"]))]);
  assert_eq!(forest.problems("b"), [StackProblem::Cycle(names(&["b", "a", "c"]))]);
  assert_eq!(forest.problems("c"), [StackProblem::Cycle(names(&["c", "b", "a"]))]);
  // a child of the cycle is not part of it
  assert_eq!(forest.problems("d"), []);
  assert_eq!(forest.problems("e"), []);
  assert_eq!(forest.order, ["a", "b", "c", "d", "e"]);
}

#[test]
fn leaves_that_differ_only_in_case_collide() {
  let (grouped, forest) = group_stacked(&["(Login) Add form", "(auth/login) Validate", "(docs) Document"]);

  assert_eq!(grouped.branches.keys().collect::<Vec<_>>(), ["Login", "login", "docs"]);
  assert_eq!(forest.problems("Login"), [StackProblem::LeafCollision("login".to_string())]);
  assert_eq!(forest.problems("login"), [StackProblem::LeafCollision("Login".to_string())]);
  assert_eq!(forest.problems("docs"), []);
}

#[test]
fn a_syntax_error_is_reported_on_the_last_level() {
  let (grouped, forest) = group_stacked(&["(a//b) Odd", "(b) Even", "(/) Nothing", "ABC-1 Fix bug"]);

  assert_eq!(ids(&grouped, "b"), ["c0", "c1"]);
  assert_eq!(forest.problems("b"), [StackProblem::Syntax(names(&["c0"]))]);
  // `(/)` names no branch
  assert_eq!(grouped.unassigned.iter().map(|commit| commit.id.as_str()).collect::<Vec<_>>(), ["c2"]);
  // an issue branch is a root
  assert_eq!(ids(&grouped, "ABC-1"), ["c3"]);
  assert_eq!(forest.declared_parent("ABC-1"), None);
  assert_eq!(forest.order, ["b", "ABC-1"]);
}

#[test]
fn stack_branch_names_are_leaves() {
  assert_eq!(stack_branch_name_of("(auth/login) Add form").as_deref(), Some("login"));
  assert_eq!(stack_branch_name_of("fixup! (auth/login) Add form").as_deref(), Some("login"));
  assert_eq!(stack_branch_name_of("ABC-1 Fix bug").as_deref(), Some("ABC-1"));
  assert_eq!(stack_branch_name_of("Plain"), None);
  assert_eq!(branch_name_of("(auth/login) Add form").as_deref(), Some("auth/login"));
}

#[test]
fn stacks_are_off_unless_configured() {
  let test_repo = test_utils::git_test_utils::TestRepo::new();
  let git_executor = test_repo.git_executor();
  let repo_path = test_repo.path().to_str().unwrap();

  assert!(!stacks_enabled(git_executor, repo_path).unwrap());
  for (value, enabled) in [("true", true), ("yes", true), ("false", false), ("off", false)] {
    test_repo.set_config(STACKS_CONFIG_KEY, value).unwrap();
    assert_eq!(stacks_enabled(git_executor, repo_path).unwrap(), enabled, "{value}");
  }
  test_repo.set_config(STACKS_CONFIG_KEY, "maybe").unwrap();
  stacks_enabled(git_executor, repo_path).unwrap_err();
}

#[test]
fn a_tie_of_authors_takes_the_one_seen_first() {
  // the email keys the remote status cache of the branch: a tie must not flip between groupings
  for _ in 0..32 {
    let mut grouper = CommitGrouper::new();
    for (id, email) in [("1", "zed@example.com"), ("2", "amy@example.com"), ("3", "amy@example.com"), ("4", "zed@example.com")] {
      let mut commit = create_test_commit(id, &format!("(feature) Commit {id}"));
      commit.author_email = email.to_string();
      grouper.add_commit(commit);
    }
    let GroupedCommits { branch_emails, .. } = grouper.finish();
    assert_eq!(branch_emails["feature"].as_deref(), Some("zed@example.com"));
  }
}
