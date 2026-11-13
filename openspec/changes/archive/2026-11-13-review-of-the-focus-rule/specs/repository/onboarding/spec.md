## MODIFIED Requirements

### Requirement: Before the first sync: A focus syncs nothing before the first sync

A focus of the window and a stamp of `brd` MUST run nothing for a session that does not sync of itself: its repository
was never synced, by the app or by `brd` (the mark of a sync in the worktree), and the user asked no Sync and no
rewrite of it in the session. The first sync of a repository never synced is the user's, Auto-sync on Focus on or off.
The mark is of the worktree, as `lastSyncTime`, not of a prefix.

#### Scenario: A focus before the first sync
- **GIVEN** a repository never synced, the Ready to Sync card shown, Auto-sync on Focus on
- **WHEN** the window gains the focus
- **THEN** the session runs nothing and answers that it skipped the focus, the card stays and no sync ran
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_or_a_stamp_of_a_session_that_does_not_sync_of_itself_runs_nothing`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_and_a_stamp_sync_a_worktree_never_synced_only_once_brd_synced_it`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs nothing on a focus before the first sync, which is the user's: the Ready to Sync card stays`)

#### Scenario: A stamp of brd before the first sync
- **GIVEN** a repository never synced, the Ready to Sync card shown
- **WHEN** a mutation of `brd` that is no sync of the worktree (an agent's `brd reword`, a `brd lease get` beside it)
  touched the stamp of the repository
- **THEN** the session runs nothing and answers that it skipped the stamp, the card stays and no sync ran
- **AND** after the user's Sync a stamp syncs what a mutation of `brd` changed
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_or_a_stamp_of_a_session_that_does_not_sync_of_itself_runs_nothing`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs nothing on a stamp of brd before the first sync, as on a focus, and what a mutation of brd changed once the user synced`)

#### Scenario: A focus after the user's first Sync
- **GIVEN** the user synced the repository with the Sync of the Ready to Sync card
- **WHEN** a commit came in a terminal, and the window gains the focus
- **THEN** the session syncs it
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_or_a_stamp_of_a_session_that_does_not_sync_of_itself_runs_nothing`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_after_a_failed_sync_reads_only_the_fingerprint`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs on a focus after the user's first Sync once what a sync reads changed`)

#### Scenario: A focus after the user's rewrite
- **GIVEN** the user rewrote branches of a repository never synced, and the sync after the rewrite failed
- **WHEN** the window gains the focus with nothing changed
- **THEN** the session checks what a sync reads, as after the user's Sync, and answers the error of that sync
  [@test] crates/repository-model/src/session_test.rs (`a_focus_after_a_rewrite_whose_sync_failed_checks_the_inputs`)

#### Scenario: A prefix changed after a sync
- **GIVEN** a repository synced under one prefix
- **WHEN** its prefix is another one, never synced
- **THEN** the session of the new prefix syncs as it opens, as the mark and `lastSyncTime` are of the worktree
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs the session of a prefix set in a terminal once the focus finds it`)

#### Scenario: brd synced the repository
- **GIVEN** a window shows a repository the app never synced
- **WHEN** `brd sync` in a terminal built its branches and touched the stamp of the repository
- **THEN** the session syncs, as the sync of `brd` marked the worktree synced
  [@test] crates/sync-core/src/synced_mark_test.rs (`a_sync_marks_its_worktree_synced_and_one_git_deferred_does_not`)
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_stamp_after_brd_synced_a_worktree_the_app_never_synced_runs`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_and_a_stamp_sync_a_worktree_never_synced_only_once_brd_synced_it`)
  [@test] crates/operations/src/registry_watch_test.rs (`a_stamp_of_bd_syncs_the_session_a_window_shows`)

### Requirement: Before the first sync: A focus does not repeat a failed sync while nothing changed

A sync that failed, or that git deferred, MUST NOT be repeated by a focus while what a sync reads is as that sync read
it, with which of the files git keeps during an operation exist (`rebase-merge`, `rebase-apply`, `MERGE_HEAD`,
`CHERRY_PICK_HEAD`, `REVERT_HEAD`, `sequencer`): the focus reads the inputs once and answers the error of that sync, the
alert stays and no run starts, so a sync of a large repository that fails does not run again on every return to the
window.

#### Scenario: A focus after a failed sync with the same inputs
- **GIVEN** a sync failed
- **WHEN** a focus finds the inputs as that sync read them
- **THEN** it reads only the fingerprint, answers the error of that sync, and no run starts
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_after_a_failed_run_with_the_same_inputs_answers_its_error_without_a_run`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_after_a_failed_sync_reads_only_the_fingerprint`)

#### Scenario: A failed sync whose inputs git could not read
- **GIVEN** a sync failed after git could not read what a sync reads
- **WHEN** a focus cannot read them either
- **THEN** it answers the error of that sync and no run starts; a focus that reads them runs
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_after_a_failed_run_without_inputs_answers_its_error_while_git_cannot_read_them`)

#### Scenario: The alert stays until Sync
- **GIVEN** a sync failed and its alert shows
- **WHEN** the window gains the focus with nothing changed
- **THEN** the alert stays and no sync runs, until the user presses Sync
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`does not sync again on a focus after a failed sync while nothing changed, and syncs again on Sync`)

#### Scenario: A run the change of the prefix ended
- **GIVEN** a run ended with `prefix_changed`, which is no failure
- **WHEN** a focus finds the inputs as they were
- **THEN** it answers nothing changed, not that error
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_prefix_change_is_not_remembered`)

### Requirement: Before the first sync: A run under a prefix the config left builds nothing

A prefix changed in a terminal (`git config branchdeck.branchPrefix`) MUST show on the next focus of a session that
syncs of itself, and of a repository never synced on its first Sync: the check reads the prefix among the inputs of a
sync, and the run of the session under the old prefix builds nothing under it: it answers `prefix_changed`, which is
no failure.

#### Scenario: The run of the session under the old prefix
- **GIVEN** the prefix was changed in a terminal
- **WHEN** the session of the old prefix runs
- **THEN** it builds nothing under the old prefix and answers `prefix_changed`
  [@test] crates/repository-model/src/session_test.rs (`a_run_under_a_prefix_the_config_left_builds_nothing`)
  [@test] crates/test-server/src/session_routes_test.rs (`a_prefix_set_in_a_terminal_refuses_the_run_of_the_session_under_the_old_one`)
