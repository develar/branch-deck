use super::cherry_pick_test_utils::{cherry_pick, expect_conflict_with_contents};
use super::merge_conflict::{ConflictContext, build_merge_conflict_info, count_conflicts};
use super::model::{ConflictContents, ConflictDetail};
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use test_log::test;
use test_utils::git_test_utils::{ConflictScenario, ConflictTestBuilder, TestRepo, setup_deletion_conflict};

/// The contents of the conflict the scenario must produce; the conflict count of each file is that of its markers
fn expect_conflict_contents(test_repo: &TestRepo, scenario: &ConflictScenario) -> ConflictContents {
  let (info, contents) = expect_conflict_with_contents(test_repo, scenario);
  let counts: Vec<(&str, u32)> = contents
    .conflicting_files
    .iter()
    .map(|detail| (detail.file.as_str(), count_conflicts(&detail.merged.content)))
    .collect();
  let reported: Vec<(&str, u32)> = info.conflicting_files.iter().map(|file| (file.file.as_str(), file.conflict_count)).collect();
  assert_eq!(reported, counts);
  contents
}

/// Expects the scenario to conflict in exactly one file and returns that file's conflict
fn expect_single_file_conflict(test_repo: &TestRepo, scenario: &ConflictScenario) -> ConflictDetail {
  let mut contents = expect_conflict_contents(test_repo, scenario);
  assert_eq!(contents.conflicting_files.len(), 1);
  contents.conflicting_files.remove(0)
}

/// The cherry side's `init` block in both `EternalEventStealer` conflicts: the event check moved to a new event type
const TRANSFERRED_WRITE_ACTION_INIT: &str = "init {
  IdeEventQueue.getInstance().addPostEventListener(
    { event ->
      if (event is InternalThreading.TransferredWriteActionEvent) {
        specialEvents.add(TransferredWriteActionWrapper(event))
    }
      false
  }, disposable)
}";

#[test]
fn test_conflict_hunks_extraction() {
  let test_repo = TestRepo::new();

  // Use ConflictTestBuilder to set up the conflict scenario
  let scenario = ConflictTestBuilder::new(&test_repo)
    .with_initial_state(vec![("test.txt", "line1\\nline2\\nline3\\nline4\\nline5\\n")], "Initial commit")
    .with_target_changes(vec![("test.txt", "line1\\nline2 modified by target\\nline3\\nline4\\nline5\\n")], "Target branch changes")
    .with_cherry_changes(vec![("test.txt", "line1\\nline2 modified by cherry\\nline3\\nline4\\nline5\\n")], "Cherry-pick changes")
    .build();

  // Attempt cherry-pick which should create a conflict
  let conflict_detail = &expect_single_file_conflict(&test_repo, &scenario);

  // Verify the file name
  assert_eq!(conflict_detail.file, "test.txt");

  // Debug: log conflict detail
  tracing::debug!(?conflict_detail, "Conflict detail");

  // the merged file carries the zdiff3 markers, the base included
  let merged = &conflict_detail.merged.content;
  for marker in ["<<<<<<<", "|||||||", "=======", ">>>>>>>"] {
    assert!(merged.contains(marker), "the merged file should contain {marker}: {merged}");
  }
  assert!(merged.contains("line2 modified by target"), "the merged file should contain the target change");
  assert!(merged.contains("line2 modified by cherry"), "the merged file should contain the cherry-pick change");
  // the change of the commit, with both contents
  assert!(conflict_detail.commit_diff.new_file.content.contains("line2 modified by cherry"));
}

#[test]
fn test_delete_modify_conflict() {
  let test_repo = TestRepo::new();

  // Use the helper function for deletion conflict setup
  let scenario = setup_deletion_conflict(&test_repo);

  // Attempt cherry-pick which should create a conflict
  let conflict_detail = &expect_single_file_conflict(&test_repo, &scenario);

  // Verify the file name
  assert_eq!(conflict_detail.file, "delete_me.txt");

  // the target deleted the file: git writes the cherry side without markers
  let merged = &conflict_detail.merged.content;
  assert!(
    merged.contains("modified content") || merged.contains("<<<<<<<"),
    "the merged file should hold the cherry side of a delete/modify conflict: {merged}"
  );
  // the change of the commit is what the view shows for it, with both contents
  assert!(!conflict_detail.commit_diff.hunks.is_empty(), "the commit changes the file");
  assert!(conflict_detail.commit_diff.new_file.content.contains("modified content"));

  println!("Delete/modify conflict detail: {conflict_detail:?}");
}

#[test]
fn test_no_conflict_no_hunks() {
  let test_repo = TestRepo::new();

  // Create initial commit
  let initial_commit_hash = test_repo.create_commit("Initial commit", "base.txt", "base content\n");

  // Create target branch with changes to one file
  let target_commit_hash = test_repo.create_commit("Target branch", "target.txt", "target content\n");

  // Reset to initial
  test_repo.reset_hard(&initial_commit_hash).unwrap();

  // Create cherry-pick with changes to different file (no conflict)
  let cherry_commit_hash = test_repo.create_commit("Cherry-pick", "cherry.txt", "cherry content\n");

  // Attempt cherry-pick - should succeed without conflicts
  let result = cherry_pick(&test_repo, &cherry_commit_hash, &target_commit_hash);

  // Should succeed
  assert!(result.is_ok(), "Cherry-pick should succeed without conflicts");
}

#[test]
fn test_get_merge_conflict_content_with_markers() {
  let test_repo = TestRepo::new();

  // Use ConflictTestBuilder to set up the conflict scenario
  let scenario = ConflictTestBuilder::new(&test_repo)
    .with_initial_state(vec![("test.js", "function hello() {\n  console.log('hello');\n}\n")], "Initial commit")
    .with_target_changes(vec![("test.js", "function hello() {\n  console.log('hello from target');\n}\n")], "Target branch changes")
    .with_cherry_changes(vec![("test.js", "function hello() {\n  console.log('hello from cherry');\n}\n")], "Cherry-pick changes")
    .build();

  // This function is not public, so we'll test it indirectly through the cherry-pick process
  let conflict_detail = &expect_single_file_conflict(&test_repo, &scenario);

  // The merged content should contain conflict markers
  let conflict_content = &conflict_detail.merged.content;

  // Check for standard conflict markers
  assert!(conflict_content.contains("<<<<<<<"), "Should contain conflict start marker");
  assert!(conflict_content.contains("======="), "Should contain conflict separator");
  assert!(conflict_content.contains(">>>>>>>"), "Should contain conflict end marker");

  // Check that both target and cherry content are present
  assert!(conflict_content.contains("hello from target"), "Should contain target branch content");
  assert!(conflict_content.contains("hello from cherry"), "Should contain cherry-pick content");

  // Content is untrimmed: indentation and the final newline are preserved
  assert!(conflict_content.starts_with("function hello() {\n"), "content: {conflict_content:?}");
  assert!(conflict_content.contains("\n  console.log('hello from target');\n"), "content: {conflict_content:?}");
  assert!(conflict_content.ends_with("}\n"), "content: {conflict_content:?}");

  println!("Conflict content with markers:\n{conflict_content}");
}

#[test]
fn test_conflict_detail_structure() {
  let test_repo = TestRepo::new();

  // Use ConflictTestBuilder to set up the conflict scenario
  let scenario = ConflictTestBuilder::new(&test_repo)
    .with_initial_state(
      vec![("main.js", "// Main function\nfunction main() {\n  let x = 1;\n  let y = 2;\n  return x + y;\n}\n")],
      "Initial commit",
    )
    .with_target_changes(
      vec![("main.js", "// Main function\nfunction main() {\n  let x = 10;\n  let y = 20;\n  return x + y;\n}\n")],
      "Target changes",
    )
    .with_cherry_changes(
      vec![("main.js", "// Main function\nfunction main() {\n  let x = 100;\n  let y = 200;\n  return x + y;\n}\n")],
      "Cherry changes",
    )
    .build();

  // Perform cherry-pick
  let conflict_info = expect_conflict_contents(&test_repo, &scenario);
  let conflict_detail = &conflict_info.conflicting_files[0];

  // the merged file: its name and language, the markers and both sides
  let merged = &conflict_detail.merged;
  assert_eq!(merged.file_name, "main.js");
  assert_eq!(merged.file_lang, "js");
  assert!(merged.content.contains("<<<<<<<") && merged.content.contains(">>>>>>>"));
  assert!(merged.content.contains("let x = 10;")); // target content
  assert!(merged.content.contains("let x = 100;")); // cherry content

  // the change of the commit, from its parent's version to its own
  assert_eq!(
    conflict_detail.commit_diff.old_file.content,
    "// Main function\nfunction main() {\n  let x = 1;\n  let y = 2;\n  return x + y;\n}\n"
  );
  assert_eq!(
    conflict_detail.commit_diff.new_file.content,
    "// Main function\nfunction main() {\n  let x = 100;\n  let y = 200;\n  return x + y;\n}\n"
  );
  assert_ne!(conflict_detail.commit_diff.hunks, Vec::<String>::new());
}

#[test]
fn test_multiple_conflicting_files_with_markers() {
  let test_repo = TestRepo::new();

  // Use ConflictTestBuilder to set up the conflict scenario with multiple files
  let scenario = ConflictTestBuilder::new(&test_repo)
    .with_initial_state(
      vec![("file1.txt", "line1\nline2\nline3\n"), ("file2.txt", "content1\ncontent2\ncontent3\n")],
      "Initial commit",
    )
    .with_target_changes(
      vec![("file1.txt", "line1\nline2 target\nline3\n"), ("file2.txt", "content1\ncontent2 target\ncontent3\n")],
      "Target changes",
    )
    .with_cherry_changes(
      vec![("file1.txt", "line1\nline2 cherry\nline3\n"), ("file2.txt", "content1\ncontent2 cherry\ncontent3\n")],
      "Cherry changes",
    )
    .build();

  // Perform cherry-pick
  let conflict_info = expect_conflict_contents(&test_repo, &scenario);
  assert_eq!(conflict_info.conflicting_files.len(), 2);

  // Check both files have conflict markers
  for conflict_detail in &conflict_info.conflicting_files {
    let conflict_content = &conflict_detail.merged.content;

    assert!(conflict_content.contains("<<<<<<<"), "File {} should contain conflict markers", conflict_detail.file);
    assert!(conflict_content.contains("======="), "File {} should contain conflict separator", conflict_detail.file);
    assert!(conflict_content.contains(">>>>>>>"), "File {} should contain conflict end marker", conflict_detail.file);

    if conflict_detail.file == "file1.txt" {
      assert!(conflict_content.contains("line2 target"));
      assert!(conflict_content.contains("line2 cherry"));
    } else if conflict_detail.file == "file2.txt" {
      assert!(conflict_content.contains("content2 target"));
      assert!(conflict_content.contains("content2 cherry"));
    }

    println!("Conflict in {}: {}", conflict_detail.file, conflict_content);
  }
}

#[test]
fn test_conflict_with_empty_file() {
  let test_repo = TestRepo::new();

  // Use ConflictTestBuilder to set up the conflict scenario
  let scenario = ConflictTestBuilder::new(&test_repo)
    .with_initial_state(vec![("empty.txt", "")], "Initial commit with empty file")
    .with_target_changes(vec![("empty.txt", "target content\n")], "Target adds content")
    .with_cherry_changes(vec![("empty.txt", "cherry content\n")], "Cherry adds content")
    .build();

  // Perform cherry-pick
  let conflict_info = expect_conflict_contents(&test_repo, &scenario);
  let conflict_detail = &conflict_info.conflicting_files[0];

  let conflict_content = &conflict_detail.merged.content;

  // Should have conflict markers and both contents
  assert!(conflict_content.contains("<<<<<<<"));
  assert!(conflict_content.contains("target content"));
  assert!(conflict_content.contains("cherry content"));
  assert!(conflict_content.contains(">>>>>>>"));

  println!("Empty file conflict: {conflict_content}");
}

#[test]
fn test_zdiff3_style_conflict_markers() {
  let test_repo = TestRepo::new();

  // Create initial commit with a more complex file
  let initial_content = r#"class EternalEventStealer {
private var enabled = false
private var counter = 0

init {
  IdeEventQueue.getInstance().addPostEventListener(
    { event ->
      if (event.toString().contains("RunnableWithTransferredWriteAction")) {
        val specialDispatchEvent = SpecialDispatchEvent(event)
        specialEvents.add(specialDispatchEvent)
        IdeEventQueue.getInstance().doPostEvent(specialDispatchEvent, true)
        return@addPostEventListener true
    }
      false
  }, disposable)
}

fun enable() {
  enabled = true
}
}"#;

  // Create target branch - modify the event handling logic and add enabled check
  let target_content = &initial_content.replace("if (event.toString()", "if (enabled && event.toString()");

  // Create cherry-pick - different change to event handling
  let cherry_content = &format!("class EternalEventStealer {{\nprivate var counter = 0\n\n{TRANSFERRED_WRITE_ACTION_INIT}\n}}");

  // Use ConflictTestBuilder to set up the conflict scenario
  let scenario = ConflictTestBuilder::new(&test_repo)
    .with_initial_state(vec![("Stealer.kt", initial_content)], "Initial commit")
    .with_target_changes(vec![("Stealer.kt", target_content)], "Add enabled check")
    .with_cherry_changes(vec![("Stealer.kt", cherry_content)], "Refactor to use new event type")
    .build();

  // Attempt cherry-pick
  let conflict_detail = &expect_single_file_conflict(&test_repo, &scenario);
  let conflict_content = &conflict_detail.merged.content;

  // Verify zdiff3 style markers (includes base content with |||||||)
  assert!(conflict_content.contains("<<<<<<<"), "Should contain conflict start marker");
  assert!(conflict_content.contains("|||||||"), "Should contain base separator for zdiff3 style");
  assert!(conflict_content.contains("======="), "Should contain conflict separator");
  assert!(conflict_content.contains(">>>>>>>"), "Should contain conflict end marker");

  // Debug output to see what's being generated
  tracing::debug!(length = conflict_content.len(), "Conflict content length");
}

#[test]
fn test_complex_multi_region_conflict() {
  let test_repo = TestRepo::new();

  // Create a file with multiple distinct regions that will conflict
  let initial_content = "function processEvent(event) {
// Region 1: Event validation
if (!event) {
  return false;
}

// Region 2: Event processing
const result = handleEvent(event);

// Region 3: Cleanup
cleanup();
return result;
}";

  // Target branch: modify regions 1 and 3
  let target_content = "function processEvent(event) {
// Region 1: Enhanced validation
if (!event || !event.isValid()) {
  console.error('Invalid event');
  return false;
}

// Region 2: Event processing
const result = handleEvent(event);

// Region 3: Enhanced cleanup
try {
  cleanup();
  notifyCompletion();
} catch (e) {
  logError(e);
}
return result;
}";

  // Cherry-pick: modify regions 1 and 2
  let cherry_content = "function processEvent(event) {
// Region 1: Type-safe validation
if (!event || typeof event !== 'object') {
  throw new TypeError('Event must be an object');
}

// Region 2: Async processing
const result = await handleEventAsync(event);

// Region 3: Cleanup
cleanup();
return result;
}";

  // Use ConflictTestBuilder to set up the conflict scenario
  let scenario = ConflictTestBuilder::new(&test_repo)
    .with_initial_state(vec![("processor.js", initial_content)], "Initial commit")
    .with_target_changes(vec![("processor.js", target_content)], "Enhance validation and cleanup")
    .with_cherry_changes(vec![("processor.js", cherry_content)], "Add type safety and async")
    .build();

  // Attempt cherry-pick
  let conflict_detail = &expect_single_file_conflict(&test_repo, &scenario);
  let conflict_content = &conflict_detail.merged.content;

  // Count conflict regions (each <<<<<<< marks a new conflict)
  let conflict_start_count = conflict_content.matches("<<<<<<<").count();
  tracing::debug!(conflict_content, "Conflict content");
  tracing::debug!(conflict_start_count, "Number of conflict regions found");
  // Git merge-tree may combine nearby conflicts into a single region
  assert!(conflict_start_count >= 1, "Should have at least 1 conflict region, found {conflict_start_count}");

  // Verify content from both branches is present
  assert!(conflict_content.contains("isValid()"), "Should contain target branch validation");
  assert!(conflict_content.contains("TypeError"), "Should contain cherry-pick type check");
  assert!(conflict_content.contains("handleEventAsync"), "Should contain cherry-pick async call");
  assert!(conflict_content.contains("notifyCompletion"), "Should contain target branch cleanup");
}

#[test]
fn test_real_world_kotlin_conflict() {
  let test_repo = TestRepo::new();

  // Simulate the real-world IntelliJ IDEA conflict scenario
  // Initial state - original code structure
  let initial_content = r#"private class EternalEventStealer(disposable: Disposable) {
@Volatile
private var enabled = false
private var counter = 0

private val specialEvents = LinkedBlockingQueue<ForcedEvent>()

init {
  IdeEventQueue.getInstance().addPostEventListener(
    { event ->
      if (event.toString().contains(",runnable=${ThreadingSupport.RunnableWithTransferredWriteAction.NAME}")) {
        val specialDispatchEvent = SpecialDispatchEvent(event)
        specialEvents.add(specialDispatchEvent)
        IdeEventQueue.getInstance().doPostEvent(specialDispatchEvent, true)
        return@addPostEventListener true
    }
      false
  }, disposable)
}

fun enable() {
  enabled = true
}

fun disable() {
  enabled = false
}
}"#;

  // Target branch - adds enabled check
  let target_content = &initial_content.replace("if (event.toString()", "if (enabled && event.toString()");

  // Cherry-pick - major refactoring with new event type
  let cherry_content = &format!(
    "private class EternalEventStealer(disposable: Disposable) {{\nprivate var counter = 0\n\nprivate val specialEvents = LinkedBlockingQueue<ForcedEvent>()\n\n{TRANSFERRED_WRITE_ACTION_INIT}\n}}"
  );

  // Use ConflictTestBuilder to set up the conflict scenario
  let scenario = ConflictTestBuilder::new(&test_repo)
    .with_initial_state(vec![("SuvorovProgress.kt", initial_content)], "Initial commit")
    .with_target_changes(vec![("SuvorovProgress.kt", target_content)], "Add enabled check")
    .with_cherry_changes(vec![("SuvorovProgress.kt", cherry_content)], "Refactor to new event system")
    .build();

  // Attempt cherry-pick
  let conflict_detail = &expect_single_file_conflict(&test_repo, &scenario);
  let conflict_content = &conflict_detail.merged.content;

  // Verify specific patterns from the real conflict
  assert!(conflict_content.contains("enabled &&"), "Should contain target's enabled check");
  assert!(
    conflict_content.contains("InternalThreading.TransferredWriteActionEvent"),
    "Should contain cherry's new event type"
  );
  tracing::debug!(conflict_content, "Real world conflict content");
  tracing::debug!(count = conflict_content.matches("<<<<<<<").count(), "Number of conflict regions found");

  tracing::debug!("Real-world conflict captured successfully");
}

// Tests for batch commit info functionality that's part of merge conflict handling

/// A conflicting path is a name, never pathspec magic: the change of the commit in `:colon.txt` is git's patch, not a
/// replacement of the whole file
#[test]
fn the_change_of_a_file_named_like_pathspec_magic_is_its_patch() {
  let test_repo = TestRepo::new();
  let base = test_repo.create_commit_in_index("Base", &[(":colon.txt", "1\n2\n3\n")]);
  let cherry = test_repo.create_commit_in_index("Change", &[(":colon.txt", "1\ntwo\n3\n")]);
  let files = [PathBuf::from(":colon.txt")];
  let ctx = ConflictContext {
    git_executor: test_repo.git_executor(),
    repo_path: test_repo.path_str(),
    conflict_files: &files,
    merge_tree_oid: &cherry,
    parent_commit_id: &base,
    target_commit_id: &base,
    cherry_commit_id: &cherry,
  };

  let details = build_merge_conflict_info(ctx).unwrap().1.conflicting_files;

  assert_eq!(details[0].commit_diff.hunks, ["--- a/:colon.txt\n+++ b/:colon.txt\n@@ -1,3 +1,3 @@\n 1\n-2\n+two\n 3"]);
  assert_eq!(details[0].commit_diff.old_file.content, "1\n2\n3\n");
}

/// A conflict whose commit needs an older commit the target lacks, all with fixed dates: the ids are the same in every
/// run. Returns the context's commits `(parent, target, cherry)`
fn conflict_with_a_missing_commit(test_repo: &TestRepo) -> (String, String, String) {
  let base = test_repo.create_commit_with_timestamp("Base", "f.txt", "1\n2\n3\n4\n5\n", Some(1_700_000_000));
  test_repo.create_commit_with_timestamp("Other", "other.txt", "other\n", Some(1_700_000_100));
  let missing = test_repo.create_commit_with_timestamp("(a) Missing\n\nThe body of the missing commit.", "f.txt", "1\n2\n3\n4 missing\n5\n", Some(1_700_000_200));
  let cherry = test_repo.create_commit_with_timestamp("(a) Cherry", "f.txt", "1\n2 cherry\n3\n4 missing\n5\n", Some(1_700_000_300));
  test_repo.checkout(&base).unwrap();
  let target = test_repo.create_commit_with_timestamp("Target", "f.txt", "1\n2 target\n3\n4\n5\n", Some(1_700_000_400));
  (missing, target, cherry)
}

/// The report of a conflict, its commits, contents, diffs and missing commits, from five processes whatever the
/// number of conflicting files and missing commits: one walk for the merge base and the divergence, one `log` for the
/// commits, one for the missing commits, one `cat-file` and one `diff-tree`
#[test]
fn the_report_of_a_conflict_costs_five_processes() {
  let test_repo = TestRepo::new();
  let (parent, target, cherry) = conflict_with_a_missing_commit(&test_repo);
  let merge_tree = test_repo
    .git_executor()
    .execute_command_accepting(
      &["-c", "merge.conflictStyle=zdiff3", "merge-tree", "--write-tree", "--merge-base", &parent, &target, &cherry],
      test_repo.path_str(),
      &[1],
    )
    .unwrap();
  let merge_tree = merge_tree.lines().next().unwrap().to_string();
  let files = [PathBuf::from("f.txt")];
  let git = test_repo.git_executor().with_process_stats("test");

  let (info, contents) = build_merge_conflict_info(ConflictContext {
    git_executor: &git,
    repo_path: test_repo.path_str(),
    conflict_files: &files,
    merge_tree_oid: &merge_tree,
    parent_commit_id: &parent,
    target_commit_id: &target,
    cherry_commit_id: &cherry,
  })
  .unwrap();

  let processes: Vec<(String, u32)> = git
    .process_stats()
    .unwrap()
    .by_command()
    .into_iter()
    .map(|(command, stats)| (command, stats.count))
    .collect();
  let expected = [("cat-file", 1), ("diff-tree", 1), ("log", 2), ("rev-list", 1)];
  assert_eq!(processes, expected.map(|(command, count)| (command.to_string(), count)));

  assert_eq!(
    (info.commit.message.as_str(), info.original_parent.hash.as_str(), info.target_branch.message.as_str()),
    ("(a) Cherry", parent.as_str(), "Target")
  );
  assert_eq!(
    info.conflicting_files.iter().map(|file| (file.file.as_str(), file.conflict_count)).collect::<Vec<_>>(),
    [("f.txt", 1)]
  );
  let detail = &contents.conflicting_files[0];
  assert_eq!(
    detail.merged.content,
    format!("1\n<<<<<<< {target}\n2 target\n||||||| {parent}\n2\n=======\n2 cherry\n>>>>>>> {cherry}\n3\n4\n5\n")
  );
  assert_eq!(
    (detail.commit_diff.old_file.content.as_str(), detail.commit_diff.new_file.content.as_str()),
    ("1\n2\n3\n4\n5\n", "1\n2 cherry\n3\n4 missing\n5\n")
  );
  assert_eq!(
    detail.commit_diff.hunks,
    ["--- a/f.txt\n+++ b/f.txt\n@@ -1,5 +1,5 @@\n 1\n-2\n+2 cherry\n 3\n-4\n+4 missing\n 5"]
  );

  let analysis = &info.conflict_analysis;
  assert_eq!((analysis.merge_base_subject.as_str(), analysis.merge_base_time), ("Base", 1_700_000_000));
  let summary = &analysis.divergence_summary;
  assert_eq!(
    (summary.commits_ahead_in_source, summary.commits_ahead_in_target, summary.common_ancestor_distance),
    (2, 1, 1)
  );
  let [missing] = analysis.missing_commits.as_slice() else {
    panic!("one missing commit: {:?}", analysis.missing_commits)
  };
  assert_eq!(
    (missing.hash.as_str(), missing.message.as_str()),
    (parent.as_str(), "(a) Missing\n\nThe body of the missing commit.")
  );
  let diffs: Vec<&str> = contents.missing_commit_diffs.keys().map(String::as_str).collect();
  assert_eq!(diffs, [parent.as_str()]);
  let diff = &contents.missing_commit_diffs[&missing.hash][0];
  assert_eq!(
    (diff.old_file.content.as_str(), diff.new_file.content.as_str()),
    ("1\n2\n3\n4\n5\n", "1\n2\n3\n4 missing\n5\n")
  );
  assert!(diff.hunks[0].starts_with("diff --git a/f.txt b/f.txt\n"), "{:?}", diff.hunks);
  assert!(diff.hunks[0].ends_with("@@ -1,5 +1,5 @@\n 1\n 2\n 3\n-4\n+4 missing\n 5\n"), "{:?}", diff.hunks);
}

/// A conflict is counted by its start marker, exactly 7 characters, alone or before a space
#[test]
fn the_conflicts_of_a_merged_file_are_counted_by_their_start_markers() {
  let merged = "a\n<<<<<<< 40ee0032\nten\n||||||| afd93b3c\nthirty\n=======\nthirty seconds\n>>>>>>> c82db1ff\n<<<<<<<< not a marker\n";
  assert_eq!(count_conflicts(merged), 1);
  assert_eq!(count_conflicts("<<<<<<<\none\n=======\ntwo\n>>>>>>>\n<<<<<<< x\r\n"), 2);
  assert_eq!(count_conflicts("no conflict\n"), 0);
}
