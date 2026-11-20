## ADDED Requirements

### Requirement: Windows: A page shows the tab of another view with an event for its page

`bd_invoke` of `plugin:air|show_view {view, event?, payload?}` MUST ask `openView {view}` of the IDE and answer at
once; once the IDE answers the instance, brd MUST tell that instance `event` with `payload`, when the call names one.
The page of the IDE asks it for the tab of another view of the main window (`showMainView`); Show in Branch Deck of a
notification is the same show of `main` with `reveal-branch`. An event reaches a page that runs: a tab that the IDE
builds for the open loads without it.

#### Scenario: A branch of Reviews
- **WHEN** the tab of Reviews asks `plugin:air|show_view {view: "main", event: "reveal-branch", payload}`
- **THEN** brd asks `openView {view: "main"}`, answers the call, and tells the instance the IDE answered
  `reveal-branch` with the payload
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

## MODIFIED Requirements

### Requirement: View: The demand of a view is the demand of its mirror

`set_demand` of a view MUST name the subscription of the mirror of its instance, which the page does not know. The
views of a mirror share its subscription, so brd MUST keep the demand of each view and send the session the union of
them (`Demand::union`, as the session joins the demands of its subscriptions); the view that leaves the mirror takes
only its own demand with it.

#### Scenario: A review that demands the details of a robot
- **WHEN** a review tab sets its demand
- **THEN** the session of its repository reads what the demand asks

#### Scenario: The main view and the tab of Reviews
- **WHEN** the tab of Reviews demands Reviews and the main view then sends its demand of the branches
- **THEN** the session reads Reviews and the branches; once the tab leaves, only the branches
  [@test] crates/mcp-host/src/server_test.rs (`the_session_of_a_mirror_gets_the_union_of_the_demands_of_its_views`)
  [@test] tests/e2e/air/air-page.spec.ts (`Branches, Reviews and Landing are tabs of the tool window: the main view has no switch of the views, a view of its palette shows its tab, which reads Reviews while it shows, and a branch of Reviews shows in the tab of the branches`)

### Requirement: Tools: bd_bootstrap gives the settings, the init data and a title bar of the IDE

`bd_bootstrap` MUST give the settings of the window of the instance as `{values, version}`, the warm frame and the
roots of the main view, the data of the open of any other view, and the title bar `native`, without updates, without
a folder dialog (`folderDialog: false`) and with the views of the main window as tabs (`viewTabs: true`).

#### Scenario: The settings of the desktop app
- **WHEN** the settings window bootstraps after the desktop app saved a setting
- **THEN** `settings.values` holds the setting
  [@test] crates/mcp-host/src/process_test.rs (`brd_mcp_serves_the_settings_of_the_app_until_its_input_ends`)
