## MODIFIED Requirements

### Requirement: Before the first sync: A window on a slot leased again shows the new lease

A session a window shows while its slot of a lease pool is leased again MUST open anew for the new lease, in a new
epoch of its view, and the window MUST show nothing of the rows of the lease before, also of a branch of the same name.

#### Scenario: A window on a slot leased again
- **GIVEN** a window shows a slot of a lease pool the app synced under its lease
- **WHEN** the slot goes back and an agent leases it again, and `brd` touches the stamp
- **THEN** the session opens anew for the new lease by the code of an open, in a new epoch of the view: its table, its
  facts, its sync scheduler and the state of its frames are what an open makes, and the answer of a read of the remote
  services of the lease before applies to it no more
- **AND** each window gets one patch into the new epoch, which applies to no view, and takes the snapshot of the
  session as the view of another session, with no subscription again: the rows and "Last synced" of the lease before
  go, and the Ready to Sync card shows
- **AND** an epoch the files could not tell changes nothing
  [@test] crates/repository-model/src/session_test.rs (`a_stamp_under_another_lease_opens_the_session_anew_for_that_lease`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`shows the Ready to Sync card in the window of a slot once an agent leased the slot again, without the rows and Last synced of the lease before`)

#### Scenario: A stamp of a new lease during a run
- **GIVEN** a run of the session under the lease before is in flight, and a request waits for its follow-up
- **WHEN** the stamp of the new lease comes, with the inputs changed
- **THEN** the run ends for its request and its table goes, the session opens anew for the new lease once it ended,
  the facts of the lease before are read no more from the stamp on, and no recheck of the stamp and no follow-up of the
  session before runs: the waiting request is answered with nothing done
  [@test] crates/repository-model/src/session_test.rs (`a_stamp_under_another_lease_during_a_run_opens_the_session_anew_once_the_run_ended`)

#### Scenario: A run across a lease change writes nothing of the lease
- **GIVEN** a run of the session began under one lease of the slot
- **WHEN** the slot is returned and leased again before the run ended
- **THEN** the run writes no file of the lease of the slot: it marks no worktree synced, so the session of the new lease
  does not sync of itself, and records no landed state, so the new lease reports nothing landed of a branch it never
  had; the sync asks the epoch of the slot right before each write and writes only under the lease its session opened
  under, the lease of its view
- **AND** a run under one lease throughout writes both
  [@test] crates/repository-model/src/session_test.rs (`a_run_writes_the_files_of_the_lease_only_under_the_lease_it_began_under`)
  [@test] crates/sync-core/src/lease_state_test.rs (`a_sync_whose_lease_no_longer_holds_writes_no_file_of_the_lease`, `a_write_under_a_guard_that_no_longer_holds_leaves_the_file_of_the_lease_as_it_is`)
  [@test] crates/operations/src/lease_files_scan_test.rs (`only_their_one_writer_writes_the_files_of_a_lease_and_only_the_modules_of_the_table_name_them`)

#### Scenario: A run that begins under another lease
- **GIVEN** a session opened under one lease of the slot, and the slot holds another lease, its stamp not heard yet
- **WHEN** a sync of the session begins, asked by the user or by a rewrite
- **THEN** it syncs nothing and writes no file of the lease: the request is answered with nothing done, and the session
  opens anew for the new lease, whose sync marks it
  [@test] crates/repository-model/src/session_test.rs (`a_run_of_a_session_whose_slot_holds_another_lease_syncs_nothing_and_the_session_opens_anew`, `a_refused_run_and_a_stamp_at_once_open_the_session_anew_once`)

#### Scenario: A session of a lease the files could not tell
- **GIVEN** a session opened while the epoch of its slot could not be told, or a session whose slot reads no epoch now
- **WHEN** its runs sync
- **THEN** they write no file of a lease and no field of one, as the slot may hold another lease by then; a session of a
  known lease writes again once its slot reads that lease, and opens nothing anew meanwhile
- **AND** a session that opened while the epoch could not be told is of no lease: once its slot reads one, the first
  run or stamp that finds it opens the session anew under that lease, once, and its next sync marks the worktree and
  records the time of the sync, so the slot shows "Last synced" and no Ready to Sync card after a restart
  [@test] crates/repository-model/src/session_test.rs (`a_session_opened_under_an_unknown_epoch_writes_nothing_of_a_lease_and_opens_anew_once_the_slot_reads_one`, `an_epoch_that_cannot_be_read_writes_nothing_and_opens_nothing_anew`)
  [@test] crates/app-settings/src/projects_test.rs (`a_record_of_a_sync_is_of_the_lease_its_session_opened_under`)

#### Scenario: A late record of a sync of the lease before
- **GIVEN** a window synced a slot under one lease, and the slot is returned and leased again
- **WHEN** the window of the lease before writes what it records of the repository after that, with what it read of the
  lease before
- **THEN** the slot of the new lease has nothing of the lease before: the fields of a lease (`lastSyncTime`,
  `lastBranchCount`, `landedHiddenUntil`) are Rust's own record, of the lease it was asked under, which the writer keeps
  whatever a window writes and refuses under another lease; so its session waits for the user's first sync, shows the
  Ready to Sync card and no room for rows of a sync it never had, and its Landed island hides nothing of its own report
  [@test] crates/app-settings/src/projects_test.rs (`a_write_of_a_window_keeps_the_fields_of_the_lease_as_rust_recorded_them`, `a_record_of_a_sync_is_of_the_lease_its_session_opened_under`)
  [@test] crates/app-settings/src/service_test.rs (`every_field_of_the_state_of_a_repository_is_of_its_lease_or_of_its_windows`)
  [@test] crates/repository-model/src/session_test.rs (`a_run_records_the_time_of_its_sync_under_the_lease_its_session_opened_under`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`shows the Ready to Sync card in the window of a slot once an agent leased the slot again, without the rows and Last synced of the lease before`)

#### Scenario: A stamp of a new lease during a check
- **GIVEN** a check of the inputs of a focus of the session under the lease before is in flight, and no run
- **WHEN** the stamp of the new lease comes
- **THEN** the session opens anew at once, the focus is answered with nothing done, and the answer of the check, which
  comes later, runs nothing in the session of the new lease
  [@test] crates/repository-model/src/session_test.rs (`a_stamp_under_another_lease_during_a_check_opens_the_session_anew_at_once`)

#### Scenario: The rows of a slot leased again under the window
- **GIVEN** the user expanded, selected and focused a row of a slot, opened its preview, or a confirm, a popover or a
  dialog of one (a Force Push, Request Review…, a start, Create Pull Request), or collapsed or expanded a side stack
- **WHEN** the window takes the view of the session of a new lease of the slot, whose rows have a branch of the same name
- **THEN** no row is expanded, selected or focused, and the preview, the inline form, the card of a commit, the path of a
  stack, those confirms, popovers and dialogs and the user's choice of the side stacks of the rows before are gone, as
  on a switch: a branch of the same name of another lease is another branch
  [@test] src/model/store.test.ts (`takes the view of a session opened anew by its snapshot, from the patch into its epoch, and forgets the rows it held: a branch of the same name is not selected`)
  [@test] src/stores/branch-preview.test.ts (`closes when the window takes the view of another session of the repository: a branch of the same name there is another branch`)
  [@test] src/stores/view-rows.test.ts (`forget the confirm of a Force Push, whose accept pushes what the session before held`, `close the Request Review… popover of a branch`, `close the popover of a start and forget the clicks that wait for the plan of one`, `close the Create Pull Request dialog of a branch`)
  [@test] src/stores/branch-table-rows.test.ts (`forgets which side stacks the user collapsed or expanded once the window takes the view of another session`)
  [@test] scripts/lint-arch.test.mjs (`a store that names what identifies a row forgets the rows of a view of another session, of its test or test-support not`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`shows the rows of the new lease of a slot an agent leased again under the window, none of them selected, expanded or focused as the row of the same name of the lease before was`)

#### Scenario: Update Project, an undo or a dismiss asked on the view of the lease before
- **GIVEN** the window asked Update Project, the undo of a rewrite or the dismissal of what landed on the view of the
  session of the lease before, or the slot holds another lease than the session opened under
- **WHEN** the intent comes to the engine
- **THEN** it is refused before anything is done (`plan_changed`): no fetch, no move of the branch, the report stays
  [@test] crates/operations/src/update_test.rs (`an_update_a_dismiss_and_an_undo_asked_on_the_view_before_the_session_opened_anew_are_refused`, `an_intent_of_a_slot_leased_again_writes_nothing_into_the_files_of_the_next_lease`)

#### Scenario: An intent of a session that knew no lease of its slot
- **GIVEN** the session of the window opened while the lease of its slot could not be told, and the slot reads one now
- **WHEN** the window asks Update Project, an undo or a dismiss on its view
- **THEN** it is refused before anything is done with a code of its own (`lease_read_again`, exit 3, retried after a
  new read), as the lease may be the same, and the session opens anew under the lease read, without a sync: at once, or,
  while a run is in flight, once that run ended, so the same ask runs on the view the window gets then
- **AND** its words say to ask it again, never to sync first, also of a rewrite, whose plan that changed says to sync
- **AND** a session of another known lease is refused with `plan_changed` and opens anew at the stamp of that lease
  [@test] crates/operations/src/update_test.rs (`an_update_of_a_session_that_knew_no_lease_is_asked_again_on_the_view_of_the_lease_read`)
  [@test] crates/repository-model/src/session_test.rs (`an_intent_of_a_session_that_knew_no_lease_opens_it_anew_under_the_lease_read`, `an_intent_of_a_session_that_knew_no_lease_opens_it_anew_once_the_run_in_flight_ended`)
  [@test] crates/operations/src/witness_test.rs (`a_rewrite_is_synced_before_it_is_asked_again_only_when_its_plan_changed`)

#### Scenario: An intent whose slot is leased again while it runs
- **GIVEN** Update Project, an undo or a dismiss began under the lease of the view it was asked on
- **WHEN** the slot is returned and leased again before it writes the files of the lease
- **THEN** it writes nothing into the files of the next lease: neither what the update landed, nor the report its undo
  restores, nor the cleared report
  [@test] crates/operations/src/update_test.rs (`an_update_whose_slot_changed_its_lease_while_it_waited_records_nothing_of_what_it_landed`, `an_intent_of_a_slot_leased_again_writes_nothing_into_the_files_of_the_next_lease`)
  [@test] crates/repository-model/src/session_test.rs (`an_operation_of_a_session_writes_the_files_of_the_lease_only_under_the_lease_of_its_view`)

#### Scenario: A rewrite asked on the rows of the lease before
- **GIVEN** the window asked a rewrite on the rows of the session of the lease before
- **WHEN** the session opened anew for the new lease before the rewrite runs
- **THEN** the rewrite is refused before anything is written (`plan_changed`), as it names the epoch of the view it was
  asked on; an intent planned on the rows before has no plan of the new lease before its first sync, and a changed one
  after it
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_asked_on_the_rows_of_a_session_before_it_opened_anew_is_refused`)
