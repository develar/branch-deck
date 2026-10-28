---
name: Zoom
description: Requirements for zooming the pages of all windows from the View menu (Zoom In, Zoom Out, Actual Size), with the toolbar kept at the size of a title bar.
style: plain-1
targets:
  - ../../src-tauri/src/menu.rs
  - ../../src-tauri/src/zoom.rs
  - ../../src-tauri/src/lib.rs
  - ../../crates/app-api/src/app/windows.rs
  - ../../src-tauri/src/desktop.rs
  - ../../src-tauri/capabilities/default.json
  - ../../src-tauri/capabilities/sub-windows.json
  - ../../src/lib/zoom.ts
  - ../../src/hooks/useZoom.ts
  - ../../src/stores/settings/appSettings.ts
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/settings/AppearancePage.tsx
  - ../../src/lib/main-menu.ts
  - ../../tests/e2e/setup/tauri-mocks.ts
---

# Zoom

Status: Implemented
Date: 2026-10-15

## Summary

View → Zoom In, Zoom Out and Actual Size scale the pages of all windows, as a browser or an Electron app (Slack) does.
The level is saved and shared by all windows. The toolbar of the main window is its title bar and keeps the title
bar's size.

Electron apps get zoom from Chromium and the default menu roles. Tauri has neither: WKWebView ignores ⌘= and ⌘−, muda
has no predefined zoom items, and `zoom_hotkeys_enabled` is a JS polyfill with 20% steps that saves nothing. So the
menu items are ours and call `Webview::set_zoom` (WKWebView `pageZoom`, WebView2 `ZoomFactor`).

## Goals

- Bigger or smaller text and controls everywhere, in steps a browser user knows.
- The same level in every window, after a restart too, with no flash at 100% when a window opens.

## Non-goals

- Pinch-to-zoom and ⌘ + scroll wheel.
- A level per window or per repository.
- Zooming the toolbar: it is window chrome, like the native title bar or the Safari toolbar.

## Requirements

### Menu

- The View menu has, after Command Palette… ([Command Palette](command-palette.spec.md)), Actual Size (⌘0, Ctrl+0),
  Zoom In (⌘+, Ctrl+=) and Zoom Out (⌘−, Ctrl+-), in Safari's order, followed on macOS by Enter Full Screen. The items emit `zoom` with `reset`, `in` or `out` to the main window
  only, so one press is one step for all windows.
  [@test] ../../tests/e2e/repository/zoom.spec.ts (`should step through the levels with Zoom In, Zoom Out and Actual Size`)
- Zoom In shows ⌘+ on macOS, as in Safari and Slack. Tauri builds accelerators from key codes only (`CmdOrCtrl+=`),
  and `+` is no key but ⇧= on most layouts, so `show_zoom_in_as_plus` in `src-tauri/src/menu.rs` sets the key
  equivalent of the native item to `+`: AppKit shows ⌘+ and takes ⌘⇧= and the + of the keypad.
- Zoom In takes both keys, as in a browser. A menu item has a single shortcut, so the page handles the other one
  (`useZoom`): ⌘= on macOS, Ctrl++ (Ctrl+⇧=, or + on the keypad) elsewhere. The page sees a key first, and handling
  it keeps the menu from also getting it.
  [@test] ../../tests/e2e/repository/zoom.spec.ts (`should zoom in with Ctrl++ as well as with the Ctrl+= of the menu item`, `should zoom in with ⌘= as well as with the ⌘+ of the menu item`)
- Settings → Appearance shows the level between Zoom Out and Zoom In, then Actual Size: the items of the View menu
  (`zoomMenuItems`, also the View menu of ☰), their keys in the tooltips, so a click is one step for all windows, as a
  press of the menu is ([Settings](settings.spec.md)).
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should zoom every window from Appearance with the items of the View menu`)

### Levels

- The levels are those of Chromium from 67% to 150%: 67, 75, 80, 90, 100, 110, 125, 150 (`ZOOM_LEVELS` in
  `src/lib/zoom.ts`). Zoom In and Zoom Out stop at the ends. A stored level between two levels steps to the next one in
  that direction.
  [@test] ../../src/lib/zoom.test.ts (`steps through the levels`, `stops at the first and the last level`, `steps from a level between the levels to the next one in that direction`)

### Saved and shared

- The level is `appSettings.zoom` (absent at 100%). Every window follows it through the settings sync (`useZoom`).
  Every window writes it through the backend's writer as any setting.
- A window opens at the saved level: the window builders read it from the settings of the writer they inject
  (`AppSettings::zoom`, `src-tauri/src/settings.rs`) and zoom the webview before the first paint; a zoom that is no
  finite positive number is 100%.
  [@test] ../../src-tauri/src/zoom_test.rs (`the_zoom_of_a_window_is_the_app_settings_zoom_of_the_file`, `without_a_valid_zoom_in_the_file_the_zoom_is_100_percent`)

### The toolbar

- The toolbar of the main window keeps its 40px, its controls their size and the traffic lights their place at any
  level (`zoom: 1 / level` on `ConfigurationHeader`). Its popovers, menus and tooltips open at the page zoom, anchored
  to their trigger.
  [@test] ../../tests/e2e/repository/zoom.spec.ts (`should zoom the islands but not the toolbar, the title bar of the window`)
- Its secondary texts (the branch count, the last sync time) hide by the toolbar's own width (container queries), not
  by the viewport of the zoomed page.

### Window size

- The islands keep at least 768 CSS px (`md`). The minimum width of the main window is the larger of 960px (the toolbar)
  and 768px × level: 960px up to 125%, 1152px at 150% (`min_window_width`, `minWindowWidth`). A higher level widens a
  narrower window.
  [@test] ../../src-tauri/src/zoom_test.rs (`a_zoom_above_125_percent_widens_the_minimum_window`)
  [@test] ../../src/lib/zoom.test.ts (`keeps 768 CSS px for the islands, so a zoom above 125% widens the minimum window`)
  [@test] ../../tests/e2e/repository/zoom.spec.ts (`should zoom the islands but not the toolbar, the title bar of the window`)
- A sub-window keeps the CSS px of its islands the same way, by its label (`sub_window_min_size`, `minSubWindowSize`):
  a review 640 px beside its 800px minimum (wider above 125%), Settings 560 px beside its 600px minimum (wider above
  107%). A window gets the minimum of its level when it opens and when the level changes (`useZoom`).
  [@test] ../../src-tauri/src/zoom_test.rs (`a_zoom_above_125_percent_widens_the_minimum_sub_window`, `the_settings_window_is_smaller_and_widens_above_107_percent`)
  [@test] ../../src/lib/zoom.test.ts (`is the size of the title bar of a review up to 125%, wider above`, `is smaller for Settings, and wider above 107%`)
- Inputs and text areas are `text-sm` at any width (not the phone `text-base md:text-sm` of shadcn), so a zoomed page
  narrower than `md` keeps their size.

## User Experience

- Menu items: "Actual Size", "Zoom In", "Zoom Out" (Title Case, as in Safari).
- A failed zoom is logged ("Cannot zoom the window"), with no toast: the page keeps its previous level.

## Testing / Local Run

- `pnpm e2e`: `tests/e2e/repository/zoom.spec.ts` fires the `zoom` event. The webview mock (`plugin:webview` in
  `tests/e2e/setup/tauri-mocks.ts`) records the level in `__e2eWebviewZoom`, and the window mock records the minimum
  size in `__e2eWindowState.minSize`. A test zooms the page as the webview does, with a viewport of the window size
  divided by the level (`zoomPage` in `tests/e2e/helpers/header-helpers.ts`). A CSS `zoom` on the root is not the same:
  the page would overflow sideways.
- By hand: `pnpm tauri dev` on macOS, View shows Zoom In ⌘+; ⌘= / ⌘+ / ⌘− / ⌘0 step once each, a restart, and a sub-window (conflicting files) opened at
  150%.

## Open Questions / Risks

- E2E runs in Chromium and emulates the page zoom. The real `pageZoom` of WKWebView, the traffic lights and the
  menu shortcuts are checked by hand.

## References

- [Title Bar](title-bar.spec.md): the toolbar as the title bar.
- [UI Theme](../../docs/design/ui-theme.md): the canvas and the islands.
