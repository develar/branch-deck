## ADDED Requirements

### Requirement: run: --measure-as gives a run the measures of a class

`--measure-as <class>` MUST make a run heavy whose measures the host reads and keeps by the class, not by the command,
so a command whose text changes every run gets a measure. Its `Start` MUST carry `heavy` and `measure_as`. An agent
without the feature `measure_as` would measure the run by its command, so the client MUST refuse before the start.

#### Scenario: A run of a class
- **WHEN** a run starts with `--measure-as <class>`
- **THEN** its `Start` has `"heavy": true` and `"measure_as": "<class>"`, and after the line of the run comes `brd-host: a
  heavy run by --measure-as: on <alias> it waits until the cores that the runs of the class <class> used before are free
  beside the other heavy runs, or the whole host without a measure`
- **AND** a run without it has no `measure_as`, and its measures are those of its command
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_class_is_heavy_and_an_agent_before_it_refuses_before_the_start`)
  [@test] crates/bd-host/src/cli_test.rs (`measure_as_names_the_class_of_the_measures`)
  [@test] crates/host-link/src/proto_test.rs (`the_class_of_the_measures_is_an_optional_field_of_a_start_and_an_estimate`)

#### Scenario: The key of the measures of a class
- **WHEN** the host keeps or reads the measures of a run of a class
- **THEN** their key is the SHA-256 of the words `<measure-as>` and `<class>` (`host_link::heavy::measure_key`), whatever
  the command, its environment and its fetch directories, so no command has the key of a class
- **AND** the admission, the record of a measure, the estimate, the queue record and the history of the run all take
  that key
  [@test] crates/host-link/src/heavy_test.rs (`the_measure_key_of_a_class_is_of_the_class_alone`)

#### Scenario: An agent without the feature measure_as
- **WHEN** a run of `--measure-as` starts on an agent whose hello has no feature `measure_as`
- **THEN** the client refuses before `Start` with `bad_config` (exit 75): `the agent on <alias> predates --measure-as,
  so it would measure this run by its command and give it the whole host every time: `brd-host upgrade <alias>` sends
  this build`
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_class_is_heavy_and_an_agent_before_it_refuses_before_the_start`)

#### Scenario: A class that no name may be
- **WHEN** `--measure-as` names a class of other than 1 to 128 letters, digits, `.`, `_` and `-`, the first a letter or
  a digit, such as `gate/2`
- **THEN** the client refuses with `usage` (exit 2) before it asks the host anything, and an agent refuses such a start
  or estimate with `bad_request`
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_class_is_heavy_and_an_agent_before_it_refuses_before_the_start`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`runs_of_a_class_share_its_measure_whatever_their_command`)

## MODIFIED Requirements

### Requirement: run: The command line of run

`brd-host run` MUST take the alias of a host or the name of a set of hosts, the options of its synopsis and, after `--`, the command with its
arguments. `--fetch` MUST be repeatable, `--nice` MUST take 0 to 19, and `--max` a duration as `brd-host wait` takes
it.

#### Scenario: The synopsis of run
- **WHEN** a caller runs `brd-host run`
- **THEN** it takes this command line

```sh
brd-host run <alias|set> [--head] [--apply] [--fetch <dir>]... [--no-outputs] [--heavy] [--exclusive]
                  [--measure-as <class>] [--nice <0-19>] [--repo <path>] [--affinity <key>] [--max <duration>]
                  [--report-fd <n>] -- <command> <args>
```

### Requirement: run: A heavy run goes to a member only with its measure there

The first member of a set MUST be eligible for every run. For a heavy run, another member MUST be eligible only with its
own measure of the command, or of its class with `--measure-as`; for a run that is not heavy, every member that
answered. A member whose agent lacks the feature `measure_as` MUST have no estimate of a run of a class.

#### Scenario: A new heavy command
- **WHEN** a heavy run goes to a set whose other members have no measure of its command
- **THEN** it runs on the first member, and the line of the plan says that a heavy run does not go to the others
- **AND** an exclusive run to a set refuses with `usage` (2)
  [@test] crates/host-client/src/choose_test.rs (`a_heavy_run_without_a_measure_stays_on_the_first_member_and_an_exclusive_one_refuses`)

#### Scenario: A run of a class to a set
- **WHEN** a run of `--measure-as <class>` goes to a set
- **THEN** each member is asked by the class, and its measure of the class decides
- **AND** a member whose agent lacks the feature `measure_as` gets no request and has no estimate: `no estimate: its
  agent predates --measure-as, so it knows no measure of the class (`brd-host upgrade <alias>`)`
  [@test] crates/host-client/src/choose_test.rs (`a_run_of_a_class_asks_by_its_class_and_a_member_before_classes_has_no_estimate`)

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
  <command>` or `for the whole host (no measure; a command whose text changes every run never gets one:
  `--measure-as <class>` gives it one)`, of a run of a class `for the whole host (no measure of the class <class>
  yet)`, for each run that waits; and `  run <id> of <repository>, not heavy,
  uses <cores>: <command>` for each run that is not heavy whose command runs
- **AND** `--json` has the turn as `heavy`: the `cores`, the `holders`, the `queue` and the `light` runs, without the
  runs whose process is gone, and the first holder as `holder` too, which a client of the build before reads
- **AND** a reply of an agent of the build before, with one `holder` and no `cores`, says `heavy turn: run <id> of
  <repository> holds it <time>: <command>`, then `  run <id> of <repository> waits <time>: <command>`
  [@test] crates/bd-host/tests/cli.rs (`runs_shows_who_holds_the_heavy_turn_and_who_waits_for_how_long`, `follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-agent/src/turn_test.rs (`the_lines_of_the_heavy_turn_name_the_cores_of_each_holder_waiting_run_and_light_run`, `the_lines_of_a_heavy_turn_of_the_build_before_name_the_holder_and_each_waiting_run`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_of_a_class_names_its_class_on_its_entry`)

### Requirement: stats: The summary splits the waits from the work and groups the commands

The summary MUST give the waits (`heavy`, `exclusive`, `git`, `slot`, `pack_wait`) against the work by phase, the heavy
queue, the outcomes by exit and refusal, the runs that ended before their command, the cores, per command the runs,
the failures and the p50 and p90 of its wait and its command for the 30 that waited longest and the rest in one
entry, and the bytes by day.

#### Scenario: A day of runs
- **WHEN** the agent sums the runs of a window
- **THEN** the summary has each of these, a timeout or a cancel counts as no failure of a command, and only the 30
  commands that waited longest come one by one
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_splits_the_waits_from_the_work_and_groups_the_commands`, `the_commands_past_the_thirty_that_waited_longest_are_one_entry`, `a_day_is_its_utc_date`)

#### Scenario: The runs of a class
- **WHEN** the summary counts runs of `--measure-as <class>` with other command lines
- **THEN** they are one command, with the class as `measure_as` and the command line of the last run, and its line ends
  with `: --measure-as <class>` instead of the command line
  [@test] crates/lease-pool/src/host_history_test.rs (`the_runs_of_a_class_are_one_command_named_by_the_class`)

### Requirement: serve: An estimate says when a run would start there

`Request::Estimate` (feature `estimate`) MUST answer `has_commit`, `warm`, `cores`, `expected_cores`, `expected_wall_ms`,
`start_in_ms`, `unknown_ahead` and `os`, read without a lock. A heavy run MUST take its place in the queue as a new run
would, and the holders and the runs before it MUST end after the walls of their commands, admitted by the rule of the
turn, the load outside the heavy runs the most of the last 30 s.

#### Scenario: The shapes of an estimate
- **WHEN** a client asks an agent for an estimate
- **THEN** the request and the reply are `{"request": "estimate", "repo", "affinity", "commit", "command", "fetch",
  "heavy", "measure_as"}` and `{"reply": "estimate", …}`, `measure_as` left out without a class (feature
  `measure_as`) and the fields of a measure without one; the measures of a class count for a run of it
  [@test] crates/host-link/src/proto_test.rs (`an_estimate_and_its_reply_have_a_tagged_json_shape`, `the_class_of_the_measures_is_an_optional_field_of_a_start_and_an_estimate`)

#### Scenario: The queue played forward
- **WHEN** a heavy run would wait for holders and for runs of the queue
- **THEN** its start is when the runs before it end by the walls of their commands, the ones that fit starting together
- **AND** a run of an unknown wall, and an exclusive run that a busy host keeps out, count in `unknown_ahead`, and a run
  of the whole host on a busy free turn waits for the bound of 15 min
  [@test] crates/lease-pool/src/host_estimate_test.rs (`an_estimate_plays_the_queue_forward_by_the_walls_of_the_commands`, `an_exclusive_run_ahead_on_a_busy_host_counts_apart`)

#### Scenario: A short run in the estimate
- **WHEN** the run is short by the walls of its passed runs and its measure
- **THEN** the estimate places it ahead of the long runs that it may pass, by the place function of the queue
- **AND** the wall of a command is the one of its passed runs, else of its measures, the failed and cancelled ones too
  [@test] crates/lease-pool/src/host_estimate_test.rs (`a_short_run_is_placed_ahead_of_the_long_runs_as_the_queue_places_it`)
  [@test] crates/lease-pool/src/host_cores_test.rs (`the_wall_of_a_command_is_of_its_last_passed_runs_over_three_at_least`)

### Requirement: serve: Heavy runs share the cores of the host by their measure

Before its lend, a heavy run MUST take its cores of the heavy turn in the phase `heavy`, waiting up to 2 h, and give
them back after its give-back. Only the head of the queue MUST be admitted, or a run past an exclusive head: while it
fits beside the holders and the load of the host, or on a free turn of a quiet host or after 15 min. A new run MUST
join at the end, a short one ahead of the runs it may pass. A run without a measure MUST take the whole host; a held
lock MUST cost a look.

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

#### Scenario: Runs of a class
- **WHEN** a run of `--measure-as <class>` ended with a measure, and later runs of the class with other commands wait
- **THEN** they are admitted by the measure of the class, beside each other while they fit
- **AND** a run of a class without a measure takes the whole host, says `all its <cores of the host> cores, as the class
  <class> has no measure yet` in the line of its wait, and its entry of the turn names its class (`measure_as`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`runs_of_a_class_share_its_measure_whatever_their_command`)
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_turn_line_names_the_wait_the_cores_of_each_holder_and_the_runs_ahead`)

#### Scenario: A run that takes the whole host
- **WHEN** a heavy run whose command has no measure, or measured `<cores>` or more, comes to the head of the queue
- **THEN** it waits until no heavy run holds cores, and no heavy run is admitted beside it
- **AND** the runs after it wait behind it, even one that would fit: no run of the queue overtakes its head; only a short
  run new to the queue goes ahead of it, at most 4 times, so none waits forever
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_without_a_measure_takes_the_whole_host_and_the_head_of_the_queue_goes_first`, `a_run_without_a_measure_takes_a_busy_free_turn_after_the_load_bound_ahead_of_later_smaller_runs`, `a_run_is_passed_at_most_four_times_and_each_class_keeps_its_order`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_a_measure_share_the_cores_and_a_run_without_one_waits_for_the_whole_host`)

#### Scenario: A short run
- **WHEN** a heavy run joins the queue that is not exclusive, whose command has a measure below `<cores>`, and whose
  last passed runs took at most 2 min: the 80th percentile of the walls of the last 5 runs of the command whose exit
  was 0, of 3 at least (`walls.json` of the heavy turn)
- **THEN** it joins before the first entry from which every entry to the end may be passed: a run that is not short,
  that short runs passed fewer than 4 times, and no exclusive head whose drain began; else at the end; and it is
  admitted as the head of the queue is, beside the holders while it fits
- **AND** each run that it went ahead of counts a pass in `passes.json` of the heavy turn, which an older build does
  not rewrite; a run that supersedes a waiting one takes its passes; the short runs keep their order among themselves,
  and so do the others; the join leaves the holders as they are
- **AND** it counts the earliest join of the runs it went ahead of for its 15 min in the queue, so a head that it
  passed on a busy free turn waits no new bound
- **AND** a failed or cancelled run adds no wall; a run without a measure, one that measured `<cores>` or more, and an
  exclusive run are never short; a short run behind an exclusive head whose drain ended starts past it as before
- **AND** its log says `a short run, ~<time> by its last passed runs: goes ahead of <n> run(s) that wait for the heavy
  turn, each passed at most 4 times: run <id> of <repository> (passed <n> time(s)), …`, and `brd-host runs` says
  `run <id> of <repository> waits <time> for <cores> (measured, short: ~<time>)` of it and `, passed by <n> short
  run(s) (at most 4)` of a run that it passed
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_short_run_goes_ahead_of_the_runs_that_wait_for_more_and_runs_beside_the_holder`, `a_run_is_passed_at_most_four_times_and_each_class_keeps_its_order`, `the_passes_of_a_superseded_run_go_to_the_run_that_takes_its_place`, `a_run_without_a_measure_an_exclusive_one_or_one_without_three_passed_walls_is_not_short`, `a_short_run_takes_the_earliest_join_of_the_runs_it_passed_for_the_load_bound`, `a_table_that_an_older_build_rewrote_keeps_the_passes`, `a_short_run_waits_behind_an_exclusive_head_that_drains_and_passes_it_as_before_once_the_drain_ended`)
  [@test] crates/lease-pool/src/host_cores_test.rs (`the_wall_of_a_command_is_of_its_last_passed_runs_over_three_at_least`)
  [@test] crates/host-agent/src/turn_test.rs (`a_short_run_says_whom_it_went_ahead_of_and_the_lines_mark_both`, `the_lines_of_the_heavy_turn_mark_a_short_run_and_the_runs_it_passed`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_short_heavy_run_goes_ahead_of_a_run_that_waits_for_the_whole_host`)

#### Scenario: The busy cores of the host
- **WHEN** the host is busy with cores that no holder uses: runs that are not heavy, processes of no run
- **THEN** an admission counts the larger of the live cores of the runs that are not heavy and the busy cores of the
  host by the last sample of the agent (every 2 s, `host.json` of the heavy turn), less the live cores of the holders;
  a sample older than 10 s counts as none
- **AND** a run that takes the whole host is admitted only on a quiet host, busy with at most a fifth of its cores (at
  least 1) outside the heavy runs; a run of a measure on a free turn while it fits beside that load, or on a quiet host
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_busy_cores_of_the_host_outside_the_holders_hold_a_heavy_run_back`)

#### Scenario: A busy free turn
- **WHEN** the head of the queue waits on a free turn for a quiet host, or for room beside the busy cores of the host
  outside the heavy runs, which the runs that are not heavy keep busy, as they wait for no heavy run
- **THEN** once it has waited 15 min since it joined the queue, it is admitted whatever that load, a run without a
  measure too, and takes what it needs, the whole host or its measure
- **AND** the later runs, smaller ones that would fit beside the load too, wait behind it as before
- **AND** an exclusive run, a measurement, still waits for its quiet host and its quiet window
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_without_a_measure_takes_a_busy_free_turn_after_the_load_bound_ahead_of_later_smaller_runs`)

#### Scenario: A lock of the table that another process holds
- **WHEN** a look of a heavy run that waits finds `turn.lock` held by another process for 2 s
- **THEN** the look ends without an answer and the next look comes as before: the run keeps its place in the queue, and
  a run that has no place yet asks for one again; the wait goes on up to its 2 h, and a cancel ends it as before
- **AND** at once, and once a minute while the lock stays held, its log says `the heavy turn of the host is locked by
  <holder> for <n> s (<path of the lock>); this run keeps its place in the queue and looks again`, or `this run asks
  again for its place in the queue`; `<holder>` is the holder of the record of the lock, else `another process of the
  host`
- **AND** a process that is gone holds no lock: the system frees it with the process
- **AND** a look of a run behind the head of the queue reads the table without the lock, so it waits for no lock; the
  head of the queue, and a run whose expectation changed, take it
  [@test] crates/host-agent/src/turn_test.rs (`a_held_lock_of_the_table_retries_and_the_run_keeps_its_place_in_the_queue`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_look_behind_the_head_of_the_queue_takes_no_lock`)
  [@test] crates/sync-utils/src/host_cpu_test.rs (`the_busy_cores_are_the_busy_part_of_the_ticks_between_two_readings`, `the_ticks_of_proc_stat_count_the_work_as_busy_and_idle_and_iowait_as_not`, `the_ticks_of_this_host_grow`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`serve_samples_the_busy_cores_of_the_host_for_the_heavy_turn`)

#### Scenario: The holder of the build before
- **WHEN** the supervisor of a heavy run of the build before an upgrade holds `heavy.json`, the table of one holder
- **THEN** it holds the whole host for the runs of this build, which wait for it, and `brd-host runs` names it first
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_holder_of_the_build_before_holds_the_whole_host`)

#### Scenario: The line of the wait
- **WHEN** a heavy run waits for the turn
- **THEN** at once and then once a minute its log says `run <id> waits <time> for the heavy turn of the host, <need>,
  <time> left: <holders>, the host is busy with <cores> of <cores of the host> cores outside the heavy runs, <n> runs
  ahead: <id> of <repository> (<command>), …; a wait, not a hang: do not cancel it and do not run it on the client;
  `brd-host runs <host>` shows the heavy turn`
- **AND** `<need>` is `<cores> of its <cores of the host> cores`, `all its <cores of the host> cores, as the command
  has no measure yet (a command whose text changes every run never gets one: `--measure-as <class>` gives it one)`,
  of a run of a class `all its <cores of the host> cores, as the class <class> has no measure yet`, or `all its <cores
  of the host> cores, as the command measured <cores>`; each holder is `run <id>
  of <repository> holds <cores> for <time> (<command>, measured)`, `(<command>, live, above its measure)`, or `holds
  the whole host for <time> (<command>, no measure)`; without a holder `no heavy run holds cores`; without a fresh
  sample of the host, `runs that are not heavy use <cores>` instead of the busy cores, and only when they use cores
- **AND** after the wait it says `took the heavy turn of the host after <n> s`, and with a fresh sample of the host `,
  the host busy with <cores> of its <cores of the host> cores`, the sample that its admission read
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
