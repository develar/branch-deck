## RENAMED Requirements

- FROM: `### Requirement: follow, cancel, fetch, forget, runs: forget removes a run, and an ended run stays 24 h`
- TO: `### Requirement: follow, cancel, fetch, forget, runs: forget removes the files of a run, and the host keeps its end`

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

#### Scenario: A follow of a finished run
- **WHEN** `brd-host follow <alias> <run>` runs after the run ended and its client forgot it
- **THEN** it shows what the host keeps of its log and exits with the exit code of the command, not `unknown_run`
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)

#### Scenario: Ctrl+C during a follow
- **WHEN** the user presses Ctrl+C during `brd-host follow`, or a tool sends it SIGTERM or SIGHUP
- **THEN** the follow leaves at once with 130 and `interrupted: Ctrl+C, SIGTERM or SIGHUP left the follow of the run <run>, which
  goes on on <alias>: `brd-host follow <alias> <run>` follows it again`, sends no cancel, and the run goes on on the host
  [@test] crates/host-client/src/run_test.rs (`ctrl_c_leaves_a_follow_and_the_run_goes_on`)
  [@test] crates/bd-host/tests/cli.rs (`sigterm_to_a_follow_leaves_it_and_the_run_goes_on`)

### Requirement: follow, cancel, fetch, forget, runs: forget removes the files of a run, and the host keeps its end

`brd-host forget <alias> <run>` MUST remove the files of an ended run on the host (its stage, its outputs and its
changes) and keep its spec, its end, its phases and the last 1 MiB of its log. The host MUST keep an ended run for
24 h at most, and of the forgotten runs only the last 100 by their end. A forget cut short MUST be finished by the next
forget or the hourly cleanup.

#### Scenario: A forget of an ended run
- **WHEN** `brd-host forget <alias> <run>` runs for an ended run
- **THEN** the files of the run are gone from the host, and the host keeps its end and the tail of its log
- **AND** a fetch of it refuses with `bad_request` (its files are gone), and a second forget answers `the run <run>
  was forgotten already`
  [@test] crates/host-agent/src/run_test.rs (`a_forgotten_run_keeps_its_end_and_its_log_and_its_files_go`)
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)

#### Scenario: The log of a forgotten run
- **WHEN** the log of a forgotten run is longer than 1 MiB
- **THEN** it keeps its last 1 MiB from the start of an event, after the line `brd-host: the run was forgotten, and its
  log keeps its last 1024 KiB: the first <n> bytes of its log are gone`
- **AND** a follow that opened the log before the cut reads it whole, and a follow from an offset of the log before
  the cut goes on there, or at that line for an offset in the part that went
  [@test] crates/host-agent/tests/serve/follow.rs (`a_follow_across_the_forget_that_cuts_the_log_keeps_its_bytes_and_its_offsets`)
  [@test] crates/host-agent/src/run_test.rs (`the_log_of_a_forgotten_run_keeps_its_tail_from_the_start_of_an_event`)

#### Scenario: More forgotten runs than the host keeps
- **WHEN** a forget or the hourly cleanup finds more than 100 forgotten runs
- **THEN** the ones past the last 100 by their end go, and a run that ended and that nobody forgot stays
  [@test] crates/host-agent/src/run_test.rs (`the_forgotten_runs_past_the_latest_by_their_end_go`)

#### Scenario: A forget cut short
- **WHEN** a forget stopped after it marked the run, with files or the log of the run left as they were
- **THEN** the run counts as forgotten, and the next forget or the hourly cleanup finishes the removal and cuts the log
  once
- **AND** two forgets of one run at once cut its log once, the cleanup skipping a run whose forget goes on
  [@test] crates/host-agent/src/run_test.rs (`a_forget_cut_short_is_finished_by_the_next_one`, `two_forgets_of_one_run_at_once_cut_its_log_once_and_the_record_names_the_log`)

#### Scenario: An ended run that nobody forgets
- **WHEN** a run ended and nobody forgot it
- **THEN** the host keeps it with its files for 24 h
  [@test] crates/host-agent/src/run_test.rs (`a_run_that_ended_a_day_ago_is_removed`)

### Requirement: follow, cancel, fetch, forget, runs: runs lists the runs that the agent keeps

`brd-host runs <alias> [--json]` MUST list the runs that the agent keeps, the finished and forgotten ones too, each
with its state, its start, its affinity, commit, command and the steps of 0.1 s or more, and for a finished run its
exit and how long it took, then the heavy turn of the host. An agent without the feature `heavy` has no heavy turn to
list.

#### Scenario: The runs of a host
- **WHEN** `brd-host runs <alias>` runs while a run goes on
- **THEN** it lists that run as `<run>  running, started <YYYY-MM-DD HH:MM:SS>` with its affinity, commit, command and
  the steps of 0.1 s or more
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-client/src/phases_test.rs (`a_run_line_says_its_state_its_start_and_how_long_a_finished_run_took`)

#### Scenario: The finished runs of a host
- **WHEN** `brd-host runs <alias>` runs after a run ended
- **THEN** it lists the run as `<run>  finished, exit <code>, started <YYYY-MM-DD HH:MM:SS>, took <duration>`, the
  start in the time zone of the client, with `, its files kept` while nobody forgot it, and without `took` from an
  agent that sends no end
- **AND** `--json` has the run as `RunInfo` with `ended_ms`, and `forgotten` once its client forgot it; a run that lost
  its supervisor has no `ended_ms` and no `took`, as the agent saw the loss later
  [@test] crates/host-agent/src/run_test.rs (`a_lost_run_has_no_time_of_its_end`)
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-client/src/phases_test.rs (`a_run_line_says_its_state_its_start_and_how_long_a_finished_run_took`)
  [@test] crates/host-link/src/proto_test.rs (`the_end_and_the_forget_of_a_run_info_are_optional_fields_within_the_schema`)

#### Scenario: The heavy turn of a host
- **WHEN** `brd-host runs <alias>` runs while a heavy run holds the heavy turn and another waits for it
- **THEN** after the runs it says `heavy turn: run <id> of <repository> holds it <time>: <command>`, then
  `  run <id> of <repository> waits <time>: <command>` for each run that waits, and `heavy turn: free` when no run holds
  it
- **AND** `--json` has the turn as `heavy`: the `holder` and the `queue`, without the runs whose process is gone
  [@test] crates/bd-host/tests/cli.rs (`runs_shows_who_holds_the_heavy_turn_and_who_waits_for_how_long`, `follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-agent/src/turn_test.rs (`the_lines_of_the_heavy_turn_name_the_holder_and_each_waiting_run_with_their_time`)
