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

#### Scenario: A reclaim and a takeover that fails
- **GIVEN** a sync in an idle lease marked its worktree synced
- **WHEN** `brd lease reclaim --idle` returns it, or a get takes it over and its return fails, which gives the slot back
- **THEN** the slot has no mark
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_takes_out_the_mark_of_a_sync_in_the_slot`, `a_takeover_that_fails_leaves_no_mark_of_a_sync_in_the_slot`)
