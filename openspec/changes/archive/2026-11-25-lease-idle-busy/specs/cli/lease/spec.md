## ADDED Requirements

### Requirement: Status: An idle lease is busy while another session works in its slot

The status MUST name a `ready` lease whose holder is gone `busy`, with the `processes` in its slot, while a process
other than its Bazel servers works there, and keep the work a check found: a resumed session runs as another process.
It MUST scan the processes once, only when such a lease exists; a slot with only its Bazel servers MUST stay as it was.

#### Scenario: A resumed session works in the slot
- **GIVEN** a lease whose holder is gone, with local changes a get found
- **WHEN** a process of another session works in its slot and `brd lease status` runs
- **THEN** its `idle` is `busy` with that process in `processes` (`kind: other`), and `paths` and `checked_at` stay
  [@test] crates/bd-lease/src/lease_test.rs (`an_idle_lease_with_a_process_of_another_session_in_its_slot_is_busy`)

#### Scenario: Its Bazel server alone
- **WHEN** only a Bazel server of the slot runs there
- **THEN** its `idle` stays `idle` or `idle_with_work`, since a get stops the server itself
  [@test] crates/bd-lease/src/lease_test.rs (`an_idle_lease_with_a_process_of_another_session_in_its_slot_is_busy`)

#### Scenario: No lease whose holder is gone
- **WHEN** every holder of the pool runs
- **THEN** the status scans no process
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_scans_the_processes_only_for_a_lease_whose_holder_is_gone`)

## MODIFIED Requirements

### Requirement: Status: The status lists every slot from the state alone

`brd lease status` MUST list every slot from the state alone, with its `path`, `state` (`free`, `preparing`, `ready`,
`templating`, `returning`, `cleaning`, `abandoned`, `unfinished`), `dirty`, `missing`, `lease` and `reclaimed`, the `free_bytes` of
the volume of the pool and the `trash_bytes` a killed run left; it MUST run no `git status`. `--processes` MUST add the
processes in each slot. It writes nothing.

#### Scenario: The status of the pool
- **WHEN** `brd lease status` runs
- **THEN** it lists every slot from the state without a `git status`
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_reads_the_state_without_a_git_status`)

#### Scenario: A slot whose add never finished
- **WHEN** a free slot has no name and was never returned
- **THEN** its `state` is `unfinished`
  [@test] crates/bd-lease/src/unfinished_test.rs (`a_slot_whose_add_never_finished_is_removed_by_the_next_get_that_adds_a_slot`)

#### Scenario: The holder of a lease
- **WHEN** a slot has a lease
- **THEN** `lease` has `holder_pid`, `holder_alive`, `idle` for a ready lease whose holder is gone (`idle`,
  `idle_with_work` with what a check found, or `busy` with the processes of another session in its slot), and
  `taking_over` while a get checks one
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_tells_whether_the_holder_of_a_lease_is_alive`, `an_idle_lease_with_a_process_of_another_session_in_its_slot_is_busy`)

#### Scenario: A read
- **WHEN** `brd lease status` runs, with or without `--processes`
- **THEN** it writes no ref, config, index or working tree
  [@test] crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)
