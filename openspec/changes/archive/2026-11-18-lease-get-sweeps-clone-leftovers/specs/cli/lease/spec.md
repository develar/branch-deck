## ADDED Requirements

### Requirement: Get: A get that takes the clones lock moves the leftovers of killed clones into the trash

A get that takes the clones lock MUST move into the trash every number directory of its pool that no state of a pool
of the machine names and that holds only `<repository>.cloning`, which a get killed during its clone leaves; under that
lock no clone of the repository runs, so such a directory is no slot. It MUST tell the step and leave the trash to a
later sweep, never deleting it under the lock. An empty number directory, one that holds more, and every slot stay.

#### Scenario: The leftover of a killed clone
- **GIVEN** a number directory of the pool that holds only `<repository>.cloning`, an empty one and one that holds more
- **WHEN** a get takes the clones lock to add a slot
- **THEN** it moves the first into the trash and tells the step, and the others stay
- **AND** a get that reuses a free slot takes no clones lock and sweeps nothing
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_that_adds_a_slot_moves_the_leftovers_of_killed_clones_into_the_trash`)
