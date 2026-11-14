## ADDED Requirements

### Requirement: Get: The epoch of a slot is the lease it holds

The epoch of a worktree (`lease_pool::epoch::worktree_epoch`) MUST be the id of the lease its slot holds, `returned
<n>` of a free slot by the number of its last return, and none for a worktree that is no slot of a pool; it MUST be
read without git, from the `.git` file of the worktree and the state of the pool. The app keeps what it records of a
slot by its path under this epoch ([Onboarding](../../repository/onboarding/spec.md)).

#### Scenario: A slot leased again
- **WHEN** a slot is leased, returned and leased again
- **THEN** its epoch is each lease in turn, and that of its return between them; the main checkout has none
  [@test] crates/bd-lease/src/lease_test.rs (`the_epoch_of_a_slot_is_its_lease_and_changes_with_each_return_and_get`)
