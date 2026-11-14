## ADDED Requirements

### Requirement: trim: A signal ends trim at once, and the trim goes on on the host

The first Ctrl+C, SIGTERM or SIGHUP MUST end `brd-host trim` at once with 130 and the line `brd-host: Ctrl+C or SIGTERM
ends this process; the trim goes on on the host`.

#### Scenario: SIGTERM during a trim
- **WHEN** `brd-host trim <alias>` gets SIGTERM while it waits for the host
- **THEN** it ends at once with 130 and says that the trim goes on on the host
  [@test] crates/bd-host/tests/cli.rs (`sigterm_ends_a_trim_or_an_upgrade_at_once_and_says_what_goes_on_on_the_host`)

### Requirement: upgrade: A signal ends upgrade at once, and the host keeps a binary that came whole

The first Ctrl+C, SIGTERM or SIGHUP MUST end `brd-host upgrade` at once with 130 and the line `brd-host: Ctrl+C or
SIGTERM ends this process; the host goes on with an upgrade whose binary came whole`.

#### Scenario: SIGTERM during an upgrade
- **WHEN** `brd-host upgrade <alias>` gets SIGTERM while it waits for the host
- **THEN** it ends at once with 130 and says that the host goes on with an upgrade whose binary came whole
  [@test] crates/bd-host/tests/cli.rs (`sigterm_ends_a_trim_or_an_upgrade_at_once_and_says_what_goes_on_on_the_host`)

## MODIFIED Requirements

### Requirement: follow, cancel, fetch, forget, runs: follow shows a run from its first line

`brd-host follow <alias> <run>` MUST follow the output of a run from its first line, with the bytes of each stream as
`run` writes them, and exit with the exit code of the command at its end. The first Ctrl+C, SIGTERM or SIGHUP MUST leave
the follow at once with 130, and the run goes on.

#### Scenario: A follow of a run
- **WHEN** `brd-host follow <alias> <run>` runs
- **THEN** it shows the output of the run from its first line, with the bytes of each stream as `run` writes them
- **AND** it exits with the exit code of the command at the end of the run
  [@test] crates/bd-host/tests/cli.rs (`follow_of_an_ended_run_gives_the_bytes_of_its_stdout`)
  [@test] crates/host-agent/tests/serve/follow.rs (`a_follow_of_an_ended_run_gives_the_bytes_of_each_stream`)
  [@test] crates/host-client/src/run_test.rs (`the_output_sink_gets_the_bytes_of_each_stream`)

#### Scenario: Ctrl+C during a follow
- **WHEN** the user presses Ctrl+C during `brd-host follow`, or a tool sends it SIGTERM or SIGHUP
- **THEN** the follow leaves at once with 130 and `interrupted: Ctrl+C or SIGTERM left the follow of the run <run>, which
  goes on on <alias>: `brd-host follow <alias> <run>` follows it again`, sends no cancel, and the run goes on on the host
  [@test] crates/host-client/src/run_test.rs (`ctrl_c_leaves_a_follow_and_the_run_goes_on`)
  [@test] crates/bd-host/tests/cli.rs (`sigterm_to_a_follow_leaves_it_and_the_run_goes_on`)
