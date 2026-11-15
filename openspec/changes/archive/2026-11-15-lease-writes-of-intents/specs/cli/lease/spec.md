## MODIFIED Requirements

### Requirement: Return: A return frees the lease of the worktree

`brd lease return` MUST free the lease of the worktree of `--workspace` (default: of `-C`, else of the current
directory): remove the seeded paths (`removed`, each moved into the trash as a whole), detach HEAD when it is on a
branch (`detached_branch`, the branch stays), take out the mark of a sync in its git directory, first, and leave the
slot at its commit. A get that reuses a free slot MUST take out the mark too (an older build may have left it).

#### Scenario: The seeded paths go
- **WHEN** a lease with seeded paths is returned
- **THEN** the return removes them and names them in `removed`
  [@test] crates/bd-lease/src/lease_test.rs (`the_seed_copies_the_included_ignored_files_and_the_return_removes_them`)

#### Scenario: A lease on a branch
- **WHEN** the HEAD of the returned slot is on a branch
- **THEN** the return detaches it (`detached_branch`) and the branch stays
  [@test] crates/bd-lease/src/lease_test.rs (`a_lease_on_a_new_branch_keeps_the_branch_after_the_return`)

#### Scenario: A lease a sync marked
- **GIVEN** a sync in the lease marked its worktree synced
- **WHEN** the lease is returned and the slot leased again
- **THEN** the slot has no mark: a window that opens the new lease shows the Ready to Sync card
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_takes_out_the_mark_of_a_sync_in_the_lease`)

#### Scenario: A free slot an older return left marked
- **GIVEN** a free slot whose git directory still has the mark of a sync
- **WHEN** a get reuses it
- **THEN** the new lease has no mark: a window that opens it shows the Ready to Sync card
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_takes_out_the_mark_of_a_sync_in_the_lease`)

#### Scenario: Everything a lease left in the git directory of the slot
- **GIVEN** a lease left every file of its own in the git directory of its slot (`lease_state::LeasePaths::all`: the mark
  of a sync, of this build and of the build before, and the landed state)
- **WHEN** the lease is returned and the slot leased again, or a get reuses a free slot that still has them
- **THEN** the new lease inherits none of them, and their locks stay
  [@test] crates/bd-lease/src/lease_test.rs (`a_new_lease_of_a_slot_inherits_nothing_the_lease_before_left_in_its_git_directory`)
  [@test] crates/sync-core/src/lease_state_test.rs (`every_file_of_the_lease_goes_and_the_locks_stay`)

#### Scenario: A write of the lease before during the forget
- **GIVEN** a work of the lease before checked that the slot holds its lease and is about to write a file of the lease
- **WHEN** the return or the get of the next lease forgets the files of the lease right then
- **THEN** the forget waits for the write under the lock of the lease and takes out what it wrote, and a write after the
  forget sees the next lease and writes nothing
  [@test] crates/sync-core/src/lease_state_test.rs (`a_forget_between_the_check_of_a_write_and_the_write_takes_out_what_the_write_wrote`)

#### Scenario: A command that started under the lease before
- **GIVEN** `brd -C <slot>` started under one lease of the slot, from a directory outside the slot, which the process
  check of a return does not see
- **WHEN** the slot is returned and leased again before its sync writes
- **THEN** it writes nothing into the files of the next lease: no mark of a sync, no landed state; a command that
  starts under the next lease writes them
  [@test] crates/bd-lease/src/lease_test.rs (`a_command_in_a_slot_writes_nothing_of_the_lease_once_the_slot_holds_another`)

#### Scenario: A reclaim and a takeover that fails
- **GIVEN** a sync in an idle lease marked its worktree synced
- **WHEN** `brd lease reclaim --idle` returns it, or a get takes it over and its return fails, which gives the slot back
- **THEN** the slot has no mark
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_takes_out_the_mark_of_a_sync_in_the_slot`, `a_takeover_that_fails_leaves_no_mark_of_a_sync_in_the_slot`)
