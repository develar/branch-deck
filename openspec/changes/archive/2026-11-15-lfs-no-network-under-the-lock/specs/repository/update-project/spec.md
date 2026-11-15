## MODIFIED Requirements

### Requirement: Update: The objects of Git LFS of the move are fetched before the lock

An update or an undo whose files follow MUST fetch the objects of Git LFS its move needs and the store lacks before
the repository lock, so the smudge under the lock reads the store only; a failed fetch MUST fail the update before the
lock. A resolve session MUST fetch those that change from HEAD to the upstream tip before the lock of
its start and of each `brd update --continue`; where the repository skips download errors and the fetch failed, its
finish under the lock MUST download nothing.

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

#### Scenario: A resolve session of a repository that skips download errors
- **GIVEN** `lfs.skipdownloaderrors` is true, and the fetch before the lock fails
- **WHEN** `brd update --continue` finishes the session, moving to the commit it replays rather than the upstream tip
- **THEN** the finish downloads nothing under the lock, and its smudge writes the pointer
  [@test] crates/bd-update/src/lfs_prefetch_test.rs (`a_session_of_a_repository_that_skips_download_errors_finishes_without_the_network`)
