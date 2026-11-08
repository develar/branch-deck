---
name: Settings
description: Requirements for the Settings window, a sub-window of its own with a sidebar of pages in two groups (Branch Deck - General, Appearance, Diffs, Notifications, Accounts, Tools; Repositories - Branches, Landing, Agents), the pages of the git config under the scope tabs All repositories | the selected repository, opened by the main window from the menu, the keys and the buttons that name a page; the order of the branch prefixes.
style: plain-1
targets:
  - ../../src-tauri/src/menu.rs
  - ../../src-tauri/src/lib.rs
  - ../../src-tauri/src/settings.rs
  - ../../crates/app-api/src/app/settings.rs
  - ../../crates/app-api/src/events.rs
  - ../../crates/app-settings/src/service.rs
  - ../../crates/app-settings/src/file.rs
  - ../../crates/app-settings/src/lib.rs
  - ../../crates/bd-core/src/context.rs
  - ../../src-tauri/src/title_bar.rs
  - ../../crates/test-server/src/state.rs
  - ../../crates/test-server/src/host.rs
  - ../../src/stores/settings/repositoryState.ts
  - ../../src-tauri/src/zoom.rs
  - ../../crates/app-api/src/app/windows.rs
  - ../../src-tauri/src/desktop.rs
  - ../../src-tauri/capabilities/settings.json
  - ../../src-tauri/capabilities/sub-windows.json
  - ../../src-tauri/capabilities/title-bar.json
  - ../../src/lib/settings-window.ts
  - ../../src/lib/settings-pages.ts
  - ../../src/lib/zoom.ts
  - ../../src/lib/sub-window.ts
  - ../../src/lib/main-menu.ts
  - ../../src/lib/menu.ts
  - ../../src/lib/pull-request.ts
  - ../../src/router.ts
  - ../../src/pages/SettingsPage.tsx
  - ../../src/pages/IndexPage.tsx
  - ../../src/components/ui/tabs.tsx
  - ../../src/components/settings/SettingRow.tsx
  - ../../src/components/settings/ScopedConfigRow.tsx
  - ../../src/components/settings/ScopeTabs.tsx
  - ../../src/components/settings/ConfigControls.tsx
  - ../../src/components/settings/AccountsPage.tsx
  - ../../src/components/settings/BranchesPage.tsx
  - ../../src/hooks/useScopedConfig.ts
  - ../../src/hooks/useSettingsScope.ts
  - ../../src/hooks/useConfigDraft.ts
  - ../../src/lib/settings-close.ts
  - ../../src/components/settings/GeneralPage.tsx
  - ../../src/components/settings/AppearancePage.tsx
  - ../../src/components/settings/DiffsPage.tsx
  - ../../src/components/settings/AgentsPage.tsx
  - ../../src/components/shared/InstallAgentCliButton.tsx
  - ../../src/hooks/useAgentCliStatus.ts
  - ../../src/components/settings/ToolsPage.tsx
  - ../../src/hooks/useConfigWrite.ts
  - ../../src/components/shared/DiffViewToggle.tsx
  - ../../src/components/settings/NotificationsPage.tsx
  - ../../src/components/settings/GitHubSettings.tsx
  - ../../src/components/settings/YouTrackSettings.tsx
  - ../../src/stores/auth.ts
  - ../../src/stores/youtrack-issues.ts
  - ../../src/stores/cli-tools.ts
  - ../../src/components/settings/LandingPage.tsx
  - ../../src/hooks/useRepositoryConfigChanges.ts
  - ../../src/components/settings/BranchPrefixSetting.tsx
  - ../../src/components/repository/YourTurnCard.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/BranchPrefixHelp.tsx
  - ../../src/components/shared/ShortcutHint.tsx
  - ../../src/hooks/useZoom.ts
  - ../../src/hooks/useTheme.ts
  - ../../src/lib/theme.ts
  - ../../src/index.css
  - ../../src/stores/ai-suggestions.ts
  - ../../src/stores/settings/appSettings.ts
  - ../../src/stores/settings/repositorySettings.ts
  - ../../src/stores/settings/persistent-store.ts
  - ../../src/stores/repository.ts
  - ../../crates/sync-core/src/branch_prefix.rs
  - ../../crates/app-settings/src/resolve.rs
  - ../../crates/app-settings/src/branch_prefixes.rs
  - ../../crates/app-api/src/context.rs
  - ../../crates/app-api/src/reads/repository.rs
  - ../../crates/app-settings/src/windows.rs
  - ../../crates/domain-types/src/settings.rs
  - ../../crates/sync-core/src/repository_config.rs
  - ../../crates/sync-core/src/config_snapshot.rs
  - ../../crates/app-api/src/app/repository_config.rs
  - ../../src/lib/repository-config.ts
  - ../../src/lib/branch-prefix.ts
  - ../../src/hooks/useRepositoryConfig.ts
  - ../../src/hooks/useStackIndependence.ts
  - ../../src/components/branch/SafePushPopover.tsx
  - ../../tests/e2e/setup/tauri-mocks.ts
  - ../../tests/e2e/helpers/settings-helpers.ts
  - ../../tests/e2e/helpers/sub-window-helpers.ts
---

# Settings

Status: Implemented
Date: 2026-10-17

## Summary

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

### The window

- Settings is the sub-window `settings` at `/settings` (`SettingsPage`, lazy), titled "Settings", 880 × 640 every time
  it opens (`SETTINGS_WINDOW_SIZE`): the window-state plugin does not restore its size (`with_denylist` in
  `src-tauri/src/lib.rs`), so a size saved by a build of a smaller window never keeps its pages cramped. Its minimum is
  640 × 440, where a row puts its control under its label (a container query); a page needs 600 CSS px, so a zoom
  above 107% widens it: `max(640, 600 × level)` (`sub_window_min_size` in `src-tauri/src/zoom.rs`, `minSubWindowSize`
  in `src/lib/zoom.ts`, which `useZoom` sets by the label of the window).
  [@test] ../../src-tauri/src/zoom_test.rs (`the_settings_window_is_smaller_and_widens_above_107_percent`)
  [@test] ../../src/lib/zoom.test.ts (`is smaller for Settings, and wider above 107%`)
- Only the main window opens it (`openSettingsWindow` of `IndexPage`, the handler of `open_settings`); the window starts
  from the settings of the backend's writer as they are at the open. Every entry point asks the main window: the native menu item sends
  `open_settings` to the main window, `openSettings` (`runAppCommand`) runs there or sends it there. Opened again, the
  open window comes to the front with the new open.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)
- The window has the permissions of the sub-windows and of the title bar (`sub-windows.json`, `title-bar.json`), and
  those of its pages (`settings.json`): the test notification, the code of the GitHub sign-in on the clipboard, and
  the notification settings of the system (`x-apple.systempreferences:`, `ms-settings:`).
  [@test] ../../src-tauri/src/title_bar_test.rs (`every_window_gets_the_permissions_of_the_title_bar`)
- The window closes with the main window: when the main window is destroyed, every sub-window is (`close_sub_windows`
  in `src-tauri/src/lib.rs`), and the app quits with the last window. Without the main window a sub-window would wait
  for the answers of the main window.

### Entry points

- Settings… of the native menu (⌘, in the app menu on macOS; File → Settings…, Ctrl+, on Windows and Linux with the
  system title bar), in whichever window has the focus.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)
- The last control of the toolbar, after Sync, in every state (no repository, an invalid path): the `ghost` `icon-sm`
  Settings button, "Settings" and ⌘, (Ctrl+,) in its tooltip. It replaces the popover of Sync, whose one switch,
  Auto-sync on Focus, is on General. While something of the setup waits for the user, the button has a dot and lists
  it first ([Setup Dot](setup-attention.spec.md)).
  [@test] ../../tests/e2e/repository/header.spec.ts (`should open Settings from the button after Sync, its tooltip naming the key`, `should offer Settings before a repository is chosen`)
- ☰ File → Settings… and Ctrl+, where the page draws the title bar (Windows, Linux); Ctrl+, of a sub-window asks the
  main window.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should open from the keyboard and run the items`, `should take the keys of the native menu: Ctrl+R syncs, Ctrl+, opens the settings`, `should send Sync and Settings from a sub-window to the main window`)
- "Notification Settings…" of the Your Turn card opens Notifications.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open Settings at Notifications from the card menu, whose switches the main window keeps`)
- "Open Settings" of the branch prefix guide opens Branches, All repositories, with the field focused; the guide that
  Settings itself opens has no such button.
  [@test] ../../tests/e2e/welcome-card/welcome-card.spec.ts (`opens Settings at the field of the prefix from the guide, and at the one of the repository`)
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open the guide of the branch prefix without a button that opens Settings`)
- "Sign In…" of the GitHub hint (no token, or a rejected one) opens Accounts at GitHub (`account-github`); a build
  without the sign-in names it "GitHub Settings…". "YouTrack Settings…" of an issue and the Setup dot's YouTrack item
  open Accounts at YouTrack (`account-youtrack`); a CLI's item of the dot opens Tools at its row (`cli-teamcity`,
  `cli-youtrack`).
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`, `should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)
- "Set GitHub Repository…" of the Safe Push popover, on its review line of a repository only on Space whose GitHub
  repository is unknown ([Patronus](../branch/patronus.spec.md#code-owner-review)), opens Landing on the tab of the
  repository at the field of the GitHub repository; "Open <repository>" of the prefix of all repositories (a repository
  with a prefix of its own) and "Open Repository Settings" of the prefix guide open Branches on the tab of the
  repository at the field of its prefix. "Open Agent Settings" of the hints of the main window open Agents on the tab of
  the repository at the rule they name; its toast after Install Agent CLI opens Agents.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should name the approvals a Safe Push needs on Space, and start with the merge request found open`)
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should name the branches by the repository's own prefix over the app's, in the main window too`)
  [@test] ../../tests/e2e/welcome-card/welcome-card.spec.ts (`opens Settings at the field of the prefix from the guide, and at the one of the repository`)
- An open names its scope (`SettingsTarget.scope`, `"repository"` for a field of the repository); without one it shows
  All repositories, never the scope of an open before, where a change could go to the repository unseen.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open a page of the git config at the scope an open names, else at All repositories, never the one of before`)
  [@test] ../../src/lib/settings-window.test.ts (`shows all repositories unless an open names the repository, never the scope of an open before`)
- Every entry point shows `SettingsIcon` ([UI Theme](../../docs/design/ui-theme.md): one icon per concept).

### Pages

- The sidebar is a vertical Base UI `Tabs` list on the canvas (`TabsList variant="list"`) in two groups, each under its
  muted name: "Branch Deck" (General, Appearance, Diffs, Notifications, Accounts, Tools) and "Repositories" (Branches,
  Landing, Agents), the selected one in the selected row of the tables (`bg-primary/15`). ↑ and ↓ switch the page
  (`activateOnFocus`). The page is one island (`Card`), which scrolls where its tallest state or a large zoom leaves it
  no room; the page left is hidden at once (`data-ending-style`), so two never stack.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)
- General: "Sync", Auto-sync on Focus, the setting of the item of the File menu, whose check the main window keeps;
  "AI", Suggest branch names ([Branch Name Suggestions](../branch/branch-name-suggestions.spec.md)).
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)
  [@test] ../../tests/e2e/inline-branch-creator-ai/inline-branch-creator-ai.spec.ts (`should turn AI on and off in Settings`)
- A row has what it does on the left and its control on the right (a switch, the buttons of a few values, a field), as
  in IntelliJ and System Settings; below 28rem of the row the control goes under its label.
- Appearance: "Theme", System · Light · Dark (`appSettings.theme`, System by default): every window shows the tokens
  of the choice at once (`data-theme` of the root, `useTheme`), System those of the system, again whenever the system
  changes; the frame of the window follows (the theme of Tauri, `null` for System), set only once the choice differs
  from the frame's.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should set the theme of every window from Appearance, System following the system`)
  [@test] ../../src/lib/theme.test.ts (`follows the system for System, and keeps Light and Dark whatever the system shows`)
- Appearance: "Zoom", Zoom Out · the level · Zoom In, then Actual Size: the items of the View menu by id
  (`zoomMenuItems`), their keys in the tooltips, the same command, so every window zooms one step
  ([Zoom](zoom.spec.md)); "Title bar" (Windows, Linux), the system title bar ([Title Bar](title-bar.spec.md#setting)).
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should zoom every window from Appearance with the items of the View menu`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should offer the system title bar in Settings → Appearance, applied after a restart`, `should show a fixed system title bar in the settings with its reason`)
- Diffs: one setting of every diff view in each row, as IntelliJ's diff settings: Layout (Split · Unified · Inline)
  and Whitespace (the menu above the diffs, which says the choice here), the controls above every diff too; then "Soft
  wraps" and "Ligatures", each with what it does ([Commit Files](../commit/commit-files.spec.md#soft-wraps)).
  [@test] ../../tests/e2e/commit-files/commit-files.spec.ts (`should break a long line with Soft Wraps on in Settings, and scroll it with them off`)
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should set every diff from Diffs, the Whitespace menu naming its choice`)
- Notifications: the four switches of the OS notifications, the permission of the OS, and "Dock icon" (not Windows)
  ([Review Requests](review-requests.spec.md#settings)). The permission is asked when the page shows, on each open of
  the window and when the window gets the focus.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should ask the OS for the permission to notify again when the window gets the focus`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open Settings at Notifications from the card menu, whose switches the main window keeps`, `should count the review requests, behind its switch`)
- Accounts: the sign-ins of the app. "GitHub", the token and the app's own sign-in ([GitHub
  Sign-in](github-sign-in.spec.md)), asked again on each open of the window and when any window signs in or out
  (`get_auth_status`: at the start of the window, again on each later open, `useAuthStatusOnOpen` of the Settings page,
  and on `auth-changed`, `useAuthSync` of every window); "YouTrack", the sign-in to the YouTrack of the issue links
  ([YouTrack](youtrack.spec.md)), the `youtrack` part of the same status, read with GitHub's in one read. The tab has
  the dot of the setup while the app's YouTrack sign-in stopped working ([Setup Dot](setup-attention.spec.md)).
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`, `should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)
  [@test] ../../tests/e2e/youtrack/youtrack-sign-in.spec.ts (`should sign in in the browser to the YouTrack the issue links name, and sign out`, `should ask to sign in again once the sign-in expired`)
- Tools: first `brd`, whose skill coding agents read: its path with a copy button and the skill files, paths under the
  home directory as `~` (`agent_cli_status`, asked on each open of the window and when it gets the focus), or "The agent
  skill is not installed" with Install Agent CLI, the install of Help → Install Agent CLI…, whose toast has no action
  here; then the `teamcity` and `youtrack` CLIs, their version, sign-ins and the action each needs
  ([Command-Line Tools](cli-tools.spec.md)). The sign-ins of the CLIs are their own, not those of Accounts.
  [@test] ../../tests/e2e/settings/agents-settings.spec.ts (`should install the agent skill from the page, which Tools then names`)
- Branches, Landing and Agents: the git config ([Scope](#scope), [Branches](#branches), [Landing](#landing),
  [Agents](#agents)).
- A page that reads a status (Accounts, Tools) says it reads until the first answer ("Reading the GitHub sign-in…",
  "Reading the YouTrack sign-in…", "Reading the tools…", once the read took 300 ms, `LoadingLine`), never an
  empty section or a spinner without end. A failed read says why, with Retry ("Cannot read the GitHub sign-in" and the
  reason, `ReadFailure`), above the last status when one is known, until a read answers (`readError` of each store);
  Retry says it reads again until the answer.
  [@test] ../../tests/e2e/settings/status-reads.spec.ts (`should say it reads the GitHub sign-in until the answer, and a failed read with Retry`, `should say it reads the YouTrack sign-in until the answer, and a failed read with Retry`, `should say it reads the tools until the answer, and a failed read with Retry`)
  [@test] ../../src/stores/auth.test.ts (`says why a read failed and keeps the last status, until a read answers; Retry reads again, the failure gone meanwhile`)
- Every page but Tools and Landing fits 880 × 640 without scrolling in its tallest state, a page of the git config on
  both tabs: the code of the GitHub sign-in, the warning that the OS refused the notifications (one line with Open
  System Settings), the Dock group on macOS and Linux, every line a row can show (the repository's own value with
  Reset, the system config, a value of the worktree config, an invalid value, the pull request off by the route). Tools
  with a CLI on two servers and the install command of the other, and Landing with the companions of a repository, may
  scroll. No page is wider than the island, also at 640 × 440.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should fit every page at 880 × 640 without scrolling`, `should fit every page at 880 × 640 without scrolling, the Dock badge too`, `should keep every page within the island at the smallest window`)

### The page an open shows

- An open says what it shows (`SettingsTarget`, the data of the window): `{ page?, scope?, focus?, server? }`. With a
  page it shows that page; without one it keeps the page shown, and a new window opens at the page shown last
  (`appSettings.settingsPage`, General by default). A page of an older build is the page that has its settings now
  (`github` and `youtrack` Accounts, `repository` Branches, `companions` Landing; `currentSettingsPage`, also for the
  saved page), one the sidebar doesn't know is General. A new open switches the page and the scope during the render,
  not in an effect; the saved page follows the page shown, the scope is never saved.
  [@test] ../../src/lib/settings-window.test.ts (`shows the page an open names`, `keeps the current page for an open without one`, `shows the page that has the settings of a page of before, and General for a page the sidebar doesn't know`)
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should show the page that has the settings of a page of an older build`)
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)
- Each open focuses the field it names on the page it shows (`data-settings-focus`: `branchPrefix` on Branches,
  `githubRepository` and `landing` on Landing, a rule on Agents, once their settings are read and the field is enabled;
  `account-github` and `account-youtrack` on Accounts; `cli-brd`, `cli-teamcity` and `cli-youtrack` on Tools), else
  the tab of the page, once Base UI registered the tabs: a tab focused before keeps a stale highlight, and the first ↓
  would go nowhere. A field the page never shows is waited for 5 s at most.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should focus the field an open names`, `should open at the page an open names, else at the page shown last, its tab focused`)
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should name the branches by the repository's own prefix over the app's, in the main window too`)

### Keys

- Escape closes the window, as in every sub-window; in a field with typing it takes the typing back first, and in an
  open popover it closes the popover only (Base UI prevents the event). The footer says `Esc Close`; no ↑↓ hint, which
  holds only while the sidebar has the focus.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should close on Escape, which takes the typing of a field back first`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request`)

### Writes through the backend

- Every window writes the settings through the one writer of the backend (`set_setting`, `SettingsService`), which
  applies them in the order they arrive, sends each change to every window (`settings_changed`, with its version and
  the window that asked) and saves `settings.json` within 500 ms, with a temporary file renamed over it
  ([Distributed Settings](../../docs/design/distributed-settings.md)): the zoom zooms every window, Auto-sync on Focus
  checks the item of the native menu (`set_auto_sync_menu_checked`), a changed AI mode clears the last AI failure of
  every window (`ai-suggestions.ts`). The writer refuses a value of a field it reads that is invalid (a zoom that is no
  positive number), and the window shows the value the writer has, with "Cannot change the settings".
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should save the diff view the window chooses through the writer of the backend`)
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should zoom every window from Appearance with the items of the View menu`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)
  [@test] ../../crates/app-settings/src/service_test.rs (`the_changes_are_saved_in_the_order_they_arrive_so_the_last_one_is_on_disk`, `an_invalid_value_of_a_field_rust_knows_is_refused_and_changes_nothing`)
  [@test] ../../src/stores/settings/persistent-store.test.ts (`puts the writer's value back when it refuses a change, and says so`)
- A window's write changes only the settings it changed (the fields of an object, an array whole): a setting another
  window changed a moment before (the Activity panel opened in the main window) stays, also when the window had not
  heard of it yet. A window sends its next change after the answer of the one before, so the writer gets them in
  order, and applies only a change newer than the last one it applied: a late answer or echo never rolls a setting
  back, and a change equal to a value the window had before still arrives (Actual Size after a review window hid its
  sidebar at 110%). The Activity log, the robot history and the state of the repositories reach only the main window.
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should keep the Activity panel the main window opened when a review window hides its sidebar right after`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should follow Actual Size of the main window in a review window that changed another field in between`)
  [@test] ../../crates/app-settings/src/service_test.rs (`two_origins_that_patch_two_fields_of_one_key_at_once_keep_both`, `a_change_older_than_the_applied_version_is_an_echo`, `the_changes_since_a_snapshot_name_each_key_changed_after_it_once_with_its_value_now`)
  [@test] ../../crates/app-settings/src/windows_test.rs (`a_sub_window_gets_no_key_only_the_main_window_keeps`)
  [@test] ../../src/stores/settings/persistent-store.test.ts (`sends the fields a change made, not the whole object, a field back at its default as null`, `sends the next change only after the answer of the one before, the changes made meanwhile as one`)
  [@test] ../../src/stores/settings/persistent-store.test.ts (`keeps the field of its own change the writer has not answered when another window's change of the key arrives`, `never rolls a key back with a late echo of an older change`)
  [@test] ../../src/stores/settings/persistent-store.test.ts (`sends an array whole, an empty one as no value`, `starts a key only the main window keeps empty in a sub-window and takes none of its changes`)
- A change made while a window's page loads reaches no listener of it, so every page takes the changes after the
  version of the snapshot it started from, by the same versions: a sub-window with its load (`sub_window_loaded`
  answers the changes after the version its build remembered, `SubWindowState::built`), applied before its content
  settles and the window shows (`sub_window_ready`); the main window once it listens (`get_settings_changes`; its
  reload starts from the snapshot of the app's start). Until it caught up, the main window holds its own changes; then
  it sends them, a field on top of the writer's value, and drops a whole array (the recent projects) the writer has a
  newer value of, which the array built on the old snapshot would overwrite. A zoom changed while a review window
  loaded is the window's when it shows, and the Activity panel the main window opened meanwhile stays when the review
  window changes another field. The window's webview starts at the zoom of its snapshot, which its page changes when
  the level it has differs (`zoomAtStart`, `useZoom`).
  [@test] ../../tests/e2e/settings/settings.spec.ts (`zooms a review window whose page loaded after Actual Size`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should keep the Activity panel the main window opened when a review window hides its sidebar right after`)
  [@test] ../../crates/app-api/src/windows_test.rs (`a_page_that_loads_takes_the_changes_after_the_snapshot_of_its_build`, `a_page_that_loads_takes_no_change_older_than_its_snapshot_nor_a_key_only_the_main_window_keeps`)
  [@test] ../../src/lib/sub-window.test.ts (`takes the settings changed while the page loaded once it listens, before it says its shell and its content`)
  [@test] ../../src/stores/settings/persistent-store.test.ts (`takes the changes the load of the window hands over by their versions, before its store exists too`)
  [@test] ../../src/stores/settings/persistent-store-main.test.ts (`holds its changes until it caught up with the writer, and drops a whole array the writer has a newer one of`)
- The file keeps only what differs from the defaults: a field back at its default is removed, and an object or an
  array left empty removes its key. Keys the backend does not know are kept as they are. The defaults of the fields
  Rust reads (the app's prefix, the new Patronus API, the zoom, the sync on focus, the system title bar) are Rust's
  alone: the page takes them from the bindings (`APP_SETTINGS_DEFAULTS`), so a value the page sends back as its default
  is the one Rust and `brd` read as the default too.
  [@test] ../../crates/app-settings/src/service_test.rs (`a_field_patch_merges_into_the_key_and_a_null_field_is_removed`, `a_field_patch_that_removes_the_last_field_removes_the_key`)
  [@test] ../../crates/app-settings/src/file_test.rs (`unknown_keys_round_trip`)
  [@test] ../../src/stores/settings/appSettings.test.ts (`takes the defaults of the fields Rust reads from Rust, so a value back at its default means the same to both`)
  [@test] ../../src-tauri/src/bindings_test.rs (`the_bindings_are_what_the_export_writes`)
- What each sync records of a repository (`repositoryState` by its path: when the last sync ended, how many branches it
  had, how long the steps of a sync and of Update Project took, when Branch Deck fetched the baseline last, up to when
  the Landed island hides what landed) lives in a file of its own, `repository-state.json`
  beside `settings.json`, behind the same writer and its versions, and only the main window has it: a sync writes that
  file alone, never `settings.json`, which `brd` reads beside it. A `settings.json` of before that kept these fields in
  its recent projects is read once: the writer moves them (a value the state file has for the repository wins), saves
  the state file, and `settings.json` loses them with its next save; every other key of it is kept.
  [@test] ../../crates/app-settings/src/service_test.rs (`a_patch_of_the_repository_state_leaves_the_settings_file_untouched`, `a_settings_file_of_before_moves_the_fields_of_a_sync_to_the_repository_state_and_drops_them_on_its_next_save`, `the_moved_values_of_a_settings_file_of_before_are_saved_without_a_change`)
  [@test] ../../crates/app-settings/src/file_test.rs (`the_fields_of_the_repository_state_leave_the_recent_projects_by_path_and_nothing_else_changes`)
  [@test] ../../crates/app-settings/src/windows_test.rs (`a_sub_window_gets_no_key_only_the_main_window_keeps`)
  [@test] ../../src/stores/settings/repositoryState.test.ts (`sends the writer only the repository a sync changed, as one field of repositoryState`, `drops a malformed field or repository, never the others`)
- Every window starts from the writer's copy of the settings, set before its page's scripts (`__TAURI_STORE__`, with
  its version `__TAURI_STORE_VERSION__`; `init_script` of `src-tauri/src/settings.rs`), and with the defaults from a
  settings file that does not parse (one cut within a write): it never starts without its settings because of a syntax
  error in its init script.
  [@test] ../../crates/app-settings/src/service_test.rs (`the_snapshot_of_a_cut_file_is_a_json_object_of_the_defaults`)
- A failed save keeps the settings in memory, is tried again by itself (the delay doubling up to a minute) and by the
  next change, and the main window says so once per streak of failures ("Cannot save the settings",
  `settings_save_failed`). The app saves what is unsaved before it quits (`RunEvent::ExitRequested`) and before Restart
  of the title bar setting (`restart_app`).
  [@test] ../../crates/app-settings/src/service_test.rs (`an_unwritable_directory_reports_the_error_once_a_streak_and_memory_keeps_the_value`, `a_failed_save_is_retried_by_itself_once_the_directory_is_writable_again`, `a_flush_soon_after_a_patch_saves_it_without_waiting_for_the_debounce`)
  [@test] ../../src/stores/settings/persistent-store-main.test.ts (`tells the user in the main window that the settings could not be saved`)
- The pages read only what every window has: the settings and the backend. The branch prefix field asks Rust for the
  prefix of the selected repository itself (`resolve_branch_prefix`, again on focus and once the writer has another
  app prefix), and stays enabled while a sync runs: a sync resolves the prefix when it starts, so a change names the
  branches of the next one.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`)

### Scope

- Branches, Landing and Agents show the `branchdeck.*` git config, which `brd` reads too (`get_repository_config`, one
  `git config --show-scope --get-regexp`, [Git Operations](../../docs/design/git-operations.md#repository-settings)),
  read when the page shows, when the window gets the focus (a `git config` or `brd` meanwhile) and on a switch of the
  repository. Above the page the scope tabs (Base UI `Tabs`, ←/→): "All repositories" and the name of the selected
  repository (its path in the tooltip), with the line under them: "What every repository uses, in the app and brd", or
  "Changes here apply only to <repository>, saved in its .git/config". Without a repository there are no tabs, only
  the value of every repository (the global config alone); a failed read is a `destructive` callout ("Cannot read the
  settings of <repository>", "Cannot read your global Git config") with the reason and Retry. A switch of the repository
  in the main window flashes its name in the tab and says it to a screen reader.
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should show only the value of every repository without a repository, and say how to set the rest`, `should write nothing when the repository is switched while a field holds typing`, `should save Report Only, its source this repository, which the main window syncs, and read it again when the window gets the focus`)
- On "All repositories" a row shows the value of every repository (`everyRepositoryValue`: the global or system entry,
  else the default, whatever the repository sets) and a change writes the global config (`set_repository_config` with
  `scope: "global"`). A row says only what differs (`describeScopedSource`): "<repository> has its own value: Off" with
  Open <repository>, which shows its tab, or "Set in the system Git config; a change goes to your global Git config,
  which wins over it". The prefix of the app and the API of Patronus, settings of the app that every repository uses,
  are on this tab too.
- On the tab of the repository a row shows the value that holds there and a change writes its `.git/config`. The value
  of every repository has no line, as an unmodified setting of VS Code; the repository's own value has a bar beside the
  row and "Only in <repository>" (or "Same as all repositories", which can go) with Reset, which removes it there, its
  tooltip the result ("Removes it from .git/config: back to On, from all repositories", "… back to Off, the default").
  A row only a repository has (the route, the GitHub repository) says its default instead ("Detected from origin",
  "Pull Request by default: the repository is not on Space"), and a global value of it "Set in your global Git config,
  which applies to every repository".
  [@test] ../../src/lib/repository-config.test.ts (`says on the tab of the repository when the value is its own, with what Reset gives back`, `says on the tab of every repository where the selected one keeps a value of its own, and the system config`, `shows on the tab of every repository its value, whatever the repository sets`, `names the default of a row only a repository has, and a global value of it as one of every repository`)
  [@test] ../../tests/e2e/settings/agents-settings.spec.ts (`should change a rule for every repository in the global config, and give the repository its own value`)
- A value of the worktree config or of `git -c` and `GIT_CONFIG_*` wins over `.git/config`: on the tab of the
  repository its control is disabled with that reason ("Set in config.worktree, which overrides .git/config: change it
  there", "Set by git -c or GIT_CONFIG_*, which overrides .git/config"), as the backend refuses the write; the value of
  every repository can still change. An invalid value says why, where it holds; a boolean is invalid exactly when `git
  config --type=bool` refuses it (` true `, `08`, `1kb`), and `0x10`, `1k` or `-1` are true as git reads them; `0b1`
  is invalid on every platform, though the git of glibc 2.38+ reads it as true.
  [@test] ../../src/lib/repository-config.test.ts (`can't change a key the worktree config or the command line sets`)
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_fallback_is_the_last_global_or_system_value_parsed`, `a_value_of_the_worktree_config_refuses_the_write`)
  [@test] ../../crates/sync-core/src/config_snapshot_test.rs (`a_boolean_is_what_git_makes_of_it`)
  [@test] ../../tests/e2e/settings/agents-settings.spec.ts (`should disable the rule of the repository while its worktree config sets it, and say so`)
- The git key of a row is its copy button beside the label ("Copy branchdeck.agentLease"), which the keyboard reaches.
- A switch or a button writes at once, a field on Enter and when it is left, never on a key, and Escape takes the
  typing back before it closes the window. The typing is checked before a write as the backend checks it
  (`isValidGitHubRepository`, `isValidBranchPrefix`, `isValidPushSizeLimit`), refused in place by what is wrong,
  without "Cannot …", since nothing was written: "acme/app/x is not a GitHub repository: use owner/name", for a prefix
  the words of the prefix of all repositories (`INVALID_PREFIX_TEXT`). A field writes to the repository the typing began
  in: a switch of the repository or of the tab drops the typing (one page per repository and tab, `key`) and writes
  nothing. A window closed with valid typing (⌘W, the close button) saves it first: one listener of the window waits for
  the saves the fields registered (`saveBeforeClose`).
  [@test] ../../src/lib/repository-config.test.ts (`takes owner/name without whitespace, as the backend does`)
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should refuse a GitHub repository that is not owner/name in place, and save one that is, which the main window asks GitHub about`, `should write nothing when the repository is switched while a field holds typing`, `should save valid typing when the window closes, and nothing else`)
- Two writes at once both land: a write of the app never fails on another write of the app, and a `config.lock` of
  another process is waited for up to a second.
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`two_writes_at_once_both_succeed`, `a_write_waits_for_a_config_lock_another_process_holds`, `a_write_fails_on_a_config_lock_that_stays`)
- Answers in order: every answer of the settings, of a read and of a write, counts the config writes of the app before
  its read (`seq`, one counter of the process for the local and the global config), and the page shows an answer only
  if it counts as many as the settings shown or more (`newer`). So a prefix saved while a mode's write runs is not
  taken back when the mode answers last, with the prefix it read before, nor by a read a focus started before a
  write; a read again of as many writes is taken, with what a `git config` or `brd` changed meanwhile. A failed write
  changes nothing shown, and an answer for another repository is dropped.
  [@test] ../../src/lib/repository-config.test.ts (`keeps the newest of answers that arrive in reverse order`, `takes an answer of as many writes: a read again finds a git config of meanwhile`)
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should keep a prefix saved while a mode's write runs, when the mode's older answer comes last`)
- A failed write is a toast in the words of the change (`notifyError`): for the repository "Cannot change the symbol
  dependencies of app", "Cannot change the GitHub repository of app", "Cannot change the branch prefix of app" (and the
  rest of `writeErrorTitle`), for every repository "Cannot change how every repository resolves conflicts by words",
  "Cannot change the push size limit of every repository" (`globalWriteErrorTitle`).
  [@test] ../../src/lib/repository-config.test.ts (`titles a failed write by what it changes`)
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should tell a failed write in a toast`)
- After a write the backend tells the main window (`repository_config_changed` `{ repositoryPath, key }`), which acts
  for the selected repository only: the branch prefix loads again, and a changed one is another session, which the
  window opens and syncs for a repository synced before ([Onboarding](onboarding.spec.md)); the symbol mode, the
  resolve by words, the review plan and the route a sync, and the stack-independence checks of the Safe Push popover
  ask again; the GitHub repository a sync and the pull requests (`refreshPullRequests("manual")`); the push size limit
  and the rules of agents nothing (a push reads the limit as it starts, only agents read the rules). A repository never
  synced only loads its prefix: its first sync is the user's. A write waits for no sync: during one it is written at
  once, and the sync it asks for runs once after that one.
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should turn the symbol dependencies off, and back to the default, the main window syncing each change`, `should write the symbol dependencies during a held sync at once, the main window syncing it once after that sync`, `should save Report Only, its source this repository, which the main window syncs, and read it again when the window gets the focus`, `should refuse a GitHub repository that is not owner/name in place, and save one that is, which the main window asks GitHub about`, `should name the branches by the repository's own prefix over the app's, in the main window too`, `should open and sync the session of a prefix written here, for a repository synced before`)

### Branches

- "Names": on All repositories the prefix of every repository without one of its own (`BranchPrefixSetting`,
  `globalUserBranchPrefix` of the settings, which `brd` reads too; [Onboarding](onboarding.spec.md), [Branch prefix
  order](#branch-prefix-order)), which says a repository's own prefix with Open <repository>; on the tab of the
  repository "Branch prefix" ("Used here instead of the prefix of all repositories", `branchdeck.branchPrefix`), whose
  placeholder and Reset give the prefix of all repositories back ("Removes it from .git/config: back to jane, the
  default"): the prefix of the app wins over a global or system git one (`withAppPrefix`).
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`)
  [@test] ../../src/lib/repository-config.test.ts (`takes the prefix of all repositories over a global git one, as the branches are named`)
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should give the prefix back to all repositories once the repository's own is unset, and say so`)
- "Sync": "Symbol dependencies", Off · Report Only · On (`aria-pressed`, the sentence of the mode chosen: "Commits
  depend only on the lines they change", "Shown by brd commits, never used for the branches", "Commits also depend on
  the commits that add the symbols they use"; `branchdeck.symbols`, [Symbol
  Dependencies](../commit/symbol-dependencies.spec.md)); "Resolve conflicts by words", `branchdeck.resolveByWords`, on by
  default (the sync, Update Project and `brd update` commit a conflict of git that its words resolve, [Git
  Operations](../../docs/design/git-operations.md#resolve-by-words)).
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should resolve conflicts by words in the global config, and say where the repository's own value wins`)
- "Review": "Review plan (experimental)", `branchdeck.reviewExperimental`, off by default: Mark Reviewed, Since Your
  Review, the overview of the plan and the marks of the files in the branch review ([Review
  Plan](../branch/review-plan.spec.md)); a review window opened after the sync takes the new value.
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should turn the review plan on and off for every repository from Branches`)

### Landing

- On the tab of the repository, "Route into master" ("How the work of the repository lands into master, in the app and
  brd"): "Land by", Safe Push · Push · Pull Request (`branchdeck.landing`; `patronus`, `push`, `pull-request`), the
  route in effect pressed and its sentence under the label ("A push of its copy lands each piece, without a robot or the
  CI of a pull request; a straight stack is a stack of GitHub"), which says where the pull requests of a stack go: into
  master on Safe Push, a stack of GitHub off it ([Stacks of
  GitHub](../branch/stacked-branches.spec.md#stacks-of-github)). Unset, the default of the host of `origin` is pressed
  and the line says why (`RepositoryConfig.landingDefault`, Rust's one default: "Safe Push by default: the repository
  is on Space", "Pull Request by default: the repository is not on Space"); a click on it names it in `.git/config`, and
  Reset removes it ("Removes it from .git/config: back to Pull Request, the default off Space"). After a write the main
  window syncs a repository synced before, so its rows take the steps of the route. The window has no session of the
  repository: the write access and the rules of the target are said by the step of each row, not here. Then "GitHub
  repository" (`owner/name`, "Where pull requests are opened", the detected one as the placeholder and its line,
  "Detected from origin", "Detected from the Space mirror", "None detected: neither origin nor its Space mirror is on
  GitHub"; `branchdeck.githubRepository`, [Pull Requests](../branch/pull-requests.spec.md)).
  [@test] ../../tests/e2e/settings/landing-settings.spec.ts (`should land the repository by a push into master once chosen, the main window taking the step, and back to the default`)
  [@test] ../../src/lib/repository-config.test.ts (`says where the GitHub repository comes from while no config sets it`)
- A route is the repository's own: the tab of every repository shows no route, GitHub repository nor companions, only
  "The route into master, the GitHub repository and the companions are set for each repository" with Open
  <repository> ("Select a repository to set its route into master, its GitHub repository and its companions" without
  one). The backend refuses a write of `branchdeck.landing` to the global config whatever its value: a global
  `patronus` would refuse every repository off Space (`notOnSpace`), a global `pull-request` would take Safe Push from
  the monorepo; a global `push` reads as unset with its reason ([Landing Routes](../branch/landing-routes.spec.md)).
  The route is read and written with the other keys (`RepositoryConfig.landing`, the change `landing`; "Cannot change
  how app lands into master"), and a sync and every `brd` command read it in their one read of the config.
  [@test] ../../tests/e2e/settings/landing-settings.spec.ts (`should refuse a route for every repository, which would break the repositories off Space`)
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_route_is_one_of_three_and_a_push_of_the_global_config_is_unset`, `a_route_is_written_only_to_the_local_config`)
  [@test] ../../crates/sync-core/src/config_snapshot_test.rs (`the_route_comes_from_git_config_and_is_none_without_it`, `a_push_route_of_the_global_config_is_none`)
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`the_default_route_follows_the_host_of_origin_in_the_one_read_of_its_push_url`)
- "Companions" on the tab of the repository: the repositories whose branches land in an order with it
  ([Declared Needs](../branch/declared-needs.spec.md#companion-repositories), `CompanionsSetting`).
  [@test] ../../tests/e2e/settings/companions-settings.spec.ts (`should add a companion repository from the folder dialog, refuse a folder that is no repository, and remove it`)
- "Push" ("A push of the app or brd that adds more files is refused before it is sent") on both tabs: "Push size
  limit", a size (`50M` the placeholder and the default, `1G`, `512K`), `0` or `off` for none
  ([Outgoing](outgoing.spec.md#push)), the value of every repository with the repository's own as every key of the git
  config ([Scope](#scope)). Typing the backend would refuse is refused in place (`isValidPushSizeLimit`): "lots is not
  a size such as 50M or 1G, nor 0 or off", the backend's words for an invalid value too. A write syncs nothing: a push
  reads the limit as it starts.
  [@test] ../../src/lib/repository-config.test.ts (`takes a size with a binary unit, 0 or off, as the backend does`, `titles a failed write by what it changes`)
  [@test] ../../crates/sync-core/src/repository_config_test.rs (`invalid_values_are_refused_before_git_runs`, `the_push_size_limit_is_a_size_or_off_as_written`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should refuse a push over the push size limit before it is sent, saying what it adds, and push once the limit is off`)
- "Safe Push" on All repositories, how Branch Deck and brd read the robots of Patronus: the switch "New Patronus API"
  (`appSettings.patronusNewApi`, off by default, a setting of the app): the progress of a running robot in one request
  and its merge profile in the robot list, the old requests while Patronus has them off
  ([Patronus](../branch/patronus.spec.md)). `brd` reads the setting from the settings file ([Agent
  CLI](../../docs/contributing/agent-cli.md)).
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`the_details_a_window_demands_ask_the_folded_progress_only_with_the_new_api_of_the_settings`)

### Agents

- One section, "Coding agents", "Agents such as Claude Code and Codex ask brd workspace before their first edit; what
  you tell an agent wins" ([brd workspace](../cli/workspace.spec.md)), with three rules, each a switch with what it
  does: "Work in its own worktree" ("Each agent leases a worktree; its commits arrive on your branch one by one",
  `branchdeck.agentLease`, on by default), "Name the branch in each commit subject" ("(auth) Add login form: the commit
  goes into the branch auth", `branchdeck.agentSubjectBranch`, on by default) and "Push the branch and open its pull
  request" ("After each commit arrives on your branch, the agent pushes it and opens its pull request without asking",
  `branchdeck.agentPullRequest`, off by default), each the value of every repository with the repository's own
  ([Scope](#scope)). No write syncs: only agents read the rules. A failed write is a toast: "Cannot change where agents
  work in every repository", "Cannot change how agents write commit subjects in <repository>", and so on.
  [@test] ../../tests/e2e/settings/agents-settings.spec.ts (`should change a rule for every repository in the global config, and give the repository its own value`, `should disable the rule of the repository while its worktree config sets it, and say so`)
- The pull request after a commit does not apply where the repository lands by a push: Rust says so
  (`RepositoryConfig.agentPullRequestOff`, the reason `brd workspace` gives too, [brd workspace](../cli/workspace.spec.md)),
  and the row shows the reason with Change Route, which opens Landing on the tab of the repository at "Land by"; on its
  tab the switch is off and disabled, on All repositories it stays the value of every repository and the line says "In
  <repository>: …".
  [@test] ../../tests/e2e/settings/agents-settings.spec.ts (`should turn the pull request of agents off where the repository lands by a push, with the way to its route`)
- While the agent skill is not installed (`agent_cli_status`), the page starts with "Agents read these rules once the
  skill is installed" and Install Agent CLI, whose toast has no action here; the skill itself is on Tools. The same
  status and button tell the main window where agents work on the way to the first branches
  ([Onboarding](onboarding.spec.md#agents)).
  [@test] ../../tests/e2e/settings/agents-settings.spec.ts (`should install the agent skill from the page, which Tools then names`)

### Branch prefix order

- The branches of a repository are named by the specific prefix first, in the app as in `brd`, resolved in Rust only
  (`resolve_branch_prefix` in `crates/app-settings/src/resolve.rs`): the repository's own `branchdeck.branchPrefix` (its
  local or worktree git config, or `git -c` and `GIT_CONFIG_*`), else the app's prefix (General,
  `globalUserBranchPrefix`), else the one of the global or system git config. Each is named as the branches are
  (trimmed, no trailing slash), and one that nothing is left of counts as unset. The resolved prefix names its source
  (`ResolvedBranchPrefix { prefix, source }`: `cli`, `repository_config`, `app_setting`, `global_config`, or `none`
  exactly when there is no prefix), as `prefix_source` of `brd status` does.
  [@test] ../../crates/app-settings/src/resolve_test.rs (`the_specific_prefix_wins`, `the_prefix_is_named_as_the_branches_are_and_one_of_slashes_only_is_unset`, `the_source_is_written_in_snake_case`)
- The prefix of git is read with its scope (one `git config --show-scope`; the global config alone for an empty path).
  [@test] ../../crates/sync-core/src/branch_prefix_test.rs (`a_local_prefix_is_repository_scoped`, `the_scope_of_the_prefix_is_read_with_it`, `a_prefix_of_git_c_is_repository_scoped`)
- The commands of the sync family take no prefix from the page: the session commands (`subscribe_view`,
  `get_snapshot`, `sync`, `set_window_focus`, whose session is keyed by the resolved prefix),
  `get_archived_branch_commits`, `get_unapplied_branch_commits` and `execute`, whose operation runs in the session of
  that key (a rewrite of the engine: the unapply, the reapply, the deletes of an archived and an unapplied branch),
  resolve it per command, with the app's prefix of the settings
  when the command starts and the git prefix of a memo by repository path (`BranchPrefixes` in
  `crates/app-settings/src/branch_prefixes.rs`, a state of the app and of the E2E server), so a command pays no git
  process for it. Each read of the git config keeps its git prefix there: the subscription of a window, which reads the
  config now (`session_key_now`), the `resolve_branch_prefix` of Settings, the config snapshot of every sync of a
  session, and a write of `branchdeck.branchPrefix` in
  Settings → Branches (a write to the global config forgets every repository's). The views of the last sync are kept
  by the resolved prefix; a sync of a repository without a prefix is refused ("Cannot sync without a branch prefix: set
  one in Settings → Branches"). The commands of Patronus, of a push, of a pull request, of the branch review and of the
  independence of a stack name the repository only too and resolve the prefix the same way (`PrefixInputs` of
  `crates/app-api/src/context.rs`, [Patronus](../branch/patronus.spec.md)). The conflict fix
  (`find_conflict_fix`), whose parameters `brd` shares, gets the prefix the page shows, the one of the view of its
  session.
  [@test] ../../crates/app-settings/src/branch_prefixes_test.rs (`git_is_read_once_per_repository_and_the_app_prefix_of_each_command_applies`, `a_failed_read_keeps_nothing`, `a_refresh_replaces_the_git_prefix_and_a_clear_forgets_every_repository`)
  [@test] ../../crates/test-server/src/branch_prefix_test.rs (`the_sync_names_the_branches_by_the_app_prefix_where_the_repository_sets_none`, `the_repository_s_own_prefix_wins_and_a_repository_without_any_is_not_synced`)
  [@test] ../../crates/sync-core/src/sync_inputs_test.rs (`changes_with_the_prefix_alone`)
  [@test] ../../src/model/store.test.ts (`names only the repository: the session resolves the prefix, and a sync waits for no load of the page`)
- The page shows the names with what Rust resolved, deciding nothing: the main window takes the prefix of the view of
  the session it subscribed to (`TableHeader.prefix`), which Rust resolves for each subscription from the git config
  as it is then, and reads none of its own. It subscribes again, holding the view it shows (`resubscribeSession`),
  after a write of the key in Settings → Branches (`repository_config_changed`), once the writer has another app
  prefix (`onWriterChange` of `src/stores/settings/persistent-store.ts`: the change of any window, its own once
  answered), and when a run of its session answers `prefix_changed`: the same session answers nothing new, the session
  of another prefix its view. Branches, All repositories, asks `resolve_branch_prefix` itself and shows the repository's own prefix and the
  global git one from the answer's source.
  [@test] ../../src/model/store.test.ts (`subscribes again holding its view: the same session answers nothing, another one's view replaces the rows`, `subscribes again when the git config names another prefix than the session's, quietly`)
  [@test] ../../src/stores/settings/persistent-store.test.ts (`tells a reader of Rust the value the writer has: another window's change, its own once answered, no late echo`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`names the branches by the repository's own prefix over the app's, and says so in Settings`, `names the branches by the app's prefix where the repository has none`)
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should name the branches by the repository's own prefix over the app's, in the main window too`)
- No window caches the prefix: the view of a session names it, the frame the warm start shows first too, and a switch
  forgets the prefix of the repository left with its session. The `cachedBranchPrefix` a settings file of before kept
  in each project of `recentProjects` is dropped by the load (`RETIRED_PROJECT_FIELDS` of `crates/app-settings/src/file.rs`),
  so the next save writes the file without it, and the page's schema keeps none.
  [@test] ../../crates/app-settings/src/file_test.rs (`the_cached_prefix_of_a_project_of_before_is_dropped_by_the_load_and_the_next_save`)
  [@test] ../../src/stores/settings/repositorySettings.test.ts (`keeps no prefix a window resolved before: the view of the session of the repository names it`)

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
  [@test] ../../crates/app-api/src/events_test.rs (`a_change_of_the_settings_goes_to_the_windows_that_get_its_key`)
  [@test] ../../crates/test-server/src/app_events_test.rs (`the_events_of_the_app_reach_the_pages_of_their_windows_and_end_with_the_repository`)
- `tests/e2e/settings/repository-settings.spec.ts`: the test server reads and writes the local config of the test
  repository and a global config of its own (`GIT_CONFIG_GLOBAL`, `AppState::global_config_of`), never the
  developer's or the system config; the sync of the repository does not read that global file. After a write the test
  server sends `repository_config_changed` to the pages of the main window (`set_repository_config` of app-api, as in
  the app).
  [@test] ../../crates/app-api/src/events_test.rs (`a_config_write_goes_to_the_main_window_and_a_journal_change_to_every_window`)
- By hand: `pnpm tauri dev`; ⌘, from the menu, from a review window and again with the window open (it comes to the
  front); every page at 100% and 150%; closing the main window closes Settings; Branches on the tab of a repository: a
  symbol mode syncs the main window, a prefix of the repository renames its branches, `git config --worktree` disables
  a row; Agents of a repository that lands by a push: the pull request rule off with Change Route.

## References

- [Zoom](zoom.spec.md), [Title Bar](title-bar.spec.md), [Review Requests](review-requests.spec.md),
  [GitHub Sign-in](github-sign-in.spec.md), [Onboarding](onboarding.spec.md).
- [Distributed Settings](../../docs/design/distributed-settings.md): the one writer of the settings and how every window
  follows it.
