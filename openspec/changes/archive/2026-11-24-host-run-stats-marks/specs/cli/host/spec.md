## MODIFIED Requirements

### Requirement: stats: A heavy run writes its place and its wait in the queue

The supervisor of a heavy run MUST write `queue.json` after its wait for the heavy turn, whatever its end: its place
as the rule of the queue gave it, the holders then, the cores of the host, its expected cores, its need, whether it was
short and how often short runs passed it (`passes.json`), and its wait behind others and for the load of the host.

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
  [@test] crates/lease-pool/src/host_history_test.rs (`a_run_in_the_queue_waits_behind_others_or_for_the_load_of_the_host`, `the_clock_of_the_queue_splits_the_wait_by_the_spot_of_each_look`, `the_need_of_a_heavy_run_follows_its_measure`)
