## ADDED Requirements

### Requirement: Lock and engine: The objects of Git LFS of the move are fetched before the lock

A drop and its undo whose files follow MUST fetch the objects of Git LFS of the files the move changes and the store
lacks before the repository lock, so the smudge filter under the lock reads the store only; a repository without the
store of LFS MUST pay no process for it, and a fetch that fails MUST fail the drop before the lock, nothing written.

#### Scenario: A drop and its undo in a repository of Git LFS
- **GIVEN** the drop brings back a file of Git LFS whose object the store lacks
- **WHEN** it runs, and its undo brings back a file whose object the store lost
- **THEN** each runs one `git lfs fetch` of that file before the lock, and the smudge under the lock reads the store
  [@test] crates/bd-update/src/lfs_prefetch_test.rs (`a_drop_and_its_undo_fetch_the_objects_of_git_lfs_before_the_lock`)

#### Scenario: Objects the store has, and a failed fetch
- **WHEN** the store has the objects the drop needs
- **THEN** it fetches nothing
- **AND** a fetch that fails fails the drop before the lock, and HEAD and the files stay
  [@test] crates/bd-update/src/lfs_prefetch_test.rs (`a_move_whose_objects_are_stored_fetches_nothing_and_a_failed_fetch_moves_nothing`)

#### Scenario: The processes of the fetch
- **WHEN** a drop moves a file of Git LFS
- **THEN** a repository without the store adds no process; with the object stored the drop adds one `diff-tree`, one
  `check-attr` and one `cat-file`; without it also one `config` and one `git lfs fetch`
  [@test] crates/sync-core/src/process_budget_test.rs (`a_drop_of_a_file_of_git_lfs_fetches_what_the_store_lacks_before_the_lock`)
