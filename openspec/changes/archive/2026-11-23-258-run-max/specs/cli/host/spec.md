## ADDED Requirements

### Requirement: run: --max waits for the end as wait does

With `--max <duration>` a run MUST follow no output: after its pack it MUST wait for the end of the run as `brd-host
wait` does, at most `--max` from the start of the call, and finish the run as a wait does, its output into
`<run>.log`, then say the summary of a wait. Past `--max` it MUST refuse with `still_running` (124), send no cancel,
and drop the base sync, and the run goes on. It MUST NOT leave before the pack reached the host.

#### Scenario: A run that goes on past --max
- **WHEN** a run with `--max` goes on past it
- **THEN** the call exits 124 with `still_running`, sends no cancel and no follow, and the run goes on
- **AND** the pack reached the host before the call left
  [@test] crates/host-client/src/run_test.rs (`run_max_leaves_past_its_max_without_a_cancel_and_never_before_its_pack`)

#### Scenario: A run that ends in time
- **WHEN** a run with `--max` ends before it
- **THEN** its output goes to the log of the run and not to stdout, its files come, its end is written, the run is
  forgotten, and the call exits with the exit code of the command
  [@test] crates/host-client/src/run_test.rs (`run_max_finishes_a_run_that_ends_in_time_as_a_wait_does`)

#### Scenario: Ctrl+C during a run with --max
- **WHEN** the user presses Ctrl+C during a run with `--max`
- **THEN** the run is cancelled on the host and the call waits for its end, as without `--max`; a second Ctrl+C leaves
  with 130

## MODIFIED Requirements

### Requirement: run: The command line of run

`brd-host run` MUST take the alias of a host, the options of its synopsis and, after `--`, the command with its
arguments. `--fetch` MUST be repeatable, `--nice` MUST take 0 to 19, and `--max` a duration as `brd-host wait` takes
it.

#### Scenario: The synopsis of run
- **WHEN** a caller runs `brd-host run`
- **THEN** it takes this command line

```sh
brd-host run <alias> [--head] [--apply] [--fetch <dir>]... [--no-outputs] [--heavy] [--exclusive] [--nice <0-19>]
            [--repo <path>] [--affinity <key>] [--max <duration>] [--report-fd <n>] -- <command> <args>
```

### Requirement: The report of a run: The exits of the report

`exit` MUST be the exit code of `brd-host`. `command_exit` MUST be the exit code of the command, `null` when it did
not run or its end is unknown; a command that a signal ended answers 128 plus the signal number.

#### Scenario: A command that ran to its end
- **WHEN** the command exited
- **THEN** `exit` is the exit code of `brd-host`, and `command_exit` the exit code of the command
  [@test] crates/host-client/src/report_test.rs (`the_report_of_a_run_is_the_golden_json_of_the_spec`)

#### Scenario: A command that did not run or whose end is unknown
- **WHEN** the command did not run, or its end is unknown
- **THEN** `command_exit` is `null`

#### Scenario: A command that a signal ended
- **WHEN** a signal ended the command
- **THEN** `command_exit` is 128 plus the signal number

#### Scenario: A run that goes on past --max
- **WHEN** a run with `--max` leaves the run going
- **THEN** `exit` is 124, `refusal` has the code `still_running`, and `command_exit` is `null`
