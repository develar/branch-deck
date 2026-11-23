## MODIFIED Requirements

### Requirement: Update: The files follow by an index of the paths the move changes

Where the index file has 16 MB or more, the files of a move MUST follow by a two-way `read-tree -m -u` on an index of
the paths the move changes, cut out of the user's index and put back after it, and its exact dry run MUST merge that
index without a write. Both MUST answer, and the move MUST leave the index, the files and the status, as the same
merge on the whole index does; the entries put back MUST have the stat data of their files.

#### Scenario: A move without local changes
- **GIVEN** a move that changes, adds and deletes files, turns a file into a directory and a directory into a file,
  changes an executable bit and a symlink, under attributes that convert line ends and run a filter
- **WHEN** the files follow by the index of the changed paths
- **THEN** the index, the files and the status are those of the merge on the whole index, and the dry runs of both
  refuse nothing
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_clean_move_by_the_small_index_is_the_move_of_the_whole_index`)

#### Scenario: A move in a large index
- **GIVEN** an index file of 16 MB or more
- **WHEN** a branch moves with its files, after the exact dry run of the move
- **THEN** the dry run merges the index of the changed paths without a write and names a local change on one of them
- **AND** the move merges it, puts its entries back, deletes its mark and releases the index lock
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_move_in_a_large_index_follows_by_the_small_index_and_its_dry_run_merges_it`)

#### Scenario: Local changes on the paths of the move
- **GIVEN** a local edit, a staged change, a staged deletion or an untracked file on a path the move writes
- **WHEN** the files follow
- **THEN** git refuses the move and its dry run with the message it gives on the whole index, and nothing is written
  [@test] crates/git-ops/src/worktree_update_test.rs (`local_changes_on_the_paths_of_a_move_refuse_both_indexes_with_nothing_written`)
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_staged_deletion_of_every_path_of_a_move_refuses_the_small_index`)

#### Scenario: Local changes beside the paths of the move
- **GIVEN** staged, unstaged and untracked changes on other paths, or an assume-unchanged entry there
- **WHEN** the files follow
- **THEN** those changes and that flag stay as on the whole index
  [@test] crates/git-ops/src/worktree_update_test.rs (`local_changes_beside_the_paths_of_a_move_stay_in_both_indexes`)

#### Scenario: A file that was only touched or has the new content
- **GIVEN** a file of the move that was only touched, staged as the new tree has it, or deleted in the working tree
- **WHEN** the files follow
- **THEN** the move takes it as the whole index does
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_file_that_was_only_touched_or_has_the_new_content_follows_in_both_indexes`)

#### Scenario: The attributes of the index
- **GIVEN** a local edit of a `.gitattributes` above a file the move adds, or `core.autocrlf`
- **WHEN** the files follow
- **THEN** the file is written by the attributes of the index, as on the whole index
- **AND** its entry has the stat data of that file, as `read-tree -u` records it on the whole index, so the status is
  that of the whole index: the file is not modified though by the edited attributes it is not its entry
  [@test] crates/git-ops/src/worktree_update_test.rs (`the_attributes_of_a_move_are_those_of_the_index_in_both_indexes`)
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_gitattributes_differs_from_its_entry_by_its_lines_not_by_their_ends`)

#### Scenario: A staged file in a directory that becomes a file
- **GIVEN** a staged new file in a directory the move turns into a file
- **WHEN** the files follow
- **THEN** its entry and its file go, as on the whole index
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_staged_file_in_a_directory_that_becomes_a_file_goes_in_both_indexes`)

#### Scenario: The stat data of the entries put back
- **WHEN** the files followed by the index of the changed paths
- **THEN** the entries of the changed paths have the stat data of their files, and no copy of the index stays
  [@test] crates/git-ops/src/worktree_update_test.rs (`the_entries_the_small_index_puts_back_have_the_stat_data_of_their_files`)
