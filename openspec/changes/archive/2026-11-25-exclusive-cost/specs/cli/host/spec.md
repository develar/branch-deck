## MODIFIED Requirements

### Requirement: stats: A heavy run writes its place and its wait in the queue

The supervisor of a heavy run MUST write `queue.json` after its wait for the heavy turn, whatever its end: its place
as the rule of the queue gave it, the holders then, the cores of the host, its expected cores, its need, whether it was
short and how often short runs passed it (`passes.json`), and its wait behind others and for the load of the host; an
exclusive run MUST also write the parts in which it drained the host and waited for the holders before, and their
starts.

#### Scenario: Two heavy runs
- **GIVEN** a heavy run that holds the turn
- **WHEN** a second heavy run waits for it, and both end
- **THEN** `queue.json` of the first names the place 0, no holder and no wait, and that of the second the place 0, one
  holder and its wait behind it
- **AND** a run that is not heavy has no `queue.json`
  [@test] crates/host-agent/tests/serve/heavy.rs (`heavy_runs_of_the_host_run_one_at_a_time_and_a_run_that_is_not_heavy_goes_beside_them`)
  [@test] crates/host-agent/src/history_test.rs (`the_watch_of_a_heavy_run_writes_its_place_and_its_wait_in_the_queue`)

#### Scenario: What a look of the wait finds
- **WHEN** a look finds the run behind a holder or a run before it, or at the head of the queue with no holder
- **THEN** the span until the next look counts behind the others, or for the load of the host, the need follows the
  measure as the heavy turn counts it, and the mark of the run in the queue names its class and its passes
- **AND** for an exclusive head in its drain the span counts for the load of the host and for its drain too
  (`drain_ms`), and the first look of the drain gives `drain_at_ms`; for an exclusive head that waits for the holders
  before its drain it counts behind them and for that wait too (`pending_ms`, `pending_at_ms`); a run that is not
  exclusive writes none of them
- **AND** a need of a later build reads as `other`
  [@test] crates/lease-pool/src/host_history_test.rs (`a_run_in_the_queue_waits_behind_others_or_for_the_load_of_the_host`, `the_clock_of_the_queue_splits_the_wait_by_the_spot_of_each_look`, `the_need_of_a_heavy_run_follows_its_measure`, `a_need_of_a_later_build_reads_as_other`)
  [@test] crates/host-agent/src/history_test.rs (`the_watch_of_a_heavy_run_writes_its_place_and_its_wait_in_the_queue`, `the_watch_of_an_exclusive_run_writes_its_drain`, `the_watch_of_an_exclusive_run_writes_its_wait_for_the_holders`)

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
  same by cause with the most idle core minutes first, and the minutes and the runs of the dry backfill; its line is
  `heavy turn by the minute: <n> min recorded, heavy runs waited in <n>, <cores> cores idle on average: <cause> <n> min
  at <cores>, …; a backfill would have started <n> run(s) in <n> min (a dry run)`
- **AND** a window without a minute, and an agent of an older build, give no `series`
  [@test] crates/lease-pool/src/host_series_test.rs (`the_summary_of_the_minutes_gives_the_idle_cores_by_cause`)
  [@test] crates/host-agent/src/history_test.rs (`the_summary_reads_the_lines_of_its_window_and_the_ended_runs_that_the_agent_keeps`)
