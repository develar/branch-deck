## ADDED Requirements

### Requirement: The lock: The objects of Git LFS of the moves are fetched before the lock

A pick and its undo MUST fetch the objects of Git LFS of the files their moves change (the target's and the source's
that follows) and the store lacks before the repository lock, so the smudge filter under the lock reads the store only.

#### Scenario: A pick and its undo in a repository of Git LFS
- **GIVEN** the picked commit changes a file of Git LFS whose object the store lacks
- **WHEN** the pick runs, and its undo brings back a file whose object the store lost
- **THEN** each runs one `git lfs fetch` of that file before the lock, and the smudge under the lock reads the store
  [@test] crates/bd-update/src/lfs_prefetch_test.rs (`a_pick_and_its_undo_fetch_the_objects_of_git_lfs_before_the_lock`)
