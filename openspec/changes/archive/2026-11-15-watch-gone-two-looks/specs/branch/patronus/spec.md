## MODIFIED Requirements

### Requirement: Pushing into a branch whose Safe Push runs: A mutation of brd syncs the session a window shows

After a mutation of `brd` (an agent's `brd group`, `brd push`), the session a window shows of a repository synced
before (by the app, or by `brd`, whose sync marks the worktree) MUST sync once what a sync reads changed, without a
focus: `brd` touched the stamp of the repository, which the window watches, and looks at every 2 s when the watch tells
nothing.

#### Scenario: brd touched the stamp of the repository
- **GIVEN** a window shows the session of a repository synced before
- **WHEN** `brd` touched the stamp of the repository after a mutation
- **THEN** the session syncs without a focus, only when what a sync reads changed
  [@test] crates/operations/src/registry_watch_test.rs (`a_stamp_of_bd_syncs_the_session_a_window_shows_at_the_next_look_with_no_event_of_the_watch`)
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_stamp_of_bd_runs_only_when_the_inputs_changed_as_a_focus`)
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should sync when bd touched the stamp of the repository after a mutation, without a focus`)

#### Scenario: The repository went away
- **GIVEN** a window shows the session of a repository, and the repository is deleted (its git directory goes)
- **WHEN** the watch finds its git directory missing at two looks in a row
- **THEN** the sessions of the repository close and the window's subscription ends, and no sync runs, for a stamp
  that went away with it either: a sync would run git in a directory that is gone
  [@test] crates/operations/src/registry_watch_test.rs (`a_look_that_finds_the_repository_gone_closes_its_sessions_and_syncs_nothing`)
  [@test] crates/operations/src/registry_watch_test.rs (`the_sessions_close_once_the_repository_is_missing_at_two_looks_in_a_row_not_at_one`)

#### Scenario: A volume away for a moment
- **GIVEN** a window shows the session of a repository on an external disk or a network share
- **WHEN** a look finds its git directory missing and the next finds it back, or a stat of it fails another way
  (`EACCES`, `EIO`)
- **THEN** the sessions stay, a failed stat is told once in the log, and the next look that can tell reads the registry
  [@test] crates/operations/src/registry_watch_test.rs (`the_sessions_close_once_the_repository_is_missing_at_two_looks_in_a_row_not_at_one`)
  [@test] crates/operations/src/registry_watch_test.rs (`a_stat_error_other_than_not_found_keeps_the_sessions_and_the_next_look_asks_again`)
