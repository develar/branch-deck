## MODIFIED Requirements

### Requirement: Get: The shrinks and evictions of the machine take turns under one lock

The shrinks and the evictions of the gets of the machine MUST take turns under one lock
(`~/.branch-deck/pressure.lock`; without a home, beside the state of the pool): a get that waits tells it
(`waiting for the shrink or the eviction of another get …`) and measures again after its turn, or as soon as the
volume has its minimum, without the turn, so gets side by side clean no slot that the volume did not need and none
waits for deletes it needs no more.

#### Scenario: A get that waited for the shrink of another
- **WHEN** a get waited for the shrink of another get and the volume has the target after it
- **THEN** it cleans nothing
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_that_waited_for_the_shrink_of_another_cleans_nothing_once_the_volume_has_the_target`)

#### Scenario: The volume has the minimum before the turn
- **WHEN** a get waits for the shrink of another get and the slots that one deleted give the volume the minimum
- **THEN** the get stops waiting, cleans nothing and leases while the other still holds the lock
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_that_waits_for_the_shrink_of_another_goes_on_once_the_volume_has_the_minimum_before_the_turn`)
