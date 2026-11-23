## ADDED Requirements

### Requirement: In the IDE: The tool window shows the main window

In the IDE the tool window SHALL show the main view as the main window of the desktop app: its switcher of Branches,
Reviews and Landing, its Activity button and side panel. A switch, a reveal of a branch and a focus of the field of a
link SHALL happen in that page. Activity SHALL open as a window of the IDE where the frame has no room for the panel.

#### Scenario: The switcher of the tool window
- **WHEN** the IDE shows the tool window of Branch Deck
- **THEN** the main view shows the switcher and the Activity button, and the palette and the switcher show Reviews
  and Landing in the same page, with no other view of the IDE
  [@test] tests/e2e/air/air-page.spec.ts (`Branches, Reviews and Landing switch in the one frame of the tool window, as in the desktop app: the palette shows Reviews in the page, a branch of Reviews shows in its branches, and the IDE shows no view of its own for them`)
  [@test] crates/bd-cli/src/install_test.rs (`the_ide_app_file_is_the_example_of_the_contract_with_the_command_of_this_binary`)

#### Scenario: A branch of Reviews
- **WHEN** the user clicks an own pull request of a branch in Reviews
- **THEN** the main view shows the branches with the row of the branch open, as the desktop app does
  [@test] tests/e2e/air/air-page.spec.ts (`Branches, Reviews and Landing switch in the one frame of the tool window, as in the desktop app: the palette shows Reviews in the page, a branch of Reviews shows in its branches, and the IDE shows no view of its own for them`)

#### Scenario: Add Pull Request by Link…
- **WHEN** the user picks "Add Pull Request by Link…" in the palette
- **THEN** the main view shows Reviews, and its field of a link has the focus
  [@test] tests/e2e/air/air-page.spec.ts (`Add Pull Request by Link… shows Reviews in the main view and focuses its field`)

#### Scenario: Activity of the header
- **WHEN** the user clicks Activity in the row of the main view
- **THEN** the side panel opens in the frame; where the frame has no room for it, the IDE opens the Activity window
  [@test] tests/e2e/air/air-page.spec.ts (`Activity of the header opens the side panel in the frame of the tool window, and the Activity window of the IDE where the frame has no room for the panel`)

#### Scenario: A branch of Activity
- **WHEN** the user clicks a branch in an entry of the Activity window
- **THEN** the IDE shows the tool window, whose main view opens the row of the branch
  [@test] tests/e2e/air/air-page.spec.ts (`a branch that the Activity window reveals shows in the main view: the IDE shows the tool window with the row open`)

## REMOVED Requirements

### Requirement: In the IDE: Each view is a tab of the tool window

**Reason**: the tool window has one frame, the main view with its switcher, as the main window of the desktop app

**Migration**: `In the IDE: The tool window shows the main window`

### Requirement: In the IDE: A switch to the view of another tab shows that tab

**Reason**: no view of the main window is in another tab: every switch happens in the page

**Migration**: `In the IDE: The tool window shows the main window`

### Requirement: In the IDE: The page of a tab shows the repository of the main view

**Reason**: Reviews and Landing are views of the main view, which holds the session of its repository

**Migration**: `In the IDE: The tool window shows the main window`
