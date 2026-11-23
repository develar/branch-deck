## MODIFIED Requirements

### Requirement: Before the first sync: The answer of the check of a focus is recorded as a sync's

The answer of the check of a focus MUST be recorded as a sync's: `unchanged` says "Checked just now", a run it started
counts as the sync of the repository. The check that found nothing changed MUST be a fact of the session, not of the
window that asked it: its time is a part of the view (`inputsCheckedAt`), so every window of the session, the tool
window of the IDE too, says "Checked just now", until a run ends.

#### Scenario: The check found nothing changed
- **WHEN** the check of a gained focus answers `unchanged`
- **THEN** the session puts its time into the view before the answer, and every window of it gets it in one patch
- **AND** the header of each says "Checked just now"; the check of a window that watches the session anew says nothing
  [@test] crates/repository-model/src/session_test.rs (`a_focus_that_finds_nothing_changed_is_told_to_every_window_until_a_run_ends`)
  [@test] src/model/store.test.ts (`tells the session the focus of the window, and shows the check the session says found nothing changed`)

#### Scenario: A run takes the check back
- **GIVEN** the check of a focus found nothing changed
- **WHEN** a run of the session ends after it
- **THEN** the view has no check any more, and every window says "Last synced" again
  [@test] crates/repository-model/src/session_test.rs (`a_focus_that_finds_nothing_changed_is_told_to_every_window_until_a_run_ends`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs on focus only when what a sync reads changed, and says when it checked`)

#### Scenario: The check started a run
- **WHEN** the check of a focus starts a run
- **THEN** the window records it as the sync of the repository, Sync busy from its answer to the run's last patch
  [@test] src/model/store.test.ts (`records the sync a focus started, Sync busy from its answer to the run's last patch`)
