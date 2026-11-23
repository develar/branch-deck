## MODIFIED Requirements

### Requirement: Starting a Safe Push or a Dry Run: A start pressed during a sync starts at once

A Safe Push start pressed while a sync no write of the session asked is running (⌘R, a focus, a stamp of `brd`) MUST
start at once on the table of the last sync. It MUST NOT wait for that sync or start another one: the held sync ends
once.

#### Scenario: A start during a sync
- **WHEN** a Safe Push start is pressed while a sync is running
- **THEN** it starts at once, on the table of the last sync, and the sync ends once
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push pressed during a sync at once, on the table of the last sync, the sync ending once`)
  [@test] crates/operations/src/run_frame_test.rs (`a_land_during_a_running_sync_does_not_wait_for_it`)

#### Scenario: A start that comments during a sync
- **WHEN** a start that comments on its pull request is pressed during a sync
- **THEN** it comments once, and the sync ends once
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should comment once for a start pressed during a sync, the sync ending once`)

### Requirement: Starting a Safe Push or a Dry Run: A plan that changed is refused before any effect

A fingerprint or a confirmation the plan no longer has MUST refuse the start as `plan_changed` before any effect, in the
words of the start. A witness whose local branches alone moved (a sync of the session rebuilt them, or shows the
rebuild of another process) MUST instead wait for the sync that shows them and decide the start again once, on what
the user accepted; a remote-tracking ref that moved, or a second move, refuses it.

#### Scenario: What the start was decided on changed
- **WHEN** the plan prepared again no longer has the fingerprint or a confirmation the user accepted
- **THEN** the start is refused before any effect: "What the Safe Push of auth was decided on changed before it began
  (…); nothing was done: start it again"
  [@test] crates/operations/src/land_execute_test.rs (`a_start_refused_before_its_first_effect_cancels_pushes_and_starts_nothing`)

#### Scenario: Its branch was rebuilt before its first effect
- **WHEN** the virtual branch of a start moved before its first effect, rebuilt by another process after the sync of
  the session
- **THEN** the start waits for the sync of the session that shows it and lands that build, decided again once
  [@test] crates/operations/src/update_test.rs (`a_start_whose_branch_another_process_rebuilt_lands_that_build_after_the_sync_that_shows_it`)
  [@test] crates/operations/src/land_execute_test.rs (`only_a_waited_landing_with_its_target_ref_moved_gets_one_retry_signal`)

## ADDED Requirements

### Requirement: Starting a Safe Push or a Dry Run: A start pressed during the sync of a write waits for it

A start pressed while the sync a write of the session asked (Update Project, a rewrite, a fetch) has not ended MUST wait
for that sync, which rebuilds the branches it lands, and start on the table it commits while the user accepted every
confirmation of it, never refused as a plan that changed; its target is fetched meanwhile, once, and it waits for the
facts of the rebuilt branch the click had answered (its review).

#### Scenario: A start during the sync of Update Project
- **WHEN** a Safe Push start is pressed while Update Project runs, and the sync it asks rebuilds the branch
- **THEN** the start waits behind the update and for that sync, then pushes the branch it rebuilt once and starts once,
  with no "Cannot start"
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Push pressed during Update Project once the sync of the update rebuilt the branch: one push of the rebuilt branch, one start`)
  [@test] crates/operations/src/update_test.rs (`a_start_during_the_sync_of_a_write_waits_for_it_and_lands_the_branch_it_rebuilt`)
  [@test] crates/operations/src/run_frame_test.rs (`a_land_during_the_sync_of_a_write_waits_for_it`, `an_operation_waits_for_the_follow_up_of_a_write_asked_during_a_running_sync`)
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_wait_for_the_writes_joins_the_run_of_the_write_in_flight`, `a_wait_for_the_writes_joins_the_follow_up_of_a_write_asked_during_a_run_or_a_check`, `a_wait_for_the_writes_without_one_unsynced_is_answered_at_once`, `a_wait_for_the_writes_hears_the_error_of_the_sync_of_the_write`)
