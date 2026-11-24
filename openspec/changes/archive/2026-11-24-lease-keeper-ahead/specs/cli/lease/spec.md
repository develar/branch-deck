## ADDED Requirements

### Requirement: Host slots: The keeper prunes ahead of the minimum

When the volume of the pool has `branchdeck.leaseMinFree` but less than one and a half of it, `host_slots::keep_free`
MUST run the shrink alone, up to one and a half of the minimum (`Relieved::ahead`): the `ready` agent leases of the
volume by the cargo rules of their leases, then the free slots with `keep` 0 and `age` 0, but the free slot each pool's
next get takes, and stop at the threshold. It MUST take the lock of the shrink of the machine only when no get holds
it.

#### Scenario: The prune ahead
- **GIVEN** a leased slot with stale units and two free slots with their build output
- **WHEN** the volume has the minimum but less than one and a half of it at a round of the keeper
- **THEN** the leased slot loses its stale units only, and the shrink stops once the volume has the threshold
- **AND** at a later round below the threshold the free slot returned longest ago loses its build output
- **AND** the free slot of the next get keeps its output, though the volume is still below the threshold
- **AND** no reclaim, trim, eviction or relief of a host slot runs
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_prunes_ahead_of_the_minimum_and_keeps_the_free_slot_of_the_next_get`)

#### Scenario: Above the threshold, or a get shrinks
- **WHEN** the volume has one and a half of the minimum, or a get holds the lock of the shrink of the machine
- **THEN** the keeper deletes nothing
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_prunes_ahead_of_the_minimum_and_keeps_the_free_slot_of_the_next_get`)

### Requirement: Host slots: A prune ahead deletes the left trash first

Before its shrink, a prune ahead of the minimum MUST delete the trash that earlier deletes left in the pools of the
machine on the volume: the rest of a stopped delete and the trash of gone runs, up to the threshold. It MUST run no
shrink when the volume has the threshold after it.

#### Scenario: The trash that earlier deletes left
- **GIVEN** the trash of a pool holds what a stopped delete left
- **WHEN** the keeper prunes ahead of the minimum
- **THEN** it deletes that trash first, and runs no shrink when the volume has the threshold after it
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_prunes_ahead_of_the_minimum_and_keeps_the_free_slot_of_the_next_get`)

### Requirement: Host slots: A prune ahead relieves no host slot

A prune ahead that ends below the minimum MUST relieve no host slot in its round (`ReliefRound`), as the volume had the
minimum at its measure. The next round measures the volume again and runs the whole relief.

#### Scenario: A prune ahead that ends below the minimum
- **GIVEN** an idle host slot with build output
- **WHEN** the volume has the minimum at the measure of a round, and a prune ahead ends below it
- **THEN** no host slot is relieved in that round
- **AND** the next round measures the volume below the minimum and runs the whole relief, the idle host slot among it
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_prune_ahead_that_ends_below_the_minimum_leaves_the_host_slots_to_the_next_round`)
