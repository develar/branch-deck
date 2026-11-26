## ADDED Requirements

### Requirement: serve: A heavy run without a measure takes the measure of its program or the cores the host assumes

A heavy run whose command has no measure MUST take the measure of its program, under which each measure MUST go too;
without one, the cores the host assumes of one (half of its cores by default), not the whole host. A holder of them
MUST hold the most it used of the last 5 min when that is more.

#### Scenario: The measure of its program
- **WHEN** a heavy run starts whose command has no measure, such as `scripts/bench gate --base <commit>` on a new
  commit, and runs of its program (`scripts/bench gate`) ended with a measure
- **THEN** it is admitted by the measure of its program, as by a measure of its own, and `brd-host runs` says `for
  <cores> (measured by the runs of its program)` while it waits
- **AND** its own measure wins once a run of its command has one; a run of a class of `--measure-as` names no program;
  a wall is of its command alone, so its program makes no run short
  [@test] crates/host-link/src/heavy_test.rs (`the_program_of_a_command_is_its_first_word_and_subcommand_or_the_work_of_its_script`)
  [@test] crates/lease-pool/src/host_cores_test.rs (`a_command_without_a_measure_takes_the_measure_of_its_program`)
  [@test] crates/host-agent/src/turn_test.rs (`a_run_is_expected_by_its_command_its_program_or_the_cores_the_host_assumes`)

#### Scenario: No measure at all
- **WHEN** a heavy run starts whose command and program have no measure, on a host of `<cores>` cores
- **THEN** it takes the cores the host assumes of one: `branchdeck.hostUnmeasuredCores` of the global git config of the
  host, a number of cores below `<cores>`, else half of `<cores>`; it is admitted beside the holders while they fit,
  and on a free turn without waiting for a quiet host
- **AND** its entry of the turn has `expected` of those cores and `expectation: assumed`, and the line of its wait
  says `<cores> of its <cores> cores, as the command has no measure yet (the host reserves the cores it assumes of a
  run without one until a run of it, or of its program, measures it)`
- **AND** as a holder it holds the most it used of the last 5 min when that is more, in its grace too, so a run that
  the default misjudged keeps the next runs out from its next sample; past its grace it counts as any holder
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_without_a_measure_takes_the_cores_the_host_assumes_of_one`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_without_a_measure_take_the_cores_the_host_assumes_of_one`)
  [@test] crates/host-agent/src/turn_test.rs (`a_run_is_expected_by_its_command_its_program_or_the_cores_the_host_assumes`)

#### Scenario: Older builds, an exclusive run and the history
- **WHEN** a supervisor of an older build rewrites the turn, the holder of the build before holds it, or a run is
  exclusive
- **THEN** a rewrite drops `expectation` and counts `expected` as a measure; the holder of the build before, without a
  measure, still holds the whole host; an exclusive run still waits for a quiet host
- **AND** the history names the need `assumed`, which `brd-host stats` does not count as one for the whole host
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_without_a_measure_takes_the_cores_the_host_assumes_of_one`, `the_holder_of_the_build_before_holds_the_whole_host`)
  [@test] crates/lease-pool/src/host_history_test.rs (`the_need_of_a_heavy_run_follows_its_measure`)

## MODIFIED Requirements

### Requirement: serve: Heavy runs share the cores of the host by their measure

Before its lend, a heavy run MUST take its cores of the heavy turn in the phase `heavy`, up to 2 h, and free them
after its give-back. Only the head of the queue MUST be admitted, or a run past an exclusive head: while it
fits beside the holders and the load of the host, or on a free turn of a quiet host or after 15 min. A new run MUST
join by its group's turn, a short one ahead of those it may pass. A holder MUST count its measure or assumed cores for 10 min;
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
- **AND** a run of the queue without a measure takes the cores the host assumes of one (below), and the measure at the
  end of the run is as before
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_holder_without_a_measure_holds_the_whole_host_in_its_grace_and_then_its_use_of_the_last_5_min`, `a_holder_of_a_measure_counts_its_measure_in_its_grace_and_then_its_use_of_the_last_5_min`, `a_holder_without_a_measure_holds_the_whole_host_still_without_a_window_that_leaves_room`, `a_holder_of_a_measure_past_its_grace_counts_at_least_the_most_of_its_last_5_min`, `the_window_of_the_live_use_keeps_the_most_of_each_half_minute_of_the_last_5_min`)

#### Scenario: Runs of a class
- **WHEN** a run of `--measure-as <class>` ended with a measure, and later runs of the class with other commands wait
- **THEN** they are admitted by the measure of the class, beside each other while they fit
- **AND** a run of a class without a measure takes the cores the host assumes of one, says `<cores> of its <cores of
  the host> cores, as the class <class> has no measure yet (…)` in the line of its wait, and its entry of the turn
  names its class (`measure_as`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`runs_of_a_class_share_its_measure_whatever_their_command`)
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_turn_line_names_the_wait_the_cores_of_each_holder_and_the_runs_ahead`)

#### Scenario: A run that takes the whole host
- **WHEN** a heavy run whose command measured `<cores>` or more, or an entry of a build before the assumed cores without
  a measure, comes to the head of the queue
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
  <cores>)` for one that takes the whole host, and `holds <cores> for <time> (no measure: it held the whole host, or the
  cores of a run without one, for its first 10 min, now its live use of the last 5 min)` for one without a measure
  past its grace, `holds <cores> for <time> (no measure: the host reserves the cores it assumes of a run without one
  until a run of it, or of its program, measures it, or the most it used since its start)` for one of the assumed
  cores in its grace, `(measured <cores>: it
  held that for its first 10 min, now its live use of the last 5 min)` for one of a measure; `  run <id> of <repository> waits <time> for <cores> (measured):
  <command>`, `for <cores> (measured by the runs of its program)`, `for <cores> (no measure yet: the cores the host
  assumes of a run without one)`, or `for the whole host (no measure; a command whose text changes every run never
  gets one: `--measure-as <class>` gives it one)` of an entry of a build before, of a run of a class `for the whole host (no measure of the class <class>
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
