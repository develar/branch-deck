---
name: Zoom
description: Requirements for zooming the pages of all windows from the View menu (Zoom In, Zoom Out, Actual Size), with the toolbar kept at the size of a title bar.
style: plain-1
targets:
  - src-tauri/src/menu.rs
  - src-tauri/src/zoom.rs
  - src-tauri/src/lib.rs
  - crates/app-api/src/app/windows.rs
  - src-tauri/src/desktop.rs
  - src-tauri/capabilities/default.json
  - src-tauri/capabilities/sub-windows.json
  - src/lib/zoom.ts
  - src/hooks/useZoom.ts
  - src/stores/settings/appSettings.ts
  - src/components/repository/ConfigurationHeader.tsx
  - src/components/settings/AppearancePage.tsx
  - src/lib/main-menu.ts
  - tests/e2e/setup/tauri-mocks.ts
---

# Zoom

Status: Implemented
Date: 2026-10-15

## Purpose

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

### Requirement: Menu: The View menu has Actual Size, Zoom In and Zoom Out

The View menu SHALL have, after Command Palette… ([Command Palette](../command-palette/spec.md)), Actual Size (⌘0,
Ctrl+0), Zoom In (⌘+, Ctrl+=) and Zoom Out (⌘−, Ctrl+-), in Safari's order, followed on macOS by Enter Full Screen.

#### Scenario: The items of the View menu
- **WHEN** the user opens the View menu
- **THEN** after Command Palette… come Actual Size (⌘0, Ctrl+0), Zoom In (⌘+, Ctrl+=) and Zoom Out (⌘−, Ctrl+-), in
  Safari's order
- **AND** on macOS Enter Full Screen follows them

### Requirement: Menu: The items emit `zoom` to the main window only

The items MUST emit `zoom` with `reset`, `in` or `out` to the main window only, so one press is one step for all
windows.

#### Scenario: One press, one step for all windows
- **WHEN** the user chooses Zoom In, Zoom Out or Actual Size
- **THEN** the item emits `zoom` with `in`, `out` or `reset` to the main window only
- **AND** all windows take one step
  [@test] tests/e2e/repository/zoom.spec.ts (`should step through the levels with Zoom In, Zoom Out and Actual Size`)

### Requirement: Menu: Zoom In shows ⌘+ on macOS

Zoom In MUST show ⌘+ on macOS, as in Safari and Slack: `show_zoom_in_as_plus` in `src-tauri/src/menu.rs` sets the key
equivalent of the native item to `+`, so AppKit shows ⌘+ and takes ⌘⇧= and the + of the keypad.

#### Scenario: The native item on macOS
- **GIVEN** Tauri builds accelerators from key codes only (`CmdOrCtrl+=`), and `+` is no key but ⇧= on most layouts
- **WHEN** the View menu is shown on macOS
- **THEN** Zoom In shows ⌘+
- **AND** ⌘⇧= and the + of the keypad zoom in

### Requirement: Menu: Zoom In takes both keys, the page handles the other one

Zoom In MUST take both keys, as in a browser. A menu item has a single shortcut, so the page handles the other one
(`useZoom`): ⌘= on macOS, Ctrl++ (Ctrl+⇧=, or + on the keypad) elsewhere. The page sees a key first, and handling it
MUST keep the menu from also getting it.

#### Scenario: ⌘= on macOS
- **GIVEN** macOS
- **WHEN** the user presses ⌘=
- **THEN** the page zooms in one step, as with the ⌘+ of the menu item
  [@test] tests/e2e/repository/zoom.spec.ts (`should zoom in with ⌘= as well as with the ⌘+ of the menu item`)

#### Scenario: Ctrl++ elsewhere
- **GIVEN** another platform
- **WHEN** the user presses Ctrl++ (Ctrl+⇧=, or + on the keypad)
- **THEN** the page zooms in one step, as with the Ctrl+= of the menu item, and the menu does not also get the key
  [@test] tests/e2e/repository/zoom.spec.ts (`should zoom in with Ctrl++ as well as with the Ctrl+= of the menu item`)

### Requirement: Menu: Settings Appearance shows the level with the items of the View menu

Settings → Appearance SHALL show the level between Zoom Out and Zoom In, then Actual Size: the items of the View menu
(`zoomMenuItems`, also the View menu of ☰), their keys in the tooltips, so a click is one step for all windows, as a
press of the menu is ([Settings](../settings/spec.md)).

#### Scenario: A click in Appearance
- **WHEN** the user clicks Zoom In, Zoom Out or Actual Size in Settings → Appearance
- **THEN** every window takes one step, as a press of the menu does
- **AND** the level shows between Zoom Out and Zoom In, the keys in the tooltips
  [@test] tests/e2e/settings/settings.spec.ts (`should zoom every window from Appearance with the items of the View menu`)

### Requirement: Levels: The levels are those of Chromium from 67% to 150%

The levels MUST be those of Chromium from 67% to 150%: 67, 75, 80, 90, 100, 110, 125, 150 (`ZOOM_LEVELS` in
`src/lib/zoom.ts`).

#### Scenario: Zoom In and Zoom Out step through the levels
- **WHEN** the user zooms in or out
- **THEN** the level steps to the next one of 67, 75, 80, 90, 100, 110, 125, 150
  [@test] src/lib/zoom.test.ts (`steps through the levels`)

### Requirement: Levels: Zoom In and Zoom Out stop at the ends

Zoom In and Zoom Out MUST stop at the ends.

#### Scenario: At the first or the last level
- **WHEN** the user zooms out at the first level or zooms in at the last one
- **THEN** the level stays
  [@test] src/lib/zoom.test.ts (`stops at the first and the last level`)

### Requirement: Levels: A stored level between two levels steps to the next one

A stored level between two levels MUST step to the next one in that direction.

#### Scenario: A stored level between two levels
- **GIVEN** a stored level between two levels
- **WHEN** the user zooms in or out
- **THEN** the level steps to the next one in that direction
  [@test] src/lib/zoom.test.ts (`steps from a level between the levels to the next one in that direction`)

### Requirement: Saved and shared: The level is a setting every window follows

The level MUST be `appSettings.zoom` (absent at 100%). Every window MUST follow it through the settings sync
(`useZoom`) and write it through the backend's writer as any setting.

#### Scenario: A window changes the level
- **WHEN** a window changes the level
- **THEN** it writes `appSettings.zoom` through the backend's writer as any setting, absent at 100%
- **AND** every window follows it through the settings sync (`useZoom`)

### Requirement: Saved and shared: A window opens at the saved level

A window MUST open at the saved level: the window builders read it from the settings of the writer they inject
(`AppSettings::zoom`, `src-tauri/src/settings.rs`) and zoom the webview before the first paint; a zoom that is no
finite positive number is 100%.

#### Scenario: A saved level
- **WHEN** a window opens
- **THEN** its builder reads the level from the settings of the writer it injects (`AppSettings::zoom`,
  `src-tauri/src/settings.rs`) and zooms the webview before the first paint
  [@test] src-tauri/src/zoom_test.rs (`the_zoom_of_a_window_is_the_app_settings_zoom_of_the_file`)

#### Scenario: No valid zoom in the settings
- **WHEN** the saved zoom is no finite positive number
- **THEN** the window opens at 100%
  [@test] src-tauri/src/zoom_test.rs (`without_a_valid_zoom_in_the_file_the_zoom_is_100_percent`)

### Requirement: The toolbar: The toolbar keeps its size at any level

The toolbar of the main window MUST keep its 40px, its controls their size and the traffic lights their place at any
level (`zoom: 1 / level` on `ConfigurationHeader`).

#### Scenario: The page at 150%
- **WHEN** the level is 150%
- **THEN** the islands are zoomed
- **AND** the toolbar keeps its 40px and its controls their size, every control on its line
  [@test] tests/e2e/repository/zoom.spec.ts (`should zoom the islands but not the toolbar, the title bar of the window`)

### Requirement: The toolbar: Its popovers, menus and tooltips open at the page zoom

The popovers, menus and tooltips of the toolbar MUST open at the page zoom, anchored to their trigger.

#### Scenario: A popover of the toolbar at a zoomed level
- **WHEN** a popover, menu or tooltip of the toolbar opens at a zoomed level
- **THEN** it is at the page zoom, anchored to its trigger

### Requirement: The toolbar: Its secondary texts hide by its own width

The secondary texts of the toolbar (the branch count, the last sync time) MUST hide by the toolbar's own width
(container queries), not by the viewport of the zoomed page.

#### Scenario: A narrow toolbar
- **WHEN** the toolbar is too narrow for its secondary texts
- **THEN** the branch count and the last sync time hide by the toolbar's own width, not by the viewport of the zoomed
  page

### Requirement: Window size: The minimum width of the main window keeps the islands 768 CSS px

The islands MUST keep at least 768 CSS px (`md`). The minimum width of the main window is the larger of 960px (the
toolbar) and 768px × level: 960px up to 125%, 1152px at 150% (`min_window_width`, `minWindowWidth`). A higher level
MUST widen a narrower window.

#### Scenario: A zoom above 125%
- **WHEN** the level rises above 125%
- **THEN** the minimum width of the main window is 768px × level: 1152px at 150%, beyond the 960px of the toolbar
- **AND** a narrower window widens
  [@test] src-tauri/src/zoom_test.rs (`a_zoom_above_125_percent_widens_the_minimum_window`)
  [@test] src/lib/zoom.test.ts (`keeps 768 CSS px for the islands, so a zoom above 125% widens the minimum window`)
  [@test] tests/e2e/repository/zoom.spec.ts (`should zoom the islands but not the toolbar, the title bar of the window`)

#### Scenario: Up to 125%
- **WHEN** the level is at most 125%
- **THEN** the minimum width of the main window is 960px
  [@test] src/lib/zoom.test.ts (`keeps 768 CSS px for the islands, so a zoom above 125% widens the minimum window`)

### Requirement: Window size: A sub-window keeps the CSS px of its islands by its label

A sub-window MUST keep the CSS px of its islands the same way, by its label (`sub_window_min_size`, `minSubWindowSize`):
a review 640 px beside its 800px minimum (wider above 125%), Settings 600 px beside its 640px minimum (wider above
107%), Activity 400 px beside its 440px minimum (wider above 110%).

#### Scenario: A review
- **WHEN** a review window is zoomed
- **THEN** it keeps 640 px for its islands beside its 800px minimum, wider above 125%
  [@test] src-tauri/src/zoom_test.rs (`a_zoom_above_125_percent_widens_the_minimum_sub_window`)
  [@test] src/lib/zoom.test.ts (`is the size of the title bar of a review up to 125%, wider above`)

#### Scenario: Settings
- **WHEN** the Settings window is zoomed
- **THEN** it keeps 600 px for its islands beside its 640px minimum, wider above 107%
  [@test] src-tauri/src/zoom_test.rs (`the_settings_window_is_smaller_and_widens_above_107_percent`)
  [@test] src/lib/zoom.test.ts (`is smaller for Settings, and wider above 107%`)

#### Scenario: Activity
- **WHEN** the Activity window is zoomed
- **THEN** it keeps 400 px for its islands beside its 440px minimum, wider above 110%
  [@test] src/lib/zoom.test.ts (`is smaller still for Activity, and wider above 110%`)

### Requirement: Window size: A window gets the minimum of its level

A window MUST get the minimum of its level when it opens and when the level changes (`useZoom`).

#### Scenario: The level of the main window changes
- **WHEN** the level changes
- **THEN** the main window gets the minimum of the new level: 1152px at 150%, 960px again at Actual Size
  [@test] tests/e2e/repository/zoom.spec.ts (`should zoom the islands but not the toolbar, the title bar of the window`)

#### Scenario: A window opens
- **WHEN** a window opens
- **THEN** it gets the minimum of its level

### Requirement: Window size: Inputs and text areas are text-sm at any width

Inputs and text areas MUST be `text-sm` at any width (not the phone `text-base md:text-sm` of shadcn), so a zoomed
page narrower than `md` keeps their size.

#### Scenario: A zoomed page narrower than `md`
- **WHEN** a zoomed page is narrower than `md`
- **THEN** its inputs and text areas keep their size, `text-sm`

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

- [Title Bar](../title-bar/spec.md): the toolbar as the title bar.
- [UI Theme](../../../../docs/design/ui-theme.md): the canvas and the islands.
