## MODIFIED Requirements

### Requirement: Get: A get records the process it runs for

A get MUST record the process it runs for, the nearest ancestor that is no shell or wrapper (the coding agent, the
terminal), with its start time (`holder_pid`, `holder_started`): the `pid` of a lease is of the get, which exits at
once. On Windows the ancestors MUST come from one snapshot of the process list (the pid, the parent and the exe of
each), a parent that started after its child ending the chain.

#### Scenario: The holder of a lease
- **WHEN** a get records its lease
- **THEN** the lease names the nearest ancestor that is no shell or wrapper with its start time
- **AND** the status tells whether that holder is alive
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_tells_whether_the_holder_of_a_lease_is_alive`)

#### Scenario: On Windows
- **WHEN** a get runs on Windows, under a `bash` or a `cmd` of the agent
- **THEN** the lease names the agent, read from the snapshot of the process list
- **AND** a parent pid that a younger process got since ends the chain there, and a process whose start the system
  won't tell (a service of another user) is no holder
  [@test] crates/sync-utils/src/process_scan_test.rs (`the_chain_skips_the_runners_of_windows_and_ends_at_a_parent_younger_than_its_child`)
  [@test] crates/sync-utils/src/process_scan_test.rs (`the_ancestors_of_this_process_are_read_on_every_platform`)

### Requirement: Reclaim: Reclaim applies the rule of the takeover to every idle lease

`brd lease reclaim --idle [--dry-run] [--keep <n>] [--age <duration>]` MUST apply the rule of the takeover to every
idle lease of the pool, each checked anew: one whose work landed is returned as the takeover returns it, under a CAS on
its lease, its holder still gone and its slot as the check found it, its slot free with its record. The output is
`lease_reclaim` of `brd schema`, exit 0.

#### Scenario: An idle lease whose work landed
- **WHEN** reclaim finds an idle lease whose work landed
- **THEN** it returns it as the takeover returns it (`reclaimed`, each with `landed`, `stopped_servers`, `cleaned`)
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_reclaim`)

#### Scenario: An idle lease with work
- **WHEN** reclaim finds an idle lease with work
- **THEN** it keeps the lease and records its check (`kept`, as `idle_with_work`)
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)

#### Scenario: A lease it cannot judge
- **WHEN** a lease is not quiet for an hour, busy, changed meanwhile or its check failed
- **THEN** it is `skipped` with its reason

#### Scenario: A lease whose holder is unknown
- **GIVEN** a `ready` lease whose get recorded no holder process
- **WHEN** reclaim runs, with `--dry-run` or not
- **THEN** it is `skipped` as `holder_unknown`, checked by nothing, and stays `ready`
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_names_a_lease_without_a_holder_as_holder_unknown`)

#### Scenario: A dry run
- **WHEN** reclaim runs with `--dry-run`
- **THEN** it checks the same and writes nothing

### Requirement: Reclaimed: The session that holds the slot is told by its process

The session that holds it MUST be told by its process: refused unless the holder (pid and start time) is an ancestor of
bd, on Windows too. The check MUST read the state, no git, with a process scan only in such a slot; a chain of
ancestors cut below the init process MUST refuse nothing.

#### Scenario: The holder is an ancestor
- **WHEN** the holder recorded with its pid and start time is an ancestor of bd
- **THEN** the command is not refused
  [@test] crates/sync-utils/src/process_scan_test.rs (`an_ancestor_is_a_process_above_this_one_with_its_start_time`)

#### Scenario: A chain cut below the init process
- **WHEN** the chain of the ancestors ends at a parent younger than its child, past a link whose start the system
  won't tell or not
- **THEN** the holder is neither found nor denied, and the command is not refused
  [@test] crates/sync-utils/src/process_scan_test.rs (`the_chain_skips_the_runners_of_windows_and_ends_at_a_parent_younger_than_its_child`)
