## MODIFIED Requirements

### Requirement: Get: Each return records its number and its time

Each return MUST record the number of the return in the pool (`returned`, one past the highest number the pool gave,
`last_returned`) and its time (`returned_at`, Unix seconds, never before the last return of the pool). The number orders
the free slots of one pool; the time orders the free slots of different pools for the shrink, and within one pool it
follows the number. A state without the time reads as returned before every slot that has one, across pools.

#### Scenario: A state without the time of a return
- **WHEN** a free slot has `returned` but no `returned_at`
- **THEN** its number orders it in its pool, and the shrink reads it as returned before every slot that has a time

#### Scenario: A return after a step back of the clock
- **GIVEN** a clock that went back since the last return of the pool
- **WHEN** a slot is returned
- **THEN** its `returned_at` is the time of the last return of the pool
- **AND** a get takes it first, and the trim and the shrink take it last, as before the step
  [@test] crates/lease-pool/src/lease_test.rs (`a_return_after_a_step_back_of_the_clock_is_the_warmest_free_slot_of_its_pool`)

#### Scenario: A return after a reclaim took the highest number
- **GIVEN** a reclaim gave the slot of the highest number of the pool a number below the other free slots
- **WHEN** a slot is returned later
- **THEN** its `returned` is one past that highest number, so no free period of a slot repeats the number of an
  earlier one
  [@test] crates/lease-pool/src/lease_test.rs (`no_free_period_of_a_slot_has_the_epoch_of_an_earlier_one`)

### Requirement: Get: The epoch of a slot is the lease it holds

The epoch of a worktree (`worktree_epoch`) MUST be the id of the lease its slot holds (`returned <n>` while free),
`NotASlot` for a worktree that is no slot, and `Unknown` when the files that tell cannot be read or parsed; it MUST be
read without git and without the lock of the pool, of each slot only its path, lease id, last return and reclaimed
lease. The app keeps what it records of a slot under it ([Onboarding](../../repository/onboarding/spec.md)).

#### Scenario: A slot leased again
- **WHEN** a slot is leased, returned and leased again
- **THEN** its epoch is each lease in turn, and that of its return between them; the main checkout has none
  [@test] crates/bd-lease/src/lease_test.rs (`the_epoch_of_a_slot_is_its_lease_and_changes_with_each_return_and_get`)

#### Scenario: A reclaim gives a number of an earlier free period
- **GIVEN** a slot returned with a number, leased, returned and leased again
- **WHEN** a reclaim gives it that first number again
- **THEN** its epoch is `returned <n> reclaimed <lease id>`, with the reclaimed lease, and none of its earlier free
  periods
  [@test] crates/lease-pool/src/lease_test.rs (`no_free_period_of_a_slot_has_the_epoch_of_an_earlier_one`)

#### Scenario: A state of a later build, and one that cannot be parsed
- **GIVEN** a later build wrote the state of the pool, with a state of a lease and a field this build does not know
- **THEN** the epoch of the slot is still its lease
- **AND** a state that cannot be parsed makes it `Unknown`, never `NotASlot`
  [@test] crates/bd-lease/src/lease_test.rs (`the_epoch_reads_a_state_of_a_later_build_and_is_unknown_for_one_it_cannot_parse`)

#### Scenario: A submodule, a separate git directory, a `.git` file that cannot be read
- **GIVEN** a submodule and a checkout of `git init --separate-git-dir`, whose `.git` files name a git directory without
  a `commondir`
- **THEN** their epoch is `NotASlot`, as the main checkout's, and a `.git` file that cannot be read makes it `Unknown`
  [@test] crates/bd-lease/src/lease_test.rs (`a_submodule_and_a_separate_git_directory_are_no_slot_and_an_unreadable_git_file_is_unknown`)

#### Scenario: A state of the same size and time
- **WHEN** a state of the pool is written over the one before with its size and its time
- **THEN** the epoch is read from the new state
  [@test] crates/bd-lease/src/lease_test.rs (`the_epoch_reads_a_state_anew_that_keeps_the_size_and_the_time_of_the_one_before`)

### Requirement: Reclaim: Reclaim applies the rule of the takeover to every idle lease

`brd lease reclaim --idle [--dry-run] [--keep <n>] [--age <duration>]` MUST apply the rule of the takeover to every idle
lease of the pool, each checked anew: one whose work landed is returned as the takeover returns it, under a CAS on its
lease, its holder still gone and its slot as the check found it, its slot free with its record, returned as the
coldest free slot of its pool. The output is `lease_reclaim` of `brd schema`, exit 0.

#### Scenario: An idle lease whose work landed
- **WHEN** reclaim finds an idle lease whose work landed
- **THEN** it returns it as the takeover returns it (`reclaimed`, each with `landed`, `stopped_servers`, `cleaned`)
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_reclaim`)

#### Scenario: An idle lease with work
- **WHEN** reclaim finds an idle lease with work
- **THEN** it keeps the lease and records its check (`kept`, as `idle_with_work`)
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)

#### Scenario: The other free slots after a reclaim
- **GIVEN** free slots returned after an idle lease went quiet, measured by `brd lease disk`
- **WHEN** reclaim returns the idle lease
- **THEN** each of them keeps its `returned` and its epoch
- **AND** the next `brd lease disk` takes their bytes from its cache and measures only the reclaimed slot
  [@test] crates/pool-disk/src/disk_test.rs (`a_reclaim_leaves_the_measures_of_the_other_free_slots_valid`)

#### Scenario: A lease it cannot judge
- **WHEN** a lease is not quiet for an hour, busy, changed meanwhile or its check failed
- **THEN** it is `skipped` with its reason

#### Scenario: A lease whose holder is unknown
- **GIVEN** a `ready` lease whose get recorded no holder process
- **WHEN** reclaim runs, with `--dry-run` or not
- **THEN** it is `skipped` as `holder_unknown`, checked by nothing, and stays `ready`
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_names_a_lease_without_a_holder_as_holder_unknown`)

#### Scenario: A dry run
- **WHEN** reclaim runs with `--dry-run`
- **THEN** it checks the same and writes nothing

#### Scenario: The return time of a reclaimed slot
- **GIVEN** free slots of the pool, some returned before an idle lease went quiet and some after
- **WHEN** reclaim returns the idle lease
- **THEN** its `returned` is below that of every other free slot of the pool and is not its number before the lease
- **AND** `last_returned` keeps the highest number of the pool
- **AND** its `returned_at` is the earliest of the time its lease went quiet (the later of its get and the last change
  of the reflog of the slot) and the times of the other free slots, none when one of them has none
- **AND** a brd that orders by the number alone and a brd that orders by the time first both take it last for a get
  and first for the trim and the shrink
  [@test] crates/lease-pool/src/lease_test.rs (`a_reclaimed_slot_is_the_coldest_free_slot_of_its_pool_by_number_and_by_time`)
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_reclaims_an_idle_lease_and_shrinks_its_slot_before_the_hot_one`)

#### Scenario: A number of an earlier free period
- **GIVEN** a reclaim gave its slot a number that a measure of `brd lease disk` of an earlier free period has
- **WHEN** `brd lease disk` runs
- **THEN** it measures the reclaimed slot again, as its measure names the reclaimed lease
- **AND** the other free slots keep their measures
  [@test] crates/pool-disk/src/disk_test.rs (`a_reclaimed_slot_with_the_number_of_an_earlier_free_period_is_measured_again`)

#### Scenario: No number left below the other free slots
- **GIVEN** a pool whose lowest free slot has the number 0, or 1 while the reclaimed slot had 0
- **WHEN** reclaim returns an idle lease
- **THEN** the other free slots move up by `RECLAIM_ROOM`, and the reclaimed slot takes the number below them
- **AND** a leased slot keeps its number
  [@test] crates/lease-pool/src/lease_test.rs (`a_reclaim_with_no_number_left_below_moves_the_free_slots_up_once`)

### Requirement: Disk: The bytes of a slot are kept and refreshed by what changed

The bytes of a slot MUST be kept in `$GIT_COMMON_DIR/branch-deck/disk.json` with its `returned`, the lease that a
reclaim or a takeover took from it, whether it was leased, its FSEvents mark and the sizes of disjoint nodes under its
roots. On macOS the default refresh MUST ask FSEvents what changed since that mark: an unchanged slot stays cached
regardless of its age or lease; a changed path walks its nearest saved node. A slot that left the pool leaves the file.

#### Scenario: A changed file
- **WHEN** FSEvents names a changed file of a slot
- **THEN** the refresh walks its nearest saved node, and a path no node holds selects a root
- **AND** the report streams the cached overview, then each finished slot
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_cached_overview_then_each_finished_slot`, `a_changed_file_selects_its_nearest_saved_node_and_uncached_paths_select_a_root`)

#### Scenario: A full walk
- **WHEN** the history is missing or incomplete, the roots changed, the slot is on another volume, or `--fresh` is given
- **THEN** the full slot is walked

#### Scenario: Linux, Windows and the output bases of the main worktree
- **WHEN** the report runs on Linux or Windows, or measures the output bases of the main worktree
- **THEN** Linux and Windows keep their returned/leased cache policy, with 15 minutes for a leased slot
- **AND** the output bases of the main worktree hold for 15 minutes on every platform
