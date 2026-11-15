## ADDED Requirements

### Requirement: Disk: The report tells each step on stderr

`brd lease disk` MUST print a line on stderr when the report from `disk.json` is ready, with the count of slots to
check or measure. On macOS, when a slot waits for the FSEvents check, it MUST print a line after the check. It MUST
then print a line for each walked slot, with its bytes and its count, and a last line; stdout MUST stay one JSON object.

#### Scenario: The lines of a walk
- **WHEN** `brd lease disk --fresh` walks the three slots of a pool
- **THEN** the steps are the start with three slots to check or measure, one walked step for each slot, counted 1 to
  3, then the end
- **AND** a run with `--cached` tells only the start, with no slot to check or measure, and the end
  [@test] crates/bd-lease/src/trim_test.rs (`the_disk_report_tells_each_step_on_stderr`)

#### Scenario: The count after the FSEvents check
- **WHEN** the FSEvents check finds one of two measured slots changed
- **THEN** the check tells one slot to walk, and the walked step of that slot counts 1 of 1
  [@test] crates/pool-disk/src/disk_test.rs (`a_slot_event_counts_the_slots_that_the_fsevents_check_left`)
