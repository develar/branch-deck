## MODIFIED Requirements

### Requirement: Get: The epoch of a slot is the lease it holds

The epoch of a worktree (`worktree_epoch`) MUST be the id of the lease its slot holds (`returned <n>` while free),
`NotASlot` for a worktree that is no slot, and `Unknown` when the files that tell cannot be read or parsed; it MUST be
read without git and without the lock of the pool, of each slot only its path, lease id and last return. The app keeps
what it records of a slot under it ([Onboarding](../../repository/onboarding/spec.md)).

#### Scenario: A slot leased again
- **WHEN** a slot is leased, returned and leased again
- **THEN** its epoch is each lease in turn, and that of its return between them; the main checkout has none
  [@test] crates/bd-lease/src/lease_test.rs (`the_epoch_of_a_slot_is_its_lease_and_changes_with_each_return_and_get`)

#### Scenario: A state of a later build, and one that cannot be parsed
- **GIVEN** a later build wrote the state of the pool, with a state of a lease and a field this build does not know
- **THEN** the epoch of the slot is still its lease
- **AND** a state that cannot be parsed makes it `Unknown`, never `NotASlot`
  [@test] crates/bd-lease/src/lease_test.rs (`the_epoch_reads_a_state_of_a_later_build_and_is_unknown_for_one_it_cannot_parse`)
