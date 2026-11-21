## MODIFIED Requirements

### Requirement: serve: Heavy runs share the cores of the host by their measure

Before its lend, a heavy run MUST take its cores of the heavy turn in the phase `heavy`, waiting up to 2 h first come
first, and give them back after its give-back. Only the head of the queue MUST be admitted, or a run past an exclusive
head: while the cores of the holders, the busy cores outside them and its own fit the host, or on a free turn of a quiet
host or after 15 min in the queue. A run without a measure MUST take the whole host. A held lock MUST cost a look, never
the run.

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
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_run_without_a_measure_takes_the_whole_host_and_the_head_of_the_queue_goes_first`, `a_run_without_a_measure_takes_a_busy_free_turn_after_the_load_bound_ahead_of_later_smaller_runs`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_a_measure_share_the_cores_and_a_run_without_one_waits_for_the_whole_host`)

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
  has no measure yet` or `all its <cores of the host> cores, as the command measured <cores>`; each holder is `run <id>
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

### Requirement: serve: An exclusive run holds a quiet host alone

An exclusive run MUST be admitted only without a holder, with no run that is not heavy in its command, and when every
sample of the host of the last 30 s is quiet. While it holds, no heavy run MUST be admitted, and each new run that is
not heavy MUST wait for its end before its lend, in the step `exclusive`, up to 1 h. The holder MUST also be in
`exclusive.json`, which no older build rewrites. No heavy run MUST pass it before the end of its drain.

#### Scenario: The admission of an exclusive run
- **WHEN** an exclusive run comes to the head of the queue
- **THEN** it waits until no heavy run holds cores and no run that is not heavy is in its command, of any repository
  (a run that is not heavy is in `light` of the turn from the start of its command to its end, at 0 cores in a lull
  too), and then until every sample of the host of the last 30 s is quiet (`host.json` keeps their most); a window of
  less than 30 s, or a lull of one sample, does not admit it, and the wait goes on up to its 2 h
- **AND** then no heavy run is admitted beside it, one of a measure that would fit neither
- **AND** the line of its wait says `all its <cores> cores of a quiet host, as an exclusive run`, the busy cores of the
  host outside the heavy runs, `waits for run <id> of <repository> to end: <command>` (`waits for <n> runs to end: run
  <id> of <repository> (<command>), …`) and `the host was busy with up to <cores> of <cores of the host> cores in the
  last 30 s`, so the run sees what keeps it out
  [@test] crates/lease-pool/src/host_turn_test.rs (`an_exclusive_run_waits_for_a_quiet_host_and_holds_it_alone`, `an_exclusive_run_waits_for_the_runs_in_their_command_and_a_quiet_window`, `the_window_of_the_host_keeps_the_samples_of_the_last_30_s`)
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_lines_of_an_exclusive_run_and_of_a_run_that_waits_for_it`, `the_wait_of_an_exclusive_run_names_the_runs_in_their_command_and_the_window`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`an_exclusive_run_waits_for_a_run_in_its_command_to_end`)

#### Scenario: A new run while an exclusive run drains the host
- **WHEN** a run that is not heavy starts while an exclusive run is at the head of the queue and no heavy run holds
  cores, so it waits for the runs in their command to end
- **THEN** the new run waits before its lend, in the step `exclusive`, and its log says `run <id> waits <time> for a
  measurement: run <id> of <repository> waits for the host to drain (<command>); this run starts when the measurement
  ends, or in at most <time>`
- **AND** 15 min after the start of the drain (`heavy/drain.json`) the bar lifts and the new runs start again, while the
  exclusive run waits on and the heavy runs behind it that fit start past it; once it holds the host they wait for its
  end; behind a heavy holder it drains nothing
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_draining_exclusive_run_bars_new_runs_for_at_most_the_bound`)
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_lines_of_an_exclusive_run_and_of_a_run_that_waits_for_it`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_new_run_waits_while_an_exclusive_run_drains_the_host_for_at_most_the_bound`)

#### Scenario: A heavy run past an exclusive run whose drain ended
- **WHEN** an exclusive run at the head of the queue cannot start, and its drain of 15 min has ended without a quiet
  host, as runs that are not heavy run on for hours
- **THEN** the first heavy run behind it that is not exclusive starts past it once the admission of the head of the
  queue admits that run: beside the holders and the busy cores outside them, or on a free turn of a quiet host or after
  its own 15 min in the queue; the runs after it follow in their order
- **AND** no run passes it before its drain ends: behind a heavy holder, where its drain has not begun, and while it
  drains; a run that does not fit does not pass, and a later smaller run waits behind it
- **AND** the exclusive run keeps its place at the head, no run passes it while it can start, its drain does not start
  again, and it starts once no heavy run holds cores, no run that is not heavy is in its command and the host was quiet
  for 30 s
- **AND** the line of its wait ends `; it drains the host once no heavy run holds cores, and no heavy run starts past
  it before its drain ends`, `; it drains the host for <time> more: the new runs that are not heavy wait for it, and no
  heavy run starts past it`, or `; its drain ended <time> ago without a quiet host: the heavy runs behind it that fit
  start past it, and it starts once the host is quiet`
- **AND** the line of a run behind it names it `<id> of <repository> (<command>, exclusive)` and ends `; run <id>
  drains the host once no heavy run holds cores, and no run starts past it before its drain ends`, `; run <id> drains
  the host for <time> more, and no run starts past it before then`, or `; the drain of run <id> ended <time> ago
  without a quiet host: the heavy runs behind it start past it in their order while they fit`
- **AND** a holder that passed it is `run <id> of <repository> holds <cores> for <time> (<command>, measured, past the
  exclusive run <id>)` in a line, and its log after the wait says `took the heavy turn of the host after <n> s, past the
  exclusive run <id> of <repository> at the head of the queue, whose drain ended without a quiet host`, at once too
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_heavy_run_that_fits_starts_past_an_exclusive_run_whose_drain_ended`, `no_run_passes_an_exclusive_run_before_its_drain_ends_nor_one_that_does_not_fit`)
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_turn_line_names_the_drain_of_an_exclusive_head_and_the_run_that_passed_it`)
  [@test] crates/host-agent/src/turn_test.rs (`the_line_after_the_wait_names_the_exclusive_run_that_the_run_passed`)

#### Scenario: A run that is not heavy and an exclusive run
- **WHEN** a run that is not heavy starts while an exclusive run holds the turn
- **THEN** at once and then once a minute its log says `run <id> waits <time> for a measurement: run <id> of
  <repository> holds the host for <time> (<command>); a measurement holds the host; this run starts when it ends`
- **AND** it takes its slot once the exclusive run gave the turn back, with the step `exclusive` first of the host;
  after 1 h it refuses with `slot_timeout`; a run of another repository waits the same
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_run_that_is_not_heavy_waits_for_an_exclusive_run_and_starts_after_it`, `a_run_of_another_repository_waits_for_an_exclusive_run`)
  [@test] crates/host-agent/src/turn_test.rs (`a_run_that_is_not_heavy_waits_for_the_end_of_an_exclusive_run`)

#### Scenario: A table that an older build rewrote
- **WHEN** a supervisor of a build before `exclusive` rewrites `turn.json` at a sample of its run and drops the field
  `exclusive` of the holder
- **THEN** the runs that are not heavy still wait, as they read the holder from `exclusive.json`, and `brd-host runs`
  still names the measurement; the next look of the exclusive run puts the field back, and its end removes the file
  [@test] crates/lease-pool/src/host_turn_test.rs (`an_exclusive_holder_outlives_a_rewrite_of_the_table_by_an_older_build`)

#### Scenario: brd-host runs and an exclusive run
- **WHEN** `brd-host runs` shows the heavy turn while an exclusive run holds it or waits
- **THEN** a holder is `run <id> of <repository> holds the whole host for a measurement for <time> (exclusive):
  <command>`, and a waiting one `waits <time> for a quiet host (exclusive)`
- **AND** at the head of the queue a waiting one says its drain from `head_drain` of the reply: `(exclusive; it drains
  the host once no heavy run holds cores, and no run starts past it before then)`, `(exclusive; it drains the host for
  <time> more, and no run starts past it before then)`, or `(exclusive; its drain ended <time> ago without one, and the
  heavy runs behind it start past it while they fit)`; a reply of an agent before the pass has no `head_drain`
- **AND** a holder that started past it says so: `run <id> of <repository> holds <cores> for <time> (measured, past the
  exclusive run <id>): <command>`
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_lines_of_an_exclusive_run_and_of_a_run_that_waits_for_it`)
  [@test] crates/host-agent/src/turn_test.rs (`a_run_that_is_not_heavy_waits_for_the_end_of_an_exclusive_run`, `the_lines_of_the_heavy_turn_name_the_drain_of_an_exclusive_head_and_who_passed_it`)

