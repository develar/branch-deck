## MODIFIED Requirements

### Requirement: Get: The shrinks and evictions of the machine take turns under one lock

The shrinks and the evictions of the gets of the machine MUST take turns under one lock
(`~/.branch-deck/pressure.lock`; without a home, beside the state of the pool): a get that waits tells it and
measures again after its turn, or as soon as the volume has its minimum, without the turn, so gets side by side clean
no slot that the volume did not need and none waits for deletes it needs no more.

#### Scenario: A get that waited for the shrink of another
- **WHEN** a get waited for the shrink of another get and the volume has the target after it
- **THEN** it cleans nothing
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_that_waited_for_the_shrink_of_another_cleans_nothing_once_the_volume_has_the_target`)

#### Scenario: The volume has the minimum before the turn
- **WHEN** a get waits for the shrink of another get and the slots that one deleted give the volume the minimum
- **THEN** the get stops waiting, cleans nothing and leases while the other still holds the lock
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_that_waits_for_the_shrink_of_another_goes_on_once_the_volume_has_the_minimum_before_the_turn`)

#### Scenario: A wait of minutes
- **WHEN** a get waits for the shrink or the eviction of another get
- **THEN** it says so (`waiting for the shrink or the eviction of another get …`), whose, for how long that one holds
  the lock, and the free space of the volume against its minimum, and says it again once a minute
  [@test] crates/lease-pool/src/lease_test.rs (`the_wait_for_the_shrink_of_another_names_the_holder_its_time_and_the_free_space`)

### Requirement: Get: The shrink stops at the minimum and a tenth of it

Before each slot, and every 256 entries of the delete of a slot, the get MUST measure (one `statfs`) and stop once the
volume has the minimum and a tenth of it, what it did not delete left to a later sweep: without the margin every
later get would find the volume just below the minimum again and clean one more slot, and a slot of a large
repository takes minutes to delete, the volume past the target needing none of it.

#### Scenario: One slot lifts the volume past the minimum
- **WHEN** the shrink of one slot lifts the volume past the minimum and a tenth of it
- **THEN** the shrink cleans only that slot
  [@test] crates/bd-lease/src/lease_test.rs (`a_shrink_that_one_slot_lifts_past_the_minimum_cleans_only_that_slot`)

#### Scenario: The volume has the target within the delete of a slot
- **WHEN** the volume has the minimum and a tenth of it while the shrink deletes the build output of a slot
- **THEN** the delete stops at its next measure, the rest stays in the trash for the next get below the minimum, and
  the get leases at once
  [@test] crates/bd-lease/src/lease_test.rs (`a_shrink_stops_inside_the_delete_of_a_slot_once_the_volume_has_the_target`)
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_delete_stops_at_the_ask_its_stop_holds_and_leaves_the_rest`)
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_trash_whose_delete_stopped_is_left_to_the_next_sweep`)

### Requirement: Get: The eviction stops at the minimum without the margin of the shrink

The eviction MUST measure (one `statfs`) before each slot, and while the delete of a slot runs, and stop once the
volume has the minimum, without the margin of the shrink: a slot is a whole worktree, and with slots of 15 GiB a margin
of 20 GiB would delete one more after the volume is already above the minimum.

#### Scenario: The volume reaches the minimum
- **WHEN** an evicted slot brings the volume to the minimum
- **THEN** the eviction stops
  [@test] crates/bd-lease/src/lease_test.rs (`the_eviction_stops_at_the_minimum_without_the_margin_of_the_shrink`)

#### Scenario: The volume reaches the minimum within the delete of a slot
- **WHEN** the volume has the minimum while the eviction deletes a slot
- **THEN** its deletes stop at their next look, and the rest stays in the trash for a later sweep
  [@test] crates/lease-pool/src/trim_test.rs (`a_delete_side_by_side_stops_when_its_stop_holds_and_leaves_the_paths`)
