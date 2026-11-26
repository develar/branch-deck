## ADDED Requirements

### Requirement: run: --group names the group of a run in the heavy queue

`--group <group>`, else `BRD_HOST_GROUP` when it is not empty, MUST name the group of a run in the heavy queue, by
default its affinity, and its `Start` and `Estimate` MUST carry it as `group`. An agent without the feature `group`
MUST get no `group`: it queues the run first come first, so the client does not refuse, and a heavy run says so.

#### Scenario: A run of a group
- **WHEN** a run starts with `--group <group>`, or with `BRD_HOST_GROUP=<group>` set once by an orchestrator for all its
  agents
- **THEN** its `Start` and its `Estimate` have `"group": "<group>"`, the flag over the variable
- **AND** a run without either, or with an empty variable, has no `group`, and its group is its affinity
  [@test] crates/bd-host/src/cli_test.rs (`group_names_the_group_of_the_run_by_the_flag_or_the_environment`)
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_group_names_it_in_the_start_and_an_agent_before_groups_queues_it_first_come_first`)
  [@test] crates/host-client/src/choose_test.rs (`the_estimate_of_a_run_of_a_group_names_its_group`)
  [@test] crates/host-link/src/proto_test.rs (`the_group_of_a_run_is_an_optional_field_of_a_start_and_an_estimate`)

#### Scenario: An agent without the feature group
- **WHEN** a run of a group starts on an agent whose hello has no feature `group`
- **THEN** its `Start` has no `group`, the run goes on, and a heavy one says `brd-host: the agent on <alias> predates
  the groups of the heavy queue, so this run of the group <group> waits first come first: `brd-host upgrade <alias>`
  sends this build`
- **AND** an older agent that gets `group` in an estimate ignores it and estimates the end of its queue, where it
  queues the run
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_group_names_it_in_the_start_and_an_agent_before_groups_queues_it_first_come_first`)

#### Scenario: A group that no name may be
- **WHEN** `--group` or `BRD_HOST_GROUP` names a group of other than 1 to 128 letters, digits, `.`, `_` and `-`, the
  first a letter or a digit, such as `a b`
- **THEN** the client refuses with `usage` (exit 2) before it asks the host anything, and an agent refuses such a start
  or estimate with `bad_request`
  [@test] crates/host-client/src/run_test.rs (`a_run_of_a_group_names_it_in_the_start_and_an_agent_before_groups_queues_it_first_come_first`)
  [@test] crates/host-link/src/heavy_test.rs (`a_group_is_named_by_the_rule_of_a_host_ref_name`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`the_heavy_queue_takes_turns_between_the_groups_of_its_runs`)

### Requirement: serve: The heavy queue takes turns between the groups of its runs

A new heavy run MUST join behind the last entry it may not pass: the head, a run of its group, a short run, a run
that the turns passed 8 times, and a run with no more runs of its group before it than its own group has waiting; so
first come first inside a group and exactly first come first with one waiting run per group. The turns MUST keep
none of the short lane's state.

#### Scenario: A group of many runs and a later run of another group
- **GIVEN** a group of many heavy runs from checkouts of their own, such as the agents of one orchestrating session
- **WHEN** a heavy run of another group, or of a checkout without a group, joins the queue later
- **THEN** it waits behind the first waiting run of each group before it, not behind all of them, and the runs of each
  group keep their order
- **AND** the head of the queue alone is admitted, as before
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_queue_takes_turns_between_groups_and_each_group_keeps_its_order`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`the_heavy_queue_takes_turns_between_the_groups_of_its_runs`)

#### Scenario: Runs without a group
- **WHEN** heavy runs of other checkouts join without `--group`, each checkout with at most one run waiting, also a
  checkout whose run just took the turn and that joins again
- **THEN** each is of the group of its checkout, and they wait first come first exactly as before the groups
- **AND** a second waiting run of one checkout waits behind the first waiting runs of the other checkouts
  [@test] crates/lease-pool/src/host_turn_test.rs (`without_groups_the_runs_of_checkouts_with_one_run_waiting_each_are_first_come_first`)

#### Scenario: How often a run is passed
- **WHEN** runs of other groups join while a run waits, any count of groups, while the head can start or not
- **THEN** a run is passed by the turns at most 8 times, and only by a run of a group with fewer runs waiting than are
  before it in its own group; the first waiting run of a group is passed by none
- **AND** the count is in `groups.json` of the heavy turn, and a run that leaves the queue without the turn, a cancel
  or a start of `admit_now` refused `busy`, gives its passes back
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_is_passed_by_the_turns_at_most_eight_times_whatever_the_count_of_groups`)

#### Scenario: The short lane, the head, the supersede and the starts at once
- **WHEN** a short run, an exclusive head, a run that supersedes or a start of `admit_now` meets the turns of the groups
- **THEN** a run of the turns adds no pass of the short lane (`passes.json`), sets no join of its own from another run
  and counts its load bound from its own join; a short run still joins ahead of the runs it may pass, a backlog that
  the turns passed too, and no run of the turns goes ahead of a short run or of the head
- **AND** a run that supersedes a run of its group takes its place and its passes; one of another group takes its own
  turn, and the passes of the run it replaces go back; an exclusive run, its drain and its pass, and a start of
  `admit_now` keep their rules
  [@test] crates/lease-pool/src/host_turn_test.rs (`the_short_lane_the_head_and_the_supersede_keep_their_rules_beside_the_turns`, `a_run_that_supersedes_a_run_of_another_group_takes_its_own_turn`, `a_short_run_goes_ahead_of_the_runs_that_wait_for_more_and_runs_beside_the_holder`, `a_heavy_run_that_fits_starts_past_an_exclusive_run_whose_drain_ended`)
  [@test] crates/host-agent/tests/serve/admit.rs (`of_two_starts_that_admit_at_once_on_a_free_host_one_runs_and_the_other_is_busy`)

#### Scenario: A table that an older build rewrote
- **WHEN** a supervisor of an older build rewrites `turn.json` without `group`, and `passes.json` without the fields it
  does not know, as builds 1.24 to 1.27 do
- **THEN** no turn changes, as the groups and their passes are in `groups.json`, which no older build reads or writes,
  and only the lines lose the group
- **AND** a run that `groups.json` does not name is of the group of its entry, its `--group`, else its affinity, else
  alone: the queue goes on and no run is lost
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_rewrite_by_an_older_build_changes_no_turn_and_without_the_groups_a_run_is_of_its_checkout`)

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
                  [--measure-as <class>] [--group <group>] [--nice <0-19>] [--repo <path>] [--affinity <key>]
                  [--max <duration>] [--report-fd <n>] -- <command> <args>
```

### Requirement: serve: Heavy runs share the cores of the host by their measure

Before its lend, a heavy run MUST take its cores of the heavy turn in the phase `heavy`, up to 2 h, and free them
after its give-back. Only the head of the queue MUST be admitted, or a run past an exclusive head: while it
fits beside the holders and the load of the host, or on a free turn of a quiet host or after 15 min. A new run MUST
join by its group's turn, a short one ahead of those it may pass. A holder MUST count its measure, else the whole host, for 10 min;
a held lock MUST cost a look.

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
- **AND** a holder in its first 10 min counts the larger of its expected cores and the live cores of its last sample,
  so a command that got heavier holds more at once
  [@test] crates/lease-pool/src/host_turn_test.rs (`heavy_runs_of_a_measure_share_the_cores_while_they_fit`, `the_live_use_of_a_holder_and_of_the_runs_that_are_not_heavy_count`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_a_measure_share_the_cores_and_a_run_without_one_waits_for_the_whole_host`)

#### Scenario: A holder past its grace
- **WHEN** a heavy run that is not exclusive holds the turn, 10 min passed since its admission, and its supervisor
  noted its live cores at each sample of the last 5 min, none more than 1 min after the one before (`recent` of its
  entry, the most of each half minute)
- **THEN** it counts the most of those samples, and of its last sample, instead of its expected cores, or of the whole
  host without a measure or with one of `<cores>` or more, and the head of the queue is admitted beside it while it
  fits
- **AND** a burst counts at its next sample: what it holds rises with it; a holder quiet for longer than 5 min that
  bursts then runs over the runs admitted beside it, the risk taken
- **AND** before those 10 min, while its samples span less than 5 min, when its last sample is more than 1 min old, or
  when the most of them leaves less than a fifth of the host beside it, it counts as before; past those 10 min never
  less than the most of its samples of the last 5 min, its last sample too, so a holder of a measure whose peak passes
  its measure counts that peak
- **AND** a supervisor of an older build that rewrites `turn.json` drops `recent`: the holder counts as before until a
  new window spans 5 min
- **AND** a run of the queue without a measure still waits for every holder, and the measure at the end of the run is
  as before
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_holder_without_a_measure_holds_the_whole_host_in_its_grace_and_then_its_use_of_the_last_5_min`, `a_holder_of_a_measure_counts_its_measure_in_its_grace_and_then_its_use_of_the_last_5_min`, `a_holder_without_a_measure_holds_the_whole_host_still_without_a_window_that_leaves_room`, `a_holder_of_a_measure_past_its_grace_counts_at_least_the_most_of_its_last_5_min`, `the_window_of_the_live_use_keeps_the_most_of_each_half_minute_of_the_last_5_min`)

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
  of <repository> holds <cores> for <time> (<command>, measured)`, `(<command>, live, above its measure)`, `holds
  the whole host for <time> (<command>, no measure)`, or past its grace `(<command>, no measure: it held the whole host
  for its first 10 min, now its live use of the last 5 min)` and `(<command>, measured <cores>: it held that for its
  first 10 min, now its live use of the last 5 min)`; without a holder `no heavy run holds cores`; without a fresh
  sample of the host, `runs that are not heavy use <cores>` instead of the busy cores, and only when they use cores
- **AND** a run of `--group` is `run <id> in the group <group> waits …`, and a holder or a run ahead of one `<id> of
  <repository> in the group <group>`
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
  `(live, above its measure)` when it uses more, `holds the whole host for <time> (no measure)` or `(measured
  <cores>)` for one that takes the whole host, and `holds <cores> for <time> (no measure: it held the whole host for its
  first 10 min, now its live use of the last 5 min)` for one without a measure past its grace, `(measured <cores>: it
  held that for its first 10 min, now its live use of the last 5 min)` for one of a measure; `  run <id> of <repository> waits <time> for <cores> (measured):
  <command>` or `for the whole host (no measure; a command whose text changes every run never gets one:
  `--measure-as <class>` gives it one)`, of a run of a class `for the whole host (no measure of the class <class>
  yet)`, for each run that waits; and `  run <id> of <repository>, not heavy,
  uses <cores>: <command>` for each run that is not heavy whose command runs; a holder or a waiting run of `--group`
  is `run <id> of <repository> in the group <group> …`
- **AND** `--json` has the turn as `heavy`: the `cores`, the `holders`, the `queue` and the `light` runs, without the
  runs whose process is gone, and the first holder as `holder` too, which a client of the build before reads; an entry
  of a run of `--group` names it as `group`, an entry without one is of the group of its `affinity`
- **AND** a reply of an agent of the build before, with one `holder` and no `cores`, says `heavy turn: run <id> of
  <repository> holds it <time>: <command>`, then `  run <id> of <repository> waits <time>: <command>`
  [@test] crates/bd-host/tests/cli.rs (`runs_shows_who_holds_the_heavy_turn_and_who_waits_for_how_long`, `follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-agent/src/turn_test.rs (`the_lines_of_the_heavy_turn_name_the_cores_of_each_holder_waiting_run_and_light_run`, `the_lines_of_a_heavy_turn_of_the_build_before_name_the_holder_and_each_waiting_run`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_of_a_class_names_its_class_on_its_entry`, `the_queue_takes_turns_between_groups_and_each_group_keeps_its_order`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`the_heavy_queue_takes_turns_between_the_groups_of_its_runs`)

### Requirement: stats: The agent keeps a line of each ended run for 90 days

`serve` MUST append the line of an ended run to `history/<YYYY-MM>.jsonl` of the month of its end as its run
directory goes, by its age or past the kept forgotten runs, read before the removal and written after it, once per
run; the hourly cleanup MUST remove a month whose every line is more than 90 days old.

#### Scenario: A run directory goes
- **WHEN** the run directory of an ended run goes, a day after its end or past the kept forgotten runs
- **THEN** its line is in the month of its end: the run, the repository, the affinity, the commit, the command in one
  line of at most 200 characters, the key of its measures, its group of `--group`, `heavy`, `exclusive`, its start and end, its exit and
  refusal, `lost`, its phases, its cores, its bytes and its queue
- **AND** a run that goes on stays and has no line
  [@test] crates/host-agent/src/history_test.rs (`a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes`, `a_forgotten_run_past_the_kept_ones_leaves_its_line_with_its_whole_output`)

#### Scenario: The months past 90 days
- **WHEN** the hourly cleanup runs
- **THEN** a month of the history, of runs or of bases, whose every line is more than 90 days old goes, and the other
  files stay
  [@test] crates/host-agent/src/history_test.rs (`the_months_of_the_history_past_ninety_days_go`)

### Requirement: stats: The summary splits the waits from the work and groups the commands

The summary MUST give the waits (`heavy`, `exclusive`, `git`, `slot`, `pack_wait`) against the work by phase, the heavy
queue, the outcomes by exit and refusal, the runs that ended before their command, the cores, per command the runs,
the failures and the p50 and p90 of its wait and its command for the 30 that waited longest and the rest in one
entry, the same split by group of the heavy queue, and the bytes by day.

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

#### Scenario: The waits and the work by group
- **WHEN** the agent sums runs of groups of the heavy queue: of `--group`, from several checkouts, and runs without one
- **THEN** `groups` has per group, its `--group` else the affinity of its runs, the runs, the heavy runs, the waits,
  the wait for the heavy turn and the work, the longest wait first, the groups past the 30 that waited longest in
  `other_groups`
- **AND** its line is `<n> run(s), <n> heavy, waited <time> (<time> for the heavy turn), worked <time>: <group>` under
  `groups by their wait:`, and the rest `<n> other group(s): <n> run(s), waited <time>, worked <time>`
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_sums_the_waits_and_the_work_by_group`, `the_lines_of_a_summary`)
  [@test] crates/host-agent/src/history_test.rs (`a_run_leaves_its_line_in_the_month_of_its_end_as_its_directory_goes`)

#### Scenario: The cost of the exclusive commands
- **WHEN** the agent sums a window with exclusive runs
- **THEN** the summary has `exclusive_costs`, per exclusive command the most first: its runs, the time they held the
  heavy turn (their phases after `heavy`), the time they drained the host, its runs without a drain of record, the other
  runs whose wait in `heavy` or `exclusive` overlapped a drain or a hold, the time of the overlap, the time its runs
  waited for the holders before their drains, the heavy runs that waited behind them then, and its outcomes
- **AND** its line is `<n> run(s) held the host <time> and drained it <time>, <n> run(s) waited <time> in them; before
  the drains they waited <time> for the holders, while <n> run(s) waited <time> behind them, outcomes exit <code> <n>,
  …: <command>` under `exclusive commands by their cost:`, the part after `;` only with such a wait
  [@test] crates/lease-pool/src/host_history_test.rs (`the_summary_gives_the_cost_of_each_exclusive_command`)

#### Scenario: The idle cores by the series
- **WHEN** the agent sums a window that holds minutes of the series of the heavy turn
- **THEN** the summary has `series`: the minutes, the minutes in which heavy runs waited, their mean idle cores, the
  same by cause with the most idle core minutes first, and the minutes with a dry start and the runs it started; its
  line is `heavy turn by the minute in the last <days> day(s): <n> min recorded, heavy runs waited in <n>, <cores> cores
  idle on average: <cause> <n> min at <cores>, …; a backfill would have started <n> run(s) in <n> min (a dry run)`,
  `<days>` at most the 30 that the series keeps, and a minute without a cause is `the run that can start fitted`
- **AND** a window without a minute, and an agent of an older build, give no `series`
  [@test] crates/lease-pool/src/host_series_test.rs (`the_summary_of_the_minutes_gives_the_idle_cores_by_cause`)
  [@test] crates/host-agent/src/history_test.rs (`the_summary_reads_the_lines_of_its_window_and_the_ended_runs_that_the_agent_keeps`)

### Requirement: serve: An estimate says when a run would start there

`Request::Estimate` (feature `estimate`) MUST answer `has_commit`, `warm`, `cores`, `expected_cores`, `expected_wall_ms`,
`start_in_ms`, `unknown_ahead`, `os` and `slots`, read without a lock. A heavy run MUST take its place in the queue as a new run
would, and the holders and the runs before it MUST end after the walls of their commands, admitted by the rule of the
turn, the load outside the heavy runs the most of the last 30 s.

#### Scenario: The shapes of an estimate
- **WHEN** a client asks an agent for an estimate
- **THEN** the request and the reply are `{"request": "estimate", "repo", "affinity", "commit", "command", "fetch",
  "heavy", "measure_as", "group"}` and `{"reply": "estimate", …}`, `measure_as` left out without a class (feature
  `measure_as`), `group` without a group, and the fields of a measure without one; the measures of a class count for
  a run of it
- **AND** `slots` is the slot limit of the repository, the smaller of `branchdeck.hostSlots` and the limit of the last
  shrink, left out by an agent before it and when the limit cannot be read
  [@test] crates/host-agent/tests/serve/lend.rs (`a_host_without_a_slot_for_the_repository_refuses_the_run_at_once_with_no_slots`)
  [@test] crates/host-link/src/proto_test.rs (`an_estimate_and_its_reply_have_a_tagged_json_shape`, `the_class_of_the_measures_is_an_optional_field_of_a_start_and_an_estimate`, `the_group_of_a_run_is_an_optional_field_of_a_start_and_an_estimate`)

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

#### Scenario: A run of a group in the estimate
- **WHEN** the run is of a group, its `group` else its affinity
- **THEN** the estimate places it by the turns of the groups, by the round and the place functions of the queue: behind
  the first waiting run of a group of many, not behind all of them
  [@test] crates/lease-pool/src/host_estimate_test.rs (`a_run_of_another_group_is_placed_by_the_turns_as_the_queue_places_it`)
