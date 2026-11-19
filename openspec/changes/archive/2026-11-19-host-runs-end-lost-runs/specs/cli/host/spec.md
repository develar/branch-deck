## MODIFIED Requirements

### Requirement: serve: A follow ends a run that lost its supervisor

A run that lost its supervisor changes no file, so the follow MUST look at it at least every 1 s and end it; a cancel,
a forget and the list of the runs (`Request::Runs`, which `brd-host runs` shows) MUST end it too, and the hourly cleanup
ends the rest. A run is alive while a process holds the flock of its `lock`, so a pid that another process took since
keeps no run going, and a lost heavy run holds no cores of the heavy turn.

#### Scenario: A killed supervisor
- **WHEN** the supervisor of a run is killed
- **THEN** the follow looks at the run at least every 1 s and ends it, for a cancel too
  [@test] crates/host-agent/tests/serve/signals.rs (`a_killed_supervisor_ends_the_run_for_cancel_and_follow`)

#### Scenario: The list of the runs after a killed supervisor
- **WHEN** `brd-host runs` lists a run whose supervisor was killed
- **THEN** the run reads as finished, not as running, refused `agent_error` with `the supervisor of the run ended
  without an exit code: it was killed, or the host restarted, so whether the command ran to its end is unknown; run it
  again`
  [@test] crates/host-agent/tests/serve/signals.rs (`a_killed_supervisor_ends_its_run_in_the_list_of_the_runs`)
