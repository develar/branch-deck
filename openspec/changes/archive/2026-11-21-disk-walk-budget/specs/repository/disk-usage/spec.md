## MODIFIED Requirements

### Requirement: Report: The first event is the cached overview

The first `disk_usage` event MUST be `Cached`, carrying the full saved overview before any tree walk; it shows each
slot's `measuredAt` age and whether its bytes are cached.

#### Scenario: The saved overview comes before any walk
- **WHEN** the dialog reads `disk_usage`
- **THEN** the first event is `Cached`, the full saved overview, before any tree walk
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`)
  [@test] crates/app-api/src/app/disk_test.rs (`disk_usage_events_serialize_their_channel_type`)

#### Scenario: The age of each slot's bytes
- **WHEN** the cached overview is shown
- **THEN** each slot shows its `measuredAt` age and whether its bytes are cached
  [@test] src/presenters/disk.test.ts (`words when slot bytes were measured and disk progress`)

### Requirement: Report: The progress counts the slots as their walks complete

After the check of the history, `Planning` MUST set the exact number of slots to walk and the number that keep their
saved bytes as the history came late; a `Slot` event MUST replace a slot as its walks complete and advance the measured
count. Host answers MUST arrive as `Machine`, and `Done` ends progress.

#### Scenario: Each finished slot replaces its row and advances the count
- **WHEN** the walk of a slot completes
- **THEN** a `Slot` event replaces that slot and advances the measured count of the slots `Planning` set
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`, `a_slot_event_counts_the_slots_that_the_history_check_left`)
  [@test] src/presenters/disk.test.ts (`words when slot bytes were measured and disk progress`)
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)

#### Scenario: The hosts and the end of the progress
- **WHEN** the hosts answer and the read ends
- **THEN** the host answers arrive as `Machine`, and `Done` ends progress
  [@test] crates/app-api/src/app/disk_test.rs (`disk_usage_events_serialize_their_channel_type`)

### Requirement: Report: Measure Again walks every slot in full

Measure Again MUST read with `fresh`: every slot walked again in full, whatever `disk/` holds and whatever the history
says, without the walk budget; the answer shown MUST stay until the new one comes.

#### Scenario: Measure Again reads with `fresh`
- **WHEN** the user chooses Measure Again
- **THEN** the dialog reads with `fresh`, every slot walked again in full, however long it takes
- **AND** the answer shown stays until the new one comes
  [@test] crates/pool-disk/src/disk_test.rs (`the_walk_budget_leaves_the_slots_it_did_not_reach_to_the_next_report`)

## ADDED Requirements

### Requirement: Report: The walks run in a budget and save each slot as it ends

The walks of a report MUST run in a budget of 90 s unless `fresh`: the changed parts first, then the full walks, the
least recently measured first, ending one after another in that order. Each slot MUST be saved as its walks end (a
file a slot in `disk/`); what the budget did not reach MUST keep its saved bytes, or none, and is walked by the next
report.

#### Scenario: The budget ends before the walks
- **WHEN** the budget of a report ends before the walks of a slot
- **THEN** the slot keeps its saved bytes, or shows none, and the next report walks it
- **AND** a slot whose walks ended is saved
  [@test] crates/pool-disk/src/disk_test.rs (`the_walk_budget_leaves_the_slots_it_did_not_reach_to_the_next_report`)
  [@test] crates/fs-walk/src/walk_test.rs (`each_walk_is_told_once_as_it_ends_and_a_stop_names_no_bytes_of_an_unfinished_one`)

### Requirement: Report: One report walks a pool at a time

A report MUST walk a pool only under `disk/walk.lock` of the pool, taken at once or not at all: while another report
holds it, the report MUST show the saved bytes of the pool and write nothing.

#### Scenario: Another report walks the pool
- **WHEN** a report reads a pool that another report walks
- **THEN** it shows the saved bytes, "another report measures now", and writes nothing
  [@test] crates/pool-disk/src/disk_test.rs (`another_report_that_walks_the_pool_leaves_this_one_the_saved_bytes`)

### Requirement: Report: A slot says why it shows saved bytes that may be stale

A slot whose saved bytes were kept though it may have changed MUST say why beside their age: the history came late,
the next report measures it (the budget), another report measures now.

#### Scenario: The reasons beside the age
- **WHEN** a slot keeps its saved bytes as the history came late, the budget ended, or another report walks
- **THEN** its line says "Measured 2 hours ago, the file history came late", "…, the next report measures it" or
  "…, another report measures now"
  [@test] src/presenters/disk.test.ts (`says why a slot shows saved bytes that may be stale`)
