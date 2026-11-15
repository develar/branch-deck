## MODIFIED Requirements

### Requirement: After the update: Hidden is remembered for the repository

Hidden MUST be remembered for the repository (`landedHiddenUntil` of its `repositoryState`, the newest `landedAt`
seen), so a restart doesn't show it again; what lands later MUST show again. It is a field of the lease of a slot of a
lease pool, written by Rust under the lease of the view it was asked on (`hide_landed`): each lease has its own report.

#### Scenario: The island was hidden
- **WHEN** a sync hid the island
- **THEN** `landedHiddenUntil` of the `repositoryState` of the repository is the newest `landedAt` seen, and a
  restart doesn't show the island again
  [@test] tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`, `should keep the landed report after a sync hid the island, in the panel`)

#### Scenario: A hide of the view of the lease before
- **WHEN** the island of a view of the lease before of a slot hides what it showed, the slot leased again
- **THEN** nothing is hidden of the report of the new lease
  [@test] crates/app-settings/src/projects_test.rs (`a_record_of_a_sync_is_of_the_lease_its_session_opened_under`, `a_write_of_a_window_keeps_the_fields_of_the_lease_as_rust_recorded_them`)

#### Scenario: More lands later
- **WHEN** something lands after the hidden report
- **THEN** the island shows again
