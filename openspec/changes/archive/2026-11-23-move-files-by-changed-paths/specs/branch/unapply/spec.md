## MODIFIED Requirements

### Requirement: Refusals: The index lock of another process makes a rewrite wait

The index lock of another process MUST make the unapply, the reapply and their undo wait (at most 30 s per write) and
an ownerless one be removed, as for `brd pick`; one that stays MUST answer `locked` with HEAD where it was. The wait
MUST come before the repository lock, so no other writer waits for it.

#### Scenario: Another process holds the index lock
- **WHEN** another process holds the index lock
- **THEN** the rewrite waits for it, at most 30 s per write, before the repository lock
- **AND** a lock that stays answers `locked`, HEAD where it was
  [@test] crates/history-rewrite/src/repository_lock_test.rs (`a_move_whose_files_follow_waits_for_the_index_lock_before_the_repository_lock`)

#### Scenario: An index lock that no process owns
- **WHEN** no process owns the index lock
- **THEN** it is removed and the files follow
  [@test] crates/git-ops/src/pick_test.rs (`a_move_removes_an_index_lock_that_no_process_owns_and_lets_the_files_follow`)

#### Scenario: A lock that appears while the move takes the repository lock
- **WHEN** an index lock appears while the move takes the repository lock
- **THEN** the move releases the repository lock and waits outside it
