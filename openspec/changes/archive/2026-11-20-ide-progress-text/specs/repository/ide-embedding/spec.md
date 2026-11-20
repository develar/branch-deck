## MODIFIED Requirements

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
- **THEN** brd asks `setProgress {view: "main", fraction: 0.37, text: "Safe Push"}`, and the tab of the main view reads
  "Branches Safe Push 37%"
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/mcp-host/src/server_test.rs (`the_progress_of_the_dock_is_set_progress_of_the_main_view_with_the_text_of_what_runs`)
  [@test] src/lib/dock-text.test.ts (`names what runs before the percent, a progress without an estimate by its text alone, and clears without a text`)

#### Scenario: A Dry Run without an estimate
- **WHEN** the Dock follows a Dry Run that has no estimate left
- **THEN** brd asks `setProgress {view: "main", text: "Dry Run"}` without `fraction`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/mcp-host/src/server_test.rs (`the_progress_of_the_dock_is_set_progress_of_the_main_view_with_the_text_of_what_runs`)
  [@test] src/lib/robot-overview.test.ts (`shows the running robot nearest to finishing with its kind, indeterminate without an estimate, nothing while none runs`)
