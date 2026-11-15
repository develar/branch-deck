## MODIFIED Requirements

### Requirement: follow, cancel, fetch, forget, runs: cancel interrupts a run on the host

`brd-host cancel <alias> <run>` MUST ask the agent to interrupt the run: SIGINT to the process group of the command,
SIGINT again 1 s later to each process of the group but the command that started as it came (1 s before it or later),
SIGKILL after 120 s, or no command when it has not started. It MUST return when the agent has the request.

#### Scenario: A cancel of a running command
- **WHEN** `brd-host cancel <alias> <run>` runs while the command runs
- **THEN** the agent sends SIGINT to the process group of the command
- **AND** `cancel` returns when the agent has the request
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)

#### Scenario: A process that started as the SIGINT came
- **WHEN** a process of the group started as the SIGINT of a cancel came, so it never saw it (a shell that got the
  SIGINT between two commands starts the next one before its trap), and it still runs 1 s later
- **THEN** it gets SIGINT, and the line of the run says `brd-host: SIGINT again to the processes that started as the
  SIGINT came: <pid> (<command>)`, so the run ends without the SIGKILL of the grace time
- **AND** the command itself, a process that started more than 1 s before the SIGINT and one of another group get none
  [@test] crates/host-agent/tests/serve/signals.rs (`a_process_that_started_as_the_sigint_came_gets_it_again_and_the_run_ends_without_the_grace`)
  [@test] crates/host-agent/src/supervise_test.rs (`a_process_that_started_as_the_sigint_came_is_a_newcomer`)

#### Scenario: A command that outlives SIGINT
- **WHEN** the command still runs 120 s after the SIGINT of a cancel
- **THEN** the agent sends SIGKILL to its process group

#### Scenario: A cancel before the command started
- **WHEN** `brd-host cancel <alias> <run>` runs before the command started
- **THEN** no command starts
