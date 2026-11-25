## MODIFIED Requirements

### Requirement: Get: Every status of the pool may write the index of the slot

Every `status` of the pool MUST be allowed to write the slot's index (`GIT_OPTIONAL_LOCKS=1`), so its refreshed stat
data, fsmonitor token and untracked cache stay. A get that adds a slot by a clone MUST run one `status` of the slot
when nothing untracked is left: the `status` of its clean also finds the local changes.

#### Scenario: The untracked cache stays warm
- **WHEN** a get and its return ran in a slot
- **THEN** a `status` that may not write finds the untracked cache filled
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_of_a_slot_leaves_its_untracked_cache_warm`)

#### Scenario: The clean of a clone and a discard
- **WHEN** the clean of a clone or the discard of a return removes the untracked files of a slot
- **THEN** its `status` writes the index, and a `status` that may not write finds the untracked cache filled
- **AND** the clean answers the local changes of its last `status`, so the add of a clone runs no other `status`
  [@test] crates/git-ops/src/lease_slot_test.rs (`the_clean_of_a_clone_and_a_discard_leave_the_untracked_cache_warm`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_of_a_new_slot_by_a_clone`)
