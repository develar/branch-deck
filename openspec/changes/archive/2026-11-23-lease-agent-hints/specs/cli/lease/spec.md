## ADDED Requirements

### Requirement: Return: A holder returns its one lease from any checkout

`brd lease return --holder <name>` MUST return the one lease of that holder (`--holder` of its get), from any checkout
of the repository, as `--lease-id` of it does. A holder of several leases MUST be refused (`invalid_arguments`, exit 2),
the message naming each id and slot and `next_steps` the return of each by its id; a holder of none is `no_lease`.

#### Scenario: One lease, several and none
- **WHEN** `brd lease return --holder <name>` runs in the shared checkout
- **THEN** the one lease of the holder is returned; several are refused with the return of each in `next_steps`, and
  none is `no_lease`
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_by_holder_returns_the_one_lease_of_the_holder`)

### Requirement: Return: A slot busy only with commands of the holder names the return that ends them

A `slot_busy` of a return whose processes are all commands of the holder (`kind: holder`) MUST name
`brd lease return --workspace <slot> --stop-own` as the first of `next_steps`; with a process of another session it
MUST name none.

#### Scenario: Only commands of the holder
- **WHEN** a background command of the holder is the only process in the slot, and the return runs without `--stop-own`
- **THEN** `next_steps` starts with the return with `--stop-own`
  [@test] crates/bd-lease/src/lease_test.rs (`a_background_command_of_the_holder_is_named_holder_and_keeps_the_slot_without_stop_own`)

#### Scenario: A process of another session
- **WHEN** a process of another session is in the slot beside them
- **THEN** `next_steps` is empty
  [@test] crates/bd-lease/src/lease_test.rs (`stop_own_never_ends_a_process_of_another_session`)

### Requirement: Pick: The update of a conflict names the message of its commit by its patch

The `update` of a conflict of `brd lease pick` MUST leave out the `body` of the commit of its round: its `patch` file
holds the message, and `brd update --status` in the lease names the body.

#### Scenario: A commit with a body
- **WHEN** the commit of a lease that conflicts has a body
- **THEN** `update.session.commit` has no `body`, its `patch` has it
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`)

### Requirement: Status: List is the status

`brd lease list` MUST be `brd lease status`, a hidden alias, as agents call it.

#### Scenario: Lease list
- **WHEN** `brd lease list` runs
- **THEN** it prints what `brd lease status` prints
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_reads_the_state_without_a_git_status`)
