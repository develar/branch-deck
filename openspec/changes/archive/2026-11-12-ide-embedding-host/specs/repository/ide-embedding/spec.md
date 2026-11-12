## ADDED Requirements

### Requirement: Tools: The action Sync Virtual Branches is the tool sync

A `tools/call` of `sync` with the origin `action` MUST sync the repository of the main view, as Sync does in the
desktop app. No list of tools names it, and no other origin calls it.

#### Scenario: The shortcut of Sync
- **WHEN** the user presses Sync Virtual Branches in the IDE
- **THEN** brd syncs the repository that the main view shows

### Requirement: Tools: bd_bootstrap gives the settings, the init data and a title bar of the IDE

`bd_bootstrap` MUST give the settings of the window of the instance as `{values, version}`, the warm frame and the
roots of the main view, the data of the open of any other view, and the title bar `native`.

#### Scenario: The settings of the desktop app
- **WHEN** the settings window bootstraps after the desktop app saved a setting
- **THEN** `settings.values` holds the setting
  [@test] crates/mcp-host/src/process_test.rs (`brd_mcp_serves_the_settings_of_the_app_until_its_input_ends`)

### Requirement: View: The demand of a view is the demand of its mirror

`set_demand` of a view MUST name the subscription of the mirror of its instance, which the page does not know.

#### Scenario: A review that demands the details of a robot
- **WHEN** a review tab sets its demand
- **THEN** the session of its repository reads what the demand asks

### Requirement: Windows: A plugin call of the page is the desktop of the IDE

`bd_invoke` of `plugin:window|set_badge_count`, `plugin:window|set_progress_bar` and `plugin:notification|notify`
MUST be `x-air-app/setBadge` and `x-air-app/setProgress` of `main` and `x-air-app/notify`.

#### Scenario: The badge
- **WHEN** the page sets the badge of the Dock to 2
- **THEN** brd asks `setBadge {view: "main", count: 2}`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_change_of_a_setting_is_an_event_and_the_badge_marks_the_tool_window`)

### Requirement: Windows: A second open of a shown view hands it its data

When the IDE answers an open with an instance the window showed already, brd MUST send it the data of the open
(`init-data-updated`) and its title (`setViewTitle`).

#### Scenario: The review of the same branch
- **WHEN** a review tab shows a branch and the user opens its review again
- **THEN** the tab gets `init-data-updated` with the data and the title of the new open
  [@test] crates/test-server/src/mcp_corpus_test.rs (`an_open_of_a_review_is_an_open_view_keyed_by_its_target`)

### Requirement: Repositories: A change of the roots tells the main view

When `roots/list` answers other Git roots than before, brd MUST send the main view the event `roots-changed` with them.

#### Scenario: A root added to the project
- **WHEN** the project gets a second Git root
- **THEN** the main view hears `roots-changed` with both roots
