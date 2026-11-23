## ADDED Requirements

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
  [@test] crates/git-ops/src/worktree_update_test.rs (`the_attributes_of_a_move_are_those_of_the_index_in_both_indexes`)

#### Scenario: A staged file in a directory that becomes a file
- **GIVEN** a staged new file in a directory the move turns into a file
- **WHEN** the files follow
- **THEN** its entry and its file go, as on the whole index
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_staged_file_in_a_directory_that_becomes_a_file_goes_in_both_indexes`)

#### Scenario: The stat data of the entries put back
- **WHEN** the files followed by the index of the changed paths
- **THEN** the entries of the changed paths have the stat data of their files, and no copy of the index stays
  [@test] crates/git-ops/src/worktree_update_test.rs (`the_entries_the_small_index_puts_back_have_the_stat_data_of_their_files`)

### Requirement: Update: An index the small one does not model merges whole

An index file below 16 MB, a sparse checkout, a skip-worktree or unmerged entry of the index, an assume-unchanged
entry or a submodule on a path of the move, and a small index that cannot be built MUST make the move and its dry run
merge the whole index, as before.

#### Scenario: A small index file
- **GIVEN** a repository whose index file is below 16 MB
- **WHEN** a move is checked and its files follow
- **THEN** the dry run merges a copy of the whole index and the move the whole index, in the processes they had
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_small_index_file_merges_whole`)

#### Scenario: Flags, a sparse checkout and an unmerged entry
- **GIVEN** a skip-worktree entry, an assume-unchanged entry on a path of the move, a sparse checkout or an unmerged entry
- **WHEN** the files follow
- **THEN** the whole index merges, with its answer
  [@test] crates/git-ops/src/worktree_update_test.rs (`an_index_the_small_one_does_not_model_merges_whole`)

#### Scenario: A submodule among the changed paths
- **WHEN** a move changes a submodule
- **THEN** the whole index merges
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_move_of_a_submodule_merges_whole`)

### Requirement: Update: The move holds the index lock from its files to its index

A move by the index of the changed paths MUST take the index lock of the worktree before the branch moves and hold it
until the index followed the files, so no git of the user writes the index between them. A move that does not run
MUST release it.

#### Scenario: A git of the user during the move
- **GIVEN** a move whose files are prepared
- **WHEN** a `git add` of the user runs before the index followed the files
- **THEN** it fails on the index lock, and runs once the move is done
- **AND** a prepared move that is dropped releases the lock
  [@test] crates/git-ops/src/worktree_update_test.rs (`the_small_index_holds_the_index_lock_from_its_cut_to_its_put_back`)

### Requirement: Update: An index that cannot follow takes the files back

When the index cannot follow the files of a move, the files MUST go back to the old tree and the branch MUST move
back, nothing changed.

#### Scenario: The index cannot be written
- **GIVEN** the files of a move followed by the index of the changed paths
- **WHEN** the copy of the user's index cannot be made
- **THEN** the files go back, the lock is released, and the index, the files and the status are as before the move
  [@test] crates/git-ops/src/worktree_update_test.rs (`an_index_that_cannot_follow_takes_the_files_of_the_small_index_back`)

### Requirement: Update: A move killed between its files and its index is finished

A move killed after it wrote files and before its index followed MUST be finished by the next update, pick or undo:
a file that has the new content already counts as followed, the rest is written, and the index follows. A local
change on a path the move did not write MUST move the branch back (`FilesBlocked`), the change kept.

#### Scenario: Killed between the files and the index
- **GIVEN** the mark of a move, the files of its new commit and the index of its old one
- **WHEN** the next move finishes it
- **THEN** the index follows, the branch stays, the mark goes, and the state is that of the move on the whole index
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_move_killed_between_its_files_and_its_index_is_finished_by_the_next_one`)

#### Scenario: Killed in the middle of its files
- **GIVEN** the mark of a move and only some of its files written
- **WHEN** the next move finishes it
- **THEN** the rest is written and the index follows
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_move_killed_in_the_middle_of_its_files_is_finished_by_the_next_one`)

#### Scenario: A local change on a path the killed move did not write
- **GIVEN** the mark of a move and a local change on a path it did not write
- **WHEN** the next move finishes it
- **THEN** the branch moves back as `FilesBlocked` with that path, the mark goes and the change stays
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_killed_move_with_a_local_change_on_a_path_it_did_not_write_moves_the_branch_back`)

### Requirement: Update: The entries of an index are written by `worktree_update.rs` only

`update-index --index-info` MUST run in `crates/git-ops/src/worktree_update.rs` only, on its temporary indexes and on
the copy that takes the place of the user's index; the usage scan MUST fail on any other.

#### Scenario: The production code
- **WHEN** the usage scan reads the production code
- **THEN** no code but `worktree_update.rs` runs `update-index --index-info`, with its exact number of argument lists
  [@test] crates/git-executor/src/git_usage_scan_test.rs (`index_entries_are_written_by_the_move_of_the_files_only`)

## MODIFIED Requirements

### Requirement: Update: The index lock of another process makes the move wait

The index lock of another process MUST make the move wait (at most 30 s per write) and an ownerless one be removed,
as for `brd pick`; one that stays MUST answer `locked`, nothing changed: before the branch moves, or with the branch
moved back where the whole index merges.

#### Scenario: An index lock no process owns
- **GIVEN** an index lock that no process owns
- **WHEN** the move runs
- **THEN** the lock is removed and the files follow
  [@test] crates/git-ops/src/pick_test.rs (`a_move_removes_an_index_lock_that_no_process_owns_and_lets_the_files_follow`)

#### Scenario: An index lock of another process
- **GIVEN** another process holds the index lock
- **WHEN** the move writes the index
- **THEN** it waits, at most 30 s per write

#### Scenario: An index lock that stays
- **GIVEN** the index lock stays for the whole wait
- **WHEN** the move runs
- **THEN** the update answers `locked`, nothing changed: the branch did not move, or moved back where the whole index
  merges
