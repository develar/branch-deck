## MODIFIED Requirements

### Requirement: Pick: Local changes on a path the pick writes refuse it

Local changes on a path that the pick or the follow writes MUST refuse it as `uncommitted_overlap` (exit 4) with the
`paths`, the message naming the worktree; local changes on other paths MUST stay. A file in a directory that a move
replaces with a file, other than the ones it deletes, MUST refuse it the same way, and so MUST an unmerged entry of the
index on any path, with the unmerged paths: git updates no file of such an index.

#### Scenario: Local changes in the way in either worktree
- **GIVEN** local changes on a path that the pick or the follow writes, in the target or the source
- **WHEN** the pick runs
- **THEN** it is refused as `uncommitted_overlap` (exit 4) with the `paths`, the message naming the worktree
  [@test] crates/git-ops/src/pick_test.rs (`local_changes_on_a_path_the_pick_writes_block_it_in_either_worktree`)
  [@test] crates/bd-update/src/pick_test.rs (`local_changes_in_the_way_and_a_wrong_source_are_refused`)

#### Scenario: Local changes on other paths
- **GIVEN** local changes on paths that neither the pick nor the follow writes
- **WHEN** the pick lands
- **THEN** those changes stay

#### Scenario: An untracked file in a directory the pick replaces with a file
- **GIVEN** an untracked file in a directory that the pick replaces with a file
- **WHEN** the pick runs
- **THEN** its check finds it blocked by that file, before the lock, and nothing moves
- **AND** with only the tracked files the pick deletes in the directory, it lands
  [@test] crates/git-ops/src/pick_test.rs (`an_untracked_file_in_a_directory_the_pick_replaces_with_a_file_blocks_it`)

#### Scenario: An unmerged entry on another path
- **GIVEN** an unmerged entry of the index on a path the pick does not write, no merge in progress
- **WHEN** the pick runs
- **THEN** its check finds it blocked by that path, and nothing moves
  [@test] crates/git-ops/src/pick_test.rs (`an_unmerged_entry_on_another_path_blocks_the_pick_at_its_check`)

### Requirement: The lock: The check runs before the repository lock

The check (the range, the replay, the local changes on the paths each move changes by their index entries and content)
MUST run before the repository lock, and another writer MUST get the lock meanwhile. It MUST read no copy of the whole
index but where those paths find a local change or in a sparse checkout, where the exact dry run decides.

#### Scenario: The replay and the check of the files before the lock
- **WHEN** a pick with `--follow` runs
- **THEN** the range, the replay and the check of the paths of both moves run before the repository lock, with no dry
  run on a copy of an index and one `ls-files -u` per worktree
  [@test] crates/bd-update/src/pick_test.rs (`the_check_of_a_pick_runs_before_the_lock`)
  [@test] crates/git-ops/src/pick_test.rs (`the_check_of_a_pick_dry_runs_the_index_only_where_its_paths_cannot_tell`)

#### Scenario: Another writer during the replay
- **WHEN** another writer asks for the repository lock while a pick replays
- **THEN** it gets the lock meanwhile
  [@test] crates/bd-update/src/pick_test.rs (`another_writer_takes_the_lock_while_a_pick_replays`)

### Requirement: The lock: Under the lock only the witness, the local changes and the moves

Under the repository lock the pick MUST run only the marks of interrupted moves, the witness (the target's branch, both
HEADs, the upstream), the local changes on the paths the moves write (with what the check read: an index entry again
only when the index file changed, a file only when its stat data did; the exact dry run only where that finds one) and
the two moves.

#### Scenario: The processes under the lock
- **WHEN** a pick with `--follow` holds the repository lock
- **THEN** it reads the marks of interrupted moves, the witness, and the local changes on the paths the moves write by
  their index entries and content, only what changed since the check, and runs the two moves, nothing else
  [@test] crates/bd-update/src/pick_test.rs (`the_check_of_a_pick_runs_before_the_lock`)
  [@test] crates/git-ops/src/pick_test.rs (`the_check_of_a_pick_dry_runs_the_index_only_where_its_paths_cannot_tell`)

## ADDED Requirements

### Requirement: The lock: Files refused after the move block the pick

When git refuses the files of the target after its branch moved (a local change the check did not see, or one after
it), the branch MUST move back, and after the lock the exact dry run MUST name the paths: the pick is refused as
`uncommitted_overlap` with them, nothing moved.

#### Scenario: A file in the way after the check
- **GIVEN** an untracked file appears on a path the pick adds after its check
- **WHEN** the move of the target runs
- **THEN** the branch moves back (`pick rolled back`), the repository lock is released, and the pick is blocked by that
  path
  [@test] crates/git-ops/src/pick_test.rs (`files_refused_after_the_move_block_the_pick_with_their_paths`)
