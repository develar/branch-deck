## ADDED Requirements

### Requirement: Get: A get still below the minimum after the trash reclaims the idle leases

A get whose volume is still below the minimum free space after the trash MUST return the idle leases of every pool of
the machine whose work landed, by the rule of `brd lease reclaim --idle`, once, before the trim, only of the slots on
its volume, under the lock of the shrink and never under the clones lock; its answer MUST name them (`reclaimed`), and a
`low_disk_space` after it how many it returned and the bytes their clean rules freed.

#### Scenario: Idle leases below the minimum
- **GIVEN** a free slot, an idle lease whose work landed and an idle lease with unlanded work
- **WHEN** a get finds the volume below the minimum free space after the trash
- **THEN** it returns the lease whose work landed, its slot free with its record (`reclaimed`)
- **AND** it keeps the lease with work and records its check (`idle_with_work`)
- **AND** a `low_disk_space` after it names the returned lease
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_below_the_minimum_free_space_reclaims_the_idle_leases_whose_work_landed`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_below_the_minimum_that_reclaims_an_idle_lease`)

#### Scenario: The shrink takes the reclaimed slot first
- **GIVEN** a free slot returned after an idle lease whose work landed went quiet, and build output in both slots
- **WHEN** a get below the minimum returns the idle lease
- **THEN** the shrink cleans the returned slot first
- **AND** the get leases the other free slot, warm
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_reclaims_an_idle_lease_and_shrinks_its_slot_before_the_hot_one`)

#### Scenario: No idle lease on the volume
- **WHEN** no pool of the machine holds an idle lease on the volume of the get
- **THEN** the get tells no step for it, takes no lock and runs no git for it
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_deletes_the_trash_without_the_clones_lock`)

## MODIFIED Requirements

### Requirement: Get: A kept idle lease is checked again only when its verdict moved

A later get MUST check a kept idle lease again only when what the verdict rests on moved: the tree work while the slot
(its `HEAD`, the length and the mtime of its reflog) is unchanged, the commits also while the home tip is; else it costs
one `rev-parse`. One get MUST check at most two idle leases in full for its takeover and two for its reclaim below the
minimum, the ones checked longest ago first; the keeper checks every one.

#### Scenario: Neither the slot nor home moved
- **WHEN** a later get finds the slot and the home tip as the check found them
- **THEN** it keeps the verdict for one `rev-parse`
  [@test] crates/bd-lease/src/takeover_test.rs (`a_kept_idle_lease_is_checked_again_only_when_its_slot_or_home_moved`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_that_passes_over_an_idle_lease_it_checked`)

#### Scenario: Many idle leases
- **WHEN** a get finds more than two idle leases to check
- **THEN** it checks at most two of them in full

#### Scenario: The reclaim below the minimum
- **GIVEN** an idle lease whose work a get before found
- **WHEN** a later get below the minimum free space reclaims, and neither the slot nor home moved
- **THEN** it keeps the verdict for one `rev-parse`
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_below_the_minimum_passes_over_an_idle_lease_it_checked`)

#### Scenario: Many idle leases below the minimum
- **WHEN** a get below the minimum free space finds more than two idle leases whose verdict moved
- **THEN** it checks two of them in full, the ones checked longest ago first, and a later get checks the rest
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_below_the_minimum_checks_two_idle_leases_in_full_and_the_next_get_the_rest`)

### Requirement: Host slots: The keeper keeps the minimum free space without a get

`host_slots::keep_free`, which the keeper of brd-host runs once a minute, MUST keep the minimum free space of the volume
of the pool without a get: below `branchdeck.leaseMinFree` the trash, the reclaim of the idle leases, the trim of every
pool of the machine, the shrink and the eviction, each when the one before left the volume below it, as a get takes them
("Get"). A host slot is no idle lease, and the other steps take free slots only: it stays.

#### Scenario: The keeper below the minimum
- **WHEN** the volume of the pool is below `branchdeck.leaseMinFree` at a round of the keeper
- **THEN** it trims the pools of the machine without a get
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_trims_the_pools_below_the_minimum_free_space_without_a_get`)

#### Scenario: No minimum, no free space or an upgrade of git
- **WHEN** there is no minimum, the system tells no free space, or a live process upgrades git
- **THEN** the keeper deletes nothing

#### Scenario: The keeper reclaims idle leases
- **GIVEN** an idle lease whose work landed
- **WHEN** the volume of the pool is below `branchdeck.leaseMinFree` at a round of the keeper
- **THEN** it returns the lease without a get, its slot free with its record, and names it (`reclaimed`)
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_reclaims_the_idle_leases_whose_work_landed_without_a_get`)

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
- **AND** it comes before every free slot returned after that time, in `returned` too
  [@test] crates/lease-pool/src/lease_test.rs (`a_reclaimed_slot_comes_before_the_free_slots_returned_after_its_lease_went_quiet`)
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_reclaims_an_idle_lease_and_shrinks_its_slot_before_the_hot_one`)
