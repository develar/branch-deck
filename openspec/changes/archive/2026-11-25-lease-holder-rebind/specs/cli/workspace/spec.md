## ADDED Requirements

### Requirement: A resumed session in its slot becomes the holder of its lease

`brd workspace` in a slot whose holder is gone MUST make the session of brd the holder of the lease by the rule of
[brd lease](../lease/spec.md#requirement-get-a-session-that-runs-brd-in-its-slot-becomes-the-holder-of-its-lease) and
say so in `holder_rebound`, omitted otherwise. After it the lease MUST be listed with `holder_alive` true and no `idle`.

#### Scenario: A resumed session runs brd workspace in its slot
- **GIVEN** a lease whose holder is gone
- **WHEN** its resumed session runs `brd workspace` in the slot
- **THEN** the answer has `holder_rebound: true`, and `active_leases` lists the lease with `holder_alive` true
  [@test] crates/bd-lease/src/lease_test.rs (`brd_workspace_and_a_write_in_the_slot_rebind_the_holder_and_a_read_does_not`)

#### Scenario: A coordinator
- **WHEN** a session runs `brd -C <slot> workspace` from outside the slot
- **THEN** the answer has no `holder_rebound`, and the holder stays as it was
  [@test] crates/bd-lease/src/lease_test.rs (`brd_workspace_and_a_write_in_the_slot_rebind_the_holder_and_a_read_does_not`)
