## ADDED Requirements

### Requirement: stats: serve keeps a series of the heavy turn by the minute

While a heavy run holds cores or waits, a writer of `serve` MUST append one minute a minute to
`history/turn-<YYYY-MM-DD>.jsonl`: the busy cores, the holders, the head and its fit, why no run started, and the runs a
backfill behind the head would start, which nothing starts. The sampler MUST hand it each sample without a wait. A day
MUST stop at 4 MiB; the cleanup MUST remove the days past 30.

#### Scenario: A minute of the heavy turn
- **WHEN** a heavy run holds cores, a second one waits for the whole host, and a minute passes
- **THEN** the series of the day has a minute that names the holder, what it counts and its last sample, the head, its
  need and whether it fits, the cores of the host, the mean and the most busy cores of the minute and the busy cores
  outside the heavy runs, and the cause `whole_host`
- **AND** a turn without a heavy run writes no minute
- **AND** the writer runs on a thread of its own: the sampler hands it each sample through a queue of 64 without a
  wait, so a writer that lags, fails or panics leaves each sample of `host.json` fresh; a failure is said in the log
  once per state, a panic ends the minute and not the writer
- **AND** a cause or a drain state of a later build reads as `other`, so a minute and a summary of a later agent read
  [@test] crates/host-agent/tests/serve/heavy.rs (`serve_writes_the_series_of_the_heavy_turn_while_heavy_runs_hold_or_wait`)
  [@test] crates/lease-pool/src/host_series_test.rs (`a_minute_names_the_holders_the_head_and_why_no_run_started`, `a_value_of_a_later_build_reads_as_other`)
  [@test] crates/host-agent/src/series_test.rs (`the_samples_of_a_minute_give_their_mean_and_their_most`, `a_slow_or_failing_series_leaves_the_samples_of_the_host_fresh`, `the_writer_of_the_series_says_a_failure_once_per_state_and_outlives_a_panic`)

#### Scenario: Why no run started
- **WHEN** a minute judges the run that can start: the head, or the run that may pass an exclusive head whose drain
  ended
- **THEN** its cause is `exclusive` while an exclusive run holds, `drain`, `pending` or `quiet` for an exclusive head
  that drains, waits for the holders, or waits for a quiet host with no run behind it to pass, `whole_host` for a run of
  the whole host, `whole_holder` for a holder whose command runs and that counts the whole host, `slots` when it would
  fit but for the holders before their command, else `cores`
- **AND** a run that fits has no cause: it starts at its next look
  [@test] crates/lease-pool/src/host_series_test.rs (`a_minute_names_the_holders_the_head_and_why_no_run_started`, `a_minute_names_the_phase_of_an_exclusive_run`)

#### Scenario: The dry backfill
- **WHEN** the head of the queue cannot start
- **THEN** the minute names each run behind it, to the end of the queue, that is not exclusive, has a measure below the
  whole host, fits now with the head left out and no load bound beside the holders and the dry starts before it, and
  has a passed wall `W` of at most 30 min with `1.25 W` within the estimated start `S` of the head, with its cores, `W`,
  the head and `S`
- **AND** the writer keeps the dry starts from minute to minute: a started run is no candidate again while it waits,
  counts as a holder until its `W` ends, counts a pass of its head, and leaves with its place in the queue
- **AND** it names none with an unknown wall before the head, an exclusive run that holds or drains, or a head passed
  4 times by short runs and dry starts; behind an exclusive head whose drain ended the head is the run that may pass
  it; no run starts by it, and the files of the heavy turn stay as they were
  [@test] crates/lease-pool/src/host_series_test.rs (`the_dry_backfill_names_the_run_that_would_start_behind_the_head`, `a_minute_names_the_phase_of_an_exclusive_run`)

#### Scenario: The days of the series
- **WHEN** minutes come on two UTC days, a day holds 4 MiB, and the hourly cleanup runs
- **THEN** each day has its own file, a full day takes no more minutes, a read gives the minutes of its window, and a day more than 30 days old goes while the history of the runs stays
  [@test] crates/host-agent/src/series_test.rs (`the_series_keeps_a_file_a_day_bounded_by_its_bytes_and_its_age`)

## MODIFIED Requirements

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
