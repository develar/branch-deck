## MODIFIED Requirements

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
- **AND** 5 min after the start of the drain (`heavy/drain.json`) the bar lifts and the new runs start again, while the
  exclusive run waits on and the heavy runs behind it that fit start past it; once it holds the host they wait for its
  end; behind a heavy holder it drains nothing
  [@test] crates/lease-pool/src/host_turn_test.rs (`a_draining_exclusive_run_bars_new_runs_for_at_most_the_bound`)
  [@test] crates/lease-pool/src/host_runs_test.rs (`the_lines_of_an_exclusive_run_and_of_a_run_that_waits_for_it`)
  [@test] crates/host-agent/tests/serve/heavy.rs (`a_new_run_waits_while_an_exclusive_run_drains_the_host_for_at_most_the_bound`)

#### Scenario: A heavy run past an exclusive run whose drain ended
- **WHEN** an exclusive run at the head of the queue cannot start, and its drain of 5 min has ended without a quiet
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
