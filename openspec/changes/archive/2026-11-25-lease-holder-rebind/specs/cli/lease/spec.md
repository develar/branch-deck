## ADDED Requirements

### Requirement: Get: A session that runs brd in its slot becomes the holder of its lease

`brd workspace` and a write of the worktree or the branch of a slot, run inside it, MUST make the session of brd
(`holder_pid`, `holder_started`) the holder of a `ready` lease of an agent whose holder is gone, by a compare-and-swap
under the pool lock, unless a get takes it over, the slot has a `reclaimed` record, or a process between the session
and brd leads its session with a controlling terminal. A read and a command of the pool MUST change no holder.

#### Scenario: A resumed session in its slot
- **GIVEN** a lease whose holder is gone
- **WHEN** its resumed session runs brd in the slot or below it, a shell between them that leads no session, with the
  terminal of the session or none, and the session itself with one or not (an agent with a TUI in a tab of a terminal)
- **THEN** the session is the holder, `holder_alive` is true, and the lease is no longer idle
- **AND** a get passes over the lease and takes over another idle one
  [@test] crates/bd-lease/src/lease_test.rs (`a_session_in_its_slot_becomes_the_holder_of_a_lease_whose_holder_is_gone`, `brd_workspace_and_a_write_in_the_slot_rebind_the_holder_and_a_read_does_not`, `a_get_passes_over_a_rebound_lease_and_takes_over_an_idle_one`)

#### Scenario: A live holder costs one read
- **WHEN** the holder of the lease of the slot lives
- **THEN** brd reads the state once, with no list of the processes and no pool lock
  [@test] crates/bd-lease/src/lease_test.rs (`a_session_in_its_slot_becomes_the_holder_of_a_lease_whose_holder_is_gone`)

#### Scenario: A shell that owns a terminal
- **WHEN** a shell that leads its session with a controlling terminal (a tab of a terminal, a pane of tmux, the
  terminal of an IDE) runs brd below the session, or no session is below the init process
- **THEN** the holder stays as it was
  [@test] crates/bd-lease/src/lease_test.rs (`a_shell_that_owns_a_terminal_between_the_session_and_brd_rebinds_nothing`)
  [@test] crates/sync-utils/src/process_scan_test.rs (`a_process_leads_its_session_only_when_it_started_it`)

#### Scenario: A coordinator, a takeover and a host slot
- **WHEN** brd runs outside the slot with `-C` into it, a get takes the lease over, before the read of brd or between
  its read and its write, the slot has the `reclaimed` record of a takeover, or the lease is of a host slot
- **THEN** the holder stays as it was
  [@test] crates/bd-lease/src/lease_test.rs (`a_brd_outside_the_slot_a_takeover_a_reclaimed_slot_and_a_host_slot_rebind_nothing`, `brd_workspace_and_a_write_in_the_slot_rebind_the_holder_and_a_read_does_not`, `a_command_of_the_pool_and_a_write_from_outside_rebind_nothing`)

#### Scenario: A command of the pool
- **WHEN** a get, a reclaim, a clean, a trim, an adopt or a return by `--lease-id` or `--holder` runs in the slot
- **THEN** the holder stays as it was, and `brd lease reclaim --idle` treats the lease as idle
  [@test] crates/bd-lease/src/lease_test.rs (`a_command_of_the_pool_and_a_write_from_outside_rebind_nothing`)

#### Scenario: A rebind that fails
- **WHEN** the state of the pool cannot be written
- **THEN** the command goes on, and the holder stays as it was
  [@test] crates/bd-lease/src/lease_test.rs (`a_failed_rebind_fails_no_command`)

#### Scenario: A read
- **WHEN** a read runs in the slot (`brd lease status`)
- **THEN** the holder stays as it was
  [@test] crates/bd-lease/src/lease_test.rs (`brd_workspace_and_a_write_in_the_slot_rebind_the_holder_and_a_read_does_not`)

#### Scenario: The known limits
- **GIVEN** a lease that a get got by a takeover, whose slot keeps the `reclaimed` record, or a session that runs brd
  only through `-C` from outside the slot
- **WHEN** its holder is gone and the session runs brd again
- **THEN** the session never becomes the holder, and the lease stays idle
- **AND** on Windows, where the scan tells no terminal, a shell of a person in the slot rebinds as a coding agent does
- **AND** a wrapper that a person types in a terminal (`just`, `pnpm`, `cargo run`) is the session and becomes the
  holder for a moment, until it exits
- **AND** a harness that runs each command in a pseudo terminal of its own, whose shell leads that session, is not
  rebound

## MODIFIED Requirements

### Requirement: Get: A gone holder alone frees no lease

A gone holder alone MUST free no lease: its slot may hold unlanded work, and a resumed session runs as another
process, which becomes the holder once it runs brd in the slot. A get that adds a new slot MUST name the `ready` leases
whose holder process is gone (`idle_leases`, each with its `lease_id` and `idle`), and so MUST `low_disk_space`.

#### Scenario: A get adds a slot beside a lease whose holder is gone
- **GIVEN** a `ready` lease whose holder process is gone
- **WHEN** a get adds a new slot, or refuses with `low_disk_space`
- **THEN** the lease stays and the answer names it in `idle_leases`
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_tells_whether_the_holder_of_a_lease_is_alive`)

#### Scenario: A resumed session
- **WHEN** a resumed session of the lease runs `brd workspace` or a command that writes in the slot
- **THEN** it becomes the holder, and the lease is no longer idle
  [@test] crates/bd-lease/src/lease_test.rs (`brd_workspace_and_a_write_in_the_slot_rebind_the_holder_and_a_read_does_not`)
