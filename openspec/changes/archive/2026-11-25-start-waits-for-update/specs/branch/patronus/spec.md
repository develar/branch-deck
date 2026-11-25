## MODIFIED Requirements

### Requirement: Starting a Safe Push or a Dry Run: A change during the effects skips the start

A rewrite after the push (the virtual branch moved off the pushed commit to other commits), a robot of what it lands
started meanwhile or a closed pull request of its comment MUST skip the start as `plan_changed`: the toast says what
changed and what was done, with "Start Safe Push" (a new plan, without the push and the cancel it made) and Open in
Patronus of the robot it canceled. A branch rebuilt after the push onto another base with the same commits (the sync of
an Update Project onto the new master: the same changes in the same order, each with its author, time and message)
MUST NOT skip it: the start lands the pushed commits.

#### Scenario: A rewrite while the push sends
- **WHEN** the virtual branch moves off the pushed commit to other commits while the push sends
- **THEN** the start is skipped and the toast says what was done ("the Safe Push it replaced was canceled and the push
  stays")
  [@test] crates/operations/src/land_execute_test.rs (`a_rewrite_while_the_push_sends_skips_the_start_and_the_outcome_tells_what_was_done`)
  [@test] src/presenters/outcome.test.ts (`offers the start again when the facts changed after the push, naming the robot it canceled; a failed effect says its error`)

#### Scenario: A rebuild onto a new baseline while the push sends
- **WHEN** Update Project rebuilds the virtual branch onto the new master with the same commits while the push sends
- **THEN** the start goes on, its robot on the pushed commits, with no "Cannot start"
  [@test] crates/operations/src/land_execute_test.rs (`a_rebuild_onto_a_new_baseline_while_the_push_sends_starts_on_the_pushed_commits`)
  [@test] crates/git-ops/src/rebuilt_branch_test.rs (`the_pushed_commits_copied_onto_another_base_are_the_same_commits`, `a_copy_with_another_message_or_author_is_no_rebuild`, `another_change_or_another_number_of_commits_is_no_rebuild`, `a_parent_of_the_leaf_is_rebuilt_at_the_same_place`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Push whose branch Update Project rebuilt onto the new master while its push sent, on the pushed commits`)

#### Scenario: A robot of what it lands started meanwhile
- **WHEN** a robot of what the start lands started meanwhile
- **THEN** the start is skipped
  [@test] crates/operations/src/land_execute_test.rs (`a_robot_of_what_it_lands_started_meanwhile_skips_the_start`)

### Requirement: Starting a Safe Push or a Dry Run: A start pressed during the sync of a write waits for it

A start pressed while the sync a write of the session asked (Update Project, a rewrite, a fetch) has not ended MUST wait
for that sync, which rebuilds the branches it lands, and start on the table it commits while the user accepted every
confirmation of it, never refused as a plan that changed; its target is fetched meanwhile, once, and it waits for the
facts of the rebuilt branch the click had answered (its review). A start pressed while Update Project fetches, before
the update holds HEAD, MUST wait behind the update all the same, its row "Queued behind Updating the project…", and
Stop MUST end that wait with nothing done; a start that lands by a push into the target does not wait for it, as it
updates after its push.

#### Scenario: A start during the sync of Update Project
- **WHEN** a Safe Push start is pressed while Update Project runs, and the sync it asks rebuilds the branch
- **THEN** the start waits behind the update and for that sync, then pushes the branch it rebuilt once and starts once,
  with no "Cannot start"
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Push pressed during Update Project once the sync of the update rebuilt the branch: one push of the rebuilt branch, one start`)
  [@test] crates/operations/src/update_test.rs (`a_start_during_the_sync_of_a_write_waits_for_it_and_lands_the_branch_it_rebuilt`)
  [@test] crates/operations/src/run_frame_test.rs (`a_land_during_the_sync_of_a_write_waits_for_it`, `an_operation_waits_for_the_follow_up_of_a_write_asked_during_a_running_sync`)
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_wait_for_the_writes_joins_the_run_of_the_write_in_flight`, `a_wait_for_the_writes_joins_the_follow_up_of_a_write_asked_during_a_run_or_a_check`, `a_wait_for_the_writes_without_one_unsynced_is_answered_at_once`, `a_wait_for_the_writes_hears_the_error_of_the_sync_of_the_write`)

#### Scenario: A start during the fetch of Update Project
- **WHEN** a Safe Push start is pressed while Update Project fetches
- **THEN** the start is queued behind the update, nothing pushed or started, then pushes the branch the sync of the
  update rebuilt once and starts once, with no "Cannot start"
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Push pressed during the fetch of Update Project once its sync rebuilt the branch: one push of the rebuilt branch, one start`)
  [@test] crates/operations/src/update_test.rs (`a_start_during_the_fetch_of_an_update_waits_for_it_and_decides_on_the_table_of_its_sync`)

#### Scenario: Stop during the wait behind the fetch
- **WHEN** the user stops a start queued behind the fetch of Update Project
- **THEN** the start ends as stopped with nothing done, and the update goes on
  [@test] crates/operations/src/update_test.rs (`stop_ends_the_wait_of_a_start_behind_the_fetch_of_an_update`)
