## MODIFIED Requirements

### Requirement: serve: Heavy runs share the cores of the host by their measure

Before its lend, a heavy run MUST take its cores of the heavy turn in the phase `heavy`, waiting up to 2 h, and give
them back after its give-back. Only the head of the queue MUST be admitted, or a run past an exclusive head: while it
fits beside the holders and the load of the host, or on a free turn of a quiet host or after 15 min. A new run MUST
join last, a short one ahead of those it may pass. A holder MUST count its measure, else the whole host, for 10 min;
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
  when the most of them leaves less than a fifth of the host beside it, it counts as before
- **AND** a supervisor of an older build that rewrites `turn.json` drops `recent`: the holder counts as before until a
  new window spans 5 min
- **AND** a run of the queue without a measure still waits for every holder, and the measure at the end of the run is
  as before
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_holder_without_a_measure_holds_the_whole_host_in_its_grace_and_then_its_use_of_the_last_5_min`, `a_holder_of_a_measure_counts_its_measure_in_its_grace_and_then_its_use_of_the_last_5_min`, `a_holder_without_a_measure_holds_the_whole_host_still_without_a_window_that_leaves_room`, `the_window_of_the_live_use_keeps_the_most_of_each_half_minute_of_the_last_5_min`)

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
  uses <cores>: <command>` for each run that is not heavy whose command runs
- **AND** `--json` has the turn as `heavy`: the `cores`, the `holders`, the `queue` and the `light` runs, without the
  runs whose process is gone, and the first holder as `holder` too, which a client of the build before reads
- **AND** a reply of an agent of the build before, with one `holder` and no `cores`, says `heavy turn: run <id> of
  <repository> holds it <time>: <command>`, then `  run <id> of <repository> waits <time>: <command>`
  [@test] crates/bd-host/tests/cli.rs (`runs_shows_who_holds_the_heavy_turn_and_who_waits_for_how_long`, `follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)
  [@test] crates/host-agent/src/turn_test.rs (`the_lines_of_the_heavy_turn_name_the_cores_of_each_holder_waiting_run_and_light_run`, `the_lines_of_a_heavy_turn_of_the_build_before_name_the_holder_and_each_waiting_run`)
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_of_a_class_names_its_class_on_its_entry`)

### Requirement: serve: A run measures the cores of its command

The supervisor of every run MUST sample the CPU of its command group, of the Bazel servers of its slot and of their
descendants, each with the CPU of its reaped children, every 10 s, and at the end measure the mean, the peak and the
level of work. A command of 10 s or more MUST add its measure to the last 5 of its repository and command, by a key
without the paths of the directories that the run fetches.

#### Scenario: The samples of a run
- **WHEN** the command of a run runs
- **THEN** each sample rewrites `cores.json` of the run and notes the live cores in the heavy turn: of a holder as its
  `live`, of a run that is not heavy in `light`
- **AND** at the end its log says `the command used <mean> cores on average and <peak> at its peak, of the <cores> of
  the host`: the mean is the larger of the sampled CPU and the CPU of the reaped children of the supervisor over the
  wall time, the peak the most of one sample, and the level of work, in `level` of `cores.json` of the run, the live
  cores at or above which 30 % of the core seconds of its samples ran
- **AND** a process that started before the command, such as a Bazel server of the slot that a later sample finds,
  counts from that sample on, and a sample counts at most the cores of the machine
- **AND** a process that started and ended between two samples, a compiler or a test, counts in the process that
  reaped it, and a process that a sample saw alive counts once after its reap: its time at that sample is a credit
  that the growth of an ancestor that a sample looks at pays off
  [@test] crates/host-agent/src/cores_test.rs (`the_samples_count_the_cpu_of_the_command_group_and_the_end_its_measure`, `a_process_older_than_the_command_counts_only_from_its_first_sample`, `a_sample_counts_at_most_the_cores_of_the_machine`, `a_child_that_ends_between_two_samples_counts_in_the_process_that_reaped_it`, `a_child_seen_alive_counts_once_after_its_parent_reaped_it`)
  [@test] crates/sync-utils/src/process_scan_test.rs (`the_cpu_time_of_a_process_grows_while_it_computes`, `the_cpu_times_of_a_process_count_its_reaped_children`)
  [@test] crates/sync-utils/src/cores_test.rs (`cores_read_and_write_with_one_decimal_and_are_the_mean_of_a_cpu_time`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_a_measure_share_the_cores_and_a_run_without_one_waits_for_the_whole_host`)

#### Scenario: The measure follows the command
- **WHEN** a run of a command ends, a cancelled one too
- **THEN** its measure joins the last 5 of the command, and the next run expects the second largest of their levels of
  work, each at least its mean (the largest of up to 4), so a command that got heavier is expected heavier at its next
  run, and one that got lighter frees its cores within a few runs
- **AND** a run that builds on 17 cores for minutes and runs on one core for hours expects 17, not its mean; a steady
  run its level, about its mean
- **AND** a measure without a level, of an older build or that a supervisor of one rewrote, counts its mean
- **AND** a command of less than 10 s adds no measure, and a command without a measure for 30 days is forgotten
  [@test] crates/lease-pool/src/host_cores_test.rs (`the_expectation_of_a_command_follows_its_last_runs`, `a_command_without_a_measure_for_30_days_is_forgotten`, `the_level_of_work_of_a_bursty_run_is_its_bursts_and_of_a_steady_one_its_level`, `the_expectation_of_a_command_is_of_the_levels_of_its_runs_and_of_the_mean_without_one`)

#### Scenario: The key of the measures of a command
- **WHEN** two runs of a command differ only by the directories that they fetch (`--fetch`), such as `zsh -lc "mkdir
  -p out/air/light1 && … > out/air/light1/run.json"` with `--fetch out/air/light1` and the same with `light5`
- **THEN** both add to and read the same measures: the key of the measures (`host_link::heavy::measure_key`) is the
  key of the supersede with each path of a fetched directory, in a word or in the script of a shell, as one word
- **AND** a path that the run does not fetch stays in the key, as no word of a command tells an output from an input,
  and a command that names no fetched directory keeps the key of the supersede
- **AND** a heavy run that waits reads the measures again at each look, so a measure that came while it waited counts
  at its next look
  [@test] crates/host-link/src/heavy_test.rs (`the_measure_key_of_a_command_puts_its_fetched_directories_as_one_word`)
  [@test] crates/host-agent/src/turn_test.rs (`a_measure_that_comes_while_a_run_waits_counts_at_its_next_look`)
