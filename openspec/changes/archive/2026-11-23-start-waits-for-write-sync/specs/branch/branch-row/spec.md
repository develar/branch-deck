## MODIFIED Requirements

### Requirement: Status cell: A push pressed during a sync waits for it

The push of a branch MUST stay enabled during a sync: a push pressed during the sync waits for it in the session
(`WaitingForSync`, the operation of the view) with the spinner of a push and pushes the branch as the sync left it, over
the remote status the sync brought; so does one pressed while the sync a write of the session asked waits behind a
focus check or as a follow-up. A branch that moved before its first effect is pushed as the sync that shows it left it.

#### Scenario: A fetch moved the remote branch
- **WHEN** a push is pressed during a sync whose fetch moved the remote branch
- **THEN** it waits for the sync with the spinner of a push
- **AND** it makes one push over its new tip, not a refused one and a second, the row as it was meanwhile
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should push a branch pressed during a sync once, over the remote tip the sync brought, the row as it was meanwhile`)

#### Scenario: Its branch was rebuilt before the push
- **WHEN** another process rebuilt the virtual branch of a push after the sync of the session, before its first effect
- **THEN** the push waits for the sync of the session that shows it and pushes that build once, never refused as a
  plan that changed
  [@test] crates/operations/src/push_test.rs (`a_push_whose_branch_another_process_rebuilt_pushes_that_build_after_the_sync_that_shows_it`)
