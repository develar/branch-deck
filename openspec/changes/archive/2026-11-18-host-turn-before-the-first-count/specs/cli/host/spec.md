## MODIFIED Requirements

### Requirement: follow, cancel, fetch, forget, runs: runs lists the runs that the agent keeps

`brd-host runs <alias> [--json]` MUST list the runs that the agent keeps, the finished and forgotten ones too, each
with its state, its start, its affinity, commit, command and the steps of 0.1 s or more, and for a finished run its
exit and how long it took, or the refusal that ended it, then the heavy turn of the host. An agent without the feature
`heavy` has no heavy turn to list.

#### Scenario: The runs of a host
- **WHEN** `brd-host runs <alias>` runs while a run goes on
- **THEN** it lists that run as `<run>  running, started <YYYY-MM-DD HH:MM:SS>` with its affinity, commit, command and
  the steps of 0.1 s or more
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-client/src/phases_test.rs (`a_run_line_says_its_state_its_start_and_how_long_a_finished_run_took`)

#### Scenario: The cores of a run
- **WHEN** `brd-host runs <alias>` runs after the command of a run started
- **THEN** its line says `, cores <mean> mean, <peak> peak` after its start and its time, with ` so far` while it
  runs
- **AND** `--json` has them as `cores` of `RunInfo`: `{"mean", "peak"}`; a run before its command and an older agent
  send none
  [@test] crates/host-client/src/phases_test.rs (`a_run_line_says_the_mean_and_the_peak_cores_of_its_command`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_a_measure_share_the_cores_and_a_run_without_one_waits_for_the_whole_host`)

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

#### Scenario: A run that a refusal ended
- **WHEN** `brd-host runs <alias>` runs after a refusal of the agent ended a run, such as `superseded`
- **THEN** it lists the run as `<run>  finished, refused <code>, started …`, and on the next line `    <code>: <message>`
  with the alias of the host for `<host>`
- **AND** `--json` has the refusal as `refused` of `RunInfo`: `{"code", "message"}`, the code by its name; an older
  agent sends none
  [@test] crates/host-client/src/phases_test.rs (`a_refused_run_says_its_code_and_its_message_with_the_alias`)
  [@test] crates/host-link/src/proto_test.rs (`the_end_and_the_forget_of_a_run_info_are_optional_fields_within_the_schema`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_later_heavy_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`)

#### Scenario: The heavy turn of a host
- **WHEN** `brd-host runs <alias>` runs while heavy runs hold cores of the heavy turn and another waits for it
- **THEN** after the runs it says `heavy turn: <held> of <cores> cores held` (`heavy turn: free, <cores> cores` without
  a holder; `heavy turn: <n> run(s) hold the whole host; no run of this build counted its cores yet` while only the
  holder of the build before holds it), then `  run <id> of <repository> holds <cores> for <time> (measured): <command>` for each holder, with
  `(live, above its measure)` when it uses more, and `holds the whole host for <time> (no measure)` or `(measured
  <cores>)` for one that takes the whole host; `  run <id> of <repository> waits <time> for <cores> (measured):
  <command>` or `for the whole host (no measure)` for each run that waits; and `  run <id> of <repository>, not heavy,
  uses <cores>: <command>` for each run that is not heavy whose command runs
- **AND** `--json` has the turn as `heavy`: the `cores`, the `holders`, the `queue` and the `light` runs, without the
  runs whose process is gone, and the first holder as `holder` too, which a client of the build before reads
- **AND** a reply of an agent of the build before, with one `holder` and no `cores`, says `heavy turn: run <id> of
  <repository> holds it <time>: <command>`, then `  run <id> of <repository> waits <time>: <command>`
  [@test] crates/bd-host/tests/cli.rs (`runs_shows_who_holds_the_heavy_turn_and_who_waits_for_how_long`, `follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-agent/src/turn_test.rs (`the_lines_of_the_heavy_turn_name_the_cores_of_each_holder_waiting_run_and_light_run`, `the_lines_of_a_heavy_turn_of_the_build_before_name_the_holder_and_each_waiting_run`)
