## MODIFIED Requirements

### Requirement: Before the first sync: A focus syncs nothing before the first sync

A focus of the window and a stamp of `brd` MUST run nothing for a session that does not sync of itself: its repository
was never synced, by the app or by `brd` (the mark of a sync in the worktree), and the user asked no Sync and no
rewrite of it in the session. The first sync of a repository never synced is the user's, Auto-sync on Focus on or off.

#### Scenario: A focus before the first sync
- **GIVEN** a repository never synced, the Ready to Sync card shown, Auto-sync on Focus on
- **WHEN** the window gains the focus
- **THEN** the session runs nothing and answers that it skipped the focus, the card stays and no sync ran
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_or_a_stamp_of_a_session_that_does_not_sync_of_itself_runs_nothing`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_and_a_stamp_sync_a_worktree_never_synced_only_once_brd_synced_it`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs nothing on a focus before the first sync, which is the user's: the Ready to Sync card stays`)

#### Scenario: A focus after the user's first Sync
- **GIVEN** the user synced the repository with the Sync of the Ready to Sync card
- **WHEN** a commit came in a terminal, and the window gains the focus
- **THEN** the session syncs it
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_or_a_stamp_of_a_session_that_does_not_sync_of_itself_runs_nothing`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_after_a_failed_sync_reads_only_the_fingerprint`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs on a focus after the user's first Sync once what a sync reads changed`)

#### Scenario: brd synced the repository
- **GIVEN** a window shows a repository the app never synced
- **WHEN** `brd sync` in a terminal built its branches and touched the stamp of the repository
- **THEN** the session syncs, as the sync of `brd` marked the worktree synced
  [@test] crates/sync-core/src/synced_mark_test.rs (`a_sync_marks_its_worktree_synced_and_one_git_deferred_does_not`)
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_stamp_after_brd_synced_a_worktree_the_app_never_synced_runs`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_and_a_stamp_sync_a_worktree_never_synced_only_once_brd_synced_it`)
  [@test] crates/operations/src/registry_watch_test.rs (`a_stamp_of_bd_syncs_the_session_a_window_shows`)
