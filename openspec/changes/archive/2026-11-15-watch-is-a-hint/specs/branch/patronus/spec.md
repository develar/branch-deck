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
- **THEN** the window reads the registry at once, before its next look, and syncs for a stamp whose stat changed
  [@test] crates/operations/src/registry_watch_test.rs (`an_event_that_may_be_a_change_of_the_directory_asks_for_a_look`)
  [@test] crates/operations/src/registry_watch_test.rs (`an_event_of_any_name_of_the_directory_reads_the_registry_and_syncs_for_the_stamp_before_the_next_look`)
  [@test] crates/operations/src/registry_watch_test.rs (`the_watch_of_the_system_tells_that_a_record_left_the_registry_with_no_look`)

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
