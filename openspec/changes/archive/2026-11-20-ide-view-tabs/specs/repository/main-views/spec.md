## ADDED Requirements

### Requirement: In the IDE: Each view is a tab of the tool window

In the IDE (`viewTabs` of the title bar that `brd mcp` gives), Branches, Reviews and Landing SHALL each be a tab of the
tool window, in this order before Activity, and each a page of its own. No page SHALL show the switcher: the tabs of the
IDE name where the user is.

#### Scenario: The tabs of the tool window
- **WHEN** the IDE shows the tool window of Branch Deck
- **THEN** its tabs are Branches (the main view), Reviews, Landing and Activity, and the main view has no switcher
  [@test] tests/e2e/air/air-page.spec.ts (`Branches, Reviews and Landing are tabs of the tool window: the main view has no switch of the views, a view of its palette shows its tab, which reads Reviews while it shows, and a branch of Reviews shows in the tab of the branches`)
  [@test] crates/bd-cli/src/install_test.rs (`the_ide_app_file_is_the_example_of_the_contract_with_the_command_of_this_binary`)

### Requirement: In the IDE: A switch to the view of another tab shows that tab

In the IDE, a switch to a view that another tab shows (the palette, Show Statistics, Show All Reviews, an app command,
the reveal of a branch from Reviews) SHALL ask `brd mcp` for that tab, with an event for its page when the switch asks
it something: `reveal-branch` reveals the branch in the tab of the branches, and `focus-review-link` gives the field of
a link of Reviews the focus. A switch to the own view of the page shows it in the page.

#### Scenario: Reviews of the palette of the main view
- **WHEN** the user picks Reviews in the palette of the main view
- **THEN** the IDE shows the tab of Reviews, and the main view keeps the branches
  [@test] tests/e2e/air/air-page.spec.ts (`Branches, Reviews and Landing are tabs of the tool window: the main view has no switch of the views, a view of its palette shows its tab, which reads Reviews while it shows, and a branch of Reviews shows in the tab of the branches`)
  [@test] src/stores/main-view.test.ts (`asks brd for the tab of a view of another tab, with the event for its page, and shows its own view itself`)

#### Scenario: A branch of Reviews
- **WHEN** the user presses Enter on an own pull request of a branch in the tab of Reviews
- **THEN** the IDE shows the tab of the branches, whose page reveals the branch as Go to of the palette does
  [@test] tests/e2e/air/air-page.spec.ts (`Branches, Reviews and Landing are tabs of the tool window: the main view has no switch of the views, a view of its palette shows its tab, which reads Reviews while it shows, and a branch of Reviews shows in the tab of the branches`)
  [@test] src/stores/main-view.test.ts (`reveals a branch in the tab of the branches, whose page reveals it, and asks no reveal of this page`)

### Requirement: In the IDE: The page of a tab shows the repository of the main view

The page of the tab of Reviews or Landing SHALL show the repository that the main view shows, under its selector, and
hold the session of it and the stores of its facts, as the review window does. It SHALL follow a pick of another
repository in any tab. It SHALL read the rest of Reviews or the statistics while the IDE shows it, and it SHALL mount
nothing that only the main view owns: the Dock, the subscription that starts the sync of a session, the update, the
news of the session, the palette.

#### Scenario: The tab of Reviews
- **WHEN** the IDE shows the tab of Reviews
- **THEN** its page shows the repository of the main view and its sections, and the session reads the rest of Reviews
  [@test] tests/e2e/air/air-page.spec.ts (`Branches, Reviews and Landing are tabs of the tool window: the main view has no switch of the views, a view of its palette shows its tab, which reads Reviews while it shows, and a branch of Reviews shows in the tab of the branches`)
