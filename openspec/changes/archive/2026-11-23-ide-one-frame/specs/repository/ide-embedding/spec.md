## MODIFIED Requirements

### Requirement: Tools: bd_bootstrap gives the settings, the init data and a title bar of the IDE

`bd_bootstrap` MUST give the settings of the window of the instance as `{values, version}`, the warm frame and the
roots of the main view, the data of the open of any other view, and the title bar without updates, without a folder
dialog (`folderDialog: false`) and with a text of the progress in the title of the tool window
(`progressText: true`). It is `overlay` when the page passes an overlay title bar of its host context, else `native`.

#### Scenario: The settings of the desktop app
- **WHEN** the settings window bootstraps after the desktop app saved a setting
- **THEN** `settings.values` holds the setting
  [@test] crates/mcp-host/src/process_test.rs (`brd_mcp_serves_the_settings_of_the_app_until_its_input_ends`)

#### Scenario: A window whose row is in the title bar of the IDE
- **GIVEN** the host context of the Settings window names `{kind: "overlay", height: 40, insetStart: 80}`
- **WHEN** its page calls `bd_bootstrap` with that title bar
- **THEN** the title bar is `{kind: "overlay", doubleClick: "host", height: 40}` with the same flags
- **AND** a place whose host context names no title bar gets `native`
  [@test] crates/mcp-host/src/process_test.rs (`brd_mcp_serves_the_settings_of_the_app_until_its_input_ends`)

### Requirement: Windows: The badge and the progress of the Dock mark the tool window

The Dock badge and the Dock progress MUST be `x-air-app/setBadge` and `x-air-app/setProgress` of the view `main`. In
the IDE the progress MUST name what runs before its percent (`text`): Safe Push or Dry Run of the robot the Dock
follows, Update or Fetch of a running Update Project. A progress without an estimate MUST be the text without
`fraction`, a progress without a percent; `fraction: null` stays the clear.

#### Scenario: A robot that needs the user
- **WHEN** the attention count becomes 2
- **THEN** brd asks `setBadge {view: "main", count: 2}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

#### Scenario: A running Safe Push
- **WHEN** the Safe Push the Dock follows is at 37 %
- **THEN** brd asks `setProgress {view: "main", fraction: 0.37, text: "Safe Push"}`, and the title of the tool window
  reads "Branch Deck: Safe Push 37%"
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/mcp-host/src/server_test.rs (`the_progress_of_the_dock_is_set_progress_of_the_main_view_with_the_text_of_what_runs`)
  [@test] src/lib/dock-text.test.ts (`names what runs before the percent, a progress without an estimate by its text alone, and clears without a text`)

#### Scenario: A Dry Run without an estimate
- **WHEN** the Dock follows a Dry Run that has no estimate left
- **THEN** brd asks `setProgress {view: "main", text: "Dry Run"}` without `fraction`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/mcp-host/src/server_test.rs (`the_progress_of_the_dock_is_set_progress_of_the_main_view_with_the_text_of_what_runs`)
  [@test] src/lib/robot-overview.test.ts (`shows the running robot nearest to finishing with its kind, indeterminate without an estimate, nothing while none runs`)

### Requirement: Page: The page sends initialized once it listens

The page MUST send `ui/notifications/initialized` only once the first render of its page ran its effects, after the
listens of its events. The host delivers an event to a view only after its page sent `initialized`, and holds it
until then, so brd tells an event at once, also to the page of a view that the IDE builds for it.

#### Scenario: A main view never shown
- **WHEN** Show in Branch Deck asks the main view before the IDE built its page
- **THEN** the event `reveal-branch` reaches the page once it listens, and the main view reveals the branch
  [@test] src/lib/page-start.test.ts (`runs each waiter once the page started, and a later one at once`)

### Requirement: Windows: The focus of the main window shows the tool window

`bd_invoke` of `plugin:window|set_focus` of the instance of the main view MUST ask `openView {view: "main"}`, as the
focus brings the main window of the desktop app to the front: the IDE shows the tool window. A focus of another view
does nothing: the IDE shows its place.

#### Scenario: A reveal that Activity asks
- **WHEN** the Activity window or a review asks the main view to reveal a branch
- **THEN** the main view takes the focus of its window, and brd asks `openView {view: "main"}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`a branch that the Activity window reveals shows in the main view: the IDE shows the tool window with the row open`)

## RENAMED Requirements

- FROM: `### Requirement: Windows: The focus of the main window shows the tab of the main view`
- TO: `### Requirement: Windows: The focus of the main window shows the tool window`

## REMOVED Requirements

### Requirement: Windows: A page shows the tab of another view with an event for its page

**Reason**: the tool window has one frame, so no page asks for the tab of another view; Show in Branch Deck and the
focus of the main window show the tool window without the call `plugin:air|show_view`

**Migration**: `Windows: Show in Branch Deck reveals the branch in the main view`, `Windows: The focus of the main window shows the tool window`
