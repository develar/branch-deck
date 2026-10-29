---
name: Command Palette
description: Requirements for the command palette of the main window (⌘K) - a search over the views, Activity, the commands of the app and the branches of the table, which runs the chosen one and gives the focus back.
style: plain-1
targets:
  - ../../src/components/command-palette/CommandPalette.tsx
  - ../../src/lib/command-palette.ts
  - ../../src/stores/main-view.ts
  - ../../src/lib/main-menu.ts
  - ../../src/lib/app-commands.ts
  - ../../src-tauri/src/menu.rs
  - ../../src/pages/IndexPage.tsx
  - ../../src/hooks/useActivityPanel.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/presenters/actions.ts
  - ../../src/stores/commit-move.ts
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/lib/title-bar.ts
  - ../../src-tauri/src/title_bar.rs
  - ../../src/components/repository/RepositorySelector.tsx
  - ../../src/stores/repository.ts
---

# Command Palette

Status: Implemented
Date: 2026-10-22, the commands of the header and of Help and a list that shows them 2026-10-29

## Summary

The main window has more places than its menus show at a glance: the views, the Activity panel and window, a branch
among dozens. ⌘K opens a dialog with one search over them, over the commands of the app (Open Repository…, Sync,
Update Project, Settings…) and over the actions of the branch at hand, as the palettes of IntelliJ (Search Everywhere) and VS Code do:
type a few letters, press Enter. The actions of a branch are its menu's, as Rust decides them: one builder, one set of
names and reasons.

## Goals

- Every place of the main window and every command of the header and of the app's menu in a few keys, without the
  mouse, and in sight when the palette opens.
- The same names, keys and reasons as the menus: one builder per command ([UX Writing](../../docs/guidelines/ux-writing.md#actions)).
- The focus back where it was when nothing runs, where the command puts it when one does.

## Non-goals

- No actions of a branch other than the one at hand: a branch elsewhere is a "Go to <branch>" away.
- No palette in the sub-windows: ⌘K of any window opens the palette of the main window.
- No history and no ranking of what was chosen before.

## Requirements

### Opening

- View → Command Palette… (`CmdOrCtrl+K`) of the native menu sends `open_command_palette` to the main window; ☰ View →
  Command Palette… and Ctrl+K of a custom title bar (`matchMainMenuKey`) run the same app command. The main window comes
  to the front and opens the palette with the focus in its search.
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should open from ⌘K of the native menu, list every source and give the focus back on Escape`)
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should open from Ctrl+K and from View → Command Palette… of ☰, and run Update Project and Check for Updates…`)
  [@test] ../../src/lib/main-menu.test.ts (`maps the accelerators of the native menu to their commands`, `holds the items of the native menu, File, View and Help`)

### Sources

- The groups, in this order (`paletteCommands`): Views (`MAIN_VIEWS` of `src/stores/main-view.ts`, Branches, Reviews
  and Landing: a view joins the palette by its entry there), Activity (Activity Panel, which toggles the side panel as the header button does, and
  Activity Window), Commands (Add Pull Request by Link…, which shows Reviews with the focus in its field and its text selected
  ([Reviews](reviews.spec.md#links)); Open Repository…, the folder button of the repository selector, disabled with
  "Wait for the sync to finish" while a sync runs, as the selector is; then the items Sync Branches, Update Project,
  Settings…, Install Agent CLI…, where the build updates itself Check for Updates… (on every platform and title bar, as
  the native menu has it: `checkForUpdates` of the title bar script), Getting Started and GitHub Repository of `mainMenus`,
  disabled with the reason where the menu disables them; never a view setting, Auto-sync on Focus or the zoom), the
  actions of the branch at hand (below) and Branches ("Go to <branch>" for every branch of the table, in its order).
  [@test] ../../src/lib/command-palette.test.ts (`lists the views, Activity, the commands of the header and of the app's menu and the branches, by namespaced ids`, `hints only the keys of the window, disables Sync and Open Repository with the reason, offers updates where the build updates itself`)
  [@test] ../../src-tauri/src/title_bar_test.rs (`the_page_gets_one_json_object`)
- Actions of the branch at hand, between Commands and Branches, the group named by the branch: the branch whose row had
  the focus when the palette opened, else the one branch selected; none without one. Its commands are the items of its
  menu ([Branch Row](../branch/branch-row.spec.md#branch-menu), `getBranchContextMenuItems`, which presents the
  actions Rust decides: `domain_policy::actions`), the same names, disabled with the same reasons; an item of a
  submenu is named with it ("Patronus › Dry Run", "Copy › Copy Branch Name"). A key of the row is no key of the
  window: none is hinted.
  [@test] ../../src/lib/command-palette.test.ts (`offers the actions of the branch at hand as its menu has them, an item of a submenu named with it, disabled with the reason`)
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should offer the actions of the focused branch as its menu does and run one: Review Changes opens its review`, `should open from ⌘K of the native menu, list every source and give the focus back on Escape`)
- Every command has an id in the namespace of its source (`view:branches`, `activity:panel`, `app:sync_branches`,
  `action:auth:review-changes`, `branch:auth`); an id that a source gave before is left out, so a command shows once.
  [@test] ../../src/lib/command-palette.test.ts (`keeps one entry per id, the first source's`)
- Landing of the Views shows the Landing view, Reviews the Reviews view; Go to <branch> from either shows the branches
  first.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should change the view only by the user: a sync keeps it, a reveal of a branch shows the branches`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`, `should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu`)
- A key hint only for a key of the window (⌘R, ⌘T, ⌘,; a view's key once it has one), none for a branch or Activity.
  [@test] ../../src/lib/command-palette.test.ts (`hints only the keys of the window, disables Sync and Open Repository with the reason, offers updates where the build updates itself`)
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should open from ⌘K of the native menu, list every source and give the focus back on Escape`, `should show the views, Activity and every command of the app without a scroll in the window of the start`)
- The list is as high as the window lets it below the search, at most 36rem and 4rem short of the 80% of the window
  under the top of the palette: in the window of the start (1024×900, and down to 720 px high) the views, Activity and
  every command show without a scroll, the actions of the branch and the branches below them; a lower window scrolls
  the rest. A list cut above them hides them: the scroll bars of macOS show only while a list scrolls.
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should show the views, Activity and every command of the app without a scroll in the window of the start`)
- The search keeps the commands whose label has the typed text, ignoring case (the filter of Base UI `Combobox`), the
  first one highlighted; Enter or a click runs it. Nothing kept: "No command or branch matches".
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should go to a branch: the row expanded, selected and focused`, `should open the Activity panel with the focus in it from the keyboard, and the Activity window by a click`)
- A command that starts with the typed text comes first, in its group and its group before the others
  (`paletteGroups`): its label starts with it, or the name of the branch of a Go to does. The name of a branch typed
  and Enter go to that branch, never to an action of the branch at hand whose label names it ("Push session Only",
  "Stack On › session"): that one came first in the group order and Enter pushed or restacked instead of going.
  [@test] ../../src/lib/command-palette.test.ts (`lists the groups in their order without a text, a command and its group that start with the text first`)
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should go to the branch a search names before an action that names it, and focus its row in a collapsed stack`)

### Running

- A command runs once the palette closed and the focus went back where it was, so what the command focuses keeps the
  focus: "Go to <branch>" shows the branches, expands the row, scrolls it into view (`requestReveal`), selects it and
  focuses it, also a row a collapsed stack or the filter hid, which takes the focus as the reveal renders it (else
  the focus stayed where it was, a push button of another row too); Activity Panel chosen by Enter opens the panel
  with the focus in its log (a keyboard open, [Activity](activity.spec.md#side-panel)), by a click without; Activity
  Window opens the window and closes the panel.
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should go to a branch: the row expanded, selected and focused`, `should show the branches view`)
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should go to the branch a search names before an action that names it, and focus its row in a collapsed stack`)
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should open the Activity panel with the focus in it from the keyboard, and the Activity window by a click`)
  [@test] ../../src/lib/command-palette.test.ts (`runs what its source runs`)
- A command of the app runs as its menu item does (`onSelect` of the item of `mainMenus`), Open Repository… as the
  folder button does (`browseRepository`: the folder dialog, then the repository picked in it).
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should run the commands of the app: Sync Branches, Settings… and Install Agent CLI…`)
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should open from Ctrl+K and from View → Command Palette… of ☰, and run Update Project and Check for Updates…`)
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should open a repository and the help of the app as the header and the Help menu do`, `should check for updates where the build updates itself beside the traffic lights, as the app menu does`, `should check for updates where the build updates itself under the system title bar, as its Help menu does`)
- Escape or a click outside closes the palette with nothing run and gives the focus back to the element that had it
  (the element itself: a row is focusable, not tabbable).
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should open from ⌘K of the native menu, list every source and give the focus back on Escape`)

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

- [Title Bar](title-bar.spec.md): the menus and their keys.
- [Activity](activity.spec.md): the side panel and the window.
