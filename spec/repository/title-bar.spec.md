---
name: Title Bar
description: Requirements for the toolbar of the main window as its title bar, as in the IntelliJ new UI: beside the macOS traffic lights, with its own window buttons on Windows, below the native title bar on Linux.
style: plain-1
targets:
  - ../../src-tauri/src/lib.rs
  - ../../src-tauri/capabilities/default.json
  - ../../src/lib/title-bar.ts
  - ../../src/hooks/useWindowFrameState.ts
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/WindowControls.tsx
  - ../../src/pages/IndexPage.tsx
  - ../../tests/e2e/setup/tauri-mocks.ts
---

# Title Bar

Status: Implemented
Date: 2026-10-15

## Summary

The toolbar of the main window (repository, branch prefix, Update, outgoing, Activity, Sync) is its title bar, as in
the IntelliJ new UI. It lies on the canvas above the islands, 40px high. On macOS it sits beside the traffic lights of
an overlay title bar; on Windows the window has no decorations and the toolbar ends with the window buttons; Linux keeps
the native title bar above it.

## Goals

- No grey native band above the canvas: the window is one surface, as an IntelliJ window is.
- The toolbar drags the window and zooms or maximizes it on a double click, as a title bar does.

## Non-goals

- The Snap Layouts flyout of Windows 11 on hovering Maximize: it needs the native caption buttons (or the unmaintained
  `tauri-plugin-decorum`). Snapping by dragging to an edge and Win+Z keep working.
- A custom title bar on Linux: the window manager draws the shadow, the corners, the button layout (GNOME shows only
  Close, on either side) and tiling; a page can't match them.
- The sub-windows (conflicting files, missing commits): they keep the native title bar, which carries their title.

## Requirements

### Kind

- The window builder (`MAIN_TITLE_BAR` in `src-tauri/src/lib.rs`) tells the page before it loads how the title bar is
  drawn (`window.__BD_TITLE_BAR__`, `titleBar` in `src/lib/title-bar.ts`): `overlay` on macOS, `custom` on Windows,
  none on Linux. The page does not guess from the user agent.
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

- The empty parts of the toolbar drag the window (`data-tauri-drag-region="deep"`, `core:window:allow-start-dragging`);
  a double click zooms (macOS) or maximizes (Windows). The controls in it keep their clicks.
- Every control stays on the one 40px line down to the 960px minimum width of the window, beside the traffic lights.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should start the toolbar after the traffic lights of an overlay title bar, and not in fullscreen`)
  [@test] ../../tests/e2e/repository/header.spec.ts (`should keep its controls apart on one line at the minimum window width`)

## User Experience

- The toolbar is not an island: it lies on the canvas (`bg-canvas`), as the IntelliJ main toolbar does, its controls
  8px from the window edges like the islands below (`pl-2 pr-2`).
- The islands scroll in their own container under the toolbar (`main`, `overflow-y-auto`): the toolbar stays where the
  title bar is, and a row scrolled into view never lands under it.
- The toolbar text is not selectable (`select-none`), as a title bar's.

## Testing / Local Run

- `pnpm e2e`: `tests/e2e/repository/title-bar.spec.ts` sets `__BD_TITLE_BAR__` with `addInitScript`. The window calls go
  to the mock (`plugin:window` in `tests/e2e/setup/tauri-mocks.ts`): `__e2eWindowState` holds fullscreen and
  maximized, a test fires `tauri://resize`, `__e2eWindowCalls` records minimize, toggle_maximize and close.
- By hand: `pnpm install-app` on macOS (the traffic lights, dragging, double click, entering and leaving fullscreen), a
  Windows build for the window buttons (a Windows on ARM VM works, see "Windows on ARM" in `DEVELOPMENT.md`; UI
  Automation finds the buttons by their `aria-label` and invokes them).

## Open Questions / Risks

- E2E runs in Chromium: it checks the layout and the calls, not the native frame. The traffic light position and the
  Windows frame are checked by hand.

## References

- [UI Theme](../../docs/design/ui-theme.md): the canvas and the islands.
- [Activity](activity.spec.md), [Update Project](update-project.spec.md), [Outgoing Commits](outgoing.spec.md): the
  controls of the toolbar.
