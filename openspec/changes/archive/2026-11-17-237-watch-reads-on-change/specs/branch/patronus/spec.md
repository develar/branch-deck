## MODIFIED Requirements

### Requirement: Pushing into a branch whose Safe Push runs: An operation of another process shows on its row

A Safe Push, a Dry Run or a push that another process of the repository runs (an agent's `brd safe-push auth`, in this
worktree or another one under the same branch prefix) MUST show on its row in every window while it runs: the slot of
the row busy with "brd: Safe Push of auth…", its hover naming the process. The watch of the registry is a hint: a window
MUST show the change within 2 s also when the watch tells nothing, as it looks at the registry every 2 s.

#### Scenario: The Safe Push brd runs
- **GIVEN** an agent runs `brd safe-push auth` in this worktree or another one under the same branch prefix
- **WHEN** a window shows the repository
- **THEN** the slot of the row of auth is busy with "brd: Safe Push of auth…"
- **AND** its hover names the process: "`brd` (pid 4242) runs the Safe Push of auth, which holds what it needs until it ends"
  [@test] crates/operations/src/registry_watch_test.rs (`a_window_shows_the_operations_another_process_runs_under_its_prefix_at_the_next_look_with_no_event_of_the_watch`)
  [@test] src/presenters/operations.test.ts (`words an operation of another process as its row says it and as the hover names its pid`)
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should show the Safe Push bd runs on its row, refuse a start and a push of it, and push another branch beside it`)

#### Scenario: An event of the watch
- **WHEN** the watch tells an event of the directory of the registry: of the registry, of another name of it (the
  temporary file of a rename over the registry), of the directory itself, a lost event or an error
- **THEN** the window looks at once, before its next look: it reads the registry when its stat changed, and syncs for
  a stamp whose stat changed
- **AND** an open, a read or a close without a write of a file of the directory is no event of a change and asks no
  look, as inotify tells every open, the read of the look itself too
  [@test] crates/operations/src/registry_watch_test.rs (`an_event_that_may_be_a_change_of_the_directory_asks_for_a_look`)
  [@test] crates/operations/src/registry_watch_test.rs (`an_event_of_any_name_of_the_directory_reads_the_registry_and_syncs_for_the_stamp_before_the_next_look`)
  [@test] crates/operations/src/registry_watch_test.rs (`the_watch_of_the_system_tells_that_a_record_left_the_registry_with_no_look`)
  [@test] crates/sync-utils/src/dir_watch_test.rs (`an_open_a_read_or_a_close_without_a_write_wakes_no_waiter`)
  [@test] crates/sync-utils/src/dir_watch_test.rs (`the_reads_of_a_file_of_the_directory_wake_no_waiter_on_the_watch_of_the_system`)

#### Scenario: A look of the registry as it was
- **GIVEN** a window shows the session of a repository, and the watch read its registry
- **WHEN** a look finds the registry with the size, the time and the file id it had, as at an event of another file of
  its directory (the leases, the landed commits, a lock file)
- **THEN** the look reads nothing, while a write renamed over the registry is a change whatever its size and its time
- **AND** a session that joins the watch and a window that gains the focus read the registry, a read that failed is
  taken again at the next look, and the watch reads it again at most 2 s after the last read, whatever looked between,
  so an operation whose process ended leaves its row within 2 s though the file is as it was
- **AND** a session that opens after the watch read the registry shows what that read found from the read of its
  subscription
  [@test] crates/operations/src/registry_watch_test.rs (`a_look_reads_the_registry_only_once_it_changed_a_session_joined_or_its_last_read_is_a_recheck_old`)
  [@test] crates/operations/src/registry_watch_test.rs (`a_change_no_stat_tells_shows_a_recheck_after_the_last_read_though_a_look_of_an_event_came_between`)
  [@test] crates/operations/src/registry_watch_test.rs (`a_read_of_the_registry_that_failed_is_taken_again_at_the_next_look`)
  [@test] crates/operations/src/registry_watch_test.rs (`a_read_before_the_session_opened_records_nothing_as_given_so_the_subscription_shows_it`)
  [@test] crates/operations/src/registry_watch_test.rs (`a_focus_looks_at_the_journal_and_the_stamp_too`)
