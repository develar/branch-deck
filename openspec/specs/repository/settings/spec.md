---
name: Settings
description: Requirements for the Settings window, a sub-window of its own with a sidebar of pages in two groups (Branch Deck - General, Appearance, Diffs, Notifications, Accounts, Tools; Repositories - Branches, Landing, Agents), the pages of the git config under the scope tabs All repositories | the selected repository, opened by the main window from the menu, the keys and the buttons that name a page; the order of the branch prefixes.
style: plain-1
targets:
  - src-tauri/src/menu.rs
  - src-tauri/src/lib.rs
  - src-tauri/src/settings.rs
  - crates/app-api/src/app/settings.rs
  - crates/app-api/src/events.rs
  - crates/app-settings/src/service.rs
  - crates/app-settings/src/file.rs
  - crates/app-settings/src/merge.rs
  - crates/app-settings/src/watch.rs
  - crates/app-settings/src/lib.rs
  - crates/bd-core/src/context.rs
  - src-tauri/src/title_bar.rs
  - crates/test-server/src/state.rs
  - crates/test-server/src/host.rs
  - src/stores/settings/repositoryState.ts
  - src-tauri/src/zoom.rs
  - crates/app-api/src/app/windows.rs
  - src-tauri/src/desktop.rs
  - src-tauri/capabilities/settings.json
  - src-tauri/capabilities/sub-windows.json
  - src-tauri/capabilities/title-bar.json
  - src/lib/settings-window.ts
  - src/lib/settings-pages.ts
  - src/lib/zoom.ts
  - src/lib/sub-window.ts
  - src/lib/main-menu.ts
  - src/lib/menu.ts
  - src/lib/pull-request.ts
  - src/router.ts
  - src/pages/SettingsPage.tsx
  - src/pages/IndexPage.tsx
  - src/components/ui/tabs.tsx
  - src/components/settings/SettingRow.tsx
  - src/components/settings/ScopedConfigRow.tsx
  - src/components/settings/ScopeTabs.tsx
  - src/components/settings/ConfigControls.tsx
  - src/components/settings/AccountsPage.tsx
  - src/components/settings/BranchesPage.tsx
  - src/hooks/useScopedConfig.ts
  - src/hooks/useSettingsScope.ts
  - src/hooks/useConfigDraft.ts
  - src/lib/settings-close.ts
  - src/components/settings/GeneralPage.tsx
  - src/components/settings/AppearancePage.tsx
  - src/components/settings/DiffsPage.tsx
  - src/components/settings/AgentsPage.tsx
  - src/components/shared/InstallAgentCliButton.tsx
  - src/hooks/useAgentCliStatus.ts
  - src/components/settings/ToolsPage.tsx
  - src/hooks/useConfigWrite.ts
  - src/components/shared/DiffViewToggle.tsx
  - src/components/settings/NotificationsPage.tsx
  - src/components/settings/GitHubSettings.tsx
  - src/components/settings/YouTrackSettings.tsx
  - src/stores/auth.ts
  - src/stores/youtrack-issues.ts
  - src/stores/cli-tools.ts
  - src/components/settings/LandingPage.tsx
  - src/hooks/useRepositoryConfigChanges.ts
  - src/components/settings/BranchPrefixSetting.tsx
  - src/components/repository/YourTurnCard.tsx
  - src/components/repository/ConfigurationHeader.tsx
  - src/components/repository/BranchPrefixHelp.tsx
  - src/components/shared/ShortcutHint.tsx
  - src/hooks/useZoom.ts
  - src/hooks/useTheme.ts
  - src/lib/theme.ts
  - src/index.css
  - src/stores/ai-suggestions.ts
  - src/stores/settings/appSettings.ts
  - src/stores/settings/repositorySettings.ts
  - src/stores/settings/persistent-store.ts
  - src/stores/repository.ts
  - crates/sync-core/src/branch_prefix.rs
  - crates/app-settings/src/resolve.rs
  - crates/app-settings/src/branch_prefixes.rs
  - crates/app-api/src/context.rs
  - crates/app-api/src/reads/repository.rs
  - crates/app-settings/src/windows.rs
  - crates/domain-types/src/settings.rs
  - crates/sync-core/src/repository_config.rs
  - crates/sync-core/src/config_snapshot.rs
  - crates/app-api/src/app/repository_config.rs
  - src/lib/repository-config.ts
  - src/lib/branch-prefix.ts
  - src/hooks/useRepositoryConfig.ts
  - src/hooks/useStackIndependence.ts
  - src/components/branch/SafePushPopover.tsx
  - tests/e2e/setup/tauri-mocks.ts
  - tests/e2e/helpers/settings-helpers.ts
  - tests/e2e/helpers/sub-window-helpers.ts
---

# Settings

Status: Implemented
Date: 2026-10-17

## Purpose

Settings is a window of its own, as in IntelliJ and the macOS apps: a sidebar of pages on the canvas beside one island
with the page. The sidebar has two groups: "Branch Deck", the settings of the app (General, Appearance, Diffs,
Notifications, Accounts, Tools), and "Repositories", the `branchdeck.*` git config that the app and `brd` read
(Branches, Landing, Agents). A page of the git config has the scope tabs "All repositories | <repository>": the value
every repository uses, written to the global git config, and the selected repository's own, written to its
`.git/config`. It is not modal: the main window stays usable, and a change applies at once in every window. The main
window opens it, from the menu (⌘,), the keys of a custom title bar and the buttons that need a setting ("Notification
Settings…", "Sign In…", "Set GitHub Repository…"), which name the page they need.

The pages were fitted to 720 × 540 without scrolling, so every setting that did not fit got a page of its own (11 of
them), and three pages showed where a value lives in three ways: a switch that wrote wherever the value was, a global
switch with "In <repository>: Like Global · On · Off" under it, a row of `.git/config` only. Nobody could tell what
held in a repository. One model now: a tab names the scope, a row shows the value of that tab and says only what
differs (the repository's own value, with Reset).

## Goals

- Every setting of the app in one place, one page per subject, each page in its usual state without scrolling at the
  size the window opens with.
- One way to see and change where a value of the git config lives: the scope tab; a row says what holds there.
- A button that needs a setting opens the page, the scope and the field that have it.
- The same settings in every window: every window writes them through the backend, which owns `settings.json`.
- `settings.json` changes only when the user changes a setting or the list of projects: what a sync, a fetch or Update
  Project records of a repository lives beside it.

## Non-goals

- A search over the settings, Reset All.
- A write of the worktree or the system git config. "All repositories" writes the global config, the tab of the
  repository its `.git/config`; a row says where a value of another config comes from.
- A route into master for every repository: a route is the repository's own.
- A settings dialog inside another window.

## Requirements

### Requirement: The window: Settings is a sub-window that opens at 880 × 640 every time

Settings SHALL be the sub-window `settings` at `/settings` (`SettingsPage`, lazy), titled "Settings", 880 × 640 every
time it opens (`SETTINGS_WINDOW_SIZE`): the window-state plugin MUST NOT restore its size (`with_denylist` in
`src-tauri/src/lib.rs`), so a size saved by a build of a smaller window never keeps its pages cramped.

#### Scenario: A size saved by a build of a smaller window
- **GIVEN** the window-state plugin saved a size of the Settings window of a build of a smaller window
- **WHEN** Settings opens
- **THEN** it opens at 880 × 640 (`SETTINGS_WINDOW_SIZE`), the size of the window-state plugin not restored (`with_denylist` in `src-tauri/src/lib.rs`)

### Requirement: The window: Its minimum is 640 × 440 and a zoom above 107% widens it

The minimum of the window SHALL be 640 × 440, where a row puts its control under its label (a container query); a page
needs 600 CSS px, so a zoom above 107% MUST widen it: `max(640, 600 × level)` (`sub_window_min_size` in
`src-tauri/src/zoom.rs`, `minSubWindowSize` in `src/lib/zoom.ts`, which `useZoom` sets by the label of the window).

#### Scenario: The minimum at a zoom up to 107% and above it
- **WHEN** the zoom is at most 107%
- **THEN** the minimum of the Settings window is 640 × 440
- **AND** above 107% its minimum width is `max(640, 600 × level)`
  [@test] src-tauri/src/zoom_test.rs (`the_settings_window_is_smaller_and_widens_above_107_percent`)
  [@test] src/lib/zoom.test.ts (`is smaller for Settings, and wider above 107%`)

### Requirement: The window: Only the main window opens it, and every entry point asks it

Only the main window SHALL open Settings (`openSettingsWindow` of `IndexPage`, the handler of `open_settings`); the
window MUST start from the settings of the backend's writer as they are at the open. Every entry point MUST ask the
main window, and an open of the window already open MUST bring it to the front with the new open.

#### Scenario: An entry point opens Settings
- **WHEN** an entry point opens Settings
- **THEN** it asks the main window: the native menu item sends `open_settings` to the main window, `openSettings` (`runAppCommand`) runs there or sends it there
- **AND** the window starts from the settings of the backend's writer as they are at the open

#### Scenario: Opened again
- **GIVEN** the Settings window is open
- **WHEN** it is opened again
- **THEN** the open window comes to the front with the new open
  [@test] tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)

### Requirement: The window: It has the permissions of the sub-windows, the title bar and its pages

The window SHALL have the permissions of the sub-windows and of the title bar (`sub-windows.json`, `title-bar.json`),
and those of its pages (`settings.json`): the test notification, the code of the GitHub sign-in on the clipboard, and
the notification settings of the system (`x-apple.systempreferences:`, `ms-settings:`).

#### Scenario: The permissions of the window
- **WHEN** the Settings window is built
- **THEN** it has the permissions of `sub-windows.json` and `title-bar.json`
- **AND** those of `settings.json`: the test notification, the code of the GitHub sign-in on the clipboard, the notification settings of the system (`x-apple.systempreferences:`, `ms-settings:`)
  [@test] src-tauri/src/title_bar_test.rs (`every_window_gets_the_permissions_of_the_title_bar`)

### Requirement: The window: It closes with the main window

The window MUST close with the main window: when the main window is destroyed, every sub-window is (`close_sub_windows`
in `src-tauri/src/lib.rs`), and the app quits with the last window. Without the main window a sub-window would wait for
the answers of the main window.

#### Scenario: The main window is destroyed
- **GIVEN** the Settings window is open
- **WHEN** the main window is destroyed
- **THEN** every sub-window is destroyed too (`close_sub_windows` in `src-tauri/src/lib.rs`)
- **AND** the app quits with the last window

### Requirement: Entry points: Settings… of the native menu opens it from any window

Settings… of the native menu SHALL open Settings in whichever window has the focus: ⌘, in the app menu on macOS; File
→ Settings…, Ctrl+, on Windows and Linux with the system title bar.

#### Scenario: Settings… of the native menu
- **WHEN** the user chooses Settings… of the native menu (⌘, in the app menu on macOS; File → Settings…, Ctrl+, on Windows and Linux with the system title bar)
- **THEN** Settings opens, in whichever window has the focus
  [@test] tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)

### Requirement: Entry points: The Settings button is the last control of the toolbar

The last control of the toolbar, after Sync, in every state (no repository, an invalid path), SHALL be the `ghost`
`icon-sm` Settings button, "Settings" and ⌘, (Ctrl+,) in its tooltip. It replaces the popover of Sync, whose one
switch, Auto-sync on Focus, is on General. While something of the setup waits for the user, the button MUST have a dot
and list it first ([Setup Dot](../setup-attention/spec.md)).

#### Scenario: The button after Sync opens Settings
- **GIVEN** a repository is selected
- **WHEN** the user clicks the `ghost` `icon-sm` Settings button, the last control of the toolbar after Sync
- **THEN** Settings opens
- **AND** its tooltip says "Settings" and ⌘, (Ctrl+,)
  [@test] tests/e2e/repository/header.spec.ts (`should open Settings from the button after Sync, its tooltip naming the key`)

#### Scenario: Before a repository is chosen
- **GIVEN** no repository is chosen, or its path is invalid
- **THEN** the toolbar still ends with the Settings button
  [@test] tests/e2e/repository/header.spec.ts (`should offer Settings before a repository is chosen`)

#### Scenario: Sync has no popover
- **WHEN** the user looks for Auto-sync on Focus
- **THEN** Sync has no popover: its one switch, Auto-sync on Focus, is on General

#### Scenario: The setup waits for the user
- **GIVEN** something of the setup waits for the user
- **THEN** the Settings button has a dot and lists it first ([Setup Dot](../setup-attention/spec.md))

### Requirement: Entry points: The title bar of the page opens it, from a sub-window through the main window

Where the page draws the title bar (Windows, Linux), ☰ File → Settings… and Ctrl+, SHALL open Settings; Ctrl+, of a
sub-window MUST ask the main window.

#### Scenario: ☰ File → Settings… and Ctrl+, of the main window
- **GIVEN** the page draws the title bar (Windows, Linux)
- **WHEN** the user chooses ☰ File → Settings… or presses Ctrl+,
- **THEN** Settings opens
  [@test] tests/e2e/repository/title-bar.spec.ts (`should open from the keyboard and run the items`, `should take the keys of the native menu: Ctrl+R syncs, Ctrl+, opens the settings`)

#### Scenario: Ctrl+, of a sub-window
- **WHEN** the user presses Ctrl+, in a sub-window
- **THEN** the sub-window asks the main window, which opens Settings
  [@test] tests/e2e/repository/title-bar.spec.ts (`should send Sync and Settings from a sub-window to the main window`)

### Requirement: Entry points: "Notification Settings…" of the Your Turn card opens Notifications

"Notification Settings…" of the Your Turn card SHALL open Settings at Notifications.

#### Scenario: Notification Settings… of the card menu
- **WHEN** the user chooses "Notification Settings…" of the Your Turn card
- **THEN** Settings opens at Notifications
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open Settings at Notifications from the card menu, whose switches the main window keeps`)

### Requirement: Entry points: "Open Settings" of the branch prefix guide opens the prefix field

"Open Settings" of the branch prefix guide SHALL open Branches, All repositories, with the field focused; the guide
that Settings itself opens MUST have no such button.

#### Scenario: Open Settings of the guide
- **WHEN** the user clicks "Open Settings" of the branch prefix guide in the main window
- **THEN** Settings opens at Branches, All repositories, with the field of the prefix focused
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`opens Settings at the field of the prefix from the guide, and at the one of the repository`)

#### Scenario: The guide Settings opens
- **WHEN** Settings itself opens the guide of the branch prefix
- **THEN** the guide has no button that opens Settings
  [@test] tests/e2e/settings/settings.spec.ts (`should open the guide of the branch prefix without a button that opens Settings`)

### Requirement: Entry points: The sign-in buttons and the Setup dot open their account or tool

"Sign In…" of the GitHub hint (no token, or a rejected one) SHALL open Accounts at GitHub (`account-github`); a build
without the sign-in names it "GitHub Settings…". "YouTrack Settings…" of an issue and the Setup dot's YouTrack item
SHALL open Accounts at YouTrack (`account-youtrack`); a CLI's item of the dot SHALL open Tools at its row
(`cli-teamcity`, `cli-youtrack`).

#### Scenario: Sign In… of the GitHub hint
- **GIVEN** the repository has no GitHub token, or a rejected one
- **WHEN** the user clicks "Sign In…" of the GitHub hint
- **THEN** Settings opens at Accounts, at GitHub (`account-github`)
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)

#### Scenario: A build without the sign-in
- **GIVEN** a build without the sign-in
- **THEN** the button of the hint is "GitHub Settings…", which opens Accounts at GitHub
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)

#### Scenario: YouTrack Settings… and the YouTrack item of the Setup dot
- **WHEN** the user chooses "YouTrack Settings…" of an issue, or the YouTrack item of the Setup dot
- **THEN** Settings opens at Accounts, at YouTrack (`account-youtrack`)

#### Scenario: The item of a CLI in the Setup dot
- **WHEN** the user chooses the item of a CLI in the Setup dot
- **THEN** Settings opens at Tools, at the row of that CLI (`cli-teamcity`, `cli-youtrack`)

### Requirement: Entry points: The buttons of a repository's setting open its tab at the field

"Set GitHub Repository…" of the Safe Push popover SHALL open Landing on the tab of the repository at the field of the
GitHub repository; "Open <repository>" and "Open Repository Settings" SHALL open Branches on the tab of the repository
at the field of its prefix; "Open Agent Settings" SHALL open Agents on the tab of the repository at the rule it names.

#### Scenario: Set GitHub Repository… of the Safe Push popover
- **GIVEN** a repository only on Space whose GitHub repository is unknown ([Patronus](../../branch/patronus/spec.md#code-owner-review))
- **WHEN** the user clicks "Set GitHub Repository…" on the review line of the Safe Push popover
- **THEN** Settings opens at Landing on the tab of the repository, at the field of the GitHub repository
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository`)

#### Scenario: Open <repository> of the prefix of all repositories
- **GIVEN** a repository with a prefix of its own
- **WHEN** the user clicks "Open <repository>" of the prefix of all repositories
- **THEN** Branches shows the tab of the repository, at the field of its prefix
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should name the branches by the repository's own prefix over the app's, in the main window too`)

#### Scenario: Open Repository Settings of the prefix guide
- **WHEN** the user clicks "Open Repository Settings" of the prefix guide
- **THEN** Settings opens at Branches on the tab of the repository, at the field of its prefix
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`opens Settings at the field of the prefix from the guide, and at the one of the repository`)

#### Scenario: Open Agent Settings of a hint and the toast of Install Agent CLI
- **WHEN** the user clicks "Open Agent Settings" of a hint of the main window
- **THEN** Settings opens at Agents on the tab of the repository, at the rule the hint names
- **AND** the toast after Install Agent CLI opens Agents

### Requirement: Entry points: An open shows All repositories unless it names the repository

An open SHALL name its scope (`SettingsTarget.scope`, `"repository"` for a field of the repository); without one it
MUST show All repositories, never the scope of an open before, where a change could go to the repository unseen.

#### Scenario: An open without a scope after one of the repository
- **GIVEN** an open showed the scope `"repository"`
- **WHEN** an open names no scope
- **THEN** Settings shows All repositories, not the scope of the open before
  [@test] tests/e2e/settings/settings.spec.ts (`should open a page of the git config at the scope an open names, else at All repositories, never the one of before`)
  [@test] src/lib/settings-window.test.ts (`shows all repositories unless an open names the repository, never the scope of an open before`)

### Requirement: Entry points: Every entry point shows the Settings icon

Every entry point SHALL show `SettingsIcon` ([UI Theme](../../../../docs/design/ui-theme.md): one icon per concept).

#### Scenario: The icon of an entry point
- **WHEN** an entry point of Settings shows an icon
- **THEN** it is `SettingsIcon`

### Requirement: Pages: The sidebar lists the pages in two groups

The sidebar SHALL be a vertical Base UI `Tabs` list on the canvas (`TabsList variant="list"`) in two groups, each under
its muted name: "Branch Deck" (General, Appearance, Diffs, Notifications, Accounts, Tools) and "Repositories"
(Branches, Landing, Agents), the selected one in the selected row of the tables (`bg-primary/15`). ↑ and ↓ MUST switch
the page (`activateOnFocus`).

#### Scenario: The pages of the sidebar
- **WHEN** Settings opens
- **THEN** the sidebar, a vertical `Tabs` list (`TabsList variant="list"`), lists "Branch Deck" (General, Appearance, Diffs, Notifications, Accounts, Tools), then "Repositories" (Branches, Landing, Agents), each group under its muted name
- **AND** the selected page is in the selected row of the tables (`bg-primary/15`)
  [@test] tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)

#### Scenario: ↑ and ↓ switch the page
- **GIVEN** the sidebar has the focus
- **WHEN** the user presses ↑ or ↓
- **THEN** the page of the tab focused shows (`activateOnFocus`)
  [@test] tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)

### Requirement: Pages: The page is one island and the page left is hidden at once

The page SHALL be one island (`Card`), which scrolls where its tallest state or a large zoom leaves it no room; the page
left MUST be hidden at once (`data-ending-style`), so two never stack.

#### Scenario: A switch of the page
- **WHEN** the user switches the page
- **THEN** the page left is hidden at once (`data-ending-style`), so two pages never stack
  [@test] tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)

#### Scenario: No room for the page
- **WHEN** the tallest state of a page or a large zoom leaves the island (`Card`) no room
- **THEN** the island scrolls

### Requirement: Pages: General has Auto-sync on Focus and Suggest branch names

General SHALL have "Sync", Auto-sync on Focus, the setting of the item of the File menu, whose check the main window
keeps; and "AI", Suggest branch names ([Branch Name Suggestions](../../branch/branch-name-suggestions/spec.md)).

#### Scenario: Auto-sync on Focus
- **WHEN** a window changes Auto-sync on Focus
- **THEN** the main window checks the item of the File menu as the setting is
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)

#### Scenario: Suggest branch names
- **WHEN** the user turns Suggest branch names on or off under "AI"
- **THEN** the AI suggestions of branch names follow it
  [@test] tests/e2e/inline-branch-creator-ai/inline-branch-creator-ai.spec.ts (`should turn AI on and off in Settings`)

### Requirement: Pages: A row has its label on the left and its control on the right

A row SHALL have what it does on the left and its control on the right (a switch, the buttons of a few values, a
field), as in IntelliJ and System Settings; below 28rem of the row the control MUST go under its label.

#### Scenario: A narrow row
- **WHEN** a row is narrower than 28rem
- **THEN** its control goes under its label

### Requirement: Pages: Appearance sets the theme of every window

Appearance SHALL have "Theme", System · Light · Dark (`appSettings.theme`, System by default): every window MUST show
the tokens of the choice at once (`data-theme` of the root, `useTheme`), System those of the system, again whenever the
system changes; the frame of the window follows (the theme of Tauri, `null` for System), set only once the choice
differs from the frame's.

#### Scenario: A choice of the theme
- **WHEN** the user chooses Light or Dark in Appearance
- **THEN** every window shows the tokens of the choice at once (`data-theme` of the root, `useTheme`), whatever the system shows
- **AND** the frame of the window follows (the theme of Tauri), set only once the choice differs from the frame's
  [@test] tests/e2e/settings/settings.spec.ts (`should set the theme of every window from Appearance, System following the system`)
  [@test] src/lib/theme.test.ts (`follows the system for System, and keeps Light and Dark whatever the system shows`)

#### Scenario: System
- **GIVEN** "Theme" is System, the default of `appSettings.theme`
- **WHEN** the system changes its appearance
- **THEN** every window shows the tokens of the system again, the theme of Tauri `null`
  [@test] tests/e2e/settings/settings.spec.ts (`should set the theme of every window from Appearance, System following the system`)
  [@test] src/lib/theme.test.ts (`follows the system for System, and keeps Light and Dark whatever the system shows`)

### Requirement: Pages: Appearance zooms every window with the items of the View menu

Appearance SHALL have "Zoom", Zoom Out · the level · Zoom In, then Actual Size: the items of the View menu by id
(`zoomMenuItems`), their keys in the tooltips, the same command, so every window zooms one step
([Zoom](../zoom/spec.md)); and "Title bar" (Windows, Linux), the system title bar
([Title Bar](../title-bar/spec.md#setting)).

#### Scenario: The zoom buttons of Appearance
- **WHEN** the user clicks Zoom Out, Zoom In or Actual Size in Appearance
- **THEN** every window zooms one step, as by the item of the View menu (`zoomMenuItems`), whose key the tooltip names
  [@test] tests/e2e/settings/settings.spec.ts (`should zoom every window from Appearance with the items of the View menu`)

#### Scenario: Title bar
- **GIVEN** Windows or Linux
- **WHEN** the user opens Appearance
- **THEN** "Title bar" offers the system title bar ([Title Bar](../title-bar/spec.md#setting))
  [@test] tests/e2e/repository/title-bar.spec.ts (`should offer the system title bar in Settings → Appearance, applied after a restart`, `should show a fixed system title bar in the settings with its reason`)

### Requirement: Pages: Diffs sets every diff view

Diffs SHALL have one setting of every diff view in each row, as IntelliJ's diff settings: Layout (Inline · Unified ·
Split, the default first) and Whitespace (the menu above the diffs, which says the choice here), the controls above every diff too; then
"Soft wraps" and "Ligatures", each with what it does ([Commit Files](../../commit/commit-files/spec.md#soft-wraps)).

#### Scenario: Layout and Whitespace
- **WHEN** the user chooses a Layout (Inline · Unified · Split) or a Whitespace in Diffs
- **THEN** every diff view follows it
- **AND** the Whitespace menu above the diffs says the choice
  [@test] tests/e2e/settings/settings.spec.ts (`should set every diff from Diffs, the Whitespace menu naming its choice`)

#### Scenario: Soft wraps
- **WHEN** the user turns "Soft wraps" on or off in Diffs
- **THEN** every diff view follows it
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should break a long line with Soft Wraps on in Settings, and scroll it with them off`)

#### Scenario: Ligatures
- **WHEN** the user opens Diffs
- **THEN** "Ligatures" is a row with what it does

### Requirement: Pages: Notifications has the switches of the OS notifications and their permission

Notifications SHALL have the four switches of the OS notifications, the permission of the OS, and "Dock icon" (not
Windows) ([Review Requests](../review-requests/spec.md#settings)). The permission MUST be asked when the page shows, on
each open of the window and when the window gets the focus.

#### Scenario: The switches of Notifications
- **WHEN** the user opens Notifications
- **THEN** it has the four switches of the OS notifications, whose values the main window keeps, and "Dock icon" (not Windows)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open Settings at Notifications from the card menu, whose switches the main window keeps`, `should count the review requests, behind its switch`)

#### Scenario: The permission of the OS
- **WHEN** the page shows, on each open of the window, and when the window gets the focus
- **THEN** the permission of the OS to notify is asked again
  [@test] tests/e2e/settings/settings.spec.ts (`should ask the OS for the permission to notify again when the window gets the focus`)

### Requirement: Pages: Accounts has the sign-ins of GitHub and YouTrack, read again on each open

Accounts SHALL have the sign-ins of the app: "GitHub", the token and the app's own sign-in
([GitHub Sign-in](../github-sign-in/spec.md)); "YouTrack", the sign-in to the YouTrack of the issue links
([YouTrack](../youtrack/spec.md)). Their status MUST be asked again on each open of the window and when any window
signs in or out (`get_auth_status`), the `youtrack` part read with GitHub's in one read.

#### Scenario: The status is asked again
- **WHEN** the window starts, opens again, or any window signs in or out
- **THEN** the page asks `get_auth_status`: at the start of the window, again on each later open (`useAuthStatusOnOpen` of the Settings page), and on `auth-changed` (`useAuthSync` of every window)
- **AND** the `youtrack` part of the same status comes with GitHub's, in one read
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`)

#### Scenario: GitHub
- **WHEN** the user opens Accounts
- **THEN** "GitHub" has the token and the app's own sign-in
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`, `should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)

#### Scenario: YouTrack
- **WHEN** the user opens Accounts
- **THEN** "YouTrack" has the sign-in to the YouTrack of the issue links
  [@test] tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`, `should ask to sign in again once the sign-in expired`)

### Requirement: Pages: The Accounts tab has the dot of the setup while YouTrack stopped working

The tab of Accounts MUST have the dot of the setup while the app's YouTrack sign-in stopped working
([Setup Dot](../setup-attention/spec.md)).

#### Scenario: The YouTrack sign-in stopped working
- **GIVEN** the app's YouTrack sign-in stopped working
- **THEN** the tab of Accounts has the dot of the setup

### Requirement: Pages: Tools has `brd` and the CLIs

Tools SHALL show first `brd`, whose skill coding agents read: its path with a copy button and the skill files, paths
under the home directory as `~` (`agent_cli_status`, asked on each open and focus), or "The agent skill is not
installed" with Install Agent CLI; then IntelliJ IDEA ([IDE](../ide-embedding/spec.md)); then the `teamcity` and
`youtrack` CLIs, their version, sign-ins and the action each needs ([Command-Line Tools](../cli-tools/spec.md)).

#### Scenario: The agent skill is not installed
- **GIVEN** the agent skill is not installed (`agent_cli_status`)
- **WHEN** the user clicks Install Agent CLI on Tools
- **THEN** it runs the install of Help → Install Agent CLI…, whose toast has no action here
- **AND** Tools then names `brd`: its path with a copy button and the skill files, paths under the home directory as `~`
  [@test] tests/e2e/settings/agents-settings.spec.ts (`should install the agent skill from the page, which Tools then names`)

#### Scenario: The status of the tools is asked again
- **WHEN** the window opens again or gets the focus
- **THEN** `agent_cli_status` is asked again

#### Scenario: The CLIs
- **WHEN** the user opens Tools
- **THEN** after `brd` and IntelliJ IDEA it shows the `teamcity` and `youtrack` CLIs, their version, sign-ins and the action each needs
- **AND** the sign-ins of the CLIs are their own, not those of Accounts

### Requirement: Pages: Tools in the IDE installs nothing

Where the host installs nothing (`installable` of `agent_cli_status` false: the page in the IDE, whose desktop app
installs), Tools SHALL show neither `brd` nor IntelliJ IDEA, and Agents and the hint of the agents offer no install.

#### Scenario: Tools in the IDE
- **WHEN** the user opens Settings → Tools in the IDE
- **THEN** it shows the `teamcity` and `youtrack` CLIs, and no row `brd` or IntelliJ IDEA
  [@test] tests/e2e/air/air-page.spec.ts (`Settings → Tools in the IDE shows the command-line tools but no install of brd or of IntelliJ IDEA, which the desktop app installs`)

### Requirement: Pages: Branches, Landing and Agents are the git config

Branches, Landing and Agents SHALL show the git config ([Scope](#scope), [Branches](#branches), [Landing](#landing),
[Agents](#agents)).

#### Scenario: A page of the git config
- **WHEN** the user opens Branches, Landing or Agents
- **THEN** the page shows the git config, as [Scope](#scope) says

### Requirement: Pages: A page that reads a status says it reads, and a failed read with Retry

A page that reads a status (Accounts, Tools) MUST say it reads until the first answer, never an empty section or a
spinner without end. A failed read MUST say why, with Retry, above the last status when one is known, until a read
answers (`readError` of each store); Retry says it reads again until the answer.

#### Scenario: The read takes long
- **WHEN** the read of a status took 300 ms without an answer
- **THEN** the page says "Reading the GitHub sign-in…", "Reading the YouTrack sign-in…" or "Reading the tools…" (`LoadingLine`) until the first answer
  [@test] tests/e2e/settings/status-reads.spec.ts (`should say it reads the GitHub sign-in until the answer, and a failed read with Retry`, `should say it reads the YouTrack sign-in until the answer, and a failed read with Retry`, `should say it reads the tools until the answer, and a failed read with Retry`)

#### Scenario: The read fails
- **WHEN** the read of a status fails
- **THEN** the page says why with Retry ("Cannot read the GitHub sign-in" and the reason, `ReadFailure`), above the last status when one is known
- **AND** a click on Retry says it reads again until the answer, and the failure stays until a read answers
  [@test] tests/e2e/settings/status-reads.spec.ts (`should say it reads the GitHub sign-in until the answer, and a failed read with Retry`, `should say it reads the YouTrack sign-in until the answer, and a failed read with Retry`, `should say it reads the tools until the answer, and a failed read with Retry`)
  [@test] src/stores/auth.test.ts (`says why a read failed and keeps the last status, until a read answers; Retry reads again, the failure gone meanwhile`)

### Requirement: Pages: Every page but Tools and Landing fits 880 × 640 without scrolling

Every page but Tools and Landing MUST fit 880 × 640 without scrolling in its tallest state, a page of the git config on
both tabs; Tools and Landing SHALL be allowed to scroll in theirs. A page MUST NOT be wider than the island, also at
640 × 440.

#### Scenario: The tallest state of every page
- **GIVEN** the tallest state of each page: the code of the GitHub sign-in, the warning that the OS refused the notifications (one line with Open System Settings), the Dock group on macOS and Linux, every line a row can show (the repository's own value with Reset, the system config, a value of the worktree config, an invalid value, the pull request off by the route)
- **WHEN** the window is 880 × 640
- **THEN** every page but Tools and Landing fits without scrolling, a page of the git config on both tabs
  [@test] tests/e2e/settings/settings.spec.ts (`should fit every page at 880 × 640 without scrolling`, `should fit every page at 880 × 640 without scrolling, the Dock badge too`)

#### Scenario: Tools and Landing may scroll
- **GIVEN** Tools with a CLI on two servers and the install command of the other, or Landing with the companions of a repository
- **THEN** the page may scroll

#### Scenario: The smallest window
- **WHEN** the window is 640 × 440
- **THEN** no page is wider than the island
  [@test] tests/e2e/settings/settings.spec.ts (`should keep every page within the island at the smallest window`)

### Requirement: The page an open shows: An open shows the page it names, else the page shown

An open SHALL say what it shows (`SettingsTarget`, the data of the window): `{ page?, scope?, focus?, server? }`.
With a page it MUST show that page; without one it MUST keep the page shown, and a new window MUST open at the page
shown last (`appSettings.settingsPage`, General by default).

#### Scenario: An open that names a page
- **WHEN** an open names a page
- **THEN** Settings shows that page
  [@test] src/lib/settings-window.test.ts (`shows the page an open names`)
  [@test] tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)

#### Scenario: An open without a page
- **GIVEN** the Settings window is open
- **WHEN** an open names no page
- **THEN** the window keeps the page it shows
  [@test] src/lib/settings-window.test.ts (`keeps the current page for an open without one`)
  [@test] tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)

#### Scenario: A new window
- **WHEN** a new Settings window opens without a page
- **THEN** it opens at the page shown last (`appSettings.settingsPage`), General by default
  [@test] tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)

### Requirement: The page an open shows: A page of an older build is the page that has its settings now

A page of an older build MUST be the page that has its settings now (`github` and `youtrack` Accounts, `repository`
Branches, `companions` Landing; `currentSettingsPage`, also for the saved page); one the sidebar doesn't know MUST be
General.

#### Scenario: A page of before and an unknown page
- **WHEN** an open or the saved page names `github`, `youtrack`, `repository` or `companions`
- **THEN** Settings shows Accounts, Branches or Landing, the page that has those settings now (`currentSettingsPage`)
- **AND** a page the sidebar doesn't know shows General
  [@test] src/lib/settings-window.test.ts (`shows the page that has the settings of a page of before, and General for a page the sidebar doesn't know`)
  [@test] tests/e2e/settings/settings.spec.ts (`should show the page that has the settings of a page of an older build`)

### Requirement: The page an open shows: A new open switches during the render, and only the page is saved

A new open MUST switch the page and the scope during the render, not in an effect; the saved page MUST follow the page
shown, and the scope MUST NOT be saved.

#### Scenario: The page shown is saved, the scope is not
- **WHEN** the user switches the page or the scope
- **THEN** the saved page (`appSettings.settingsPage`) follows the page shown
- **AND** the scope is not saved
  [@test] tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)

### Requirement: The page an open shows: Each open focuses the field it names, else the tab of the page

Each open MUST focus the field it names on the page it shows (`data-settings-focus`), once its settings are read and
the field is enabled, else the tab of the page, once Base UI registered the tabs: a tab focused before keeps a stale
highlight, and the first ↓ would go nowhere. A field the page never shows MUST be waited for 5 s at most.

#### Scenario: An open that names a field
- **WHEN** an open names a field: `branchPrefix` on Branches, `githubRepository` and `landing` on Landing, a rule on Agents; `account-github` and `account-youtrack` on Accounts; `cli-brd`, `cli-teamcity` and `cli-youtrack` on Tools
- **THEN** the field has the focus, on a page of the git config once its settings are read and the field is enabled
  [@test] tests/e2e/settings/settings.spec.ts (`should focus the field an open names`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should name the branches by the repository's own prefix over the app's, in the main window too`)

#### Scenario: An open that names no field
- **WHEN** an open names no field
- **THEN** the tab of the page has the focus, once Base UI registered the tabs
  [@test] tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)

#### Scenario: A field the page never shows
- **WHEN** an open names a field the page never shows
- **THEN** it is waited for 5 s at most

### Requirement: Keys: Escape closes the window, after the typing of a field and an open popover

Escape SHALL close the window, as in every sub-window; in a field with typing it MUST take the typing back first, and
in an open popover it MUST close the popover only (Base UI prevents the event).

#### Scenario: Escape in a field with typing
- **GIVEN** a field of Settings holds typing
- **WHEN** the user presses Escape
- **THEN** the typing is taken back and the window stays
- **AND** the next Escape closes the window
  [@test] tests/e2e/settings/settings.spec.ts (`should close on Escape, which takes the typing of a field back first`)

#### Scenario: Escape in an open popover
- **GIVEN** a menu of Settings is open, the Whitespace menu of Diffs
- **WHEN** the user presses Escape
- **THEN** it closes the menu only, not the window (Base UI prevents the event)
  [@test] tests/e2e/settings/settings.spec.ts (`should set every diff from Diffs, the Whitespace menu naming its choice`)

#### Scenario: Escape in an open popover of another sub-window
- **GIVEN** the form of Request Changes of the review window is open
- **WHEN** the user presses Escape
- **THEN** it closes the form only, not the window, as in every sub-window
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request`)

### Requirement: Keys: The footer says Esc Close and no ↑↓ hint

The footer SHALL say `Esc Close`, and MUST NOT have a ↑↓ hint, which holds only while the sidebar has the focus.

#### Scenario: The footer of the window
- **WHEN** the Settings window shows
- **THEN** its footer says `Esc Close`, without a ↑↓ hint

### Requirement: Writes through the backend: Every window writes through the one writer of the backend

Every window MUST write the settings through the one writer of the backend (`set_setting`, `SettingsService`), which
applies them in the order they arrive, sends each change to every window (`settings_changed`, with its version and the
window that asked) and saves `settings.json` within 500 ms, with a temporary file renamed over it
([Distributed Settings](../../../../docs/design/distributed-settings.md)).

#### Scenario: A window changes a setting
- **WHEN** a window changes a setting
- **THEN** it sends the change to the writer of the backend (`set_setting`), which saves it in `settings.json`
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should save the diff view the window chooses through the writer of the backend`)

#### Scenario: Changes in a row
- **WHEN** changes arrive at the writer one after another
- **THEN** it applies them in the order they arrive, so the last one is on disk within 500 ms, written to a temporary file renamed over `settings.json`
  [@test] crates/app-settings/src/service_test.rs (`the_changes_are_saved_in_the_order_they_arrive_so_the_last_one_is_on_disk`)

#### Scenario: Every window takes the change
- **WHEN** the writer applies a change
- **THEN** it sends it to every window (`settings_changed`, with its version and the window that asked)
- **AND** the zoom zooms every window
- **AND** Auto-sync on Focus checks the item of the native menu (`set_auto_sync_menu_checked`)
- **AND** a changed AI mode clears the last AI failure of every window (`ai-suggestions.ts`)
  [@test] tests/e2e/settings/settings.spec.ts (`should zoom every window from Appearance with the items of the View menu`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)

### Requirement: Writes through the backend: The writer refuses an invalid value of a field it reads

The writer MUST refuse a value of a field it reads that is invalid (a zoom that is no positive number), and the window
MUST show the value the writer has, with "Cannot change the settings".

#### Scenario: A zoom that is no positive number
- **WHEN** a window sends a value of a field the writer reads that is invalid, a zoom that is no positive number
- **THEN** the writer refuses it and changes nothing
- **AND** the window puts the value the writer has back, with "Cannot change the settings"
  [@test] crates/app-settings/src/service_test.rs (`an_invalid_value_of_a_field_rust_knows_is_refused_and_changes_nothing`)
  [@test] src/stores/settings/persistent-store.test.ts (`puts the writer's value back when it refuses a change, and says so`)

### Requirement: Writes through the backend: A window's write changes only the settings it changed

A window's write MUST change only the settings it changed (the fields of an object, an array whole): a setting another
window changed a moment before (the Activity panel opened in the main window) stays, also when the window had not heard
of it yet.

#### Scenario: Two windows change two fields of one key
- **GIVEN** the main window opened the Activity panel a moment before
- **WHEN** a review window changes another field of the same key, before or after it heard of that change
- **THEN** both changes stay
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should keep the Activity panel the main window opened when a review window hides its sidebar right after`)
  [@test] crates/app-settings/src/service_test.rs (`two_origins_that_patch_two_fields_of_one_key_at_once_keep_both`)
  [@test] src/stores/settings/persistent-store.test.ts (`keeps the field of its own change the writer has not answered when another window's change of the key arrives`)

#### Scenario: What a change sends
- **WHEN** a window changes a field of an object or an array
- **THEN** it sends the fields the change made, not the whole object, a field back at its default as null
- **AND** an array whole, an empty one as no value
  [@test] src/stores/settings/persistent-store.test.ts (`sends the fields a change made, not the whole object, a field back at its default as null`, `sends an array whole, an empty one as no value`)

### Requirement: Writes through the backend: A window sends its changes in order and a late echo rolls nothing back

A window MUST send its next change after the answer of the one before, so the writer gets them in order, and MUST apply
only a change newer than the last one it applied: a late answer or echo never rolls a setting back, and a change equal
to a value the window had before still arrives.

#### Scenario: Changes made while one is unanswered
- **WHEN** a window makes changes while the writer has not answered the one before
- **THEN** it sends the next change only after that answer, the changes made meanwhile as one
  [@test] src/stores/settings/persistent-store.test.ts (`sends the next change only after the answer of the one before, the changes made meanwhile as one`)

#### Scenario: A late answer or echo
- **WHEN** an answer or an echo of an older change arrives after a newer one was applied
- **THEN** it is an echo and rolls nothing back
  [@test] crates/app-settings/src/service_test.rs (`a_change_older_than_the_applied_version_is_an_echo`)
  [@test] src/stores/settings/persistent-store.test.ts (`never rolls a key back with a late echo of an older change`)

#### Scenario: A change equal to a value of before
- **GIVEN** a review window hid its sidebar at 110%
- **WHEN** the main window chooses Actual Size
- **THEN** the change arrives in the review window, equal to a value it had before
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should follow Actual Size of the main window in a review window that changed another field in between`)

### Requirement: Writes through the backend: Some keys reach only the main window

The Activity log, the robot history and the state of the repositories MUST reach only the main window.

#### Scenario: A key only the main window keeps
- **WHEN** a sub-window starts, or such a key changes
- **THEN** the sub-window gets no key only the main window keeps, starts it empty and takes none of its changes
  [@test] crates/app-settings/src/windows_test.rs (`a_sub_window_gets_no_key_only_the_main_window_keeps`)
  [@test] src/stores/settings/persistent-store.test.ts (`starts a key only the main window keeps empty in a sub-window and takes none of its changes`)

### Requirement: Writes through the backend: A page takes the changes after the version of its snapshot

A change made while a window's page loads reaches no listener of it, so every page MUST take the changes after the
version of the snapshot it started from, by the same versions: a sub-window with its load, applied before its content
settles and the window shows (`sub_window_ready`); the main window once it listens (`get_settings_changes`; its reload
starts from the snapshot of the app's start).

#### Scenario: A sub-window loads
- **WHEN** a setting changes while the page of a sub-window loads
- **THEN** `sub_window_loaded` answers the changes after the version its build remembered (`SubWindowState::built`), no older one nor a key only the main window keeps
- **AND** the page applies them by their versions before its content settles and the window shows (`sub_window_ready`)
  [@test] crates/app-api/src/windows_test.rs (`a_page_that_loads_takes_the_changes_after_the_snapshot_of_its_build`, `a_page_that_loads_takes_no_change_older_than_its_snapshot_nor_a_key_only_the_main_window_keeps`)
  [@test] src/lib/sub-window.test.ts (`takes the settings changed while the page loaded once it listens, before it says its shell and its content`)
  [@test] src/stores/settings/persistent-store.test.ts (`takes the changes the load of the window hands over by their versions, before its store exists too`)
  [@test] crates/app-settings/src/service_test.rs (`the_changes_since_a_snapshot_name_each_key_changed_after_it_once_with_its_value_now`)

#### Scenario: The zoom changed while a review window loaded
- **WHEN** the zoom changes while a review window loads
- **THEN** it is the window's zoom when the window shows
  [@test] tests/e2e/settings/settings.spec.ts (`zooms a review window whose page loaded after Actual Size`)

#### Scenario: The Activity panel opened while a review window loaded
- **WHEN** the main window opens the Activity panel meanwhile, and the review window changes another field
- **THEN** the Activity panel stays open
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should keep the Activity panel the main window opened when a review window hides its sidebar right after`)

#### Scenario: The main window listens
- **WHEN** the main window listens
- **THEN** it takes the changes after its snapshot (`get_settings_changes`), its reload starting from the snapshot of the app's start

### Requirement: Writes through the backend: The main window holds its changes until it caught up

Until it caught up with the writer, the main window MUST hold its own changes; then it sends them, a field on top of
the writer's value, and drops a whole array (the recent projects) the writer has a newer value of, which the array
built on the old snapshot would overwrite.

#### Scenario: A change of the main window before it caught up
- **WHEN** the main window changes a setting before it caught up with the writer
- **THEN** it holds the change until it caught up, then sends a field on top of the writer's value
- **AND** it drops a whole array, the recent projects, the writer has a newer value of
  [@test] src/stores/settings/persistent-store-main.test.ts (`holds its changes until it caught up with the writer, and drops a whole array the writer has a newer one of`)

### Requirement: Writes through the backend: The webview starts at the zoom of its snapshot

The window's webview SHALL start at the zoom of its snapshot, which its page MUST change when the level it has differs
(`zoomAtStart`, `useZoom`).

#### Scenario: The level differs from the snapshot
- **WHEN** the page of a window has another zoom level than the one its webview started at
- **THEN** the page changes the zoom of the webview to it (`zoomAtStart`, `useZoom`)

### Requirement: Writes through the backend: The file keeps only what differs from the defaults

The file MUST keep only what differs from the defaults: a field back at its default is removed, and an object or an
array left empty removes its key. Keys the backend does not know MUST be kept as they are.

#### Scenario: A field back at its default
- **WHEN** a change sets a field back at its default (a null field)
- **THEN** the field is removed from its key, the other fields merged as they are
  [@test] crates/app-settings/src/service_test.rs (`a_field_patch_merges_into_the_key_and_a_null_field_is_removed`)

#### Scenario: The last field of a key
- **WHEN** a change removes the last field of an object, or leaves an array empty
- **THEN** the key is removed
  [@test] crates/app-settings/src/service_test.rs (`a_field_patch_that_removes_the_last_field_removes_the_key`)

#### Scenario: A key the backend does not know
- **GIVEN** `settings.json` has a key the backend does not know
- **WHEN** the writer reads and saves the file
- **THEN** the key is kept as it is
  [@test] crates/app-settings/src/file_test.rs (`unknown_keys_round_trip`)

### Requirement: Writes through the backend: The defaults of the fields Rust reads are Rust's alone

The defaults of the fields Rust reads (the app's prefix, the new Patronus API, the zoom, the sync on focus, the system
title bar) MUST be Rust's alone: the page takes them from the bindings (`APP_SETTINGS_DEFAULTS`), so a value the page
sends back as its default is the one Rust and `brd` read as the default too.

#### Scenario: A field Rust reads back at its default
- **WHEN** the page sends a field Rust reads back as its default
- **THEN** the value is the default of Rust, taken from the bindings (`APP_SETTINGS_DEFAULTS`), the one Rust and `brd` read as the default too
  [@test] src/stores/settings/appSettings.test.ts (`takes the defaults of the fields Rust reads from Rust, so a value back at its default means the same to both`)
  [@test] src-tauri/src/bindings_test.rs (`the_bindings_are_what_the_export_writes`)

### Requirement: Writes through the backend: What a sync records lives in `repository-state.json`

What each sync records of a repository (`repositoryState` by its path) MUST live in a file of its own,
`repository-state.json` beside `settings.json`, behind the same writer and its versions, and only the main window has
it: a sync writes that file alone, never `settings.json`, which `brd` reads beside it.

#### Scenario: A sync records a repository
- **WHEN** a sync records of a repository when the last sync ended, how many branches it had, how long the steps of a sync and of Update Project took, when Branch Deck fetched the baseline last, up to when the Landed island hides what landed
- **THEN** the page sends the writer only the repository the sync changed, as one field of `repositoryState`
- **AND** the writer saves `repository-state.json` and leaves `settings.json` untouched
  [@test] crates/app-settings/src/service_test.rs (`a_patch_of_the_repository_state_leaves_the_settings_file_untouched`)
  [@test] src/stores/settings/repositoryState.test.ts (`sends the writer only the repository a sync changed, as one field of repositoryState`, `drops a malformed field or repository, never the others`)

#### Scenario: Only the main window has the state of the repositories
- **WHEN** a sub-window starts
- **THEN** it gets no `repositoryState`
  [@test] crates/app-settings/src/windows_test.rs (`a_sub_window_gets_no_key_only_the_main_window_keeps`)

### Requirement: Writes through the backend: A `settings.json` of before moves the fields of a sync once

A `settings.json` of before that kept the fields of a sync in its recent projects MUST be read once: the writer moves
them (a value the state file has for the repository wins), saves the state file, and `settings.json` loses them with
its next save; every other key of it MUST be kept.

#### Scenario: A settings file of before
- **GIVEN** a `settings.json` of before keeps the fields of a sync in its recent projects
- **WHEN** the writer reads it
- **THEN** it moves them to the state of the repositories by path, a value the state file has for the repository winning, and saves the state file, also without a change
- **AND** `settings.json` loses them with its next save, every other key of it kept
  [@test] crates/app-settings/src/service_test.rs (`a_settings_file_of_before_moves_the_fields_of_a_sync_to_the_repository_state_and_drops_them_on_its_next_save`, `the_moved_values_of_a_settings_file_of_before_are_saved_without_a_change`)
  [@test] crates/app-settings/src/file_test.rs (`the_fields_of_the_repository_state_leave_the_recent_projects_by_path_and_nothing_else_changes`)

### Requirement: Writes through the backend: Every window starts from the writer's copy of the settings

Every window MUST start from the writer's copy of the settings, set before its page's scripts (`__TAURI_STORE__`, with
its version `__TAURI_STORE_VERSION__`; `init_script` of `src-tauri/src/settings.rs`), and with the defaults from a
settings file that does not parse (one cut within a write): it never starts without its settings because of a syntax
error in its init script.

#### Scenario: A settings file cut within a write
- **GIVEN** `settings.json` does not parse, cut within a write
- **WHEN** a window starts
- **THEN** its init script sets `__TAURI_STORE__` to a JSON object of the defaults, without a syntax error
  [@test] crates/app-settings/src/service_test.rs (`the_snapshot_of_a_cut_file_is_a_json_object_of_the_defaults`)

### Requirement: Writes through the backend: A failed save keeps the settings and is tried again

A failed save MUST keep the settings in memory and be tried again by itself (the delay doubling up to a minute) and by
the next change, and the main window MUST say so once per streak of failures ("Cannot save the settings",
`settings_save_failed`). The app MUST save what is unsaved before it quits (`RunEvent::ExitRequested`) and before
Restart of the title bar setting (`restart_app`).

#### Scenario: The directory of the settings is not writable
- **WHEN** a save fails
- **THEN** memory keeps the value, and the error is reported once a streak of failures
- **AND** the main window says "Cannot save the settings" (`settings_save_failed`)
  [@test] crates/app-settings/src/service_test.rs (`an_unwritable_directory_reports_the_error_once_a_streak_and_memory_keeps_the_value`)
  [@test] src/stores/settings/persistent-store-main.test.ts (`tells the user in the main window that the settings could not be saved`)

#### Scenario: The directory is writable again
- **GIVEN** a save failed
- **WHEN** the directory is writable again
- **THEN** the save is tried again by itself, the delay doubling up to a minute, or by the next change
  [@test] crates/app-settings/src/service_test.rs (`a_failed_save_is_retried_by_itself_once_the_directory_is_writable_again`)

#### Scenario: The app quits or restarts
- **WHEN** the app quits (`RunEvent::ExitRequested`) or restarts by Restart of the title bar setting (`restart_app`)
- **THEN** it saves what is unsaved first, without waiting for the debounce
  [@test] crates/app-settings/src/service_test.rs (`a_flush_soon_after_a_patch_saves_it_without_waiting_for_the_debounce`)

### Requirement: Writes through the backend: Two writers on one directory keep each other's changes

Each process that shares the app data directory (the app, `brd mcp` of an IDE) MUST have one writer, and a save MUST
take turns at the lock of the directory, read the file as it is and put on top only the keys and the fields its process
changed and has not saved: a change of the other process to another key or field stays, a key it removed stays removed.

#### Scenario: Two writers change different keys and fields
- **GIVEN** two writers on one app data directory
- **WHEN** each changes other keys and other fields of one key, and both save, also at once
- **THEN** the file has every change of both
- **AND** the writer that saved second has the changes of the first in memory
  [@test] crates/app-settings/src/two_processes_test.rs (`two_writers_that_change_different_keys_and_fields_keep_every_change`, `two_writers_that_save_at_once_lose_no_change_of_the_other`)
  [@test] crates/app-settings/src/merge_test.rs (`a_save_puts_only_the_keys_and_fields_this_process_changed_on_top_of_the_file_of_the_other`, `a_save_forgets_the_changes_it_wrote_and_keeps_the_ones_made_after_it_took_its_copy`)

#### Scenario: A key the other writer removed
- **WHEN** one writer removes a key or a field and saves, and the other saves a change of another key
- **THEN** the key and the field stay removed
  [@test] crates/app-settings/src/two_processes_test.rs (`a_key_the_other_writer_removed_stays_removed_after_a_save_of_this_one`)

#### Scenario: Both change the same field
- **GIVEN** a writer has an unsaved change of a field
- **WHEN** the other writer saves another value of that field
- **THEN** the writer keeps its own value, and its save writes it
  [@test] crates/app-settings/src/two_processes_test.rs (`an_unsaved_change_of_this_writer_stays_over_the_other_writers_save_of_the_same_field`)

#### Scenario: A file another program broke
- **WHEN** the file does not parse as a writer saves
- **THEN** the writer writes it again from memory and takes nothing of it
  [@test] crates/app-settings/src/two_processes_test.rs (`a_broken_file_another_program_wrote_takes_nothing_into_memory`)

### Requirement: Writes through the backend: A change another process saved reaches every window

A writer MUST take what the other process saved into memory, when the watch of the directory sees the file renamed and
with its own next save at the latest; each key is a change of its own, with the next version and the origin
`other-process`, which every window that gets the key hears as `settings_changed` and catches up with by its version.

#### Scenario: The other process saves a setting
- **WHEN** the writer of the other process saves a change
- **THEN** the watch reads the file, and the writer tells the change once, with its next version and the origin `other-process`
- **AND** every window that gets the key takes it (`settings_changed`), a page that loads catches up with it (`changes_since`)
  [@test] crates/app-settings/src/two_processes_test.rs (`a_change_another_writer_saved_reaches_the_subscribers_with_the_next_version`)

#### Scenario: A save before the watch told it
- **WHEN** a writer saves before its watch read the change of the other process
- **THEN** the save takes the change and tells it once, before its flush returns
  [@test] crates/app-settings/src/two_processes_test.rs (`a_save_takes_the_change_of_the_other_writer_and_tells_it_once_before_its_flush_returns`)

### Requirement: Writes through the backend: The pages read only the settings and the backend

The pages MUST read only what every window has: the settings and the backend. The branch prefix field asks Rust for the
prefix of the selected repository itself (`resolve_branch_prefix`, again on focus and once the writer has another app
prefix), and MUST stay enabled while a sync runs: a sync resolves the prefix when it starts, so a change names the
branches of the next one.

#### Scenario: The branch prefix field
- **WHEN** the branch prefix field shows, the window gets the focus, or the writer has another app prefix
- **THEN** the field asks Rust for the prefix of the selected repository (`resolve_branch_prefix`)

#### Scenario: A sync runs
- **GIVEN** a sync runs
- **THEN** the branch prefix field stays enabled, and a change names the branches of the next sync

### Requirement: Scope: The pages of the git config read it when they show, on focus and on a switch

Branches, Landing and Agents SHALL show the `branchdeck.*` git config, which `brd` reads too (`get_repository_config`,
one `git config --show-scope --get-regexp`,
[Git Operations](../../../../docs/design/git-operations.md#repository-settings)), and MUST read it when the page shows,
when the window gets the focus (a `git config` or `brd` meanwhile) and on a switch of the repository.

#### Scenario: The window gets the focus
- **GIVEN** a `git config` or `brd` changed a key meanwhile
- **WHEN** the window gets the focus
- **THEN** the page reads the config again (`get_repository_config`) and shows the value
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should save Report Only, its source this repository, which the main window syncs, and read it again when the window gets the focus`)

#### Scenario: A switch of the repository
- **WHEN** the main window switches the repository
- **THEN** the page reads the config of the new one
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should write nothing when the repository is switched while a field holds typing`)

### Requirement: Scope: The scope tabs are All repositories and the selected repository

Above the page SHALL stand the scope tabs (Base UI `Tabs`, ←/→): "All repositories" and the name of the selected
repository (its path in the tooltip), with the line under them: "What every repository uses, in the app and brd", or
"Changes here apply only to <repository>, saved in its .git/config".

#### Scenario: The tabs and their line
- **GIVEN** a repository is selected
- **WHEN** a page of the git config shows
- **THEN** the scope tabs are "All repositories" and the name of the repository, its path in the tooltip, ← and → switching them
- **AND** the line under them says "What every repository uses, in the app and brd" on the first, "Changes here apply only to <repository>, saved in its .git/config" on the second
  [@test] tests/e2e/settings/settings.spec.ts (`should open a page of the git config at the scope an open names, else at All repositories, never the one of before`)

### Requirement: Scope: Without a repository there is only the value of every repository

Without a repository there MUST be no tabs, only the value of every repository (the global config alone).

#### Scenario: No repository is selected
- **GIVEN** no repository is selected
- **WHEN** a page of the git config shows
- **THEN** it has no scope tabs, only the value of every repository, read from the global config alone
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should show only the value of every repository without a repository, and say how to set the rest`)

### Requirement: Scope: A failed read of the config is a callout with Retry

A failed read MUST be a `destructive` callout ("Cannot read the settings of <repository>", "Cannot read your global Git
config") with the reason and Retry.

#### Scenario: The read of the config fails
- **WHEN** the read of the config of the repository, or of the global config alone, fails
- **THEN** the page shows a `destructive` callout, "Cannot read the settings of <repository>" or "Cannot read your global Git config", with the reason and Retry
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should write nothing when the repository is switched while a field holds typing`)

### Requirement: Scope: A switch of the repository flashes its name in the tab

A switch of the repository in the main window MUST flash its name in the tab and say it to a screen reader.

#### Scenario: The main window switches the repository
- **WHEN** the main window switches the repository
- **THEN** the tab of the repository flashes its new name, and a screen reader is told it

### Requirement: Scope: All repositories shows and writes the value of every repository

On "All repositories" a row SHALL show the value of every repository (`everyRepositoryValue`: the global or system
entry, else the default, whatever the repository sets), and a change MUST write the global config
(`set_repository_config` with `scope: "global"`). The prefix of the app and the API of Patronus, settings of the app
that every repository uses, are on this tab too.

#### Scenario: The value of every repository
- **GIVEN** the repository sets a value of its own
- **WHEN** the user opens "All repositories"
- **THEN** the row shows the value of every repository: the last global or system entry parsed, else the default (`everyRepositoryValue`)
  [@test] src/lib/repository-config.test.ts (`shows on the tab of every repository its value, whatever the repository sets`)
  [@test] crates/sync-core/src/repository_config_test.rs (`the_fallback_is_the_last_global_or_system_value_parsed`)

#### Scenario: A change on All repositories
- **WHEN** the user changes a row on "All repositories"
- **THEN** the change writes the global config (`set_repository_config` with `scope: "global"`)
  [@test] tests/e2e/settings/agents-settings.spec.ts (`should change a rule for every repository in the global config, and give the repository its own value`)

#### Scenario: The settings of the app on All repositories
- **WHEN** the user opens "All repositories" of Branches or Landing
- **THEN** the prefix of the app and the API of Patronus, settings of the app that every repository uses, are there

### Requirement: Scope: A row of All repositories says only what differs

A row on "All repositories" MUST say only what differs (`describeScopedSource`): "<repository> has its own value: Off"
with Open <repository>, which shows its tab, or "Set in the system Git config; a change goes to your global Git config,
which wins over it".

#### Scenario: The repository keeps a value of its own, or the system config sets it
- **WHEN** the selected repository keeps a value of its own, or the system config sets the value
- **THEN** the row says "<repository> has its own value: Off" with Open <repository>, which shows its tab
- **AND** for the system config "Set in the system Git config; a change goes to your global Git config, which wins over it"
  [@test] src/lib/repository-config.test.ts (`says on the tab of every repository where the selected one keeps a value of its own, and the system config`)

### Requirement: Scope: The tab of the repository shows the value that holds there

On the tab of the repository a row SHALL show the value that holds there, and a change MUST write its `.git/config`.
The value of every repository has no line, as an unmodified setting of VS Code; the repository's own value MUST have a
bar beside the row and "Only in <repository>" (or "Same as all repositories", which can go) with Reset, which removes
it there.

#### Scenario: The repository's own value
- **WHEN** the repository has its own value of a row
- **THEN** the row has a bar beside it and "Only in <repository>" (or "Same as all repositories") with Reset
- **AND** the tooltip of Reset says the result ("Removes it from .git/config: back to On, from all repositories", "… back to Off, the default")
  [@test] src/lib/repository-config.test.ts (`says on the tab of the repository when the value is its own, with what Reset gives back`)
  [@test] tests/e2e/settings/agents-settings.spec.ts (`should change a rule for every repository in the global config, and give the repository its own value`)

#### Scenario: The value of every repository on the tab of the repository
- **WHEN** the value that holds in the repository is the value of every repository
- **THEN** the row has no line, as an unmodified setting of VS Code

#### Scenario: Reset
- **WHEN** the user clicks Reset
- **THEN** the value is removed from `.git/config`, the value of every repository holding again

### Requirement: Scope: A row only a repository has says its default

A row only a repository has (the route, the GitHub repository) MUST say its default instead ("Detected from origin",
"Pull Request by default: the repository is not on Space"), and a global value of it "Set in your global Git config,
which applies to every repository".

#### Scenario: The default and a global value of a row only a repository has
- **WHEN** the tab of the repository shows the route or the GitHub repository
- **THEN** the row says its default ("Detected from origin", "Pull Request by default: the repository is not on Space")
- **AND** a global value of it "Set in your global Git config, which applies to every repository"
  [@test] src/lib/repository-config.test.ts (`names the default of a row only a repository has, and a global value of it as one of every repository`)

### Requirement: Scope: A value that wins over `.git/config` disables the row of the repository

A value of the worktree config or of `git -c` and `GIT_CONFIG_*` wins over `.git/config`: on the tab of the repository
its control MUST be disabled with that reason, as the backend refuses the write; the value of every repository can still
change.

#### Scenario: The worktree config or the command line sets a key
- **GIVEN** the worktree config, or `git -c` and `GIT_CONFIG_*`, sets a key
- **WHEN** the tab of the repository shows its row
- **THEN** the control is disabled with the reason: "Set in config.worktree, which overrides .git/config: change it there", "Set by git -c or GIT_CONFIG_*, which overrides .git/config"
- **AND** the backend refuses a write of it
- **AND** the value of every repository can still change
  [@test] src/lib/repository-config.test.ts (`can't change a key the worktree config or the command line sets`)
  [@test] crates/sync-core/src/repository_config_test.rs (`a_value_of_the_worktree_config_refuses_the_write`)
  [@test] tests/e2e/settings/agents-settings.spec.ts (`should disable the rule of the repository while its worktree config sets it, and say so`)

### Requirement: Scope: An invalid value says why, a boolean as git reads it

An invalid value MUST say why, where it holds; a boolean is invalid exactly when `git config --type=bool` refuses it,
and `0b1` MUST be invalid on every platform, though the git of glibc 2.38+ reads it as true.

#### Scenario: Booleans as git reads them
- **WHEN** a boolean key has the value ` true `, `08` or `1kb`
- **THEN** it is invalid, as `git config --type=bool` refuses it
- **AND** `0x10`, `1k` or `-1` are true as git reads them, and `0b1` is invalid on every platform
  [@test] crates/sync-core/src/config_snapshot_test.rs (`a_boolean_is_what_git_makes_of_it`)

### Requirement: Scope: The git key of a row is its copy button

The git key of a row SHALL be its copy button beside the label ("Copy branchdeck.agentLease"), which the keyboard
reaches.

#### Scenario: The copy button of the key
- **WHEN** the user focuses the copy button beside the label of a row by the keyboard, and presses it
- **THEN** the git key of the row is copied ("Copy branchdeck.agentLease")

### Requirement: Scope: A switch writes at once, a field on Enter and when it is left

A switch or a button MUST write at once, a field on Enter and when it is left, never on a key, and Escape MUST take the
typing back before it closes the window.

#### Scenario: Typing into a field
- **WHEN** the user types into a field
- **THEN** nothing is written on a key
- **AND** the typing is written on Enter, or when the field is left
- **AND** Escape takes the typing back, the window staying
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`)

#### Scenario: A switch or a button
- **WHEN** the user turns a switch or clicks a button of a value
- **THEN** it is written at once

### Requirement: Scope: Typing is checked before a write as the backend checks it

The typing MUST be checked before a write as the backend checks it (`isValidGitHubRepository`, `isValidBranchPrefix`,
`isValidPushSizeLimit`), refused in place by what is wrong, without "Cannot …", since nothing was written.

#### Scenario: A GitHub repository that is not owner/name
- **WHEN** the user enters "acme/app/x" as the GitHub repository
- **THEN** it is refused in place, "acme/app/x is not a GitHub repository: use owner/name", without "Cannot …", and nothing is written
- **AND** owner/name without whitespace is taken, as the backend takes it
  [@test] src/lib/repository-config.test.ts (`takes owner/name without whitespace, as the backend does`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should refuse a GitHub repository that is not owner/name in place, and save one that is, which the main window asks GitHub about`)

#### Scenario: An invalid prefix
- **WHEN** the user enters a prefix the backend would refuse
- **THEN** it is refused in place in the words of the prefix of all repositories (`INVALID_PREFIX_TEXT`)

### Requirement: Scope: A field writes to the repository the typing began in

A field MUST write to the repository the typing began in: a switch of the repository or of the tab drops the typing
(one page per repository and tab, `key`) and writes nothing.

#### Scenario: The repository is switched while a field holds typing
- **GIVEN** a field holds typing
- **WHEN** the repository or the tab is switched
- **THEN** the typing is dropped and nothing is written
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should write nothing when the repository is switched while a field holds typing`)

### Requirement: Scope: A window closed with valid typing saves it first

A window closed with valid typing (⌘W, the close button) MUST save it first: one listener of the window waits for the
saves the fields registered (`saveBeforeClose`), then destroys the window, a failed save too: the window may destroy
itself (`core:window:allow-destroy` of `src-tauri/capabilities/settings.json`), since with that listener Tauri no longer
closes it.

#### Scenario: The window closes with typing
- **GIVEN** a field holds valid typing
- **WHEN** the window closes by ⌘W or the close button
- **THEN** the typing is saved, nothing else, and then the window is destroyed (`saveBeforeClose`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should save valid typing when the window closes, and nothing else`)

#### Scenario: The window may destroy itself
- **WHEN** the Settings window gets its close request
- **THEN** it may destroy itself (`core:window:allow-destroy` of `src-tauri/capabilities/settings.json`), after a failed save too
  [@test] src-tauri/src/title_bar_test.rs (`the_settings_window_may_destroy_itself_after_its_close_request`)

### Requirement: Scope: Two writes at once both land

Two writes at once MUST both land: a write of the app never fails on another write of the app, and a `config.lock` of
another process MUST be waited for up to a second.

#### Scenario: Two writes of the app at once
- **WHEN** the app writes two keys at once
- **THEN** both writes succeed
  [@test] crates/sync-core/src/repository_config_test.rs (`two_writes_at_once_both_succeed`)

#### Scenario: A `config.lock` of another process
- **GIVEN** another process holds `config.lock`
- **WHEN** the app writes the config
- **THEN** the write waits for it up to a second, and fails on a lock that stays
  [@test] crates/sync-core/src/repository_config_test.rs (`a_write_waits_for_a_config_lock_another_process_holds`, `a_write_fails_on_a_config_lock_that_stays`)

### Requirement: Scope: The page shows an answer only if it counts as many writes as the settings shown

Every answer of the settings, of a read and of a write, MUST count the config writes of the app before its read
(`seq`, one counter of the process for the local and the global config), and the page MUST show an answer only if it
counts as many as the settings shown or more (`newer`). A failed write changes nothing shown, and an answer for another
repository is dropped.

#### Scenario: Answers that arrive in reverse order
- **WHEN** a prefix is saved while a mode's write runs, and the mode answers last with the prefix it read before
- **THEN** the page keeps the saved prefix, the newest of the answers
- **AND** a read a focus started before a write does not take it back either
  [@test] src/lib/repository-config.test.ts (`keeps the newest of answers that arrive in reverse order`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should keep a prefix saved while a mode's write runs, when the mode's older answer comes last`)

#### Scenario: A read again of as many writes
- **WHEN** a read again counts as many writes as the settings shown
- **THEN** the page takes it, with what a `git config` or `brd` changed meanwhile
  [@test] src/lib/repository-config.test.ts (`takes an answer of as many writes: a read again finds a git config of meanwhile`)

#### Scenario: A failed write and an answer for another repository
- **WHEN** a write fails, or an answer is for another repository
- **THEN** nothing shown changes, the answer for another repository dropped

### Requirement: Scope: A failed write is a toast in the words of the change

A failed write MUST be a toast in the words of the change (`notifyError`).

#### Scenario: The titles of a failed write
- **WHEN** a write of the repository fails
- **THEN** the toast says "Cannot change the symbol dependencies of app", "Cannot change the GitHub repository of app", "Cannot change the branch prefix of app" (and the rest of `writeErrorTitle`)
- **AND** for every repository "Cannot change how every repository resolves conflicts by words", "Cannot change the push size limit of every repository" (`globalWriteErrorTitle`)
  [@test] src/lib/repository-config.test.ts (`titles a failed write by what it changes`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should tell a failed write in a toast`)

### Requirement: Scope: After a write the main window acts for the selected repository

After a write the backend MUST tell the main window (`repository_config_changed` `{ repositoryPath, key }`), which acts
for the selected repository only, by the key written. A repository never synced MUST NOT be synced by a write: its
first sync is the user's.

#### Scenario: The branch prefix
- **WHEN** the branch prefix of the selected repository is written
- **THEN** the branch prefix loads again
- **AND** a changed one is another session, which the window opens and syncs for a repository synced before ([Onboarding](../onboarding/spec.md))
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should name the branches by the repository's own prefix over the app's, in the main window too`, `should open and sync the session of a prefix written here, for a repository synced before`)

#### Scenario: The symbol mode, the resolve by words, the review plan, the route and the companions
- **GIVEN** the selected repository was synced before
- **WHEN** the symbol mode, the resolve by words, the review plan, the route or the companions are written
- **THEN** the main window syncs
- **AND** for the symbol mode alone the stack-independence checks of the Safe Push popover ask again
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should turn the symbol dependencies off, and back to the default, the main window syncing each change`, `should save Report Only, its source this repository, which the main window syncs, and read it again when the window gets the focus`)

#### Scenario: The GitHub repository
- **WHEN** the GitHub repository is written
- **THEN** the main window asks for the pull requests (`refreshPullRequests("manual")`), and syncs a repository synced before
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should refuse a GitHub repository that is not owner/name in place, and save one that is, which the main window asks GitHub about`)

#### Scenario: The push size limit and the rules of agents
- **WHEN** the push size limit or a rule of agents is written
- **THEN** the main window does nothing: a push reads the limit as it starts, only agents read the rules

#### Scenario: A repository never synced
- **GIVEN** the selected repository was never synced
- **WHEN** a key of it is written
- **THEN** the main window does not sync it, leaving its first sync to the user
- **AND** a write of the prefix still subscribes again, and one of the GitHub repository still asks for the pull requests

### Requirement: Scope: A write waits for no sync

A write MUST wait for no sync: during one it is written at once, and the sync it asks for runs once after that one.

#### Scenario: A write during a held sync
- **GIVEN** a sync of the main window runs
- **WHEN** the symbol dependencies are written
- **THEN** `.git/config` has them at once
- **AND** the sync the write asks for runs once, after that sync
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should write the symbol dependencies during a held sync at once, the main window syncing it once after that sync`)

### Requirement: Branches: Names on All repositories is the prefix of every repository without its own

"Names" on All repositories SHALL be the prefix of every repository without one of its own (`BranchPrefixSetting`,
`globalUserBranchPrefix` of the settings, which `brd` reads too; [Onboarding](../onboarding/spec.md),
[Branch prefix order](#branch-prefix-order)), which MUST say a repository's own prefix with Open <repository>.

#### Scenario: A change of the prefix of all repositories
- **WHEN** the user changes the prefix on All repositories
- **THEN** it is written to `globalUserBranchPrefix` of the settings, which `brd` reads too
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`)

#### Scenario: A repository with a prefix of its own
- **GIVEN** the selected repository has a prefix of its own
- **THEN** the row on All repositories says that prefix, with Open <repository>

### Requirement: Branches: Branch prefix on the tab of the repository gives the prefix of all back

On the tab of the repository "Branch prefix" ("Used here instead of the prefix of all repositories",
`branchdeck.branchPrefix`) SHALL have a placeholder and a Reset that give the prefix of all repositories back
("Removes it from .git/config: back to jane, the default"): the prefix of the app MUST win over a global or system git
one (`withAppPrefix`).

#### Scenario: The prefix of all repositories over a global git one
- **GIVEN** the app has a prefix and the global or system git config another one
- **THEN** the placeholder and the tooltip of Reset give the prefix of the app (`withAppPrefix`), as the branches are named
  [@test] src/lib/repository-config.test.ts (`takes the prefix of all repositories over a global git one, as the branches are named`)

#### Scenario: Reset of the repository's own prefix
- **GIVEN** the repository has its own `branchdeck.branchPrefix`
- **WHEN** the user clicks Reset ("Removes it from .git/config: back to jane, the default")
- **THEN** the prefix of all repositories names the branches of the repository again, and the row says so
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should give the prefix back to all repositories once the repository's own is unset, and say so`)

### Requirement: Branches: Sync has the symbol dependencies and the resolve by words

"Sync" SHALL have "Symbol dependencies", Off · Report Only · On (`aria-pressed`, the sentence of the mode chosen;
`branchdeck.symbols`, [Symbol Dependencies](../../commit/symbol-dependencies/spec.md)), and "Resolve conflicts by
words", `branchdeck.resolveByWords`, on by default (the sync, Update Project and `brd update` commit a conflict of git
that its words resolve, [Git Operations](../../../../docs/design/git-operations.md#resolve-by-words)).

#### Scenario: The mode of the symbol dependencies
- **WHEN** the user chooses a mode of "Symbol dependencies"
- **THEN** its button is pressed (`aria-pressed`) and the sentence of the mode shows: "Commits depend only on the lines they change", "Shown by brd commits, never used for the branches", "Commits also depend on the commits that add the symbols they use"
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should save Report Only, its source this repository, which the main window syncs, and read it again when the window gets the focus`)

#### Scenario: Resolve conflicts by words
- **WHEN** the user turns "Resolve conflicts by words" off or on, on All repositories
- **THEN** it is written to the global config (`branchdeck.resolveByWords`, on by default)
- **AND** the row says where the repository's own value wins
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should resolve conflicts by words in the global config, and say where the repository's own value wins`)

### Requirement: Branches: Review has the review plan, which a later review window takes

"Review" SHALL have "Review plan (experimental)", `branchdeck.reviewExperimental`, on by default: Mark Reviewed, Since
Your Review, the overview of the plan and the marks of the files in the branch review
([Review Plan](../../branch/review-plan/spec.md)); a review window opened after the sync MUST take the new value.

#### Scenario: The review plan turned off and on
- **WHEN** the user turns "Review plan (experimental)" off or on for every repository from Branches
- **THEN** a review window opened after the sync takes the new value
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should turn the review plan on and off for every repository from Branches`)

### Requirement: Landing: Route into master offers Safe Push, Push and Pull Request

On the tab of the repository, "Route into master" ("How the work of the repository lands into master, in the app and
brd") SHALL have "Land by", Safe Push · Push · Pull Request (`branchdeck.landing`; `patronus`, `push`,
`pull-request`), the route in effect pressed and its sentence under the label, which says where the pull requests of a
stack go: into master on Safe Push, a stack of GitHub off it
([Stacks of GitHub](../../branch/stacked-branches/spec.md#stacks-of-github)).

#### Scenario: Push chosen
- **WHEN** the user chooses Push in "Land by" on the tab of the repository
- **THEN** `branchdeck.landing` is `push` in `.git/config`, Push is pressed
- **AND** its sentence under the label says "A push of its copy lands each piece, without a robot or the CI of a pull request; a straight stack is a stack of GitHub"
  [@test] tests/e2e/settings/landing-settings.spec.ts (`should land the repository by a push into master once chosen, the main window taking the step, and back to the default`)

### Requirement: Landing: Unset, the route of the host of origin is pressed and says why

Unset, the default of the host of `origin` MUST be pressed and the line MUST say why (`RepositoryConfig.landingDefault`,
Rust's one default: "Safe Push by default: the repository is on Space", "Pull Request by default: the repository is not
on Space"); a click on it names it in `.git/config`, and Reset removes it ("Removes it from .git/config: back to Pull
Request, the default off Space").

#### Scenario: No route is set
- **GIVEN** the repository sets no `branchdeck.landing`
- **WHEN** the user opens Landing on the tab of the repository
- **THEN** the default of the host of `origin` is pressed, its line "Safe Push by default: the repository is on Space" or "Pull Request by default: the repository is not on Space"
  [@test] tests/e2e/settings/landing-settings.spec.ts (`should land the repository by a push into master once chosen, the main window taking the step, and back to the default`)
  [@test] crates/sync-core/src/repository_config_test.rs (`the_default_route_follows_the_host_of_origin_in_the_one_read_of_its_push_url`)

#### Scenario: The default clicked, and Reset
- **WHEN** the user clicks the route pressed by default
- **THEN** it is named in `.git/config`
- **AND** Reset removes it ("Removes it from .git/config: back to Pull Request, the default off Space")
  [@test] tests/e2e/settings/landing-settings.spec.ts (`should land the repository by a push into master once chosen, the main window taking the step, and back to the default`)

### Requirement: Landing: After a write of the route the main window syncs

After a write of the route the main window MUST sync a repository synced before, so its rows take the steps of the
route. The window has no session of the repository: the write access and the rules of the target are said by the step
of each row, not here.

#### Scenario: The route changes
- **GIVEN** the repository was synced before
- **WHEN** its route is written
- **THEN** the main window syncs it, and its rows take the steps of the route
- **AND** Landing says nothing of the write access and the rules of the target
  [@test] tests/e2e/settings/landing-settings.spec.ts (`should land the repository by a push into master once chosen, the main window taking the step, and back to the default`)

### Requirement: Landing: GitHub repository names where pull requests are opened

After the route, "GitHub repository" (`owner/name`, "Where pull requests are opened"; `branchdeck.githubRepository`,
[Pull Requests](../../branch/pull-requests/spec.md)) SHALL show the detected one as the placeholder and its line.

#### Scenario: No config sets the GitHub repository
- **GIVEN** no config sets `branchdeck.githubRepository`
- **THEN** the placeholder is the detected one, and the line says where it comes from: "Detected from origin", "Detected from the Space mirror", "None detected: neither origin nor its Space mirror is on GitHub"
  [@test] src/lib/repository-config.test.ts (`says where the GitHub repository comes from while no config sets it`)

### Requirement: Landing: All repositories shows no route, GitHub repository nor companions

A route is the repository's own: the tab of every repository MUST show no route, GitHub repository nor companions, only
"The route into master, the GitHub repository and the companions are set for each repository" with Open <repository>
("Select a repository to set its route into master, its GitHub repository and its companions" without one).

#### Scenario: Landing on All repositories
- **WHEN** the user opens Landing on All repositories
- **THEN** it shows "The route into master, the GitHub repository and the companions are set for each repository" with Open <repository>
  [@test] tests/e2e/settings/landing-settings.spec.ts (`should refuse a route for every repository, which would break the repositories off Space`)

#### Scenario: Landing without a repository
- **GIVEN** no repository is selected
- **THEN** Landing says "Select a repository to set its route into master, its GitHub repository and its companions"
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should show only the value of every repository without a repository, and say how to set the rest`)

### Requirement: Landing: The backend refuses a route of the global config

The backend MUST refuse a write of `branchdeck.landing` to the global config whatever its value: a global `patronus`
would refuse every repository off Space (`notOnSpace`), a global `pull-request` would take Safe Push from the monorepo;
a global `push` MUST read as unset with its reason ([Landing Routes](../../branch/landing-routes/spec.md)).

#### Scenario: A write of the route to the global config
- **WHEN** a route is written to the global config, whatever its value
- **THEN** the backend refuses it: a route is written only to the local config
  [@test] tests/e2e/settings/landing-settings.spec.ts (`should refuse a route for every repository, which would break the repositories off Space`)
  [@test] crates/sync-core/src/repository_config_test.rs (`a_route_is_written_only_to_the_local_config`)

#### Scenario: A global route read
- **GIVEN** the global config has `branchdeck.landing`
- **WHEN** the config is read
- **THEN** a global `push` reads as unset, with its reason
  [@test] crates/sync-core/src/repository_config_test.rs (`the_route_is_one_of_three_and_a_push_of_the_global_config_is_unset`)
  [@test] crates/sync-core/src/config_snapshot_test.rs (`a_push_route_of_the_global_config_is_none`)

### Requirement: Landing: The route is read and written with the other keys

The route SHALL be read and written with the other keys (`RepositoryConfig.landing`, the change `landing`; "Cannot
change how app lands into master"), and a sync and every `brd` command MUST read it in their one read of the config.

#### Scenario: The route in the one read of the config
- **WHEN** a sync or a `brd` command reads the config
- **THEN** the route is one of three, from the git config, and none without it
  [@test] crates/sync-core/src/repository_config_test.rs (`the_route_is_one_of_three_and_a_push_of_the_global_config_is_unset`)
  [@test] crates/sync-core/src/config_snapshot_test.rs (`the_route_comes_from_git_config_and_is_none_without_it`)

#### Scenario: A failed write of the route
- **WHEN** a write of the route of app fails
- **THEN** the toast says "Cannot change how app lands into master"

### Requirement: Landing: Companions lists the repositories that land in an order with it

"Companions" on the tab of the repository SHALL list the repositories whose branches land in an order with it
([Declared Needs](../../branch/declared-needs/spec.md#companion-repositories), `CompanionsSetting`).

#### Scenario: A companion added and removed
- **WHEN** the user adds a companion repository from the folder dialog, or removes one
- **THEN** the list of companions follows, and a folder that is no repository is refused
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should add a companion repository from the folder dialog, refuse a folder that is no repository, and remove it`)

### Requirement: Landing: Push size limit is on both tabs, and a write syncs nothing

"Push" ("A push of the app or brd that adds more files is refused before it is sent") SHALL be on both tabs: "Push size
limit", a size (`50M` the placeholder and the default, `1G`, `512K`), `0` or `off` for none
([Outgoing](../outgoing/spec.md#push)), the value of every repository with the repository's own as every key of the git
config ([Scope](#scope)). A write MUST sync nothing: a push reads the limit as it starts.

#### Scenario: A push over the limit
- **GIVEN** a push size limit
- **WHEN** a push adds more files than the limit
- **THEN** it is refused before it is sent, and once the limit is `off` the push goes
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should refuse a push over the push size limit before it is sent, saying what it adds, and push once the limit is off`)

#### Scenario: The values of the limit
- **WHEN** the limit is written
- **THEN** it is a size with a binary unit, `0` or `off`, as written
  [@test] src/lib/repository-config.test.ts (`takes a size with a binary unit, 0 or off, as the backend does`)
  [@test] crates/sync-core/src/repository_config_test.rs (`the_push_size_limit_is_a_size_or_off_as_written`)

#### Scenario: A write of the limit
- **WHEN** the limit is written
- **THEN** nothing syncs

#### Scenario: A failed write of the limit
- **WHEN** a write of the push size limit of every repository fails
- **THEN** the toast says "Cannot change the push size limit of every repository"
  [@test] src/lib/repository-config.test.ts (`titles a failed write by what it changes`)

### Requirement: Landing: Typing of the push size limit is refused in the backend's words

Typing of the push size limit the backend would refuse MUST be refused in place (`isValidPushSizeLimit`), in the
backend's words for an invalid value.

#### Scenario: Typing the backend would refuse
- **WHEN** the user types "lots" as the limit
- **THEN** it is refused in place (`isValidPushSizeLimit`): "lots is not a size such as 50M or 1G, nor 0 or off"
- **AND** the backend refuses an invalid value in the same words, before git runs
  [@test] crates/sync-core/src/repository_config_test.rs (`invalid_values_are_refused_before_git_runs`)

### Requirement: Landing: Safe Push on All repositories switches the new Patronus API

"Safe Push" on All repositories, how Branch Deck and brd read the robots of Patronus, SHALL have the switch "New
Patronus API" (`appSettings.patronusNewApi`, off by default, a setting of the app): the progress of a running robot in
one request and its merge profile in the robot list, the old requests while Patronus has them off
([Patronus](../../branch/patronus/spec.md)). `brd` MUST read the setting from the settings file
([Agent CLI](../../../../docs/contributing/agent-cli.md)).

#### Scenario: New Patronus API on and off
- **WHEN** "New Patronus API" is on
- **THEN** the details a window demands ask the folded progress of a running robot in one request
- **AND** with it off, the old requests
  [@test] crates/test-server/src/remote_fixtures_test.rs (`the_details_a_window_demands_ask_the_folded_progress_only_with_the_new_api_of_the_settings`)

### Requirement: Agents: Coding agents has the three rules of agents

Agents SHALL have one section, "Coding agents", "Agents such as Claude Code and Codex ask brd workspace before their
first edit; what you tell an agent wins" ([brd workspace](../../cli/workspace/spec.md)), with three rules, each a switch
with what it does, each the value of every repository with the repository's own ([Scope](#scope)).

#### Scenario: The three rules
- **WHEN** the user opens Agents
- **THEN** it has "Work in its own worktree" ("Each agent leases a worktree; its commits arrive on your branch one by one", `branchdeck.agentLease`, on by default)
- **AND** "Name the branch in each commit subject" ("(auth) Add login form: the commit goes into the branch auth", `branchdeck.agentSubjectBranch`, on by default)
- **AND** "Push the branch and open its pull request" ("After each commit arrives on your branch, the agent pushes it and opens its pull request without asking", `branchdeck.agentPullRequest`, off by default)

#### Scenario: A rule for every repository and the repository's own
- **WHEN** the user changes a rule on All repositories, or on the tab of the repository
- **THEN** it is the value of every repository in the global config, or the repository's own
  [@test] tests/e2e/settings/agents-settings.spec.ts (`should change a rule for every repository in the global config, and give the repository its own value`)

#### Scenario: The worktree config sets a rule
- **GIVEN** the worktree config of the repository sets a rule
- **THEN** the rule of the repository is disabled, as every key of the git config ([Scope](#scope))
  [@test] tests/e2e/settings/agents-settings.spec.ts (`should disable the rule of the repository while its worktree config sets it, and say so`)

### Requirement: Agents: A write of a rule syncs nothing, and a failed one is a toast

A write of a rule MUST NOT sync: only agents read the rules. A failed write MUST be a toast: "Cannot change where agents
work in every repository", "Cannot change how agents write commit subjects in <repository>", and so on.

#### Scenario: A rule written
- **WHEN** a rule of agents is written
- **THEN** nothing syncs

#### Scenario: A failed write of a rule
- **WHEN** a write of a rule fails
- **THEN** the toast says "Cannot change where agents work in every repository", "Cannot change how agents write commit subjects in <repository>", and so on

### Requirement: Agents: The pull request after a commit is off where the repository lands by a push

The pull request after a commit does not apply where the repository lands by a push: Rust MUST say so
(`RepositoryConfig.agentPullRequestOff`, the reason `brd workspace` gives too,
[brd workspace](../../cli/workspace/spec.md)), and the row MUST show the reason with Change Route.

#### Scenario: A repository that lands by a push
- **GIVEN** the repository lands by a push
- **THEN** the row of the pull request shows the reason with Change Route, which opens Landing on the tab of the repository at "Land by"
- **AND** on its tab the switch is off and disabled
- **AND** on All repositories it stays the value of every repository, and the line says "In <repository>: …"
  [@test] tests/e2e/settings/agents-settings.spec.ts (`should turn the pull request of agents off where the repository lands by a push, with the way to its route`)

### Requirement: Agents: Without the agent skill the page offers Install Agent CLI

While the agent skill is not installed (`agent_cli_status`), the page MUST start with "Agents read these rules once
the skill is installed" and Install Agent CLI, whose toast has no action here; the skill itself is on Tools. The same
status and button tell the main window where agents work on the way to the first branches
([Onboarding](../onboarding/spec.md#agents)).

#### Scenario: The skill is not installed
- **GIVEN** the agent skill is not installed (`agent_cli_status`)
- **WHEN** the user opens Agents
- **THEN** the page starts with "Agents read these rules once the skill is installed" and Install Agent CLI
- **AND** after the install, whose toast has no action here, Tools names the skill
  [@test] tests/e2e/settings/agents-settings.spec.ts (`should install the agent skill from the page, which Tools then names`)

### Requirement: Branch prefix order: The specific prefix names the branches first, resolved in Rust

The branches of a repository MUST be named by the specific prefix first, in the app as in `brd`, resolved in Rust only
(`resolve_branch_prefix` in `crates/app-settings/src/resolve.rs`): the repository's own `branchdeck.branchPrefix` (its
local or worktree git config, or `git -c` and `GIT_CONFIG_*`), else the app's prefix (General,
`globalUserBranchPrefix`), else the one of the global or system git config.

#### Scenario: Prefixes of several sources
- **GIVEN** the repository's own `branchdeck.branchPrefix`, the app's prefix and a prefix of the global git config
- **WHEN** the prefix is resolved
- **THEN** the repository's own wins, else the app's, else the global or system one
  [@test] crates/app-settings/src/resolve_test.rs (`the_specific_prefix_wins`)

### Requirement: Branch prefix order: Each prefix is named as the branches are

Each prefix MUST be named as the branches are (trimmed, no trailing slash), and one that nothing is left of MUST count
as unset.

#### Scenario: A prefix of slashes only
- **WHEN** a prefix has spaces around it or a trailing slash, or is slashes only
- **THEN** it is trimmed and loses the trailing slash, and one of slashes only counts as unset
  [@test] crates/app-settings/src/resolve_test.rs (`the_prefix_is_named_as_the_branches_are_and_one_of_slashes_only_is_unset`)

### Requirement: Branch prefix order: The resolved prefix names its source

The resolved prefix MUST name its source (`ResolvedBranchPrefix { prefix, source }`: `cli`, `repository_config`,
`app_setting`, `global_config`, or `none` exactly when there is no prefix), as `prefix_source` of `brd status` does.

#### Scenario: The source of the prefix
- **WHEN** a prefix is resolved
- **THEN** its source is written in snake case: `cli`, `repository_config`, `app_setting`, `global_config`, or `none` exactly when there is no prefix
  [@test] crates/app-settings/src/resolve_test.rs (`the_source_is_written_in_snake_case`)

### Requirement: Branch prefix order: The prefix of git is read with its scope

The prefix of git MUST be read with its scope (one `git config --show-scope`; the global config alone for an empty
path).

#### Scenario: The scope of the prefix
- **WHEN** the prefix of git is read
- **THEN** its scope comes with it, in one `git config --show-scope`
- **AND** a local prefix, or one of `git -c`, is scoped to the repository
  [@test] crates/sync-core/src/branch_prefix_test.rs (`a_local_prefix_is_repository_scoped`, `the_scope_of_the_prefix_is_read_with_it`, `a_prefix_of_git_c_is_repository_scoped`)

#### Scenario: An empty path
- **WHEN** the prefix of git is read for an empty path
- **THEN** only the global config is read

### Requirement: Branch prefix order: The commands of the sync family resolve the prefix per command

The commands of the sync family MUST take no prefix from the page: they resolve it per command, with the app's prefix
of the settings when the command starts and the git prefix of a memo by repository path (`BranchPrefixes` in
`crates/app-settings/src/branch_prefixes.rs`, a state of the app and of the E2E server), so a command pays no git
process for it.

#### Scenario: The commands of the sync family
- **WHEN** a command of the sync family runs: the session commands (`subscribe_view`, `get_snapshot`, `sync`, `set_window_focus`, whose session is keyed by the resolved prefix), `get_archived_branch_commits`, `get_unapplied_branch_commits` and `execute`, whose operation runs in the session of that key (a rewrite of the engine: the unapply, the reapply, the deletes of an archived and an unapplied branch)
- **THEN** the page names only the repository, and the command resolves the prefix
- **AND** git is read once per repository, the app prefix of each command applying
  [@test] crates/app-settings/src/branch_prefixes_test.rs (`git_is_read_once_per_repository_and_the_app_prefix_of_each_command_applies`)
  [@test] src/model/store.test.ts (`names only the repository: the session resolves the prefix, and a sync waits for no load of the page`)

### Requirement: Branch prefix order: Each read of the git config keeps its git prefix in the memo

Each read of the git config MUST keep its git prefix in the memo: the subscription of a window, which reads the config
now (`session_key_now`), the `resolve_branch_prefix` of Settings, the config snapshot of every sync of a session, and a
write of `branchdeck.branchPrefix` in Settings → Branches (a write to the global config forgets every repository's).

#### Scenario: A read or a write of the git config
- **WHEN** a read of the git config or a write of `branchdeck.branchPrefix` in Settings → Branches runs
- **THEN** its git prefix replaces the one of the memo
- **AND** a write to the global config forgets the prefix of every repository
  [@test] crates/app-settings/src/branch_prefixes_test.rs (`a_refresh_replaces_the_git_prefix_and_a_clear_forgets_every_repository`)

#### Scenario: A failed read
- **WHEN** a read of the git prefix fails
- **THEN** the memo keeps nothing of it
  [@test] crates/app-settings/src/branch_prefixes_test.rs (`a_failed_read_keeps_nothing`)

### Requirement: Branch prefix order: The views are kept by the resolved prefix, and a sync needs one

The views of the last sync MUST be kept by the resolved prefix; a sync of a repository without a prefix MUST be refused
("Cannot sync without a branch prefix: set one in Settings → Branches").

#### Scenario: The prefix of a sync
- **WHEN** a sync runs where the repository sets no prefix
- **THEN** it names the branches by the app prefix
- **AND** the repository's own prefix wins over it
  [@test] crates/test-server/src/branch_prefix_test.rs (`the_sync_names_the_branches_by_the_app_prefix_where_the_repository_sets_none`, `the_repository_s_own_prefix_wins_and_a_repository_without_any_is_not_synced`)

#### Scenario: The inputs of a sync change with the prefix
- **WHEN** only the prefix changes
- **THEN** the inputs of the sync change, so a view of the last sync is kept by its prefix
  [@test] crates/sync-core/src/sync_inputs_test.rs (`changes_with_the_prefix_alone`)

#### Scenario: A repository without any prefix
- **WHEN** a sync runs for a repository without any prefix
- **THEN** it is refused, "Cannot sync without a branch prefix: set one in Settings → Branches"
  [@test] crates/test-server/src/branch_prefix_test.rs (`the_repository_s_own_prefix_wins_and_a_repository_without_any_is_not_synced`)

### Requirement: Branch prefix order: The commands of the remote and the review resolve it the same way

The commands of Patronus, of a push, of a pull request, of the branch review and of the independence of a stack MUST
name the repository only too and resolve the prefix the same way (`PrefixInputs` of `crates/app-api/src/context.rs`,
[Patronus](../../branch/patronus/spec.md)).

#### Scenario: A command of Patronus, a push or a pull request
- **WHEN** a command of Patronus, of a push, of a pull request, of the branch review or of the independence of a stack runs
- **THEN** the page names the repository only, and the command resolves the prefix (`PrefixInputs`)

### Requirement: Branch prefix order: The conflict fix gets the prefix the page shows

The conflict fix (`find_conflict_fix`), whose parameters `brd` shares, MUST get the prefix the page shows, the one of
the view of its session.

#### Scenario: The conflict fix
- **WHEN** the page asks `find_conflict_fix`
- **THEN** it passes the prefix of the view of its session

### Requirement: Branch prefix order: The page shows the names with what Rust resolved

The page MUST show the names with what Rust resolved, deciding nothing: the main window takes the prefix of the view of
the session it subscribed to (`TableHeader.prefix`), which Rust resolves for each subscription from the git config as
it is then, and reads none of its own.

#### Scenario: The names of the main window
- **WHEN** the main window names the branches
- **THEN** it takes the prefix of the view of its session (`TableHeader.prefix`): the repository's own prefix over the app's, else the app's
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`names the branches by the repository's own prefix over the app's, and says so in Settings`, `names the branches by the app's prefix where the repository has none`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should name the branches by the repository's own prefix over the app's, in the main window too`)

### Requirement: Branch prefix order: The main window subscribes again when the prefix may change

The main window MUST subscribe again, holding the view it shows (`resubscribeSession`), after a write of the key in
Settings → Branches (`repository_config_changed`), once the writer has another app prefix (`onWriterChange` of
`src/stores/settings/persistent-store.ts`: the change of any window, its own once answered), and when a run of its
session answers `prefix_changed`.

#### Scenario: A subscription again
- **WHEN** the main window subscribes again, holding its view
- **THEN** the same session answers nothing new, the session of another prefix its view, which replaces the rows
  [@test] src/model/store.test.ts (`subscribes again holding its view: the same session answers nothing, another one's view replaces the rows`)

#### Scenario: A run answers `prefix_changed`
- **WHEN** a run of the session answers `prefix_changed`, the git config naming another prefix than the session's
- **THEN** the main window subscribes again, quietly
  [@test] src/model/store.test.ts (`subscribes again when the git config names another prefix than the session's, quietly`)

#### Scenario: The writer has another app prefix
- **WHEN** the writer has another app prefix, by the change of any window or its own once answered
- **THEN** `onWriterChange` tells the main window, which subscribes again; a late echo tells it nothing
  [@test] src/stores/settings/persistent-store.test.ts (`tells a reader of Rust the value the writer has: another window's change, its own once answered, no late echo`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`names the branches by the repository's own prefix over the app's, and says so in Settings`)

#### Scenario: A write of the key in Settings → Branches
- **WHEN** Settings writes `branchdeck.branchPrefix` (`repository_config_changed`)
- **THEN** the main window subscribes again
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should name the branches by the repository's own prefix over the app's, in the main window too`)

### Requirement: Branch prefix order: Branches on All repositories shows the sources of the answer

Branches, All repositories, MUST ask `resolve_branch_prefix` itself and show the repository's own prefix and the global
git one from the answer's source.

#### Scenario: The repository's own prefix on All repositories
- **GIVEN** the repository has a prefix of its own
- **WHEN** Branches shows All repositories
- **THEN** it says the repository's own prefix, from the source of the answer of `resolve_branch_prefix`
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`names the branches by the repository's own prefix over the app's, and says so in Settings`)

### Requirement: Branch prefix order: No window caches the prefix

A window MUST NOT cache the prefix: the view of a session names it, the frame the warm start shows first too, and a switch
forgets the prefix of the repository left with its session.

#### Scenario: A switch of the repository
- **WHEN** the main window switches the repository
- **THEN** it keeps no prefix it resolved before: the view of the session of the repository names it
  [@test] src/stores/settings/repositorySettings.test.ts (`keeps no prefix a window resolved before: the view of the session of the repository names it`)

### Requirement: Branch prefix order: The cached prefix of a settings file of before is dropped

The `cachedBranchPrefix` a settings file of before kept in each project of `recentProjects` MUST be dropped by the load
(`RETIRED_PROJECT_FIELDS` of `crates/app-settings/src/file.rs`), so the next save writes the file without it, and the
page's schema keeps none.

#### Scenario: A settings file of before
- **GIVEN** a project of `recentProjects` has `cachedBranchPrefix`
- **WHEN** the file is loaded and saved next
- **THEN** the load drops it, and the next save writes the file without it
  [@test] crates/app-settings/src/file_test.rs (`the_cached_prefix_of_a_project_of_before_is_dropped_by_the_load_and_the_next_save`)

## User Experience

- Window title "Settings"; groups "Branch Deck" and "Repositories"; pages "General", "Appearance", "Diffs",
  "Notifications", "Accounts", "Tools", "Branches", "Landing", "Agents"; the scope tabs "All repositories" and the name
  of the repository; sections in sentence case ("Sync", "AI", "Zoom", "Title bar", "Diffs", "Notifications", "Dock
  icon", "GitHub", "YouTrack", "Names", "Review", "Route into master", "Companions", "Push", "Safe Push", "Coding
  agents").
- Menu item "Settings…", buttons "Notification Settings…", "Sign In…", "GitHub Settings…", "Set GitHub Repository…",
  "Open Repository Settings", "Open <repository>", "Reset", "Change Route", "Retry"; the modes "Off", "Report Only",
  "On".
- A failed open is "Cannot open Settings" with the reason (`notifyError`).

## Testing / Local Run

- `pnpm e2e`: `tests/e2e/settings/settings.spec.ts`. `openSettingsWindow` of `tests/e2e/helpers/settings-helpers.ts`
  fires `open_settings` in the main window's page and shows the recorded open as a linked page
  (`openLinkedSubWindow`), whose settings changes reach every linked page; it starts from the settings the open
  recorded (`store` and `storeVersion` of the open the test server's desktop recorded, `getOpenedSubWindows`: the
  writer's copy at the open, as the app builds the window), its window built for it on the server before the page
  loads (`registerWindowBuild`), so its load takes the changes after that snapshot. `expectSettingsOpened`
  checks the last open and that it is the only new one; `reopenLinkedSubWindow` hands a later open to the shown
  window, as the backend does.
- The settings of a test live in the test server's writer of the repository (`TestRepository::store`, the app's
  `SettingsService`): `set_setting` of app-api changes them there and sends the change to every page of the repository
  whose window gets its key (`settings_changed`, on the event stream of each page, `TestEvents` of
  `crates/test-server/src/host.rs`), as the app sends it to its windows, and `testStore` of the fixtures reads and
  writes them (`/store/<repo>/<key>`).
  [@test] crates/app-api/src/events_test.rs (`a_change_of_the_settings_goes_to_the_windows_that_get_its_key`)
  [@test] crates/test-server/src/app_events_test.rs (`the_events_of_the_app_reach_the_pages_of_their_windows_and_end_with_the_repository`)
- `tests/e2e/settings/repository-settings.spec.ts`: the test server reads and writes the local config of the test
  repository and a global config of its own (`GIT_CONFIG_GLOBAL`, `AppState::global_config_of`), never the
  developer's or the system config; the sync of the repository does not read that global file. After a write the test
  server sends `repository_config_changed` to the pages of the main window (`set_repository_config` of app-api, as in
  the app).
  [@test] crates/app-api/src/events_test.rs (`a_config_write_goes_to_the_main_window_and_a_journal_change_to_every_window`)
- By hand: `pnpm tauri dev`; ⌘, from the menu, from a review window and again with the window open (it comes to the
  front); every page at 100% and 150%; closing the main window closes Settings; Branches on the tab of a repository: a
  symbol mode syncs the main window, a prefix of the repository renames its branches, `git config --worktree` disables
  a row; Agents of a repository that lands by a push: the pull request rule off with Change Route.

## References

- [Zoom](../zoom/spec.md), [Title Bar](../title-bar/spec.md), [Review Requests](../review-requests/spec.md),
  [GitHub Sign-in](../github-sign-in/spec.md), [Onboarding](../onboarding/spec.md).
- [Distributed Settings](../../../../docs/design/distributed-settings.md): the one writer of the settings and how every window
  follows it.
