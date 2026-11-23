## MODIFIED Requirements

### Requirement: The lock: Under the lock only the witness, the local changes and the moves

Under the repository lock the pick MUST run only the marks of interrupted moves, the witness (the target's branch, both
HEADs, the upstream), the local changes on the paths the moves write (with what the check read: an index entry again
only when the index file changed, a file only when its stat data did; the exact dry run only where that finds one) and
the two moves, the files of each by an index of the paths it changes where its index file has 16 MB or more.

#### Scenario: The processes under the lock
- **WHEN** a pick with `--follow` holds the repository lock
- **THEN** it reads the marks of interrupted moves, the witness, and the local changes on the paths the moves write by
  their index entries and content, only what changed since the check, and runs the two moves, nothing else
  [@test] crates/bd-update/src/pick_test.rs (`the_check_of_a_pick_runs_before_the_lock`)
  [@test] crates/git-ops/src/pick_test.rs (`the_check_of_a_pick_dry_runs_the_index_only_where_its_paths_cannot_tell`)

### Requirement: `--follow`: A pick or a follow killed before its files is finished

A pick or a follow killed between the move of HEAD and its files MUST leave the mark
`refs/branch-deck/update-pending/<worktree id>`; the next pick, update or undo MUST let the files follow first, a
detached HEAD by the reflog of `HEAD`. One killed after it wrote files and before its index followed MUST be finished
the same way, a file that has the new content already counting as followed.

#### Scenario: A move killed before its files followed
- **WHEN** a pick or a follow is killed between the move of HEAD and its files
- **THEN** the mark `refs/branch-deck/update-pending/<worktree id>` is left
- **AND** the next pick, update or undo lets the files follow first, a detached HEAD by the reflog of `HEAD`
  [@test] crates/git-ops/src/pick_test.rs (`a_pick_and_a_follow_killed_before_their_files_followed_are_finished`)

#### Scenario: A move killed between its files and its index
- **GIVEN** the mark of a pick, the files of the picked commits and the index of the commit before them
- **WHEN** the next pick, update or undo runs
- **THEN** the index follows the files, and the branch stays at the picked commits
  [@test] crates/git-ops/src/worktree_update_test.rs (`a_move_killed_between_its_files_and_its_index_is_finished_by_the_next_one`)
