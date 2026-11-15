## ADDED Requirements

### Requirement: Refusals: The objects of Git LFS of a move are fetched before the repository lock

An unapply, a reapply and their undo whose files follow MUST fetch the objects of Git LFS of the files the move changes
and the store lacks before each repository lock, so the smudge filter under the lock reads the store only.

#### Scenario: An unapply and a reapply in a repository of Git LFS
- **GIVEN** the unapply brings back a file of Git LFS whose object the store lacks
- **WHEN** it runs, and the reapply after it brings back a file whose object the store lost
- **THEN** each runs one `git lfs fetch` of that file before the lock, and the smudge under the lock reads the store
  [@test] crates/bd-update/src/lfs_prefetch_test.rs (`an_unapply_and_a_reapply_fetch_the_objects_of_git_lfs_before_the_lock`)
