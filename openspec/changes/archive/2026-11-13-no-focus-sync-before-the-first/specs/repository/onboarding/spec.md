## ADDED Requirements

### Requirement: Before the first sync: A focus syncs nothing before the first sync

A focus of the window MUST run nothing for a session that was asked no sync of another reason yet: the open of a
repository synced before, the user's Sync, a rewrite, a stamp of `brd`. The first sync of a repository never synced is
the user's, Auto-sync on Focus on or off: the Ready to Sync card stays until its Sync, and a focus from then on syncs
only when the inputs changed.

#### Scenario: A focus before the first sync
- **GIVEN** a repository never synced, the Ready to Sync card shown, Auto-sync on Focus on
- **WHEN** the window gains the focus
- **THEN** the session runs nothing and answers that it skipped the focus, and the card stays
- **AND** after the Sync of the card a focus checks what a sync reads
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_before_the_first_sync_of_a_worktree_never_synced_runs_nothing`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs nothing on a focus before the first sync, which is the user's: the Ready to Sync card stays until its Sync`)
