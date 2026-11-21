## MODIFIED Requirements

### Requirement: Report: On macOS an unchanged slot stays cached

On macOS, a slot whose saved mark has no change in the history MUST stay cached regardless of its age, its lease, its
return or a reclaim since it was measured: the history names what they changed.

#### Scenario: No change since the saved mark
- **GIVEN** macOS
- **WHEN** the history names no change of a slot since its saved mark, whatever its age, its lease or its return
- **THEN** the slot stays cached, and its mark moves on to the one taken before the replay
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`)

### Requirement: Report: On macOS a changed path remeasures its nearest saved node

On macOS, a changed path MUST remeasure the deepest saved part it lies in, the other parts keeping their bytes: a
subtree of at most 20,000 entries is walked again; a directory above the subtrees is read alone, its own files, its
directories that no part holds walked as new parts and those that are gone dropped; a root that no saved measure holds
is walked whole.

#### Scenario: A changed file
- **GIVEN** macOS
- **WHEN** a path changed since the saved mark
- **THEN** the deepest saved part it lies in is measured again
- **AND** the other parts keep their bytes
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`, `a_changed_path_selects_the_deepest_saved_part_of_the_deepest_root`)

#### Scenario: A file of a directory above the subtrees
- **WHEN** a file of a directory above the subtrees changed, a directory was created in it or one under it is gone
- **THEN** that directory is read alone, the new directory is walked as a part of its own, the gone one leaves the
  measure
- **AND** the bytes agree with a full walk
  [@test] crates/pool-disk/src/disk_test.rs (`a_directory_above_the_parts_is_read_alone_and_its_new_and_gone_directories_follow`)

#### Scenario: The parts of a walk
- **WHEN** a tree is walked
- **THEN** its parts are the subtrees of at most the limit of entries and the directories above them, each with the bytes
  of its own files, and they add up to the tree
  [@test] crates/fs-walk/src/walk_test.rs (`the_parts_of_a_tree_are_the_subtrees_of_at_most_the_node_entries_and_the_directories_above_them`)

#### Scenario: The changed paths of the events
- **WHEN** the history names changed paths
- **THEN** each goes to the measures under whose roots it lies and whose mark it follows
  [@test] crates/fs-walk/src/changes_test.rs (`canonical_path_keeps_the_nonexistent_suffix`, `an_event_goes_to_the_queries_under_whose_roots_it_lies_and_whose_mark_it_follows`)

### Requirement: Report: Other platforms cache a slot by its return and its lease

On other platforms, where there is no history, a slot MUST stay cached while it is returned and leased as when it was
measured, a leased one only for 15 minutes (`LEASED_STALE`: its builds write while it is leased); any other slot MUST be
walked in full.

#### Scenario: A platform without FSEvents
- **GIVEN** a platform other than macOS
- **WHEN** the dialog reads `disk_usage`
- **THEN** no mark of the history is taken and every query of the history is refused
  [@test] crates/fs-walk/src/changes_test.rs (`platforms_without_fsevents_keep_no_history`)

#### Scenario: A free slot returned as when it was measured
- **GIVEN** a platform other than macOS
- **WHEN** a reclaim returns another slot, and gives it a number of an earlier free period
- **THEN** the other free slots stay cached, the reclaimed one is walked, and a measure of another reclaimed lease is
  stale
  [@test] crates/pool-disk/src/disk_test.rs (`a_reclaim_leaves_the_measures_of_the_other_free_slots_valid`, `a_reclaimed_slot_with_the_number_of_an_earlier_free_period_is_measured_again`)

#### Scenario: A leased slot measured within 15 minutes
- **GIVEN** a platform other than macOS
- **WHEN** a slot, leased as when it was measured, was measured less than 15 minutes ago
- **THEN** it stays cached

#### Scenario: A leased slot measured longer ago
- **GIVEN** a platform other than macOS
- **WHEN** a leased slot was measured 15 minutes ago or longer
- **THEN** it is walked in full

## REMOVED Requirements

### Requirement: Report: A full walk where a partial sum could double-count

**Reason**: a file of `n` hard links counts `1/n` of its bytes at each link, so a sum of parts never counts a link twice;
neither a hard link nor a new one nor a renamed directory walks a slot in full

**Migration**: `Report: A full walk where the history cannot name the changes`, `Report: A late history keeps the saved bytes`

## ADDED Requirements

### Requirement: Report: The history of every measure is read in one replay by a deadline

On macOS the report MUST read the history of every measure it checks in one replay of a volume from the oldest of their
marks (a mark far behind the others replays apart), by a deadline of 10 s; each event MUST go only to the measures under
whose roots it lies and whose mark it follows, whatever the order of the ids. The new mark MUST be taken before the
replay and kept only with the measure: a write the history has not shown yet comes after it, in the next replay.

#### Scenario: Events of several slots
- **WHEN** a replay names changes under the roots of two slots and of none
- **THEN** each slot gets the changes under its roots after its mark, and a change before its mark or under no root
  goes nowhere
  [@test] crates/fs-walk/src/changes_test.rs (`an_event_goes_to_the_queries_under_whose_roots_it_lies_and_whose_mark_it_follows`)

### Requirement: Report: A full walk where the history cannot name the changes

A measure MUST be walked in full when it has no mark, a mark of another volume or one past the current event, when the
history lost events under it (`MustScanSubDirs` there, or dropped events, wrapped ids, a mount, a moved root anywhere),
and when nothing was saved; a lost subtree refuses only the measures it touches.

#### Scenario: A mark that cannot be replayed
- **GIVEN** macOS
- **WHEN** the saved mark is of another volume, past the current event or none
- **THEN** the history refuses it, and the slot is walked in full
  [@test] crates/fs-walk/src/changes_test.rs (`fsevents_refuses_a_mark_from_the_future_or_from_another_device_and_no_mark`)
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`)

#### Scenario: A history that lost events
- **WHEN** the history lost events under a slot
- **THEN** that slot is walked in full, and a lost subtree refuses only the slots under it or above it
  [@test] crates/fs-walk/src/changes_test.rs (`an_event_goes_to_the_queries_under_whose_roots_it_lies_and_whose_mark_it_follows`)
  [@test] crates/pool-disk/src/disk_test.rs (`a_late_history_keeps_the_saved_bytes_until_a_day_has_passed`)

### Requirement: Report: A late history keeps the saved bytes

A replay that does not end by its deadline MUST leave its measures with their saved bytes and their saved mark, shown
with their age and "the file history came late"; a measure the history has not checked for a day MUST be walked in full.

#### Scenario: The history comes late
- **WHEN** the replay of the history does not end by its deadline
- **THEN** the slot keeps its saved bytes and its mark, and `Planning` counts it as late, not as walked
  [@test] crates/pool-disk/src/disk_test.rs (`a_late_history_keeps_the_saved_bytes_until_a_day_has_passed`)

#### Scenario: A day without a history
- **WHEN** the history came late for a slot that it has not checked for a day
- **THEN** the slot is walked in full
  [@test] crates/pool-disk/src/disk_test.rs (`a_late_history_keeps_the_saved_bytes_until_a_day_has_passed`)
