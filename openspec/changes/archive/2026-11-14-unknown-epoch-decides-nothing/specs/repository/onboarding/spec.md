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
