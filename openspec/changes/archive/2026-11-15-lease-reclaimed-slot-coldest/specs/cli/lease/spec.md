## MODIFIED Requirements

### Requirement: Get: The clean free slot returned last is leased first

Among the clean free slots a get MUST take the one returned last first (by its number in the pool, `returned`, never
by the clock), then the ones never returned in the order they joined the pool: the slot returned last has the warmest
index and caches and the commit closest to the source.

#### Scenario: Several clean free slots
- **WHEN** a get chooses among clean free slots
- **THEN** it takes the one returned last first, also when a step back of the clock gave it an earlier time
  [@test] crates/bd-lease/src/lease_test.rs (`the_free_slot_returned_last_is_leased_first`)

#### Scenario: A reclaimed slot
- **GIVEN** a reclaimed slot and two free slots returned after its lease went quiet
- **WHEN** three gets choose among them
- **THEN** they take the free slot returned last, then the other, then the reclaimed slot
  [@test] crates/bd-lease/src/takeover_test.rs (`the_gets_take_a_reclaimed_slot_after_the_free_slots_returned_after_its_lease_went_quiet`)

### Requirement: Get: Each return records its number and its time

Each return MUST record the number of the return in the pool (`returned`, one past the last) and its time
(`returned_at`, Unix seconds, never before the last return of the pool). The number orders the free slots of one pool;
the time orders the free slots of different pools for the shrink, and within one pool it follows the number. A state
without the time reads as returned before every slot that has one, across pools.

#### Scenario: A state without the time of a return
- **WHEN** a free slot has `returned` but no `returned_at`
- **THEN** its number orders it in its pool, and the shrink reads it as returned before every slot that has a time

#### Scenario: A return after a step back of the clock
- **GIVEN** a clock that went back since the last return of the pool
- **WHEN** a slot is returned
- **THEN** its `returned_at` is the time of the last return of the pool
- **AND** a get takes it first, and the trim and the shrink take it last, as before the step
  [@test] crates/lease-pool/src/lease_test.rs (`a_return_after_a_step_back_of_the_clock_is_the_warmest_free_slot_of_its_pool`)

### Requirement: Trim: Past the limit the free slots a get would take last go first

Past the limit, the free slots of a pool MUST go one by one, the ones a get would take last first: the never returned
ones in reverse order of joining, then the ones returned longest ago. A leased slot, a host slot and a slot with a
lease of any state MUST count and stay.

#### Scenario: A pool past its limit
- **WHEN** a pool has more slots than its limit
- **THEN** its free slots past the limit go, the ones returned longest ago first, with their output bases
  [@test] crates/bd-lease/src/trim_test.rs (`a_trim_deletes_the_free_slots_past_the_limit_returned_longest_ago_first_with_their_output_bases`)

#### Scenario: A reclaimed slot past the limit
- **GIVEN** a reclaimed slot and two free slots returned after its lease went quiet
- **WHEN** a trim keeps two slots
- **THEN** the reclaimed slot goes, and the two free slots stay
  [@test] crates/bd-lease/src/takeover_test.rs (`the_trim_deletes_a_reclaimed_slot_before_the_free_slots_returned_after_its_lease_went_quiet`)

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
- **AND** its `returned_at` is the earliest of the time its lease went quiet (the later of its get and the last change
  of the reflog of the slot) and the times of the other free slots, none when one of them has none
- **AND** a brd that orders by the number alone and a brd that orders by the time first both take it last for a get
  and first for the trim and the shrink
  [@test] crates/lease-pool/src/lease_test.rs (`a_reclaimed_slot_is_the_coldest_free_slot_of_its_pool_by_number_and_by_time`)
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_reclaims_an_idle_lease_and_shrinks_its_slot_before_the_hot_one`)

#### Scenario: No number left below the other free slots
- **GIVEN** a pool whose lowest free slot has the number 0, or 1 while the reclaimed slot had 0
- **WHEN** reclaim returns an idle lease
- **THEN** the other free slots move up by `RECLAIM_ROOM`, and the reclaimed slot takes the number below them
- **AND** a leased slot keeps its number
  [@test] crates/lease-pool/src/lease_test.rs (`a_reclaim_with_no_number_left_below_moves_the_free_slots_up_once`)
