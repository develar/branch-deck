---
name: Settings
description: Requirements for the Settings window, a sub-window of its own with a sidebar of pages (General, Appearance, Notifications, GitHub), opened by the main window from the menu, the keys and the buttons that name a page.
style: plain-1
targets:
  - ../../src-tauri/src/menu.rs
  - ../../src-tauri/src/lib.rs
  - ../../src-tauri/src/zoom.rs
  - ../../src-tauri/src/commands/window_management.rs
  - ../../src-tauri/capabilities/settings.json
  - ../../src-tauri/capabilities/sub-windows.json
  - ../../src-tauri/capabilities/title-bar.json
  - ../../src/lib/settings-window.ts
  - ../../src/lib/settings-pages.ts
  - ../../src/lib/zoom.ts
  - ../../src/lib/main-menu.ts
  - ../../src/lib/menu.ts
  - ../../src/lib/pull-request.ts
  - ../../src/router.ts
  - ../../src/pages/SettingsPage.tsx
  - ../../src/pages/IndexPage.tsx
  - ../../src/components/ui/tabs.tsx
  - ../../src/components/settings/SettingRow.tsx
  - ../../src/components/settings/GeneralPage.tsx
  - ../../src/components/settings/AppearancePage.tsx
  - ../../src/components/settings/NotificationsPage.tsx
  - ../../src/components/settings/GitHubPage.tsx
  - ../../src/components/settings/GitHubSettings.tsx
  - ../../src/components/settings/BranchPrefixSetting.tsx
  - ../../src/components/repository/YourTurnCard.tsx
  - ../../src/hooks/useZoom.ts
  - ../../src/stores/ai-suggestions.ts
  - ../../src/stores/settings/appSettings.ts
  - ../../tests/e2e/setup/tauri-mocks.ts
  - ../../tests/e2e/helpers/settings-helpers.ts
  - ../../tests/e2e/helpers/sub-window-helpers.ts
---

# Settings

Status: Implemented
Date: 2026-10-17

## Summary

Settings is a window of its own, as in IntelliJ and the macOS apps: a sidebar of pages on the canvas (General,
Appearance, Notifications, GitHub) beside one island with the page. It is not modal: the main window stays usable, and
a change applies at once in every window. The main window opens it, from the menu (⌘,), the keys of a custom title bar
and the buttons that need a setting ("Notification Settings…", "Sign In…"), which name the page they need.

The settings were a modal dialog before, whose six sections scrolled in a 32rem column; the toolbar had a popover of its
own for one switch.

## Goals

- Every setting of the app in one place, one page per subject, each page without scrolling at the size the window
  opens with.
- A button that needs a setting opens the page that has it, not the top of a long list.
- The same settings in every window: Settings writes them through the main window, which owns `settings.json`.

## Non-goals

- A search over the settings, Reset All.
- A Light/Dark choice: the theme follows the system.
- A settings dialog inside another window.

## Requirements

### The window

- Settings is the sub-window `settings` at `/settings` (`SettingsPage`, lazy), titled "Settings", 720 × 540 when it
  opens. Its minimum is 600 × 420 (the sidebar beside a page); a page needs 560 CSS px, so a zoom above 107% widens
  it: `max(600, 560 × level)` (`sub_window_min_size` in `src-tauri/src/zoom.rs`, `minSubWindowSize` in
  `src/lib/zoom.ts`, which `useZoom` sets by the label of the window).
  [@test] ../../src-tauri/src/zoom_test.rs (`the_settings_window_is_smaller_and_widens_above_107_percent`)
  [@test] ../../src/lib/zoom.test.ts (`is smaller for Settings, and wider above 107%`)
- Only the main window opens it (`openSettingsWindow` of `IndexPage`, the handler of `open_settings`): it has the
  current settings, the window's start (`storeCache`). Every entry point asks it: the native menu item sends
  `open_settings` to the main window, `openSettings` (`runAppCommand`) runs there or sends it there. Opened again, the
  open window comes to the front with the new open.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)
- The window has the permissions of the sub-windows and of the title bar (`sub-windows.json`, `title-bar.json`), and
  those of its pages (`settings.json`): the test notification, the code of the GitHub sign-in on the clipboard, and
  the notification settings of the system (`x-apple.systempreferences:`, `ms-settings:`).
  [@test] ../../src-tauri/src/title_bar_test.rs (`every_window_gets_the_permissions_of_the_title_bar`)
- The window closes with the main window: when the main window is destroyed, every sub-window is (`close_sub_windows`
  in `src-tauri/src/lib.rs`), and the app quits with the last window. Without the main window a sub-window would lose
  the settings it changes and wait for the answers of the main window.

### Entry points

- Settings… of the native menu (⌘, in the app menu on macOS; File → Settings…, Ctrl+, on Windows and Linux with the
  system title bar), in whichever window has the focus.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)
- ☰ File → Settings… and Ctrl+, where the page draws the title bar (Windows, Linux); Ctrl+, of a sub-window asks the
  main window.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should open from the keyboard and run the items`, `should take the keys of the native menu: Ctrl+R syncs, Ctrl+, opens the settings`, `should send Sync and Settings from a sub-window to the main window`)
- "Notification Settings…" of the Your Turn card opens Notifications.
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open Settings at Notifications from the card menu, whose switches the main window keeps`)
- "Sign In…" of the GitHub hint (no token, or a rejected one) opens GitHub; a build without the sign-in names it
  "GitHub Settings…".
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`, `should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)
- Every entry point shows `SettingsIcon` ([UI Theme](../../docs/design/ui-theme.md): one icon per concept).

### Pages

- The sidebar is a vertical Base UI `Tabs` list on the canvas (`TabsList variant="list"`): General, Appearance,
  Notifications, GitHub, the selected one in the selected row of the tables (`bg-primary/15`). ↑ and ↓ switch the page
  (`activateOnFocus`). The page is one island (`Card`), which scrolls only where a large zoom leaves it no room; the page
  left is hidden at once (`data-ending-style`), so two never stack.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)
- General: "Branches", the branch prefix of every repository (`BranchPrefixSetting`, [Onboarding](onboarding.spec.md));
  "Sync", Auto-sync on Focus, the setting of the item of the File menu, whose check the main window keeps; "AI",
  Suggest branch names ([Branch Name Suggestions](../branch/branch-name-suggestions.spec.md)).
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`, `checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)
  [@test] ../../tests/e2e/inline-branch-creator-ai/inline-branch-creator-ai.spec.ts (`should turn AI on and off in Settings`)
- Appearance: "Zoom", Zoom Out · the level · Zoom In, then Actual Size: the items of the View menu by id
  (`zoomMenuItems`), their keys in the tooltips, the same command, so every window zooms one step
  ([Zoom](zoom.spec.md)); "Title bar" (Windows, Linux), the system title bar ([Title Bar](title-bar.spec.md#setting)).
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should zoom every window from Appearance with the items of the View menu`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should offer the system title bar in Settings → Appearance, applied after a restart`, `should show a fixed system title bar in the settings with its reason`)
- Notifications: the four switches of the OS notifications, the permission of the OS, and "Dock icon" (not Windows)
  ([Review Requests](review-requests.spec.md#settings)). The permission is asked when the page shows, on each open of
  the window and when the window gets the focus.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should ask the OS for the permission to notify again when the window gets the focus`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should open Settings at Notifications from the card menu, whose switches the main window keeps`, `should count the review requests, behind its switch`)
- GitHub: the token and the app's own sign-in ([GitHub Sign-in](github-sign-in.spec.md)), asked again on each open of
  the window and when any window signs in or out (`github-auth-changed`).
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should cancel the sign-in, and sign out`, `should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh`)
- Every page fits 720 × 540 without scrolling in its tallest state: the code of the GitHub sign-in, the warning that the
  OS refused the notifications (one line with Open System Settings), the Dock group on macOS and Linux.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should fit every page at 720 × 540 without scrolling`, `should fit every page at 720 × 540 without scrolling, the Dock badge too`)

### The page an open shows

- An open says what it shows (`SettingsTarget`, the data of the window): `{ page?, focus? }`. With a page it shows that
  page; without one it keeps the page shown, and a new window opens at the page shown last (`appSettings.settingsPage`,
  General by default). A page the sidebar doesn't show is General. A new open switches the page during the render, not
  in an effect; the saved page follows the page shown.
  [@test] ../../src/lib/settings-window.test.ts (`shows the page an open names`, `keeps the current page for an open without one`, `shows General for a page the sidebar doesn't show`)
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open at the page an open names, else at the page shown last, its tab focused`)
- Each open focuses the field it names (`data-settings-focus`, `branchPrefix` on General), else the tab of the page,
  once Base UI registered the tabs: a tab focused before keeps a stale highlight, and the first ↓ would go nowhere.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should focus the field an open names`, `should open at the page an open names, else at the page shown last, its tab focused`)

### Keys

- Escape closes the window, as in every sub-window; in a field with typing it takes the typing back first, and in an
  open popover it closes the popover only (Base UI prevents the event). The footer says `Esc Close`; no ↑↓ hint, which
  holds only while the sidebar has the focus.
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should close on Escape, which takes the typing of a field back first`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request`)

### Writes through the main window

- The window writes the settings through the main window, which applies and saves them
  ([Distributed Settings](../../docs/design/distributed-settings.md)): the zoom zooms every window, Auto-sync on Focus
  checks the item of the native menu (`set_auto_sync_menu_checked`), a changed AI mode clears the last AI failure of
  every window (`ai-suggestions.ts`).
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should save the diff view the window chooses through the main window`)
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should zoom every window from Appearance with the items of the View menu`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)
- The pages read only what every window has: the settings and the backend. The branch prefix field reads the prefix of
  git for the selected repository itself (`get_branch_prefix_from_git_config`, again on focus), and stays enabled while
  a sync runs: a sync reads the prefix when it starts, so a change names the branches of the next one.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`)

## User Experience

- Window title "Settings"; pages "General", "Appearance", "Notifications", "GitHub"; sections in sentence case
  ("Branches", "Sync", "AI", "Zoom", "Title bar", "Notifications", "Dock icon", "GitHub").
- Menu item "Settings…", buttons "Notification Settings…", "Sign In…", "GitHub Settings…".
- A failed open is "Cannot open Settings" with the reason (`notifyError`).

## Testing / Local Run

- `pnpm e2e`: `tests/e2e/settings/settings.spec.ts`. `openSettingsWindow` of `tests/e2e/helpers/settings-helpers.ts`
  fires `open_settings` in the main window's page and shows the recorded open as a linked page
  (`openLinkedSubWindow`), whose settings writes reach the main window; it starts from the settings the open recorded
  (`store` of `__e2eSubWindows`, the `storeCache` of the open). `expectSettingsOpened` checks the last open and that it
  is the only new one; `reopenLinkedSubWindow` hands a later open to the shown window, as the backend does.
- By hand: `pnpm tauri dev`; ⌘, from the menu, from a review window and again with the window open (it comes to the
  front); every page at 100% and 150% without scrolling; closing the main window closes Settings.

## References

- [Zoom](zoom.spec.md), [Title Bar](title-bar.spec.md), [Review Requests](review-requests.spec.md),
  [GitHub Sign-in](github-sign-in.spec.md), [Onboarding](onboarding.spec.md).
- [Distributed Settings](../../docs/design/distributed-settings.md): how a sub-window writes the settings.
