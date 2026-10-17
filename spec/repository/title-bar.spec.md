---
name: Title Bar
description: Requirements for the top row of every window as its title bar, as in the IntelliJ new UI: the toolbar of the main window and the row of each sub-window, beside the macOS traffic lights, with its own window buttons on Windows, below the native title bar on Linux.
style: plain-1
targets:
  - ../../src-tauri/src/lib.rs
  - ../../src-tauri/src/title_bar.rs
  - ../../src-tauri/src/commands/window_management.rs
  - ../../src-tauri/src/zoom.rs
  - ../../src-tauri/capabilities/title-bar.json
  - ../../src-tauri/capabilities/default.json
  - ../../src-tauri/capabilities/sub-windows.json
  - ../../src/lib/title-bar.ts
  - ../../src/hooks/useWindowFrameState.ts
  - ../../src/hooks/useWindowTitle.ts
  - ../../src/components/shared/WindowTitleBar.tsx
  - ../../src/components/shared/WindowPage.tsx
  - ../../src/lib/sub-window.ts
  - ../../src/components/shared/ActionButton.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/pages/BranchReviewPage.tsx
  - ../../src/pages/CommitReviewPage.tsx
  - ../../src/components/review/PullRequestReview.tsx
  - ../../src/components/conflict/ConflictWindowPage.tsx
  - ../../src/components/repository/WindowControls.tsx
  - ../../src/pages/IndexPage.tsx
  - ../../tests/e2e/setup/tauri-mocks.ts
---

# Title Bar

Status: Implemented
Date: 2026-10-15, sub-windows 2026-10-17

## Summary

The top row of every window is its title bar, as in the IntelliJ new UI: the toolbar of the main window (repository,
Update, outgoing, robots, Activity, Sync), and the row of each sub-window (where the user is on the left, the view and
the actions on the right). It lies on the canvas above the islands, 40px high (`WindowTitleBar`). On macOS it sits
beside the traffic lights of an overlay title bar; on Windows the window has no decorations and the row ends with the
window buttons; Linux keeps the native title bar above it.

## Goals

- No grey native band above the canvas: every window is one surface, as an IntelliJ window is.
- The row drags the window and zooms or maximizes it on a double click, as a title bar does.
- The 40px a native title bar took go to the content: in a review window the stack, the view and the actions take the
  row, not an island of their own.

## Non-goals

- The Snap Layouts flyout of Windows 11 on hovering Maximize: it needs the native caption buttons (or the unmaintained
  `tauri-plugin-decorum`). Snapping by dragging to an edge and Win+Z keep working.
- A custom title bar on Linux: the window manager draws the shadow, the corners, the button layout (GNOME shows only
  Close, on either side) and tiling; a page can't match them.

## Requirements

### Kind

- Every window is built the same way (`with_title_bar` in `src-tauri/src/title_bar.rs`: the main window and
  `open_sub_window`), and its initialization script tells the page before it loads how the title bar is drawn
  (`title_bar_script`, `window.__BD_TITLE_BAR__`, `titleBar` in `src/lib/title-bar.ts`): `overlay` on macOS, `custom`
  on Windows, none on Linux. The page does not guess from the user agent.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should keep the native title bar where the page doesn't draw one`)

### macOS

- The window has an overlay title bar with a hidden title; the traffic lights are centred in the 40px toolbar, 14px
  from the left as from the top (`traffic_light_position(14, 22)`: wry takes y as the height of the button container
  above the buttons, not as their top).
- The toolbar starts after the traffic lights (`pl-22`, 15px after the zoom button); in fullscreen, where they are
  hidden, it starts at the edge (`isFullscreen`, re-read on every resize).
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should start the toolbar after the traffic lights of an overlay title bar, and not in fullscreen`)

### Windows

- The window has no decorations; `shadow(true)` gives it the 1px border and, on Windows 11, the rounded corners.
- The toolbar ends with Minimize, Maximize (Restore while maximized) and Close, flush with the right edge: 46×40 as
  the native caption buttons, the Segoe Fluent Icons glyphs, `bg-muted` on hover and `bg-destructive` for Close.
  Each has an `aria-label` and a tooltip. A failed call is a "Cannot minimize the window" error.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should end a custom title bar with the window buttons`)

### Both

- The empty parts of the row drag the window (`data-tauri-drag-region="deep"`); a double click zooms (macOS) or
  maximizes (Windows). The controls in it keep their clicks. Every window has the permissions of the title bar (one
  capability, `title-bar.json`: the window state, dragging, the double click, the window buttons, the title, the
  minimum size); a test checks that every sub-window label has it.
  [@test] ../../src-tauri/src/title_bar_test.rs (`every_window_gets_the_permissions_of_the_title_bar`)
- The window-state plugin restores sizes and positions, never the decorations: a saved `decorated` would bring the
  native caption back on Windows.
- The toolbar keeps its size at any page zoom, as window chrome does ([Zoom](zoom.spec.md)).
  [@test] ../../tests/e2e/repository/zoom.spec.ts (`should zoom the islands but not the toolbar, the title bar of the window`)
- Every control stays on the one 40px line down to the 960px minimum width of the window, beside the traffic lights.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should start the toolbar after the traffic lights of an overlay title bar, and not in fullscreen`)
  [@test] ../../tests/e2e/repository/header.spec.ts (`should keep its controls apart on one line at the minimum window width`)

### Sub-windows

- A sub-window (`WindowPage`) is its row, then its islands, then its footer (the position and the keys). The row says
  where the user is on the left and holds the view and the actions on the right; the native title stays set
  (`useWindowTitle`) for the Window menu, Mission Control and screen readers.
  - `branch-review`: the stack bar; Unified/Split and the actions on the branch ([Branch Review](../branch/branch-review.spec.md)).
  - `commit-review`: "session › 11346cdf", the list and the commit; Unified/Split ([Commit Review](../commit/commit-review.spec.md)).
  - `pull-request-review`: the toolbar of the review, ↑ ↓ and "2 of 5 · Your Turn"; Unified/Split while the changes
    show, GitHub and "⋯" ([Review Requests](review-requests.spec.md#review-window)).
  - `conflicting-files`, `missing-commits`: "bug-fix › Conflicting files", the islands scrolling under the row.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`, `should end the row of a review window with the window buttons of a custom title bar`, `should keep the row of a conflict window above the islands, which scroll under it`)
- A sub-window is at least 800×500 (wider above a 125% zoom: 640 CSS px for the review, `min_sub_window_width`), so
  its row fits one line; an action with an icon shows only the icon in a row narrower than 48rem (`ActionButton`).
  [@test] ../../src-tauri/src/zoom_test.rs (`a_zoom_above_125_percent_widens_the_minimum_sub_window`)
- One window per kind, reused: an open of an open kind retitles it, brings it to the front and hands its page the new
  data. A new window is built hidden and shows once its page painted (`WindowPage` → `sub_window_ready`, at most 3 s
  later without it), so it never shows a blank frame; an open while its page still loads is handed over then, the
  latest one winning. The opens are serialized: two quick opens build one window. A new window opens centred on the
  monitor of the main window, then where it was (the window state plugin, without the visibility); it has a taskbar
  entry (Windows, Linux). Escape that nothing in the page handled and ⌘W close it.
  [@test] ../../src-tauri/src/commands/window_management_test.rs (`an_open_while_the_page_loads_waits_for_the_page`, `a_loaded_page_takes_an_open_at_once`, `a_window_built_again_after_its_close_waits_for_its_page_again`)

## User Experience

- The row is not an island: it lies on the canvas (`bg-canvas`), as the IntelliJ main toolbar does, its controls
  8px from the window edges like the islands below (`pl-2 pr-2`). A sub-window's title is `text-sm`, the parts before
  the last muted, separated by `›`.
- The islands scroll in their own container under the toolbar (`main`, `overflow-y-auto`): the toolbar stays where the
  title bar is, and a row scrolled into view never lands under it.
- The toolbar text is not selectable (`select-none`), as a title bar's.

## Testing / Local Run

- `pnpm e2e`: `tests/e2e/repository/title-bar.spec.ts` sets `__BD_TITLE_BAR__` with `addInitScript`. The window calls go
  to the mock (`plugin:window` in `tests/e2e/setup/tauri-mocks.ts`): `__e2eWindowState` holds fullscreen and
  maximized, a test fires `tauri://resize`, `__e2eWindowCalls` records minimize, toggle_maximize and close.
- By hand: `pnpm install-app` on macOS (the traffic lights, dragging, double click, entering and leaving fullscreen, in
  the main window and in each sub-window), a
  Windows build for the window buttons (a Windows on ARM VM works, see "Windows on ARM" in `DEVELOPMENT.md`; UI
  Automation finds the buttons by their `aria-label` and invokes them).

## Open Questions / Risks

- E2E runs in Chromium: it checks the layout and the calls, not the native frame. The traffic light position and the
  Windows frame are checked by hand.

## References

- [UI Theme](../../docs/design/ui-theme.md): the canvas and the islands.
- [Zoom](zoom.spec.md): the page zoom, which leaves the toolbar alone.
- [Activity](activity.spec.md), [Update Project](update-project.spec.md), [Outgoing Commits](outgoing.spec.md): the
  controls of the toolbar.
