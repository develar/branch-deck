## MODIFIED Requirements

### Requirement: Reading: The read ahead asks no index

What the read ahead of the waiting requests reads MUST ask no index (a query of a tree far from the index costs tens of
seconds, JetBrains/accio#777): while the repository has an index, or one is built, a rename across files waits for it,
"· checking renames", until a review window of the request asks the index itself; without one (no accio, or accio
with no index of the repository, `accio stats`) it is not proven, as the window says it, and the reading is final.

#### Scenario: A rename across files
- **WHEN** the read ahead reads a range that renames a function and its use in another file
- **THEN** the rename waits for the index on a machine with one, and is not proven on one without; nothing is asked
  [@test] crates/review-plan/src/reading_test.rs (`the_read_ahead_leaves_a_rename_across_files_to_the_index_and_asks_none`)

#### Scenario: A repository without an index
- **WHEN** a waiting request renames a private member of a repository accio has no index of, labelled refactoring
- **THEN** its row says "4 changes to read in 1 file · 1 file not proven", never "checking renames", its label is not flagged, and the next listing does not read it again
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say a private member renamed where the repository has no index is not proven, and not read it again on the next listing`)

### Requirement: Memory: A final reading is kept on disk

A final reading (no rename waits for the index, and an index of the repository left none unanswered) MUST be kept in
memory and in the store of the repository (`object_cache::range_reading`) by the version of the readings, its
whitespace policy and its two commits, so the next start reads no diff of it; any other reading MUST stay out of
both. A reading whose renames no index answered, of a repository without one, MUST be dropped from both once the
repository has an index, which may prove them.

#### Scenario: The next start
- **WHEN** a final reading, one that waits for the index and one of a repository without an index that no index proved are kept, and the app starts again
- **THEN** the first and the last are read from the store and kept in memory again; the one that waits is not
- **AND** once the repository has an index, the last is dropped from memory too, and a reading its index left unanswered is kept nowhere
  [@test] crates/app-api/src/reads/range_readings_test.rs (`a_final_reading_is_kept_on_disk_and_shows_in_the_next_process_at_once`)
  [@test] crates/object-cache/src/cache_test.rs (`range_readings_are_read_by_their_version_and_policy`)
