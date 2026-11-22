## MODIFIED Requirements

### Requirement: Tools: bd_bootstrap gives the settings, the init data and a title bar of the IDE

`bd_bootstrap` MUST give the settings of the window of the instance as `{values, version}`, the warm frame and the
roots of the main view, the data of the open of any other view, and the title bar without updates, without a folder
dialog (`folderDialog: false`), with the views as tabs (`viewTabs: true`) and with a text of the progress
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

## ADDED Requirements

### Requirement: Page: The IDE moves a window whose row is in its title bar

Where the host context names an overlay title bar, a left press on the empty parts of the row MUST send
`x-air-app/windowDrag {clickCount}`, 2 for the second press of a double click, and the IDE moves the window.
`is_fullscreen` MUST be true when the IDE names the inset 0. A place with a native title bar MUST send no `windowDrag`.

#### Scenario: A drag and a double click of the row of Settings
- **WHEN** the user presses the empty part of the row of the Settings window, then double-clicks it
- **THEN** the IDE gets `windowDrag` with the click count 1, then 1 and 2
  [@test] tests/e2e/air/air-page.spec.ts (`the row of the Settings window is in the title bar of the IDE: it starts after the traffic lights, a press on its empty parts moves the window and a double click is the double click of the system, full screen moves it to the edge, and the row of the tool window is no drag region`)
  [@test] src/platform/air/shim.test.ts (`A window whose row is in the title bar of the IDE: a drag and a double click of the row are windowDrag, its full screen is what the IDE says, and a change of the title bar resizes the window for the page`)

#### Scenario: Full screen
- **WHEN** the IDE enters full screen and names the inset 0 in `host-context-changed`
- **THEN** the window resizes for the page, and the row starts at the edge
- **AND** once the IDE names the inset of the traffic lights again, the row starts after them
  [@test] tests/e2e/air/air-page.spec.ts (`the row of the Settings window is in the title bar of the IDE: it starts after the traffic lights, a press on its empty parts moves the window and a double click is the double click of the system, full screen moves it to the edge, and the row of the tool window is no drag region`)
  [@test] src/platform/air/shim.test.ts (`A window whose row is in the title bar of the IDE: a drag and a double click of the row are windowDrag, its full screen is what the IDE says, and a change of the title bar resizes the window for the page`)

#### Scenario: A tool window
- **GIVEN** the host context of the main view names no title bar
- **WHEN** the page asks to drag or to zoom its window
- **THEN** the IDE gets no `windowDrag`, and the row of the tool window is no drag region
  [@test] src/platform/air/shim.test.ts (`A tool window, an editor tab or a dialog draws its own title: the page moves no window of the IDE`)
  [@test] tests/e2e/air/air-page.spec.ts (`the row of the Settings window is in the title bar of the IDE: it starts after the traffic lights, a press on its empty parts moves the window and a double click is the double click of the system, full screen moves it to the edge, and the row of the tool window is no drag region`)

#### Scenario: The app file
- **WHEN** the install writes the app file
- **THEN** the Settings window names `titleBar: overlay`, and the other places name no title bar
  [@test] crates/bd-cli/src/install_test.rs (`the_ide_app_file_is_the_example_of_the_contract_with_the_command_of_this_binary`)
