## MODIFIED Requirements

### Requirement: Get: A get still below the minimum after the trash trims the pools

A get whose volume is still below the minimum free space after the trash MUST trim the pools of the machine once
without their slots (`deleting the Bazel output bases of gone workspaces …`): the output bases of gone workspaces and
the expired notes go, never a slot, never under the clones lock, and it measures again; its answer MUST name what went
(`trimmed`, the output of `brd lease trim`), and a `low_disk_space` after it the bytes it freed. A free slot past
`branchdeck.leaseMaxSlots` stays: with more leases than the limit every free slot is past it, the one the get would
reuse too.

#### Scenario: The trim of a get
- **GIVEN** three free slots, a limit of one and an output base of a gone workspace
- **WHEN** a get is still below the minimum free space after the trash
- **THEN** it deletes the output base of the gone workspace and measures again
- **AND** every free slot stays with its output bases
- **AND** a `low_disk_space` after it names the bytes it freed
  [@test] crates/bd-lease/src/trim_test.rs (`a_get_below_the_minimum_free_space_keeps_the_free_slots_past_the_limit_and_deletes_the_output_bases_of_gone_workspaces`)

### Requirement: Get: A get still below the minimum after the shrink evicts free slots

A get whose volume is still below the minimum free space after the shrink MUST evict free slots once (`deleting free
slots of the pools …`), never under the clones lock, in the order of the shrink across every pool of the machine, each
as the trim deletes a slot, deleted before the next is chosen.

#### Scenario: The eviction of a get
- **WHEN** a get is still below the minimum free space after the shrink
- **THEN** it deletes free slots, returned longest ago first
- **AND** each goes as the trim deletes a slot: the `cleaning` marker, its Bazel servers stopped, its directory and
  its output bases through the trash, its registration and its entry dropped
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_still_below_the_minimum_after_the_shrink_deletes_free_slots_below_the_limit_returned_longest_ago_first`)

#### Scenario: The trim, the shrink and the eviction run without the clones lock
- **WHEN** a get trims, shrinks or evicts
- **THEN** it holds no clones lock
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_deletes_the_trash_without_the_clones_lock`)
  [@test] crates/bd-lease/src/trim_test.rs (`a_get_below_the_minimum_free_space_keeps_the_free_slots_past_the_limit_and_deletes_the_output_bases_of_gone_workspaces`)

### Requirement: Get: A minimum above the size of the volume neither shrinks nor evicts

A get whose minimum free space is more than the volume holds in all (`f_blocks` of the same `statfs`) MUST neither
shrink nor evict: nothing they delete could reach it, and one mistaken `branchdeck.leaseMinFree` or `--min-free` would
delete every free slot of every pool. Its `low_disk_space` MUST say that the volume holds less than the minimum.

#### Scenario: A minimum of more than the volume
- **WHEN** the minimum free space is more than the volume holds in all
- **THEN** the get deletes no build output and no free slot
- **AND** its `low_disk_space` says that the volume holds less than the minimum
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_free_space_refuses_and_names_it`)
  [@test] crates/bd-lease/src/trim_test.rs (`a_get_below_the_minimum_free_space_keeps_the_free_slots_past_the_limit_and_deletes_the_output_bases_of_gone_workspaces`)

### Requirement: Host slots: The keeper keeps the minimum free space without a get

`host_slots::keep_free`, which the keeper of brd-host runs once a minute, MUST keep the minimum free space of the volume
of the pool without a get: below `branchdeck.leaseMinFree` the trash, the reclaim of the idle leases, the trim of every
pool of the machine without its slots, the shrink and the eviction, each when the one before left the volume below it,
as a get takes them ("Get"). A host slot is no idle lease, and the shrink takes only the agent leases and the free
slots: it stays.

#### Scenario: The keeper below the minimum
- **WHEN** the volume of the pool is below `branchdeck.leaseMinFree` at a round of the keeper
- **THEN** it trims the pools of the machine without a get, and a free slot past `branchdeck.leaseMaxSlots` stays
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_keeps_the_free_slots_past_the_limit_below_the_minimum_free_space_without_a_get`)

#### Scenario: No minimum, no free space or an upgrade of git
- **WHEN** there is no minimum, the system tells no free space, or a live process upgrades git
- **THEN** the keeper deletes nothing

#### Scenario: The keeper reclaims idle leases
- **GIVEN** an idle lease whose work landed
- **WHEN** the volume of the pool is below `branchdeck.leaseMinFree` at a round of the keeper
- **THEN** it returns the lease without a get, its slot free with its record, and names it (`reclaimed`)
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_reclaims_the_idle_leases_whose_work_landed_without_a_get`)
