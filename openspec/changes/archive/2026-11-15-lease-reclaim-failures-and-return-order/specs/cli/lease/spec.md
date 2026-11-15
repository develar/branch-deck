## ADDED Requirements

### Requirement: Get: A failed reclaim waits an hour

A reclaim that fails after the check in full (the check fails, the lease changed, a Bazel server does not exit, the
return fails) MUST record its time in the lease (`reclaim_failed_at`), which counts as a check for the order of the
reclaim of a get. The reclaim of a get MUST pass over the lease for an hour after it; the keeper and `brd lease
reclaim` try it each time.

#### Scenario: A reclaim below the minimum that failed
- **GIVEN** two idle leases whose work landed but whose return fails, and an idle lease with work
- **WHEN** a get below the minimum free space checks the two in full and their returns fail
- **THEN** it records each failure, and the leases stay `ready`
- **AND** the next get passes over them and checks the lease with work
- **AND** a get an hour after a failure checks that lease again
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_below_the_minimum_passes_over_an_idle_lease_whose_reclaim_failed_and_checks_the_next`)

## MODIFIED Requirements

### Requirement: Get: The clean free slot returned last is leased first

Among the clean free slots a get MUST take the one returned last first (by the time of its return, then by its
number), then the ones never returned in the order they joined the pool: the slot returned last has the warmest
index and caches and the commit closest to the source.

#### Scenario: Several clean free slots
- **WHEN** a get chooses among clean free slots
- **THEN** it takes the one returned last first
  [@test] crates/bd-lease/src/lease_test.rs (`the_free_slot_returned_last_is_leased_first`)

### Requirement: Get: Each return records its number and its time

Each return (and a reclaim) MUST record the number of the return in the pool (`returned`) and its time
(`returned_at`, Unix seconds). The time orders the free slots, of one pool and of different pools, and the number
orders the returns of one second; a state without the time reads as returned before every slot that has one.

#### Scenario: A state without the time of a return
- **WHEN** a free slot has `returned` but no `returned_at`
- **THEN** it reads as returned before every slot that has one

### Requirement: Reclaim: Reclaim applies the rule of the takeover to every idle lease

`brd lease reclaim --idle [--dry-run] [--keep <n>] [--age <duration>]` MUST apply the rule of the takeover to every idle
lease of the pool, each checked anew: one whose work landed is returned as the takeover returns it, under a CAS on its
lease, its holder still gone and its slot as the check found it, its slot free with its record, returned as of when it
went quiet. The output is `lease_reclaim` of `brd schema`, exit 0.

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
- **GIVEN** a free slot returned after an idle lease went quiet
- **WHEN** reclaim returns the idle lease
- **THEN** its `returned_at` is the later of its get and the last change of the reflog of the slot
- **AND** its `returned` is one past the last return of the pool
- **AND** the reserve and the shrink order it before every free slot returned after that time
  [@test] crates/lease-pool/src/lease_test.rs (`a_reclaimed_slot_comes_before_the_free_slots_returned_after_its_lease_went_quiet`)
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_reclaims_an_idle_lease_and_shrinks_its_slot_before_the_hot_one`)
