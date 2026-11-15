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
  had; the sync asks the epoch of the slot right before each write and writes only under the lease the run began under
- **AND** a run under one lease throughout writes both
  [@test] crates/repository-model/src/session_test.rs (`a_run_writes_the_files_of_the_lease_only_under_the_lease_it_began_under`)
  [@test] crates/sync-core/src/lease_state_test.rs (`a_sync_whose_lease_no_longer_holds_writes_no_file_of_the_lease`, `a_write_under_a_guard_that_no_longer_holds_leaves_the_file_of_the_lease_as_it_is`)
  [@test] crates/operations/src/lease_files_scan_test.rs (`only_their_one_writer_writes_the_files_of_a_lease_and_only_the_modules_of_the_table_name_them`)

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

#### Scenario: A rewrite asked on the rows of the lease before
- **GIVEN** the window asked a rewrite on the rows of the session of the lease before
- **WHEN** the session opened anew for the new lease before the rewrite runs
- **THEN** the rewrite is refused before anything is written (`plan_changed`), as it names the epoch of the view it was
  asked on; an intent planned on the rows before has no plan of the new lease before its first sync, and a changed one
  after it
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_asked_on_the_rows_of_a_session_before_it_opened_anew_is_refused`)
