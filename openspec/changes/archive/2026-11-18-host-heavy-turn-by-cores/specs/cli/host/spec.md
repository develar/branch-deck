## RENAMED Requirements

- FROM: `### Requirement: serve: Heavy runs take the heavy turn of the host one at a time`
- TO: `### Requirement: serve: Heavy runs share the cores of the host by their measure`

## ADDED Requirements

### Requirement: serve: A run measures the cores of its command

The supervisor of every run MUST sample the CPU of its command group and of the Bazel servers of its slot every 10 s,
and at the end measure the mean (the larger of the sampled CPU and of the reaped children over the wall time) and the
peak. A command of 10 s or more MUST add its measure to the last 5 of its repository and command; a command is
expected to use their 80th percentile.

#### Scenario: The samples of a run
- **WHEN** the command of a run runs
- **THEN** each sample rewrites `cores.json` of the run and notes the live cores in the heavy turn: of a holder as its
  `live`, of a run that is not heavy in `light`
- **AND** at the end its log says `the command used <mean> cores on average and <peak> at its peak, of the <cores> of
  the host`
  [@test] crates/host-agent/src/cores_test.rs (`the_samples_count_the_cpu_of_the_command_group_and_the_end_its_measure`)
  [@test] crates/sync-utils/src/process_scan_test.rs (`the_cpu_time_of_a_process_grows_while_it_computes`)
  [@test] crates/sync-utils/src/cores_test.rs (`cores_read_and_write_with_one_decimal_and_are_the_mean_of_a_cpu_time`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_a_measure_share_the_cores_and_a_run_without_one_waits_for_the_whole_host`)

#### Scenario: The measure follows the command
- **WHEN** a run of a command ends, a cancelled one too
- **THEN** its measure joins the last 5 of the command, and the next run expects the second largest of their means
  (the largest of up to 4), so a command that got heavier is expected heavier at its next run, and one that got lighter
  frees its cores within a few runs
- **AND** a command of less than 10 s adds no measure, and a command without a measure for 30 days is forgotten
  [@test] crates/lease-pool/src/host_cores_test.rs (`the_expectation_of_a_command_follows_its_last_runs`, `a_command_without_a_measure_for_30_days_is_forgotten`)

## MODIFIED Requirements

### Requirement: serve: Heavy runs share the cores of the host by their measure

Before its lend, a heavy run MUST take its cores of the heavy turn of the host (`lease_pool::host_turn`) in the phase
`heavy`, waiting up to 2 h first come first, and give them back after its give-back; a run that is not heavy takes
none. The head of the queue, and no run after it, MUST be admitted while the cores of the holders, the live cores of
the runs that are not heavy and its own fit the cores of the host. A run without a measure MUST take the whole host.

#### Scenario: Two heavy runs and one that is not heavy
- **WHEN** a heavy run without a measure holds the turn, a second heavy run starts, and a run that is not heavy starts
- **THEN** the second waits for the turn without a slot, and its command starts only after the first gave the turn back
- **AND** the run that is not heavy runs beside the first at once
- **AND** the first gives the turn back after the give-back of its slot
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_the_host_run_one_at_a_time_and_a_run_that_is_not_heavy_goes_beside_them`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`heavy_runs_take_the_turn_one_at_a_time_first_come_first`, `a_waiting_run_takes_the_turn_once_the_holder_leaves`, `a_wait_without_the_event_takes_the_turn_at_its_next_look`, `a_wait_whose_time_is_up_answers_its_place_and_keeps_its_ticket`, `a_holder_holds_the_turn_from_the_moment_it_took_it`)

#### Scenario: Heavy runs of a measure
- **WHEN** heavy runs whose commands have a measure wait, on a host of `<cores>` cores (`branchdeck.hostCores` of the
  global git config of the host, else the cores that the system counts)
- **THEN** each in its order is admitted while the expected cores of the holders, the live cores of the runs that are
  not heavy and its own fit `<cores>`, and runs beside them
- **AND** a holder counts the larger of its expected cores and the live cores of its last sample, so a command that got
  heavier holds more at once
  [@test] crates/lease-pool/src/host_turn_test.rs (`heavy_runs_of_a_measure_share_the_cores_while_they_fit`, `the_live_use_of_a_holder_and_of_the_runs_that_are_not_heavy_count`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_a_measure_share_the_cores_and_a_run_without_one_waits_for_the_whole_host`)

#### Scenario: A run that takes the whole host
- **WHEN** a heavy run whose command has no measure, or measured `<cores>` or more, comes to the head of the queue
- **THEN** it waits until no heavy run holds cores, and no heavy run is admitted beside it
- **AND** the runs after it wait behind it, even one that would fit: none overtakes the head of the queue, so none waits
  forever
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_without_a_measure_takes_the_whole_host_and_the_head_of_the_queue_goes_first`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_a_measure_share_the_cores_and_a_run_without_one_waits_for_the_whole_host`)

#### Scenario: The holder of the build before
- **WHEN** the supervisor of a heavy run of the build before an upgrade holds `heavy.json`, the table of one holder
- **THEN** it holds the whole host for the runs of this build, which wait for it, and `brd-host runs` names it first
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_holder_of_the_build_before_holds_the_whole_host`)

#### Scenario: The line of the wait
- **WHEN** a heavy run waits for the turn
- **THEN** at once and then once a minute its log says `run <id> waits <time> for the heavy turn of the host, <need>,
  <time> left: <holders>, runs that are not heavy use <cores>, <n> runs ahead: <id> of <repository> (<command>), …; a
  wait, not a hang: do not cancel it and do not run it on the client; `brd-host runs <host>` shows the heavy turn`
- **AND** `<need>` is `<cores> of its <cores of the host> cores`, `all its <cores of the host> cores, as the command
  has no measure yet` or `all its <cores of the host> cores, as the command measured <cores>`; each holder is `run <id>
  of <repository> holds <cores> for <time> (<command>, measured)`, `(<command>, live, above its measure)`, or `holds
  the whole host for <time> (<command>, no measure)`; without a holder `no heavy run holds cores`; the part of the runs
  that are not heavy only when they use cores
- **AND** after the wait it says `took the heavy turn of the host after <n> s`
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_turn_line_names_the_wait_the_cores_of_each_holder_and_the_runs_ahead`)
  [@test] crates/host-agent/src/turn_test.rs (`a_heavy_run_says_its_wait_at_once_and_a_stop_ends_it`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_command_of_an_entry_is_one_line_of_at_most_200_characters`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`the_line_of_a_heavy_wait_comes_again_with_the_time_waited_and_the_time_left`)

#### Scenario: A cancel in the heavy queue
- **WHEN** a heavy run that waits for the turn gets a cancel
- **THEN** its wait ends within 2 s with the exit code 130 before its lend, and its ticket leaves the queue
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_cancel_in_the_heavy_queue_ends_the_run_before_its_lend_and_its_ticket_leaves`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_that_leaves_the_queue_waits_no_more`)

#### Scenario: A killed heavy run
- **WHEN** the supervisor of a heavy run that holds cores is gone, or the process of a ticket ahead
- **THEN** the next heavy run is admitted at its next look, within 5 s, without a keeper
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_killed_heavy_run_frees_the_turn_for_the_next_one`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_gone_holder_frees_the_turn_and_a_gone_ticket_leaves_the_queue`)

#### Scenario: No turn within 2 h
- **WHEN** a heavy run gets no turn within 2 h
- **THEN** it ends with `heavy_timeout`, the line of its wait and the advice to run it on the host again later, not on
  the client, and its ticket leaves the queue
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_heavy_run_that_waits_past_its_time_ends_with_heavy_timeout`)

#### Scenario: A table of another schema
- **WHEN** `turn.json` has a schema other than the one of this build, or none, as an agent of another build wrote it
- **THEN** each take, leave and read of the turn refuses, and the run ends with `agent_error`, never with a free turn
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_table_of_another_schema_refuses`)

### Requirement: run: A heavy run is heavy by --heavy or by heavy of [host] of its commit

A run MUST be heavy with `--heavy`, else when a pattern of `heavy` of `[host]` of the shipped commit matches its
command, and its `Start` MUST carry `heavy`. Its line MUST say why it is heavy. An agent without the feature `heavy`
MUST get no run of a commit whose `[host]` has `heavy`: the client refuses before the start.

#### Scenario: A heavy run
- **WHEN** a run starts with `--heavy`, or the first pattern of `heavy` that matches its command is `<pattern>`
- **THEN** its `Start` has `"heavy": true`, and after the line of the run comes `brd-host: a heavy run by --heavy: on
  <alias> it waits until the cores that its command used before are free beside the other heavy runs, or the whole
  host without a measure`, or `by `<pattern>` of [host]`
- **AND** a run whose command no pattern matches and that has no `--heavy` is no heavy run, and its `Start` has no
  `heavy`
  [@test] crates/host-client/src/run_test.rs (`a_run_is_heavy_by_a_pattern_of_its_commit_or_by_heavy_and_says_why`)
  [@test] crates/bd-host/src/cli_test.rs (`the_command_line_of_bt_gives_niceness_0_and_no_outputs`)
  [@test] crates/host-link/src/proto_test.rs (`a_heavy_run_and_the_heavy_turn_are_optional_fields_within_the_schema`)

#### Scenario: An agent without the feature heavy
- **WHEN** a `--heavy` run of a commit without `heavy` in `[host]` starts on an agent whose hello has no feature `heavy`
- **THEN** the client says `brd-host: the agent on <alias> predates heavy runs, so this one runs beside the others:
  `brd-host upgrade <alias>` sends this build`, and the run goes on as one that is not heavy
  [@test] crates/host-client/src/run_test.rs (`a_run_is_heavy_by_a_pattern_of_its_commit_or_by_heavy_and_says_why`)

#### Scenario: A commit with heavy and an agent without the feature
- **WHEN** a run, heavy or not, of a commit whose `[host]` has `heavy`, even empty, starts on an agent whose hello has no
  feature `heavy`, which would refuse every run of that commit
- **THEN** the client refuses before `Start` with `bad_config` (exit 75): `the agent on <alias> predates `heavy` of
  [host] of .branchdeck.toml, so it refuses every run of this commit: `brd-host upgrade <alias>` sends this build`
  [@test] crates/host-client/src/run_test.rs (`a_commit_with_heavy_on_an_agent_before_heavy_runs_refuses_before_the_start`)
  [@test] crates/host-link/src/config_test.rs (`the_known_keys_are_the_fields_of_serde_and_heavy_needs_its_feature`)

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
  a holder), then `  run <id> of <repository> holds <cores> for <time> (measured): <command>` for each holder, with
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

### Requirement: serve: A later heavy run of the same checkout and command supersedes the waiting one

A heavy run new to the queue MUST supersede each waiting run of its repository with the same affinity and the same
command by what it runs (`host_link::heavy::run_key`: without `nice`, `timeout`, `time`, `nohup`, `exec` or `command`
before it, with its environment): it takes the place of the first, and each ends with `superseded` before its lend.
A holder of the turn MUST never be superseded. The later run MUST say whom it replaced and which other runs of its
checkout wait.

#### Scenario: The same checkout and command
- **WHEN** a heavy run waits for the turn, and a later heavy run of the same repository, affinity and command starts
- **THEN** the later run takes the place of the waiting one in the queue, and its log says `replaces run <id>, which
  waited <time> for the heavy turn with the same checkout and command: it ends with `superseded` and runs nothing, and
  this run takes its place in the queue`
- **AND** the waiting run ends at its next look with `superseded` (exit 75), in the phase `heavy` only, with the message
  `run <id> was superseded by run <later id> of the same checkout and command before it started: nothing ran; do not
  run it again, here or on the client; follow <later id>: `brd-host follow <host> <later id>``, which its client and
  `brd-host follow` say with the alias, and `brd-host runs` lists it as `finished, refused superseded`
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_later_heavy_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_later_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`)
  [@test] crates/host-agent/src/turn_test.rs (`a_later_run_says_whom_it_replaces_and_who_else_of_its_checkout_waits`)
  [@test] crates/host-link/src/heavy_test.rs (`the_key_of_a_run_drops_the_wrappers_and_keeps_the_environment`)

#### Scenario: Another command or another checkout
- **WHEN** a later heavy run starts with another command of the same checkout, or the same command of another checkout
- **THEN** both wait, each in its place
- **AND** a later run of the same checkout with another command says `run <id> of this checkout also waits for the heavy
  turn, <time>: <command>. Both run, one after the other; if this run replaces it, cancel it: `brd-host cancel <host>
  <id>``
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_later_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`, `another_command_another_checkout_and_the_holder_are_never_superseded`)
  [@test] crates/host-agent/src/turn_test.rs (`a_later_run_says_whom_it_replaces_and_who_else_of_its_checkout_waits`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_later_heavy_run_of_the_same_checkout_and_command_supersedes_the_waiting_one`)

#### Scenario: A run that started
- **WHEN** a later heavy run of the same checkout and command starts while the run that holds the turn runs
- **THEN** the holder runs on, and the later run waits for it; an entry without an affinity and a key, of an older
  build, is never superseded
  [@test] crates/lease-pool/src/host_turn_test.rs (`another_command_another_checkout_and_the_holder_are_never_superseded`)

### Requirement: The files: Each end keeps its files at the paths of the table

brd-host MUST keep the files of the client, of the agent and of a checkout at the paths of the table below, each with
the content that the table names.

#### Scenario: The files of each end
- **WHEN** brd-host writes a file of its own
- **THEN** the file is at its path of the table, with its content

| End | File | Content |
|---|---|---|
| Client | `~/.config/brd-host/identity.crt`, `identity.key` | The certificate and the key of the client, made at the first call, mode 0600 |
| Client | `~/.config/brd-host/hosts/<alias>.json` | `{"alias", "fingerprint", "port", "hostname", "addresses", "repo"}` |
| Client | `~/.config/brd-host/bases/<alias>/<affinity>` | The commit of the base of this affinity on that host after the last base sync |
| Agent | `~/.config/brd-host/agent/identity.crt`, `identity.key` | The certificate and the key of the agent |
| Agent | `~/.config/brd-host/agent/clients` | The allow list: one `<64 hex> <name>` line per client |
| Agent | `~/.cache/brd-host/runs/<run>/` | `spec.json`, `events.log`, `exit.json`, `supervisor.log`, `outputs/`, `changes.patch` |
| Agent | `~/.cache/brd-host/` | `repos`, `login-env.json`, `listening.json`, `upgrade.json`, `upgrade.lock`, `slots/<lease id>.prepared`, `heavy/turn.json` and `heavy/turn.lock` (the heavy turn of the host, in a directory of its own), `heavy/cores.json` and `heavy/cores.lock` (the measures of the commands), `heavy/heavy.json` (the heavy turn of the build before, which this build only reads), `runs/<run>/cores.json` (the cores of the command of a run) |
| Agent | `~/Library/LaunchAgents/com.jetbrains.brd-host.plist`, `~/Library/Logs/brd-host.log` | The LaunchAgent and its log |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.json`, `<run>.patch` | The run record and the patch of `--apply` |
