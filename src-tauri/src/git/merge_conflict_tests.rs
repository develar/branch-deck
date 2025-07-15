#[cfg(test)]
mod tests {
  use super::super::cache::TreeIdCache;
  use super::super::cherry_pick::perform_fast_cherry_pick_with_context;
  use super::super::copy_commit::CopyCommitError;
  use super::super::model::BranchError;
  use crate::test_utils::git_test_utils::{ConflictTestBuilder, TestRepo, setup_deletion_conflict};

  #[test]
  fn test_conflict_hunks_extraction() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Use ConflictTestBuilder to set up the conflict scenario
    let scenario = ConflictTestBuilder::new(&test_repo)
      .with_initial_state(vec![("test.txt", "line1\\nline2\\nline3\\nline4\\nline5\\n")], "Initial commit")
      .with_target_changes(vec![("test.txt", "line1\\nline2 modified by target\\nline3\\nline4\\nline5\\n")], "Target branch changes")
      .with_cherry_changes(vec![("test.txt", "line1\\nline2 modified by cherry\\nline3\\nline4\\nline5\\n")], "Cherry-pick changes")
      .build();

    // Attempt cherry-pick which should create a conflict
    let cache = TreeIdCache::new();
    let result = perform_fast_cherry_pick_with_context(
      git_executor,
      test_repo.path().to_str().unwrap(),
      &scenario.cherry_commit,
      &scenario.target_commit,
      None,
      &cache,
    );

    // Verify it's a conflict
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      // Check that we have conflicting files
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];

      // Verify the file name
      assert_eq!(conflict_detail.file, "test.txt");

      // Debug: print conflict detail
      println!("Conflict detail: {conflict_detail:?}");

      // Verify we have a full diff
      let file_diff = &conflict_detail.file_diff;
      // old_file should be empty (to show conflict markers as additions)
      assert_eq!(file_diff.old_file.content, "");
      assert!(file_diff.new_file.content.contains("<<<<<<<"), "new_file should contain conflict markers");

      // Verify we have conflict hunks in file_diff
      assert!(!file_diff.hunks.is_empty(), "Should have conflict hunks in file_diff");

      // Check that conflict hunks contain zdiff3 conflict markers
      let has_conflict_markers = file_diff.hunks.iter().any(|hunk| {
        hunk.contains("<<<<<<<") && 
                hunk.contains("|||||||") && // zdiff3 includes base 
                hunk.contains("=======") && 
                hunk.contains(">>>>>>>")
      });
      assert!(has_conflict_markers, "Conflict hunks should contain zdiff3 conflict markers");

      // Verify the conflict content references the actual conflicting lines
      let has_target_change = file_diff.hunks.iter().any(|hunk| hunk.contains("line2 modified by target"));
      let has_cherry_change = file_diff.hunks.iter().any(|hunk| hunk.contains("line2 modified by cherry"));
      let has_base_content = file_diff.hunks.iter().any(|hunk| hunk.contains("line2")); // Original line2

      assert!(has_target_change, "Conflict hunks should contain target branch changes");
      assert!(has_cherry_change, "Conflict hunks should contain cherry-pick changes");
      assert!(has_base_content, "Conflict hunks should contain base content (zdiff3)");

      // Print conflict hunks for debugging
      println!("\\nConflict hunks for test.txt:");
      for (i, hunk) in file_diff.hunks.iter().enumerate() {
        println!("\\nHunk {}:\\n{}", i + 1, hunk);
      }
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_delete_modify_conflict() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Use the helper function for deletion conflict setup
    let scenario = setup_deletion_conflict(&test_repo);

    // Attempt cherry-pick which should create a conflict
    let cache = TreeIdCache::new();
    let result = perform_fast_cherry_pick_with_context(
      git_executor,
      test_repo.path().to_str().unwrap(),
      &scenario.cherry_commit,
      &scenario.target_commit,
      None,
      &cache,
    );

    // Verify it's a conflict
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      // Check that we have conflicting files
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];

      // Verify the file name
      assert_eq!(conflict_detail.file, "delete_me.txt");

      // Verify we have a full diff showing the modification
      let file_diff = &conflict_detail.file_diff;
      // In delete/modify conflict: old_file is empty (deleted in target), new_file has conflict content with markers
      assert_eq!(file_diff.old_file.content, ""); // Target branch deleted the file

      // Debug: print content to understand what we're getting
      println!("Delete/modify conflict new_file content: '{}'", file_diff.new_file.content);

      // For delete/modify conflicts, git might output the cherry content directly without standard conflict markers
      // since one side is deleted. Check if it contains the cherry content.
      assert!(
        file_diff.new_file.content.contains("modified content") || file_diff.new_file.content.contains("<<<<<<<"),
        "new_file should contain conflict content for delete/modify conflict"
      );

      // For delete/modify conflicts, we should have diff hunks in file_diff
      assert!(!file_diff.hunks.is_empty(), "Should have conflict hunks for delete/modify conflict");

      println!("Delete/modify conflict detail: {conflict_detail:?}");
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_no_conflict_no_hunks() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create initial commit
    let initial_commit_hash = test_repo.create_commit("Initial commit", "base.txt", "base content\n");

    // Create target branch with changes to one file
    let target_commit_hash = test_repo.create_commit("Target branch", "target.txt", "target content\n");

    // Reset to initial
    test_repo.reset_hard(&initial_commit_hash).unwrap();

    // Create cherry-pick with changes to different file (no conflict)
    let cherry_commit_hash = test_repo.create_commit("Cherry-pick", "cherry.txt", "cherry content\n");

    // Attempt cherry-pick - should succeed without conflicts
    let cache = TreeIdCache::new();
    let result = perform_fast_cherry_pick_with_context(git_executor, test_repo.path().to_str().unwrap(), &cherry_commit_hash, &target_commit_hash, None, &cache);

    // Should succeed
    assert!(result.is_ok(), "Cherry-pick should succeed without conflicts");
  }

  #[test]
  fn test_get_merge_conflict_content_with_markers() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Use ConflictTestBuilder to set up the conflict scenario
    let scenario = ConflictTestBuilder::new(&test_repo)
      .with_initial_state(vec![("test.js", "function hello() {\n  console.log('hello');\n}\n")], "Initial commit")
      .with_target_changes(vec![("test.js", "function hello() {\n  console.log('hello from target');\n}\n")], "Target branch changes")
      .with_cherry_changes(vec![("test.js", "function hello() {\n  console.log('hello from cherry');\n}\n")], "Cherry-pick changes")
      .build();

    // This function is not public, so we'll test it indirectly through the cherry-pick process
    let cache = TreeIdCache::new();
    let result = perform_fast_cherry_pick_with_context(
      git_executor,
      test_repo.path().to_str().unwrap(),
      &scenario.cherry_commit,
      &scenario.target_commit,
      None,
      &cache,
    );

    // Verify it's a conflict and check the file_diff content
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];

      // Verify we have the conflict file_diff
      let file_diff = &conflict_detail.file_diff;

      // The new_file content should contain conflict markers
      let conflict_content = &file_diff.new_file.content;

      // Check for standard conflict markers
      assert!(conflict_content.contains("<<<<<<<"), "Should contain conflict start marker");
      assert!(conflict_content.contains("======="), "Should contain conflict separator");
      assert!(conflict_content.contains(">>>>>>>"), "Should contain conflict end marker");

      // Check that both target and cherry content are present
      assert!(conflict_content.contains("hello from target"), "Should contain target branch content");
      assert!(conflict_content.contains("hello from cherry"), "Should contain cherry-pick content");

      // Verify hunks are properly formatted for git-diff-view
      assert!(!file_diff.hunks.is_empty(), "Should have diff hunks");
      let hunk = &file_diff.hunks[0];
      assert!(hunk.contains("@@"), "Hunk should contain @@ markers");
      assert!(hunk.contains("+"), "Hunk should show added lines with conflict markers");

      println!("Conflict content with markers:\n{conflict_content}");
      println!("Diff hunk:\n{hunk}");
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_conflict_detail_file_diff_structure() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

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
    let cache = TreeIdCache::new();
    let result = perform_fast_cherry_pick_with_context(
      git_executor,
      test_repo.path().to_str().unwrap(),
      &scenario.cherry_commit,
      &scenario.target_commit,
      None,
      &cache,
    );

    // Verify conflict structure
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      let conflict_detail = &conflict_info.conflicting_files[0];

      // Test file_diff structure (shows original -> conflict content)
      let file_diff = &conflict_detail.file_diff;

      // old_file should be empty (to show conflict markers as additions)
      assert_eq!(file_diff.old_file.content, "");
      assert_eq!(file_diff.old_file.file_name, "main.js");
      assert_eq!(file_diff.old_file.file_lang, "js");

      // new_file should contain the conflict content with markers
      let conflict_content = &file_diff.new_file.content;
      assert!(conflict_content.contains("<<<<<<<"));
      assert!(conflict_content.contains("======="));
      assert!(conflict_content.contains(">>>>>>>"));
      assert!(conflict_content.contains("let x = 10;")); // target content
      assert!(conflict_content.contains("let x = 100;")); // cherry content

      // Test 3-way merge view structure
      assert!(conflict_detail.base_file.is_some());
      assert!(conflict_detail.target_file.is_some());
      assert!(conflict_detail.cherry_file.is_some());

      let base_file = conflict_detail.base_file.as_ref().unwrap();
      let target_file = conflict_detail.target_file.as_ref().unwrap();
      let cherry_file = conflict_detail.cherry_file.as_ref().unwrap();

      // Verify 3-way content
      assert_eq!(base_file.content, "// Main function\nfunction main() {\n  let x = 1;\n  let y = 2;\n  return x + y;\n}\n");
      assert_eq!(
        target_file.content,
        "// Main function\nfunction main() {\n  let x = 10;\n  let y = 20;\n  return x + y;\n}\n"
      );
      assert_eq!(
        cherry_file.content,
        "// Main function\nfunction main() {\n  let x = 100;\n  let y = 200;\n  return x + y;\n}\n"
      );

      // Verify diff hunks for 3-way view
      assert!(!conflict_detail.base_to_target_diff.hunks.is_empty());
      assert!(!conflict_detail.base_to_cherry_diff.hunks.is_empty());

      println!("File diff old (target): {}", file_diff.old_file.content);
      println!("File diff new (conflict): {}", file_diff.new_file.content);
      println!("Hunks: {:?}", file_diff.hunks);
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_multiple_conflicting_files_with_markers() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

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
    let cache = TreeIdCache::new();
    let result = perform_fast_cherry_pick_with_context(
      git_executor,
      test_repo.path().to_str().unwrap(),
      &scenario.cherry_commit,
      &scenario.target_commit,
      None,
      &cache,
    );

    // Verify multiple conflicts
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      assert_eq!(conflict_info.conflicting_files.len(), 2);

      // Check both files have conflict markers
      for conflict_detail in &conflict_info.conflicting_files {
        let file_diff = &conflict_detail.file_diff;
        let conflict_content = &file_diff.new_file.content;

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

        // Verify hunks exist for git-diff-view
        assert!(!file_diff.hunks.is_empty(), "File {} should have diff hunks", conflict_detail.file);

        println!("Conflict in {}: {}", conflict_detail.file, conflict_content);
      }
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_conflict_with_empty_file() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Use ConflictTestBuilder to set up the conflict scenario
    let scenario = ConflictTestBuilder::new(&test_repo)
      .with_initial_state(vec![("empty.txt", "")], "Initial commit with empty file")
      .with_target_changes(vec![("empty.txt", "target content\n")], "Target adds content")
      .with_cherry_changes(vec![("empty.txt", "cherry content\n")], "Cherry adds content")
      .build();

    // Perform cherry-pick
    let cache = TreeIdCache::new();
    let result = perform_fast_cherry_pick_with_context(
      git_executor,
      test_repo.path().to_str().unwrap(),
      &scenario.cherry_commit,
      &scenario.target_commit,
      None,
      &cache,
    );

    // Verify conflict with proper content
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      let conflict_detail = &conflict_info.conflicting_files[0];

      let file_diff = &conflict_detail.file_diff;
      let conflict_content = &file_diff.new_file.content;

      // Should have conflict markers and both contents
      assert!(conflict_content.contains("<<<<<<<"));
      assert!(conflict_content.contains("target content"));
      assert!(conflict_content.contains("cherry content"));
      assert!(conflict_content.contains(">>>>>>>"));

      // old_file should be empty (to show conflict markers as additions)
      assert_eq!(file_diff.old_file.content, "");

      println!("Empty file conflict: {conflict_content}");
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_zdiff3_style_conflict_markers() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

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
    let target_content = r#"class EternalEventStealer {
  private var enabled = false
  private var counter = 0

  init {
    IdeEventQueue.getInstance().addPostEventListener(
      { event ->
        if (enabled && event.toString().contains("RunnableWithTransferredWriteAction")) {
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

    // Create cherry-pick - different change to event handling
    let cherry_content = r#"class EternalEventStealer {
  private var counter = 0

  init {
    IdeEventQueue.getInstance().addPostEventListener(
      { event ->
        if (event is InternalThreading.TransferredWriteActionEvent) {
          specialEvents.add(TransferredWriteActionWrapper(event))
        }
        false
      }, disposable)
  }
}"#;

    // Use ConflictTestBuilder to set up the conflict scenario
    let scenario = ConflictTestBuilder::new(&test_repo)
      .with_initial_state(vec![("Stealer.kt", initial_content)], "Initial commit")
      .with_target_changes(vec![("Stealer.kt", target_content)], "Add enabled check")
      .with_cherry_changes(vec![("Stealer.kt", cherry_content)], "Refactor to use new event type")
      .build();

    // Attempt cherry-pick
    let cache = TreeIdCache::new();
    let result = perform_fast_cherry_pick_with_context(
      git_executor,
      test_repo.path().to_str().unwrap(),
      &scenario.cherry_commit,
      &scenario.target_commit,
      None,
      &cache,
    );

    // Should produce conflicts
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];
      let conflict_content = &conflict_detail.file_diff.new_file.content;

      // Verify zdiff3 style markers (includes base content with |||||||)
      assert!(conflict_content.contains("<<<<<<<"), "Should contain conflict start marker");
      assert!(conflict_content.contains("|||||||"), "Should contain base separator for zdiff3 style");
      assert!(conflict_content.contains("======="), "Should contain conflict separator");
      assert!(conflict_content.contains(">>>>>>>"), "Should contain conflict end marker");

      // Verify that old_file content is empty (to show markers as additions)
      assert_eq!(conflict_detail.file_diff.old_file.content, "", "old_file should be empty");

      // Debug output to see what's being generated
      eprintln!("Conflict content length: {}", conflict_content.len());
      eprintln!("Number of hunks: {}", conflict_detail.file_diff.hunks.len());
      eprintln!("Hunks: {:?}", conflict_detail.file_diff.hunks);

      // Verify hunks show conflict markers as additions
      assert!(!conflict_detail.file_diff.hunks.is_empty(), "Should have hunks");
      let hunk_content = conflict_detail.file_diff.hunks.join("\n");
      assert!(hunk_content.contains("+<<<<<<<"), "Conflict markers should appear as additions");
    } else {
      panic!("Expected MergeConflict error");
    }
  }

  #[test]
  fn test_complex_multi_region_conflict() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create a file with multiple distinct regions that will conflict
    let initial_content = r#"function processEvent(event) {
  // Region 1: Event validation
  if (!event) {
    return false;
  }

  // Region 2: Event processing
  const result = handleEvent(event);
  
  // Region 3: Cleanup
  cleanup();
  return result;
}"#;

    // Target branch: modify regions 1 and 3
    let target_content = r#"function processEvent(event) {
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
}"#;

    // Cherry-pick: modify regions 1 and 2
    let cherry_content = r#"function processEvent(event) {
  // Region 1: Type-safe validation
  if (!event || typeof event !== 'object') {
    throw new TypeError('Event must be an object');
  }

  // Region 2: Async processing
  const result = await handleEventAsync(event);
  
  // Region 3: Cleanup
  cleanup();
  return result;
}"#;

    // Use ConflictTestBuilder to set up the conflict scenario
    let scenario = ConflictTestBuilder::new(&test_repo)
      .with_initial_state(vec![("processor.js", initial_content)], "Initial commit")
      .with_target_changes(vec![("processor.js", target_content)], "Enhance validation and cleanup")
      .with_cherry_changes(vec![("processor.js", cherry_content)], "Add type safety and async")
      .build();

    // Attempt cherry-pick
    let cache = TreeIdCache::new();
    let result = perform_fast_cherry_pick_with_context(
      git_executor,
      test_repo.path().to_str().unwrap(),
      &scenario.cherry_commit,
      &scenario.target_commit,
      None,
      &cache,
    );

    // Should produce conflicts
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];
      let conflict_content = &conflict_detail.file_diff.new_file.content;

      // Count conflict regions (each <<<<<<< marks a new conflict)
      let conflict_start_count = conflict_content.matches("<<<<<<<").count();
      eprintln!("Conflict content:\n{conflict_content}");
      eprintln!("Number of conflict regions found: {conflict_start_count}");
      // Git merge-tree may combine nearby conflicts into a single region
      assert!(conflict_start_count >= 1, "Should have at least 1 conflict region, found {conflict_start_count}");

      // Verify content from both branches is present
      assert!(conflict_content.contains("isValid()"), "Should contain target branch validation");
      assert!(conflict_content.contains("TypeError"), "Should contain cherry-pick type check");
      assert!(conflict_content.contains("handleEventAsync"), "Should contain cherry-pick async call");
      assert!(conflict_content.contains("notifyCompletion"), "Should contain target branch cleanup");
    } else {
      panic!("Expected MergeConflict error");
    }
  }

  #[test]
  fn test_real_world_kotlin_conflict() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

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
    let target_content = r#"private class EternalEventStealer(disposable: Disposable) {
  @Volatile
  private var enabled = false
  private var counter = 0

  private val specialEvents = LinkedBlockingQueue<ForcedEvent>()

  init {
    IdeEventQueue.getInstance().addPostEventListener(
      { event ->
        if (enabled && event.toString().contains(",runnable=${ThreadingSupport.RunnableWithTransferredWriteAction.NAME}")) {
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

    // Cherry-pick - major refactoring with new event type
    let cherry_content = r#"private class EternalEventStealer(disposable: Disposable) {
  private var counter = 0

  private val specialEvents = LinkedBlockingQueue<ForcedEvent>()

  init {
    IdeEventQueue.getInstance().addPostEventListener(
      { event ->
        if (event is InternalThreading.TransferredWriteActionEvent) {
          specialEvents.add(TransferredWriteActionWrapper(event))
        }
        false
      }, disposable)
  }
}"#;

    // Use ConflictTestBuilder to set up the conflict scenario
    let scenario = ConflictTestBuilder::new(&test_repo)
      .with_initial_state(vec![("SuvorovProgress.kt", initial_content)], "Initial commit")
      .with_target_changes(vec![("SuvorovProgress.kt", target_content)], "Add enabled check")
      .with_cherry_changes(vec![("SuvorovProgress.kt", cherry_content)], "Refactor to new event system")
      .build();

    // Attempt cherry-pick
    let cache = TreeIdCache::new();
    let result = perform_fast_cherry_pick_with_context(
      git_executor,
      test_repo.path().to_str().unwrap(),
      &scenario.cherry_commit,
      &scenario.target_commit,
      None,
      &cache,
    );

    // Should produce conflicts
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];
      let conflict_content = &conflict_detail.file_diff.new_file.content;

      // Verify specific patterns from the real conflict
      assert!(conflict_content.contains("enabled &&"), "Should contain target's enabled check");
      assert!(
        conflict_content.contains("InternalThreading.TransferredWriteActionEvent"),
        "Should contain cherry's new event type"
      );
      eprintln!("Real world conflict content:\n{conflict_content}");
      eprintln!("Number of conflict regions found: {}", conflict_content.matches("<<<<<<<").count());

      // Verify empty old_file content
      assert_eq!(conflict_detail.file_diff.old_file.content, "", "old_file should be empty");

      println!("Real-world conflict captured successfully");
    } else {
      panic!("Expected MergeConflict error");
    }
  }

  // Tests for batch commit info functionality that's part of merge conflict handling

  #[test]
  fn test_batch_commit_info_fetching() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create multiple commits
    let commit1_id = test_repo.create_commit("First commit", "file1.txt", "content1");
    let commit2_id = test_repo.create_commit("Second commit", "file2.txt", "content2");
    let commit3_id = test_repo.create_commit("Third commit with special chars: 'quotes' and \"double quotes\"", "file3.txt", "content3");

    // Test batch fetching
    let commit_ids = vec![commit1_id.as_str(), commit2_id.as_str(), commit3_id.as_str()];
    let result = super::super::merge_conflict::get_commit_info_batch(git_executor, test_repo.path().to_str().unwrap(), &commit_ids).unwrap();

    // Verify all commits were fetched
    assert_eq!(result.len(), 3, "Should fetch all 3 commits");

    // Verify commit data
    let commit1_info = result.get(&commit1_id).unwrap();
    assert_eq!(commit1_info.message, "First commit");
    assert_eq!(commit1_info.author, "Test User");
    assert!(commit1_info.author_time > 0);
    assert!(commit1_info.committer_time > 0);

    let commit2_info = result.get(&commit2_id).unwrap();
    assert_eq!(commit2_info.message, "Second commit");

    let commit3_info = result.get(&commit3_id).unwrap();
    assert_eq!(commit3_info.message, "Third commit with special chars: 'quotes' and \"double quotes\"");
  }

  #[test]
  fn test_batch_commit_info_empty_input() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Test with empty input
    let result = super::super::merge_conflict::get_commit_info_batch(git_executor, test_repo.path().to_str().unwrap(), &[]).unwrap();
    assert!(result.is_empty(), "Empty input should return empty result");
  }

  #[test]
  fn test_batch_commit_info_nonexistent_commits() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create one valid commit
    let valid_commit = test_repo.create_commit("Valid commit", "file.txt", "content");

    // Test with mix of valid and invalid commit IDs
    let commit_ids = vec![valid_commit.as_str(), "0000000000000000000000000000000000000000", "invalid_hash"];

    // This should fail because git log will error on invalid commits
    let result = super::super::merge_conflict::get_commit_info_batch(git_executor, test_repo.path().to_str().unwrap(), &commit_ids);
    assert!(result.is_err(), "Should fail when given invalid commit IDs");
  }

  #[test]
  fn test_batch_commit_info_performance() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create many commits to test batch performance
    let mut commit_ids_str = Vec::new();
    for i in 0..20 {
      let commit_id = test_repo.create_commit(&format!("Commit {i}"), &format!("file{i}.txt"), &format!("content{i}"));
      commit_ids_str.push(commit_id);
    }

    let commit_ids: Vec<&str> = commit_ids_str.iter().map(|s| s.as_str()).collect();

    // Measure batch operation
    let start = std::time::Instant::now();
    let result = super::super::merge_conflict::get_commit_info_batch(git_executor, test_repo.path().to_str().unwrap(), &commit_ids).unwrap();
    let batch_duration = start.elapsed();

    assert_eq!(result.len(), 20, "Should fetch all 20 commits");

    // Ensure batch operation is reasonably fast (should complete in under 1 second even for 20 commits)
    assert!(batch_duration.as_secs() < 1, "Batch operation took too long: {batch_duration:?}");
  }
}
