## MODIFIED Requirements

### Requirement: Lock and engine: The objects of Git LFS of the move are fetched before the lock

A drop and its undo whose files follow MUST fetch the objects of Git LFS of the files the move changes and the store
lacks before the repository lock, so the smudge filter under the lock reads the store only; a repository without the
store of LFS MUST pay no process for it, and a fetch that fails MUST fail the drop before the lock, nothing written, but
where the repository skips download errors (`lfs.skipdownloaderrors`); the fetch MUST end at a deadline (30 min).

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

#### Scenario: A repository that skips download errors
- **GIVEN** `lfs.skipdownloaderrors` is true
- **WHEN** the fetch before the lock fails
- **THEN** the drop goes on, and its smudge writes the pointer as the repository asks
  [@test] crates/bd-update/src/lfs_prefetch_test.rs (`a_repository_that_skips_download_errors_moves_when_the_fetch_failed`)

#### Scenario: Many paths, paths that read as patterns, a cancel
- **WHEN** the paths to fetch pass 8 KiB, or a path holds a comma, a blank at an end or a character of a pattern
- **THEN** they are fetched in lists of at most 8 KiB, never without `--include`, each path a pattern that matches it
- **AND** a fetch ends as the operation is canceled
  [@test] crates/git-ops/src/lfs_test.rs (`the_paths_of_a_fetch_are_lists_of_at_most_8_kib`)
  [@test] crates/git-ops/src/lfs_test.rs (`a_path_is_a_pattern_of_the_include_that_matches_it`)
  [@test] crates/git-ops/src/lfs_test.rs (`a_fetch_ends_as_its_operation_is_canceled`)

#### Scenario: A store that lfs.storage moves
- **GIVEN** `lfs.storage` keeps the store away from `$GIT_COMMON_DIR/lfs`
- **WHEN** `brd drop` runs, which reads the config anyway
- **THEN** the store is checked where the config puts it, and the drop fetches what it lacks before the lock
  [@test] crates/bd-update/src/lfs_prefetch_test.rs (`a_store_that_lfs_storage_moves_is_checked_where_the_config_puts_it`)
