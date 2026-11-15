## MODIFIED Requirements

### Requirement: Report: A sync records the branch only under the lease it began under

A sync of the app's session MUST write the landed state of the worktree only while the slot of a lease pool holds the
lease the session opened under, the lease of its view; a sync of `brd`, Update Project, its undo and a dismiss write it
at any time.

#### Scenario: A sync across a lease change
- **GIVEN** a sync of the app's session began under one lease of the slot
- **WHEN** the slot holds another lease when the sync records where the branch is
- **THEN** the landed state stays as the return of the lease before left it, so the new lease reports nothing landed of
  a branch it never had
  [@test] crates/sync-core/src/lease_state_test.rs (`a_sync_whose_lease_no_longer_holds_writes_no_file_of_the_lease`)
  [@test] crates/repository-model/src/session_test.rs (`a_run_writes_the_files_of_the_lease_only_under_the_lease_it_began_under`)
