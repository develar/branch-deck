## MODIFIED Requirements

### Requirement: Host slots: A waiting lend answers as soon as it gets a slot

`lend_waiting` MUST wait while the run is queued, as long as its process lives, and answer as soon as it gets a slot,
without a second call. It watches `branch-deck/` of the common dir before its first look, so no give-back between the
look and the wait is lost, wakes only on a change of `host-slots.json` or `leases.json`, and looks again at least every
5 s, as the end of a process makes no file event. A wait of many calls MUST keep one watch (`watch_lends`,
`lend_watching`).

#### Scenario: A slot given back during the wait
- **WHEN** a slot is given back while a run waits in `lend_waiting`
- **THEN** the run gets it without a second call
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_that_waits_gets_the_slot_once_it_is_given_back`)

#### Scenario: A long wait
- **WHEN** the run waits
- **THEN** it gives its caller the queue once a minute (`host_runs::queue_line`: `run <id> waits <s> s: <n> runs ahead, …`)
- **AND** once the time is up it answers `Queued` with its place, and the ticket stays while the process lives

#### Scenario: A process that ends while it waits
- **WHEN** the process of the run ends while it waits
- **THEN** the answer is `ProcessGone`, and its ticket leaves the queue

#### Scenario: A wait in short looks
- **WHEN** a supervisor waits for a slot in looks of 2 s, so a cancel ends its wait
- **THEN** the whole wait starts one watch of the table, before its first look
  [@test] crates/host-agent/src/slots_test.rs (`a_wait_for_a_slot_keeps_one_watch_of_the_table_for_all_its_looks`)
