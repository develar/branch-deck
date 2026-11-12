---
name: Command Palette
description: Requirements for the command palette of the main window (⌘K) - a search over the views, Activity, the commands of the app and the branches of the table, which runs the chosen one and gives the focus back.
style: plain-1
targets:
  - src/components/command-palette/CommandPalette.tsx
  - src/lib/command-palette.ts
  - src/stores/main-view.ts
  - src/lib/main-menu.ts
  - src/lib/app-commands.ts
  - src-tauri/src/menu.rs
  - src/pages/IndexPage.tsx
  - src/hooks/useActivityPanel.ts
  - src/hooks/useBranchContextActions.ts
  - src/presenters/actions.ts
  - src/stores/commit-move.ts
  - src/components/branch/BranchRow.tsx
  - src/lib/title-bar.ts
  - src-tauri/src/title_bar.rs
  - src/components/repository/RepositorySelector.tsx
  - src/stores/repository.ts
---

# Command Palette

Status: Implemented
Date: 2026-10-22, the commands of the header and of Help and a list that shows them 2026-10-29

## Purpose

The main window has more places than its menus show at a glance: the views, the Activity panel and window, a branch
among dozens. ⌘K opens a dialog with one search over them, over the commands of the app (Open Repository…, Sync,
Update Project, Settings…) and over the actions of the branch at hand, as the palettes of IntelliJ (Search Everywhere) and VS Code do:
type a few letters, press Enter. The actions of a branch are its menu's, as Rust decides them: one builder, one set of
names and reasons.

## Goals

- Every place of the main window and every command of the header and of the app's menu in a few keys, without the
  mouse, and in sight when the palette opens.
- The same names, keys and reasons as the menus: one builder per command ([UX Writing](../../../../docs/contributing/ux-writing.md#actions)).
- The focus back where it was when nothing runs, where the command puts it when one does.

## Non-goals

- No actions of a branch other than the one at hand: a branch elsewhere is a "Go to <branch>" away.
- No palette in the sub-windows: ⌘K of any window opens the palette of the main window.
- No history and no ranking of what was chosen before.

## Requirements

### Requirement: Opening: The native menu and a custom title bar open the palette

View → Command Palette… (`CmdOrCtrl+K`) of the native menu MUST send `open_command_palette` to the main window; ☰ View
→ Command Palette… and Ctrl+K of a custom title bar (`matchMainMenuKey`) MUST run the same app command.

#### Scenario: ⌘K of the native menu
- **WHEN** the user chooses View → Command Palette… (`CmdOrCtrl+K`) of the native menu
- **THEN** it sends `open_command_palette` to the main window, which opens the palette
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from ⌘K of the native menu, list every source and give the focus back on Escape`)
  [@test] src/lib/main-menu.test.ts (`maps the accelerators of the native menu to their commands`, `holds the items of the native menu, File, View and Help`)

#### Scenario: Ctrl+K and ☰ of a custom title bar
- **GIVEN** a custom title bar
- **WHEN** the user presses Ctrl+K (`matchMainMenuKey`) or chooses ☰ View → Command Palette…
- **THEN** the same app command runs and the palette opens
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from Ctrl+K and from View → Command Palette… of ☰, and run Update Project and Check for Updates…`)

### Requirement: Opening: The main window comes to the front with the focus in the search

The main window MUST come to the front and open the palette with the focus in its search.

#### Scenario: The palette opens
- **WHEN** the palette is opened
- **THEN** the main window comes to the front
- **AND** the palette opens with the focus in its search
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from ⌘K of the native menu, list every source and give the focus back on Escape`)
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from Ctrl+K and from View → Command Palette… of ☰, and run Update Project and Check for Updates…`)

### Requirement: Sources: The groups stand in their order

The groups MUST stand in this order (`paletteCommands`): Views, Activity, Commands (Add Pull Request by Link…, Show Disk
Usage…, Open Repository…, then the items of the app's menu), the actions of the branch at hand (below) and Branches.

#### Scenario: The groups of the palette
- **WHEN** the palette opens with a branch at hand
- **THEN** its groups are Views, Activity, Commands, the actions of the branch at hand and Branches, in this order
  [@test] src/lib/command-palette.test.ts (`lists the views, Activity, the commands of the header and of the app's menu and the branches, by namespaced ids`)
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from ⌘K of the native menu, list every source and give the focus back on Escape`)

#### Scenario: The order of Commands
- **GIVEN** a repository selected
- **WHEN** the palette opens
- **THEN** Commands lists Add Pull Request by Link…, Show Disk Usage…, Open Repository…, then the items of the app's menu
  [@test] src/lib/command-palette.test.ts (`offers Show Disk Usage… with a repository, before Open Repository…`)
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should show the views, Activity and every command of the app without a scroll in the window of the start`)

### Requirement: Sources: Views lists the views of MAIN_VIEWS

Views MUST list the views of `MAIN_VIEWS` of `src/stores/main-view.ts` (Branches, Reviews and Landing): a view joins
the palette by its entry there.

#### Scenario: The views
- **WHEN** the palette opens
- **THEN** Views lists Branches, Reviews and Landing, the entries of `MAIN_VIEWS`
  [@test] src/lib/command-palette.test.ts (`lists the views, Activity, the commands of the header and of the app's menu and the branches, by namespaced ids`)

### Requirement: Sources: Activity holds the Activity Panel and the Activity Window

Activity MUST hold Activity Panel, which toggles the side panel as the header button does, and Activity Window.

#### Scenario: The Activity group
- **WHEN** the palette opens
- **THEN** Activity lists Activity Panel, which toggles the side panel as the header button does, and Activity Window
  [@test] src/lib/command-palette.test.ts (`lists the views, Activity, the commands of the header and of the app's menu and the branches, by namespaced ids`)

### Requirement: Sources: Add Pull Request by Link focuses the field of Reviews

Commands MUST hold Add Pull Request by Link…, which shows Reviews with the focus in its field and its text selected
([Reviews](../reviews/spec.md#links)).

#### Scenario: Add Pull Request by Link
- **WHEN** the user runs Add Pull Request by Link…
- **THEN** Reviews shows with the focus in its field and its text selected
  [@test] tests/e2e/reviews/reviews.spec.ts (`should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu`)

### Requirement: Sources: Show Disk Usage opens Disk Usage with a repository selected

With a repository selected, Commands MUST hold Show Disk Usage…, which opens Disk Usage ([Disk
Usage](../disk-usage/spec.md)).

#### Scenario: A repository selected
- **GIVEN** a repository selected
- **WHEN** the palette opens
- **THEN** Commands holds Show Disk Usage…, which opens Disk Usage
  [@test] src/lib/command-palette.test.ts (`offers Show Disk Usage… with a repository, before Open Repository…`)

### Requirement: Sources: Open Repository is the folder button of the selector

Commands MUST hold Open Repository…, the folder button of the repository selector, disabled with "Wait for the sync to
finish" while a sync runs, as the selector is; where the host has no folder dialog (the IDE,
[IDE Embedding](../ide-embedding/spec.md)), neither has it.

#### Scenario: Open Repository during a sync
- **WHEN** a sync runs
- **THEN** Open Repository… is disabled with "Wait for the sync to finish", as the selector is
  [@test] src/lib/command-palette.test.ts (`hints only the keys of the window, disables Sync and Open Repository with the reason, offers updates where the build updates itself`)

#### Scenario: A host without a folder dialog
- **WHEN** the page runs where the host has no folder dialog
- **THEN** Commands holds no Open Repository…
  [@test] src/lib/command-palette.test.ts (`offers no Open Repository… where the host has no folder dialog`)

### Requirement: Sources: The items of the app's menu follow as the menu has them

After Open Repository…, Commands MUST hold the items Sync Branches, Update Project, Settings…, Install Agent CLI…, where
the build updates itself Check for Updates…, Getting Started and GitHub Repository of `mainMenus`, disabled with the
reason where the menu disables them.

#### Scenario: The items of mainMenus
- **WHEN** the palette opens
- **THEN** Commands lists Sync Branches, Update Project, Settings…, Install Agent CLI…, Getting Started and GitHub
  Repository of `mainMenus`, each disabled with the reason where the menu disables it
  [@test] src/lib/command-palette.test.ts (`lists the views, Activity, the commands of the header and of the app's menu and the branches, by namespaced ids`, `hints only the keys of the window, disables Sync and Open Repository with the reason, offers updates where the build updates itself`)

#### Scenario: Check for Updates where the build updates itself
- **GIVEN** a build that updates itself
- **WHEN** the palette opens, on every platform and title bar
- **THEN** Commands holds Check for Updates…, as the native menu has it: `checkForUpdates` of the title bar script
  [@test] src/lib/command-palette.test.ts (`hints only the keys of the window, disables Sync and Open Repository with the reason, offers updates where the build updates itself`)
  [@test] src-tauri/src/title_bar_test.rs (`the_page_gets_one_json_object`)

### Requirement: Sources: No view setting, Auto-sync on Focus or the zoom

Commands MUST NOT hold a view setting, Auto-sync on Focus or the zoom.

#### Scenario: The settings of the menu stay out
- **WHEN** the palette opens
- **THEN** no view setting, no Auto-sync on Focus and no zoom is listed
  [@test] src/lib/command-palette.test.ts (`lists the views, Activity, the commands of the header and of the app's menu and the branches, by namespaced ids`)

### Requirement: Sources: Branches lists a Go to for every branch of the table

Branches MUST list "Go to <branch>" for every branch of the table, in its order.

#### Scenario: The branches of the table
- **WHEN** the palette opens
- **THEN** Branches lists "Go to <branch>" for every branch of the table, in its order
  [@test] src/lib/command-palette.test.ts (`lists the views, Activity, the commands of the header and of the app's menu and the branches, by namespaced ids`)

### Requirement: Sources: The branch at hand is the focused row, else the one selected

The actions of the branch at hand MUST stand between Commands and Branches, the group named by the branch: the branch
whose row had the focus when the palette opened, else the one branch selected; none without one.

#### Scenario: A focused row
- **GIVEN** the focus on the row of a branch
- **WHEN** the palette opens
- **THEN** a group named by the branch, between Commands and Branches, holds its actions
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from ⌘K of the native menu, list every source and give the focus back on Escape`)

#### Scenario: The one branch selected, or none
- **WHEN** no row has the focus as the palette opens
- **THEN** the branch at hand is the one branch selected, and without one there is no such group

### Requirement: Sources: The actions of the branch are the items of its menu

The commands of the branch at hand MUST be the items of its menu ([Branch Row](../../branch/branch-row/spec.md#branch-menu),
`getBranchContextMenuItems`, which presents the actions Rust decides: `domain_policy::actions`), the same names,
disabled with the same reasons; an item of a submenu is named with it. A key of the row is no key of the window: none
is hinted.

#### Scenario: The items of the branch menu
- **WHEN** the palette lists the actions of the branch at hand
- **THEN** they are the items of its menu, the same names, disabled with the same reasons
- **AND** an item of a submenu is named with it ("Patronus › Dry Run", "Copy › Copy Branch Name")
- **AND** no key is hinted
  [@test] src/lib/command-palette.test.ts (`offers the actions of the branch at hand as its menu has them, an item of a submenu named with it, disabled with the reason`)

#### Scenario: An action runs as its menu item does
- **WHEN** the user runs an action of the focused branch
- **THEN** it runs as its menu item does: Review Changes opens its review
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should offer the actions of the focused branch as its menu does and run one: Review Changes opens its review`)

### Requirement: Sources: Every command has an id in the namespace of its source

Every command MUST have an id in the namespace of its source (`view:branches`, `activity:panel`, `app:sync_branches`,
`action:auth:review-changes`, `branch:auth`); an id that a source gave before is left out, so a command shows once.

#### Scenario: Namespaced ids
- **WHEN** the palette lists its commands
- **THEN** each id is in the namespace of its source (`view:branches`, `activity:panel`, `app:sync_branches`,
  `action:auth:review-changes`, `branch:auth`)
  [@test] src/lib/command-palette.test.ts (`lists the views, Activity, the commands of the header and of the app's menu and the branches, by namespaced ids`)

#### Scenario: An id given twice
- **WHEN** a source gives an id that a source gave before
- **THEN** it is left out, so the command shows once
  [@test] src/lib/command-palette.test.ts (`keeps one entry per id, the first source's`)

### Requirement: Sources: A view of the Views shows that view, a Go to shows the branches

Branches of the Views MUST show the branches, Landing the Landing view, Reviews the Reviews view; Go to <branch> from
Landing or Reviews MUST show the branches first.

#### Scenario: Branches of the Views
- **WHEN** the user runs Branches of the Views
- **THEN** the branches show
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should show the branches view`)

#### Scenario: Landing of the Views and a Go to from it
- **WHEN** the user runs Landing of the Views, and then Go to <branch>
- **THEN** the Landing view shows, and the Go to shows the branches first
  [@test] tests/e2e/landing/statistics.spec.ts (`should change the view only by the user: a sync keeps it, a reveal of a branch shows the branches`)

#### Scenario: Reviews of the Views
- **WHEN** the user runs Reviews of the Views
- **THEN** the Reviews view shows
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`, `should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu`)

### Requirement: Sources: A key hint only for a key of the window

A command MUST show a key hint only for a key of the window (⌘R, ⌘T, ⌘,; a view's key once it has one), none for a
branch or Activity.

#### Scenario: The keys of the window
- **WHEN** the palette lists Sync Branches, Update Project and Settings…
- **THEN** they hint their keys ⌘R, ⌘T and ⌘,
  [@test] src/lib/command-palette.test.ts (`hints only the keys of the window, disables Sync and Open Repository with the reason, offers updates where the build updates itself`)
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should show the views, Activity and every command of the app without a scroll in the window of the start`)

#### Scenario: A branch and Activity
- **WHEN** the palette lists a Go to of a branch or a command of Activity
- **THEN** it hints no key
  [@test] src/lib/command-palette.test.ts (`hints only the keys of the window, disables Sync and Open Repository with the reason, offers updates where the build updates itself`)
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from ⌘K of the native menu, list every source and give the focus back on Escape`)

### Requirement: Sources: The list shows the views and every command without a scroll

The list MUST be as high as the window lets it below the search, at most 36rem and 4rem short of the 80% of the window
under the top of the palette: in the window of the start (1024×900, and down to 720 px high) the views, Activity and
every command show without a scroll, the actions of the branch and the branches below them; a lower window scrolls the
rest. A list cut above them hides them: the scroll bars of macOS show only while a list scrolls.

#### Scenario: The window of the start
- **GIVEN** the window of the start, 1024×900, and down to 720 px high
- **WHEN** the palette opens
- **THEN** the views, Activity and every command show without a scroll, the actions of the branch and the branches
  below them
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should show the views, Activity and every command of the app without a scroll in the window of the start`)

#### Scenario: A lower window
- **WHEN** the window is lower
- **THEN** the list scrolls the rest

### Requirement: Sources: The search keeps the commands whose label has the text

The search MUST keep the commands whose label has the typed text, ignoring case (the filter of Base UI `Combobox`), the
first one highlighted; Enter or a click runs it. Nothing kept: "No command or branch matches".

#### Scenario: A typed text and Enter
- **WHEN** the user types a few letters, in any case, and presses Enter
- **THEN** the commands whose label has them are kept, the first one highlighted, and Enter runs it
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should go to a branch: the row expanded, selected and focused`)

#### Scenario: A click
- **WHEN** the user clicks a kept command
- **THEN** it runs
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open the Activity panel with the focus in it from the keyboard, and the Activity window by a click`)

#### Scenario: Nothing kept
- **WHEN** no label has the typed text
- **THEN** the list says "No command or branch matches"

### Requirement: Sources: A command that starts with the typed text comes first

A command that starts with the typed text MUST come first, in its group and its group before the others
(`paletteGroups`): its label starts with it, or the name of the branch of a Go to does.

#### Scenario: The order of the groups and their commands
- **WHEN** the user types a text
- **THEN** a command whose label starts with it, or a Go to whose branch's name does, comes first in its group, and its
  group before the others
  [@test] src/lib/command-palette.test.ts (`lists the groups in their order without a text, a command and its group that start with the text first`)

### Requirement: Sources: The name of a branch and Enter go to that branch

The name of a branch typed and Enter MUST go to that branch, never to an action of the branch at hand whose label
names it ("Push session Only", "Stack On › session"): that one came first in the group order and Enter pushed or
restacked instead of going.

#### Scenario: An action that names the branch
- **GIVEN** the branch at hand has an action whose label names another branch ("Push session Only", "Stack On › session")
- **WHEN** the user types the name of that branch and presses Enter
- **THEN** the palette goes to that branch, and nothing is pushed or restacked
  [@test] src/lib/command-palette.test.ts (`lists the groups in their order without a text, a command and its group that start with the text first`)
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should go to the branch a search names before an action that names it, and focus its row in a collapsed stack`)

### Requirement: Running: A command runs once the palette closed and the focus went back

A command MUST run once the palette closed and the focus went back where it was, so what the command focuses keeps the
focus.

#### Scenario: What a command runs
- **WHEN** the user runs a command of the palette
- **THEN** it runs what its source runs
  [@test] src/lib/command-palette.test.ts (`runs what its source runs`)

#### Scenario: What the command focuses keeps the focus
- **WHEN** the user runs a command that focuses something, as "Go to <branch>" focuses its row
- **THEN** the palette closes and the focus goes back where it was before the command runs
- **AND** what the command focuses keeps the focus
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should go to a branch: the row expanded, selected and focused`)

### Requirement: Running: Go to a branch reveals, selects and focuses its row

"Go to <branch>" MUST show the branches, expand the row, scroll it into view (`requestReveal`), select it and focus it,
also a row a collapsed stack or the filter hid, which takes the focus as the reveal renders it (else the focus stayed
where it was, a push button of another row too).

#### Scenario: Go to a collapsed row
- **WHEN** the user runs "Go to <branch>" of a collapsed row
- **THEN** the branches show, and the row is expanded, scrolled into view (`requestReveal`), selected and focused
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should go to a branch: the row expanded, selected and focused`)

#### Scenario: A row a collapsed stack hid
- **GIVEN** the row of the branch hidden by a collapsed stack or the filter, and the focus on a push button of another row
- **WHEN** the user runs "Go to <branch>"
- **THEN** the row takes the focus as the reveal renders it
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should go to the branch a search names before an action that names it, and focus its row in a collapsed stack`)

### Requirement: Running: Activity Panel by Enter focuses its log, Activity Window closes the panel

Activity Panel chosen by Enter MUST open the panel with the focus in its log (a keyboard open,
[Activity](../activity/spec.md#side-panel)), by a click without; Activity Window MUST open the window and close the
panel.

#### Scenario: Activity Panel by Enter
- **WHEN** the user chooses Activity Panel by Enter
- **THEN** the panel opens with the focus in its log
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open the Activity panel with the focus in it from the keyboard, and the Activity window by a click`)

#### Scenario: Activity Panel by a click
- **WHEN** the user chooses Activity Panel by a click
- **THEN** the panel opens without the focus in its log

#### Scenario: Activity Window
- **WHEN** the user chooses Activity Window
- **THEN** the window opens and the panel closes
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open the Activity panel with the focus in it from the keyboard, and the Activity window by a click`)

### Requirement: Running: A command of the app runs as its menu item does

A command of the app MUST run as its menu item does (`onSelect` of the item of `mainMenus`), Open Repository… as the
folder button does (`browseRepository`: the folder dialog, then the repository picked in it).

#### Scenario: Sync Branches, Settings and Install Agent CLI
- **WHEN** the user runs Sync Branches, Settings… or Install Agent CLI…
- **THEN** it runs as its menu item does (`onSelect` of the item of `mainMenus`)
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should run the commands of the app: Sync Branches, Settings… and Install Agent CLI…`)

#### Scenario: Update Project and Check for Updates
- **WHEN** the user runs Update Project or Check for Updates…
- **THEN** it runs as its menu item does
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from Ctrl+K and from View → Command Palette… of ☰, and run Update Project and Check for Updates…`)
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should check for updates where the build updates itself beside the traffic lights, as the app menu does`, `should check for updates where the build updates itself under the system title bar, as its Help menu does`)

#### Scenario: Open Repository and the help of the app
- **WHEN** the user runs Open Repository…, Getting Started or GitHub Repository
- **THEN** Open Repository… runs as the folder button does (`browseRepository`: the folder dialog, then the repository
  picked in it), and the help items as the Help menu does
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open a repository and the help of the app as the header and the Help menu do`)

### Requirement: Running: Escape or a click outside closes the palette and gives the focus back

Escape or a click outside MUST close the palette with nothing run and give the focus back to the element that had it
(the element itself: a row is focusable, not tabbable).

#### Scenario: Escape
- **GIVEN** the focus on a row as the palette opened
- **WHEN** the user presses Escape
- **THEN** the palette closes with nothing run, and the row itself has the focus
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from ⌘K of the native menu, list every source and give the focus back on Escape`)

#### Scenario: A click outside
- **WHEN** the user clicks outside the palette
- **THEN** it closes with nothing run and gives the focus back to the element that had it

## User Experience

- A `Dialog` at 20% of the window height, `sm:max-w-lg`, no close button: the search (`ComboboxInput`, "Type a command
  or a branch", named "Command or branch"), then the list (as high as the window lets it, scrolling) with the group labels and per item its
  icon, its label, the reason of a disabled one muted, and its key hint at the end (`KbdGroup`). The title "Command
  Palette" is for the screen reader only.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/command-palette.test.ts` (the sources, the ids, the keys), `src/lib/main-menu.test.ts`
  (the item and Ctrl+K).
- `pnpm e2e`: `tests/e2e/command-palette/command-palette.spec.ts` with the templates `issue_links` and
  `behind_upstream`. The palette is `command-palette`; the native item is `emitTauriEvent(page, "open_command_palette")`.

## References

- [Title Bar](../title-bar/spec.md): the menus and their keys.
- [Activity](../activity/spec.md): the side panel and the window.
