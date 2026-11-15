## MODIFIED Requirements

### Requirement: Update: The objects of Git LFS of the move are fetched before the lock

An update or an undo whose files follow MUST fetch the objects of Git LFS of the files its move changes and the store
lacks before the repository lock, so the smudge filter of the move under the lock reads the store only; a fetch that
fails MUST fail the update before the lock, nothing written. A resolve session MUST fetch those of the files that change
from HEAD to the upstream tip before the lock of its start and of each `brd update --continue`, whose finish runs under
the lock.

#### Scenario: An update and its undo in a repository of Git LFS
- **GIVEN** the upstream changes a file of Git LFS whose object the store lacks
- **WHEN** the update applies, and later its undo brings back a file whose object the store lost
- **THEN** each runs one `git lfs fetch` of that file at its target before the lock, and the smudge under the lock reads
  the object from the store
  [@test] crates/bd-update/src/lfs_prefetch_test.rs (`an_update_and_its_undo_fetch_the_objects_of_git_lfs_before_the_lock`)

#### Scenario: A resolve session in a repository of Git LFS
- **GIVEN** an update that conflicts while the upstream changes a file of Git LFS whose object the store lacks
- **WHEN** the update starts its resolve session, and later `brd update --continue` finishes it
- **THEN** each runs one `git lfs fetch` of that file at the upstream tip before the lock, and the finish smudges it from
  the store
  [@test] crates/bd-update/src/lfs_prefetch_test.rs (`a_session_start_and_its_continue_fetch_the_objects_of_git_lfs_before_the_lock`)
