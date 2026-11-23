## MODIFIED Requirements

### Requirement: After the update: The island hides after the user saw it

The island MUST show from the report until the first successful sync of the repository's session that completes while a
window that saw the island has focus, at least 10 s after it saw it (the island showed there with the focus): the sync
that follows the update keeps it, a later one hides what was seen, in every window and the IDE alike. The session keeps
the sightings and decides it (`landed_seen`, `domain_policy::landed_island`), not the page.

#### Scenario: A sync after the island was seen
- **GIVEN** the island showed while the window had focus
- **WHEN** a successful sync completes while the window has focus, less than 10 s after the user saw it
- **THEN** the island stays
- **AND** the first such sync at least 10 s after the user saw it hides what was seen
  [@test] tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`, `should keep the landed report after a sync hid the island, in the panel`)
  [@test] crates/domain-policy/src/landed_island_test.rs (`a_sighting_of_the_same_report_keeps_its_time_and_a_newer_one_starts_again`, `a_sync_hides_what_a_focused_window_saw_at_least_ten_seconds_ago`, `a_hide_spends_the_sightings_of_what_it_hid`)
  [@test] crates/repository-model/src/session_test.rs (`the_island_a_focused_window_saw_hides_at_the_first_sync_ten_seconds_after_for_the_repository`)
  [@test] src/stores/landed.test.ts (`tells the session what the island shows on which view while the window has focus, and nothing without it`)

### Requirement: After the update: Hidden is remembered for the repository

Hidden MUST be remembered for the repository (`landedHiddenUntil` of its `repositoryState`, the newest `landedAt`
seen), so a restart doesn't show it again; what lands later MUST show again. It is a field of the lease of a slot of a
lease pool, written by the session after its sync under the lease it opened under (`hide_landed`): each lease has its
own report.

#### Scenario: The island was hidden
- **WHEN** a sync hid the island
- **THEN** `landedHiddenUntil` of the `repositoryState` of the repository is the newest `landedAt` seen, and a
  restart doesn't show the island again
  [@test] tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`, `should keep the landed report after a sync hid the island, in the panel`)

#### Scenario: A hide of the session of the lease before
- **WHEN** the session of the lease before of a slot hides what its windows saw, the slot leased again
- **THEN** nothing is hidden of the report of the new lease, and the session of the new lease hides only what its
  windows saw
  [@test] crates/app-settings/src/projects_test.rs (`a_record_of_a_sync_is_of_the_lease_its_session_opened_under`, `a_write_of_a_window_keeps_the_fields_of_the_lease_as_rust_recorded_them`)
  [@test] crates/repository-model/src/session_test.rs (`the_island_seen_before_a_reopen_hides_under_the_lease_read_only_for_a_session_that_knew_none`)

#### Scenario: A hide of a session that knew no lease of its slot
- **GIVEN** the session of the window opened while the lease of its slot could not be told, and the slot reads one now
- **WHEN** its next sync runs
- **THEN** it syncs and hides nothing, and the session opens anew under the lease read
- **AND** the island stays seen as it was, so the next sync hides what was seen under that lease
  [@test] crates/repository-model/src/session_test.rs (`the_island_seen_before_a_reopen_hides_under_the_lease_read_only_for_a_session_that_knew_none`)

#### Scenario: More lands later
- **WHEN** something lands after the hidden report
- **THEN** the island shows again
