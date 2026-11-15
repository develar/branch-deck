## ADDED Requirements

### Requirement: Get: The shrink prunes a leased slot at most once in its age

The shrink MUST pass over a leased slot that a prune of the same lease stamped (`pruned_at`) less than the smallest
`age` of its cargo rules ago, a leased slot in another state than `ready`, and a host slot.

#### Scenario: A second shrink within the age
- **GIVEN** a leased slot that a shrink pruned
- **WHEN** a second get below the minimum shrinks within its age
- **THEN** it leaves that slot as it is, a stale unit that came since included, and goes on to the free slots
  [@test] crates/bd-lease/src/lease_test.rs (`a_second_shrink_within_the_age_passes_over_a_pruned_leased_slot`)

#### Scenario: The order of the leased slots
- **WHEN** the shrink orders the leased slots
- **THEN** the largest build output as `disk.json` last measured it goes first, then the slots without a size, leased
  longest ago first
  [@test] crates/lease-pool/src/shrink_test.rs (`the_leased_slots_go_largest_first_then_by_leased_at`)

### Requirement: Get: The shrink passes over a leased slot in which cargo builds

A leased slot whose locks of cargo are held in every profile dir its cargo rules find MUST stay as it is, named in
`shrunk.building` with its holder and those profile dirs.

#### Scenario: A build in a leased slot
- **GIVEN** a process holds `.cargo-build-lock` of the profile dir of a leased slot
- **WHEN** a get below the minimum shrinks
- **THEN** the slot keeps every unit and is named in `shrunk.building`, and the shrink goes on to the free slots
  [@test] crates/bd-lease/src/lease_test.rs (`a_shrink_passes_over_a_leased_slot_whose_cargo_lock_is_held`)

### Requirement: Clean rules: A prune of a leased slot checks its lease under the locks of cargo

A prune of a leased slot MUST take the locks of cargo of a profile dir first, then read the slot under the pool lock,
and go on only while it holds the same lease in state `ready`; else it MUST leave the slot as it is. It MUST apply no
`delete` rule and set no marker, and a prune that pruned a profile dir MUST stamp the lease (`pruned_at`).

#### Scenario: The lease changed
- **GIVEN** a return and a new get came after the shrink read the lease
- **WHEN** the prune holds the locks of cargo
- **THEN** it finds another lease, prunes nothing and stamps nothing
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_lease_that_changed_once_the_locks_are_held_leaves_the_profile_dir_as_it_is`)
  [@test] crates/lease-pool/src/lease_test.rs (`the_prune_of_a_leased_slot_whose_lease_changed_prunes_nothing`)

## MODIFIED Requirements

### Requirement: Get: A get still below the minimum after the trim shrinks the free slots

A get still below the minimum free space after the trim MUST shrink the pools of the machine once
(`pruning the stale build output of the leased slots …`), never under the clones lock, and measure again: first each
`ready` agent lease of the volume by the cargo rules of its lease with their own `keep` and `age`, the largest build
output first; then each free slot by its own rules with `keep` 0 and `age` 0 under the `cleaning` marker, the worktree
staying. It stops at the target across both.

#### Scenario: The shrink of a get
- **WHEN** a get is still below the minimum free space after the trim
- **THEN** it deletes the build output of the free slots by their clean rules with `keep` 0 and `age` 0, the
  worktrees staying
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_free_space_deletes_the_build_output_of_the_free_slots_returned_longest_ago_first`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_that_shrinks_and_evicts`)

#### Scenario: The shrink prunes a leased slot first
- **GIVEN** a leased slot with stale and kept units and a `notes.local`, and free slots with a unit
- **WHEN** a get below the minimum shrinks, and the prune of the leased slot lifts the volume past the target
- **THEN** the leased slot loses its stale units only, keeps its lease, its kept units and `notes.local`, and the free
  slots keep their units
- **AND** `shrunk.leased` names the slot with its holder, and the lease has `pruned_at`
  [@test] crates/bd-lease/src/lease_test.rs (`a_shrink_prunes_the_leased_slots_by_their_own_keep_and_age_before_the_free_slots`)

#### Scenario: Never under the clones lock
- **WHEN** a get that waited for the clones lock is below the minimum
- **THEN** it checks the minimum free space under the lock, and shrinks only after it released it
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_that_waited_for_the_clones_lock_checks_the_minimum_free_space_under_it`)

### Requirement: Clean rules: brd lease clean applies the rules of its checkout

`brd lease clean [--free] [--leased] [--dry-run] [--keep <n>] [--age <duration>] [<dir>...]` MUST apply the rules of
the checkout it runs in, read once, to the free slots (`--free`), the given worktrees, or that checkout. A slot with a
`ready` agent lease, by `--leased` or named, MUST get only the cargo rules of its lease with their own `keep` and
`age`, no marker, named by `leased_by`. A free slot with a process in it is skipped (`skipped`).

#### Scenario: A busy slot, and a dry run
- **WHEN** `brd lease clean` comes to a free slot with a process in it, or runs with `--dry-run`
- **THEN** it skips the slot, and `--dry-run` deletes nothing and says what the rules would free
  [@test] crates/bd-lease/src/lease_clean_test.rs (`clean_skips_a_busy_slot_and_a_dry_run_deletes_nothing`)

#### Scenario: The leased slots on demand
- **GIVEN** a leased slot with a stale unit and `tests/results` of a `delete` rule
- **WHEN** `brd lease clean --leased` runs, first with `--dry-run`
- **THEN** the dry run deletes nothing and names what the cargo rule would free
- **AND** the run prunes the stale unit, keeps `tests/results` and the lease, and names the holder (`leased_by`)
  [@test] crates/bd-lease/src/lease_clean_test.rs (`clean_leased_applies_only_the_cargo_rules_of_a_leased_slot`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_clean_of_the_leased_slots`)

#### Scenario: A file that does not parse
- **WHEN** the rules file does not parse
- **THEN** `brd lease clean` fails with `invalid_arguments` (exit 2)
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_bad_file_does_not_fail_the_return`)

### Requirement: Clean rules: The rules apply by default with two overrides

The rules are configured on purpose, so they MUST apply by default: the return and `brd lease clean` delete what they
name, as the user decided. `brd lease return --no-clean` MUST apply none. `--keep <n>` and `--age <duration>`, on both
commands, MUST set `keep` and `age` of every cargo rule for that call, but never of a leased slot; a bad `--age` is
`invalid_arguments`. These are the only overrides: `except` of a `delete` rule is the file's.

#### Scenario: No clean and the overrides
- **WHEN** a return runs with `--no-clean`, or a call with `--keep <n>` or `--age <duration>`
- **THEN** the return cleans nothing, and the call prunes by the given `keep` and `age`
  [@test] crates/bd-lease/src/lease_clean_test.rs (`no_clean_and_the_overrides_change_what_a_call_cleans`)

#### Scenario: No override of a leased slot
- **GIVEN** a leased slot
- **WHEN** `brd lease clean <slot> --keep 0` runs, or `brd lease clean --leased --age 0s`
- **THEN** the first skips the slot and says why, and the second is `invalid_arguments`
  [@test] crates/bd-lease/src/lease_clean_test.rs (`the_overrides_never_apply_to_a_leased_slot`)

### Requirement: Host slots: The keeper keeps the minimum free space without a get

`host_slots::keep_free`, which the keeper of brd-host runs once a minute, MUST keep the minimum free space of the volume
of the pool without a get: below `branchdeck.leaseMinFree` the trash, the reclaim of the idle leases, the trim of every
pool of the machine, the shrink and the eviction, each when the one before left the volume below it, as a get takes them
("Get"). A host slot is no idle lease, and the shrink takes only the agent leases and the free slots: it stays.

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
