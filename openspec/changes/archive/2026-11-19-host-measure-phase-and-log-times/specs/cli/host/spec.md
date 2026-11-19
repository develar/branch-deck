## MODIFIED Requirements

### Requirement: The report of a run: The phases are the steps of both ends in their order

The steps MUST be `pack` and `upload` of the client; `heavy`, `git`, `slot`, `pack_wait`, `checkout`, `prepare`,
`command`, `measure` and `stage` of the host; `fetch` and `total` of the client. A run that ends early has only the steps
up to its last one. Only a heavy run has `heavy`, and only a run that waited for an upgrade of git has `git`. `measure`
is the time of the measure of the cores of the command outside the command: the cores of the host and the first look at
the Bazel servers of the slot before it, the last sample and the record of the measure after it, so a volume that hangs
those looks shows in a step and not in the gap between the steps and `total`.

#### Scenario: The steps of a run
- **WHEN** a run goes through
- **THEN** its phases are `pack`, `upload`, `slot`, `pack_wait`, `checkout`, `prepare`, `command`, `measure`, `stage`,
  `fetch` and `total`, with `heavy` first of the host for a heavy run, and `git` only when it waited for an upgrade of
  git
  [@test] crates/host-client/src/phases_test.rs (`the_phases_are_the_steps_of_both_ends_in_their_order`)
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_the_host_run_one_at_a_time_and_a_run_that_is_not_heavy_goes_beside_them`)

#### Scenario: A run that ends early
- **WHEN** a run ends before its last step
- **THEN** its phases have only the steps up to its last one
  [@test] crates/host-client/src/phases_test.rs (`the_phases_are_the_steps_of_both_ends_in_their_order`)

## ADDED Requirements

### Requirement: The files: Each line of the log of the agent starts with its local time

Each line that `serve`, a supervisor or the watchdog writes to the log of the agent (`~/Library/Logs/brd-host.log`) and
to `supervisor.log` MUST start with the local time of the host to the millisecond, `2026-11-19 07:10:02.123`, and MUST
reach the file in one write, so a stall of a run can be set against the other lines and the system log, and the lines of
several processes never mix.

#### Scenario: A line of the log
- **WHEN** the agent writes a line to its log
- **THEN** the line is `<yyyy-mm-dd hh:mm:ss.mmm> brd-host: <message>`, or `… brd-host supervise <run>: <message>` for a
  line of a supervisor after the end of its run
  [@test] crates/host-agent/src/log_test.rs (`a_line_of_the_log_starts_with_the_local_time_to_the_millisecond`, `a_panic_after_the_end_of_a_run_reaches_the_log_of_the_agent_with_the_id_of_the_run`)
  [@test] crates/host-agent/tests/serve.rs (`the_give_back_of_a_forgotten_run_writes_its_lines_to_the_log_of_the_agent`)
