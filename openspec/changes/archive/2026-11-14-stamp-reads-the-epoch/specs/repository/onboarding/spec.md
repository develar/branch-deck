## MODIFIED Requirements

### Requirement: Before the first sync: What the app keeps of a slot of a lease pool is of its lease

What the app keeps of a worktree by its path MUST be of the epoch it was recorded under (`worktree_epoch`: the lease a
slot of a lease pool holds now), and a record of another known epoch MUST count as absent: the entry of
`repositoryState`, which the writer forgets, the frame of the session, which seeds nothing, and a session kept idle,
which a window that comes back does not join. An epoch the files cannot tell MUST decide nothing.

#### Scenario: The epoch of a slot
- **WHEN** a slot is leased, returned and leased again
- **THEN** its epoch is the lease it holds, another after each return and get; the main checkout has none
  [@test] crates/bd-lease/src/lease_test.rs (`the_epoch_of_a_slot_is_its_lease_and_changes_with_each_return_and_get`)

#### Scenario: The state of a slot of an earlier lease
- **GIVEN** the app synced a slot under one lease
- **WHEN** the slot holds another lease
- **THEN** the writer forgets the entry of the slot before the key leaves it, and the slot was not synced before
- **AND** an entry of a path outside a pool and one of the lease now stay
  [@test] crates/app-settings/src/projects_test.rs (`an_entry_of_another_epoch_of_its_worktree_is_forgotten_before_the_key_leaves_the_writer`)

#### Scenario: The frame and the kept session of an earlier lease
- **GIVEN** a frame or a session kept idle of a slot under one lease
- **WHEN** a window opens the slot under another lease
- **THEN** the frame seeds nothing and the window gets a new session, without the rows of the lease before
  [@test] crates/repository-model/src/session_test.rs (`a_frame_of_an_earlier_lease_of_the_slot_seeds_nothing`, `a_window_that_comes_back_to_a_session_kept_under_an_earlier_lease_opens_a_new_one`)

#### Scenario: The window opens a slot of a new lease
- **GIVEN** the app synced a slot an hour ago, and the slot went back and an agent leased it again
- **WHEN** the window opens it
- **THEN** the Ready to Sync card shows, no sync runs, and the header says no "Last synced"
- **AND** a slot the app synced under the lease it holds syncs as the window opens it, "Last synced" in the header
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`shows the Ready to Sync card for a slot of the lease pool the app synced under an earlier lease, and no Last synced`, `syncs a slot of the lease pool the app synced under its lease now when the window opens it, and says when`)

#### Scenario: An epoch the files cannot tell
- **GIVEN** the app recorded a sync of a slot under its lease
- **WHEN** the state of the pool cannot be read or parsed
- **THEN** the entry stays, in memory and on disk, and a write meanwhile keeps the epoch it names
- **AND** the frame seeds nothing then and is saved under the epoch it names, and a session kept idle stays
  [@test] crates/app-settings/src/projects_test.rs (`an_entry_whose_epoch_cannot_be_read_stays_as_it_is_and_on_disk`)
  [@test] crates/repository-model/src/session_test.rs (`a_frame_keeps_its_epoch_while_the_epoch_of_the_worktree_cannot_be_read`, `a_kept_session_stays_while_the_epoch_of_the_worktree_cannot_be_read`)

#### Scenario: The collapsed stacks of a slot leased again
- **GIVEN** stacks were collapsed in the table of a slot under its lease
- **WHEN** the slot holds another lease
- **THEN** the writer forgets those roots, nothing else of the project, and keeps them under an epoch the files could
  not tell
  [@test] crates/app-settings/src/projects_test.rs (`the_collapsed_stacks_of_a_slot_are_forgotten_under_another_lease_and_kept_under_an_unknown_one`)

## ADDED Requirements

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

#### Scenario: The rows of a slot leased again under the window
- **GIVEN** the user expanded, selected and focused a row of a slot, or opened its preview
- **WHEN** the window takes the view of the session of a new lease of the slot, whose rows have a branch of the same name
- **THEN** no row is expanded, selected or focused, and the preview, the inline form, the card of a commit and the path
  of a stack of the rows before are gone, as on a switch: a branch of the same name of another lease is another branch
  [@test] src/model/store.test.ts (`takes the view of a session opened anew by its snapshot, from the patch into its epoch, and forgets the rows it held: a branch of the same name is not selected`)
  [@test] src/stores/branch-preview.test.ts (`closes when the window takes the view of another session of the repository: a branch of the same name there is another branch`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`shows the rows of the new lease of a slot an agent leased again under the window, none of them selected, expanded or focused as the row of the same name of the lease before was`)
