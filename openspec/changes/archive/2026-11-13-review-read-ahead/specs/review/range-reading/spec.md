## ADDED Requirements

### Requirement: Reading: The read ahead asks no index

What the read ahead of the waiting requests reads MUST ask no index (a query of a tree far from the index costs tens of
seconds, JetBrains/accio#777): while the machine has an index, a rename across files waits for it, "· checking renames",
until a review window of the request asks the index itself; without one it is not proven, as the window says it.

#### Scenario: A rename across files
- **WHEN** the read ahead reads a range that renames a function and its use in another file
- **THEN** the rename waits for the index on a machine with one, and is not proven on one without; nothing is asked
  [@test] crates/review-plan/src/reading_test.rs (`the_read_ahead_leaves_a_rename_across_files_to_the_index_and_asks_none`)

### Requirement: Memory: A final reading is kept on disk

A final reading that is what it is whatever the index (no rename waits for one, none left unproven for want of one)
MUST be kept in memory and in the store of the repository (`object_cache::range_reading`) by the version of the
readings, its whitespace policy and its two commits, so the next start reads no diff of it; any other reading MUST
stay out of the store.

#### Scenario: The next start
- **WHEN** a final reading, one that waits for the index and one no index proved are kept, and the app starts again
- **THEN** the first is read from the store and kept in memory again; the others are not
  [@test] crates/app-api/src/reads/range_readings_test.rs (`a_final_reading_is_kept_on_disk_and_shows_in_the_next_process_at_once`)
  [@test] crates/object-cache/src/cache_test.rs (`range_readings_are_read_by_their_version_and_policy`)
