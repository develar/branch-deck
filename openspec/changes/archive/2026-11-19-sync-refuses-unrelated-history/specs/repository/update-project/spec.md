## MODIFIED Requirements

### Requirement: Check: A deferred sync runs no check

A sync deferred for a detached HEAD, an operation in progress, a checked out branch under the prefix or a history
unrelated to the baseline (`SyncDeferred`, [Onboarding](../onboarding/spec.md)) MUST run no check and report nothing.

#### Scenario: A sync on a detached HEAD
- **GIVEN** HEAD is detached, an operation is in progress or a branch under the prefix is checked out
- **WHEN** a sync is deferred (`SyncDeferred`)
- **THEN** it runs no check and reports nothing
  [@test] crates/update-core/src/update_project_test.rs (`sync_on_a_detached_head_is_deferred_before_the_update_check`)

#### Scenario: A history unrelated to the baseline
- **GIVEN** the baseline is a history unrelated to HEAD
- **WHEN** the sync is deferred at the first root of its walk
- **THEN** it runs no check: no `merge-base`, which would walk the whole history again
  [@test] crates/sync-core/src/process_budget_test.rs (`a_sync_of_a_head_unrelated_to_the_baseline_builds_nothing`)
