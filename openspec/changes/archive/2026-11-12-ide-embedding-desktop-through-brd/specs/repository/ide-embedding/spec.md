## MODIFIED Requirements

### Requirement: Tools: Every tool of brd mcp is only for the app

`tools/list` MUST list `bd_invoke`, `bd_bootstrap` and the tool of each action of the app file, `sync` and
`open_command_palette`. Each has `_meta.ui.visibility: ["app"]`, so no agent sees it.

#### Scenario: The list
- **WHEN** air-ide sends `tools/list`
- **THEN** it gets the four tools, each only for the app
  [@test] crates/mcp-host/src/server_test.rs (`every_tool_is_only_for_the_app_and_each_action_of_the_app_file_has_its_tool`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: The window of a call is the instance of its view

A command MUST run for the window of `_meta["x-air-app"].instance`. A command for its window, such as
`sub_window_loaded`, takes the instance as the window.

#### Scenario: Two editor tabs
- **WHEN** two review tabs call the same command for their window
- **THEN** each gets the answer for its own instance
  [@test] crates/test-server/src/mcp_corpus_test.rs (`two_review_tabs_each_get_the_answer_of_their_own_instance`)

### Requirement: Tools: A cancel stops a read and lets a write end

On `notifications/cancelled`, `brd mcp` MUST stop a read command and give no answer. A command that writes runs to its
end, and its events go to the view and the journal.

#### Scenario: A cancelled diff
- **WHEN** air-ide cancels a running `get_file_diffs`
- **THEN** the command stops, and no answer and no call event of it follow
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_cancelled_read_gives_no_answer_and_no_call_event`)

### Requirement: Tools: The action Sync Virtual Branches is the tool sync

A `tools/call` of `sync` with the origin `action` or `click` MUST sync the repository of the main view, as Sync does in
the desktop app. A call of an agent is refused, as for every tool.

#### Scenario: The shortcut of Sync
- **WHEN** the user presses Sync Virtual Branches in the IDE, or its button in the title of the tool window
- **THEN** brd syncs the repository that the main view shows
  [@test] tests/e2e/air/air-page.spec.ts (`the actions in the title of the tool window: Branch Deck Command Palette opens the palette of the main view, Sync Virtual Branches syncs its repository`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: The action Command Palette opens the palette of the main view

A `tools/call` of `open_command_palette` MUST send the main view the event `open_command_palette`, as the menu of the
desktop app sends it to the main window.

#### Scenario: The button in the title
- **WHEN** the user clicks Branch Deck Command Palette in the title of the tool window
- **THEN** brd sends `x-air-app/event` of `open_command_palette` with the instance of the main view, and the palette
  opens
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`the actions in the title of the tool window: Branch Deck Command Palette opens the palette of the main view, Sync Virtual Branches syncs its repository`)

### Requirement: View: The end of a session ends its mirror

When the session of a repository closes, `brd mcp` MUST send `x-air-app/viewEnded {mirror, reason}` with the reason
`sessionClosed`. brd cannot tell a repository that is gone from a session that closed, so it has no other reason.

#### Scenario: A removed repository
- **WHEN** the repository of a mirror is gone, and its session closes
- **THEN** brd sends `viewEnded` for it with the reason `sessionClosed`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_end_of_a_session_ends_its_mirror_with_the_reason_session_closed`)

### Requirement: Windows: The badge and the progress of the Dock mark the tool window

The Dock badge and the Dock progress MUST be `x-air-app/setBadge` and `x-air-app/setProgress` of the view `main`.

#### Scenario: A robot that needs the user
- **WHEN** the attention count becomes 2
- **THEN** brd asks `setBadge {view: "main", count: 2}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Windows: A plugin call of the page is the desktop of the IDE

`bd_invoke` of `plugin:window|set_badge_count`, `plugin:window|set_progress_bar` and `plugin:notification|notify`
MUST be `x-air-app/setBadge` and `x-air-app/setProgress` of `main` and `x-air-app/notify`. The page calls no tool of
the IDE for them: brd is their one path.

#### Scenario: The badge
- **WHEN** the page sets the badge of the Dock to 2
- **THEN** brd asks `setBadge {view: "main", count: 2}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_change_of_a_setting_is_an_event_and_the_badge_marks_the_tool_window`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Windows: The close of a window of the page closes its place

`bd_invoke` of `plugin:window|close` MUST send `x-air-app/closeView {instance}` with the instance of the window of the
call. So Escape closes a review tab and a dialog, as their hint says.

#### Scenario: Escape in a review tab
- **WHEN** the user presses Escape in a review tab
- **THEN** brd asks `closeView` of its instance, and the IDE closes the tab
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`Escape in a review tab closes its place: brd asks the IDE to close the view of its instance`)

### Requirement: Page: The shim maps the Tauri calls onto x-air-app

The shim of `src/platform/air/` MUST map `invoke` to `bd_invoke`, a `Channel` to the call events and `subscribe_view`
to the mirror. `listen` and `emit` map to the events of x-air-app, and each plugin of Tauri to what the IDE offers in
its place. The page mock of the E2E tests installs the same internals of Tauri (`src/platform/tauri-page.ts`).

#### Scenario: A command with a channel
- **WHEN** the page invokes `get_file_diffs` with a `Channel`
- **THEN** the channel gets each call event of the call, in order, before the promise resolves
  [@test] src/platform/air/shim.test.ts (`A command with a channel: the channel gets each call event of the call, in order, before the promise resolves`)

#### Scenario: The answers of a command
- **WHEN** a command answers a value, raw bytes, an `ApiError`, or the link is lost
- **THEN** the page gets the value, an `ArrayBuffer`, the `ApiError`, or an `internal` error with the reason
  [@test] src/platform/air/shim.test.ts (`A JSON command answers its value, a raw one its bytes, a failed one its ApiError`)

#### Scenario: A resync
- **WHEN** the subscription of the page gets `x-air-app/resync`
- **THEN** the page takes the state as a new snapshot of the view, without a reload
  [@test] src/platform/air/shim.test.ts (`subscribe_view is the mirror of the repository: its state the view, each change a patch, a resync a snapshot without a reload`)

#### Scenario: An event
- **WHEN** `brd mcp` tells an event, and the page emits one to the main window
- **THEN** the listeners of its name hear it, and the emit is `bd_invoke` of `plugin:event|emit` with its target
  [@test] src/platform/air/shim.test.ts (`listen hears the events of x-air-app, and emit goes to the views of the app through brd mcp`)

#### Scenario: The plugins
- **WHEN** the page opens a link, copies, sets the badge or the progress of the Dock, notifies, logs, or sets the size
  of its window
- **THEN** the host gets `ui/open-link`, the tool `air_copy` and `notifications/message`; the badge, the progress and
  the notification are `bd_invoke` of their plugin calls; the size does nothing
  [@test] src/platform/air/shim.test.ts (`The plugins: a link, the clipboard and the log go to the host, the badge, the progress, a notification and the close of the window are bd_invoke of their plugin calls, the size does nothing`)

### Requirement: Contract: The copy of the contract is valid against its schema

Each line of `docs/design/ide-embedding/protocol/corpus/*.ndjson` MUST be valid against `x-air-app.schema.json`, and
each result and action that the line schema takes as any value against the def of its method, as the README of the copy
names them. The example app file and each app file the corpus writes are valid against `air-app.schema.json`; a file whose entry
names a problem of the file (`invalidFile`, `unsupportedFormat`) is not.

#### Scenario: The corpus
- **WHEN** a test reads each corpus file of the copy
- **THEN** each line is valid, and the example app file too
  [@test] src/platform/air/contract.test.ts (`each line of the corpus is valid, each result and action by the def of its method, and each app file the corpus writes`, `the example app file is valid against air-app.schema.json`)
