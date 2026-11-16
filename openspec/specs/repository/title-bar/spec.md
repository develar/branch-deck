---
name: Title Bar
description: Requirements for the top row of every window as its title bar, as in the IntelliJ new UI: the toolbar of the main window and the row of each sub-window, beside the macOS traffic lights, with its own window buttons and main menu on Windows and Linux, the system title bar where a custom one doesn't fit or the user asks for it.
style: plain-1
targets:
  - src-tauri/src/lib.rs
  - src-tauri/src/title_bar.rs
  - crates/app-api/src/app/windows.rs
  - src-tauri/src/desktop.rs
  - src-tauri/src/menu.rs
  - src/hooks/useAutoUpdate.ts
  - src-tauri/src/auto_update.rs
  - src-tauri/src/zoom.rs
  - src-tauri/capabilities/title-bar.json
  - src-tauri/capabilities/default.json
  - src-tauri/capabilities/sub-windows.json
  - src/lib/title-bar.ts
  - src/hooks/useWindowFrameState.ts
  - src/hooks/useWindowTitle.ts
  - src/components/shared/WindowTitleBar.tsx
  - src/components/shared/WindowPage.tsx
  - src/lib/sub-window.ts
  - src/components/shared/ActionButton.tsx
  - src/components/repository/ConfigurationHeader.tsx
  - src/components/repository/RepositorySelector.tsx
  - src/pages/ReviewPage.tsx
  - src/components/review/ReviewBackButton.tsx
  - src/components/review/PullRequestReview.tsx
  - src/components/conflict/ConflictWindowPage.tsx
  - src/components/repository/WindowControls.tsx
  - src/main.tsx
  - src/components/shared/PageFailure.tsx
  - src/lib/page-errors.ts
  - src/lib/api-error.ts
  - src-tauri/src/api.rs
  - src/lib/resize-loops.ts
  - src/lib/observe-size.ts
  - src/lib/laid-out.ts
  - src/components/repository/MainMenuButton.tsx
  - src/lib/title-bar-mouse.ts
  - src/lib/main-menu.ts
  - src/lib/app-commands.ts
  - src/hooks/useAppCommand.ts
  - src/hooks/useMainMenuKeys.ts
  - src/components/settings/AppearancePage.tsx
  - src-tauri/capabilities/settings.json
  - src/pages/IndexPage.tsx
  - src/components/branch/BranchCreator.tsx
  - src/components/branch/BranchTableCard.tsx
  - crates/sync-core/src/repo_facts.rs
  - crates/branch-integration/src/common.rs
  - tests/e2e/setup/tauri-mocks.ts
  - src/model/store.ts
  - crates/repository-model/src/runtime.rs
---

# Title Bar

Status: Implemented
Date: 2026-10-15, sub-windows and Linux 2026-10-17, the first frame of the main window 2026-10-24

## Purpose

The top row of every window is its title bar, as in the IntelliJ new UI: the toolbar of the main window (repository,
Update, outgoing, robots, Activity, Sync, Settings), and the row of each sub-window (where the user is on the left, the view and
the actions on the right). It lies on the canvas above the islands, 40px high (`WindowTitleBar`). On macOS it sits
beside the traffic lights of an overlay title bar. On Windows and Linux the row has the window buttons where the desktop
puts them and the main menu (☰), which replaces the native menu bar, as IntelliJ does it on Linux
(`ide.linux.hide.native.title`, "Merge main menu with window title"). The system title bar stays where a custom one
doesn't fit (WSL, a tiling window manager, an unknown desktop) and where the user asks for it.

## Goals

- No grey native band above the canvas: every window is one surface, as an IntelliJ window is.
- The row drags the window and zooms or maximizes it on a double click, as a title bar does.
- The 40px a native title bar took go to the content: in a review window the stack, the view and the actions take the
  row, not an island of their own.

## Non-goals

- The Snap Layouts flyout of Windows 11 on hovering Maximize: it needs the native caption buttons (or the unmaintained
  `tauri-plugin-decorum`). Snapping by dragging to an edge and Win+Z keep working.
- Window buttons that follow a change of the desktop's button layout while the app runs: it is read at startup (as
  IntelliJ caches it).
- Right-to-left layouts: the app is left-to-right; the sides of the layout are physical.

## Concepts

### Linux

The rounded window keeps GTK's decorations. This is what Tauri apps on Linux do by hand
([dbx](https://github.com/t8y2/dbx/pull/11142), [Hatoba](https://github.com/scarletkc/Hatoba/pull/107): undecorated,
their own shadow and resize handles); keeping GTK's decorations leaves the shadow, resizing and snapping to GTK.

### Every platform

The repository, the switcher of the views and the count of the branches have a container of their own, so ☰ and the
window buttons of a custom title bar take their room first. Without it GNOME's 1077px maximized window put the names
and the count over the repository selector and Update: from 1032 to 1080px, and to 1152px once the time of the last
sync kept the room of its widest text and Activity stayed in an empty log.

### Main window

The budget of the content of the main window is 1 s from its shell: the rows of the first sync of the IntelliJ
monorepo came about 0.6 s after the page loaded.

A page that loaded is never shown by the deadline of its load because of what that did: on the monorepo at load 90 a
page loaded 2.26 s after the build, and that deadline showed the window 0.74 s later with no content and the warning
that the page did not load.

A window waits for the presentation of its webview because a page says its content settled from an effect, right after
React committed it and before WebKit painted it; a window shown then showed the frame before (the shell, or the bare
canvas when no frame had the shell) until the next rendering update reached the screen, 25 to 80 ms in the check copy
of the release app and longer on a loaded machine. Elsewhere than on macOS there is no earlier frame to show: WebKitGTK
paints nothing while its view is not visible, so a Linux window is transparent until WebKit's first frame of the page
as it is; WebView2 paints a hidden page, and Windows fades the window in from that frame.

The listing of a sync reads no commit of the archived and the remote branches under the prefix: on the monorepo they
were 452 refs, 1 to 2 s from a cold pack before the rows.

## Requirements

### Requirement: Kind: Every window is built the same way and tells its page the title bar

Every window MUST be built the same way (`with_title_bar` in `src-tauri/src/title_bar.rs`: the main window and
`open_sub_window`), and its initialization script MUST tell the page before it loads how the title bar is drawn
(`title_bar_script`, `window.__BD_TITLE_BAR__`, `titleBar` in `src/lib/title-bar.ts`), one JSON object decided once
per process (`init_title_bar`, in `setup` before the menu and the first window). The page MUST NOT guess from the user
agent.

#### Scenario: The page learns the title bar before it loads
- **WHEN** a window, the main window or one of `open_sub_window`, is built
- **THEN** its initialization script hands the page the one JSON object of the process before the page loads
- **AND** the page draws the title bar it names, never one it guesses from the user agent
  [@test] src-tauri/src/title_bar_test.rs (`the_page_gets_one_json_object`)

### Requirement: Kind: The object names the kind and whether the build updates itself

The object MUST be `{"kind":"overlay","doubleClick":…}` on macOS,
`{"kind":"custom","controls":{"start":[…],"end":[…],"style":…},"doubleClick":…,"cornerRadius":…}` on Windows and Linux,
`{"kind":"native","reason":…}` where the system title bar stays, each with `"checkForUpdates":…` (`PageTitleBar`):
whether the build updates itself (the `auto-update` feature), whatever draws the title bar, for ☰ and the command
palette (`updatesItself`).

#### Scenario: The kinds of the object
- **WHEN** the title bar is an overlay, a custom one or the system's
- **THEN** the page gets the object of that kind, each with `checkForUpdates`, true or false by the build
  [@test] src-tauri/src/title_bar_test.rs (`the_page_gets_one_json_object`)

#### Scenario: The page draws no title bar where the system's stays
- **GIVEN** the object is of the kind `native`
- **WHEN** the main window shows
- **THEN** the page draws no title bar of its own: no drag region, no window buttons, and it asks the window nothing
  [@test] tests/e2e/repository/title-bar.spec.ts (`should keep the native title bar where the page doesn't draw one`)

### Requirement: Kind: The system title bar stays for its reasons in their order

The system title bar MUST stay (`native`), in this order of reasons: `BRANCH_DECK_NATIVE_TITLE_BAR=1` (`env`); on Linux
WSL (`WSL_DISTRO_NAME` or `WSL_INTEROP`), no `XDG_CURRENT_DESKTOP` (`no-desktop`), a tiling window manager; then the
setting "Use the system title bar" (`setting`, Windows and Linux). The decision MUST be logged with its reason.

#### Scenario: A tiling window manager on Linux
- **GIVEN** Linux
- **WHEN** a part of `XDG_CURRENT_DESKTOP`, each `:` part matched, is a tiling window manager of IntelliJ's list plus
  niri
- **THEN** the system title bar stays, on Wayland too, unlike IntelliJ: an empty titlebar would take sway's title away
  [@test] src-tauri/src/title_bar_test.rs (`linux_keeps_the_system_title_bar_where_intellij_does_and_where_the_user_asked`)

#### Scenario: The reasons come in their order
- **WHEN** several reasons hold, as the environment variable, WSL, no desktop or a tiling window manager with the
  setting
- **THEN** the first of them in the order is the reason, and the setting is the reason only where no other holds
  [@test] src-tauri/src/title_bar_test.rs (`linux_keeps_the_system_title_bar_where_intellij_does_and_where_the_user_asked`)

#### Scenario: Windows keeps the system title bar only when asked
- **GIVEN** Windows, where the desktop variables mean nothing
- **WHEN** the setting is off, or on
- **THEN** the title bar is custom, or the system's with the reason `setting`
  [@test] src-tauri/src/title_bar_test.rs (`windows_keeps_the_system_title_bar_only_when_asked`)

#### Scenario: The decision is logged
- **WHEN** the process decides its title bar
- **THEN** the log names the decision with its reason

### Requirement: macOS: The traffic lights are centred in an overlay title bar

The window MUST have an overlay title bar with a hidden title; the traffic lights MUST be centred in the 40px toolbar,
14px from the left as from the top (`traffic_light_position(14, 22)`: wry takes y as the height of the button
container above the buttons, not as their top).

#### Scenario: A window on macOS
- **WHEN** a window opens on macOS
- **THEN** it has an overlay title bar without its title
- **AND** the traffic lights are 14px from the left and from the top, centred in the 40px toolbar

### Requirement: macOS: The toolbar starts after the traffic lights, in fullscreen at the edge

The toolbar MUST start after the traffic lights (`pl-22`, 15px after the zoom button); in fullscreen, where they are
hidden, it MUST start at the edge (`isFullscreen`, re-read on every resize).

#### Scenario: After the traffic lights, and at the edge in fullscreen
- **WHEN** the main window shows with an overlay title bar
- **THEN** the toolbar starts 15px after the zoom button
- **AND** once the window enters fullscreen and resizes, the toolbar starts at the edge
  [@test] tests/e2e/repository/title-bar.spec.ts (`should start the toolbar after the traffic lights of an overlay title bar, and not in fullscreen`)

### Requirement: Windows: The window has no decorations and keeps its shadow

The window MUST have no decorations; `shadow(true)` gives it the 1px border and, on Windows 11, the rounded corners.

#### Scenario: A window on Windows
- **WHEN** a window opens on Windows
- **THEN** it has no native caption, and `shadow(true)` gives it the 1px border
- **AND** on Windows 11 the rounded corners

### Requirement: Windows: The toolbar ends with the window buttons

The toolbar MUST end with Minimize, Maximize (Restore while maximized) and Close, flush with the right edge: 46×40 as
the native caption buttons, the Segoe Fluent Icons glyphs, `bg-muted` on hover and `bg-destructive` for Close. Each
MUST have an `aria-label` and a tooltip. A failed call MUST be a "Cannot minimize the window" error.

#### Scenario: The window buttons and their calls
- **WHEN** the main window shows with the custom title bar of Windows
- **THEN** Minimize, Maximize and Close end the toolbar, flush with the right edge, in the Segoe Fluent Icons glyphs
- **AND** a click on each calls the window: Maximize, then Restore while maximized
  [@test] tests/e2e/repository/title-bar.spec.ts (`should end a custom title bar with the window buttons`)

#### Scenario: A failed call
- **WHEN** the call of a button fails
- **THEN** an error says it, as "Cannot minimize the window"

### Requirement: Linux: The window keeps its decorations under an empty GTK titlebar

The window MUST keep its decorations, and get an empty GTK titlebar before GTK realizes it (`prepare_title_bar`): GTK
then draws client-side decorations without a title, as GNOME apps with a header bar have: the shadow, the resize edges
in it, the tiled, maximized and fullscreen states and the frame extents snapping needs.

#### Scenario: A window on Linux
- **WHEN** a window is built on Linux
- **THEN** it gets an empty GTK titlebar before GTK realizes it
- **AND** GTK draws its client-side decorations without a title: the shadow, the resize edges, the tiled, maximized
  and fullscreen states and the frame extents

### Requirement: Linux: Every window is built hidden and shows once its titlebar is set

Every window MUST be built hidden, the window-state plugin MUST NOT show it (no `StateFlags::VISIBLE`), and the main
thread MUST set the titlebar before the window shows (`prepare_title_bar`), then the window shows once its page settled
its content (`sub_window_ready`, [Main window](#main-window)).

#### Scenario: The window state restores no visibility
- **WHEN** the window-state plugin restores a window
- **THEN** it restores neither its decorations nor its visibility, and the rest of its state, as its size, position and
  maximized state
  [@test] src-tauri/src/title_bar_test.rs (`the_window_state_restores_neither_the_decorations_nor_the_visibility`)

### Requirement: Linux: The window buttons are those of GTK's decoration layout

The window buttons MUST be those of GTK's `gtk-decoration-layout` (GNOME's `button-layout` through XSETTINGS or the
portal, KDE's through kde-gtk-config), the start before `:` and the end after it, by GTK's rules; a missing or refused
one MUST be `:minimize,maximize,close`.

#### Scenario: GTK's rules of a layout
- **WHEN** the layout names `icon`, `menu`, `appmenu`, `spacer` or unknown names, or a button twice
- **THEN** those names are dropped, and a layout with a button twice is refused
- **AND** GNOME's default `appmenu:close` gives Close alone
  [@test] src-tauri/src/title_bar_test.rs (`reads_the_buttons_of_a_gtk_decoration_layout_by_its_rules`)

#### Scenario: A missing or refused layout
- **WHEN** the layout is missing or refused
- **THEN** the buttons are those of `:minimize,maximize,close`
  [@test] src-tauri/src/title_bar_test.rs (`a_missing_or_refused_layout_is_the_default_of_gtk`)

#### Scenario: The buttons where the layout puts them
- **WHEN** the main window shows with a custom title bar on Linux
- **THEN** the buttons stand where the desktop's layout puts them, in its look
- **AND** the buttons of a split layout split, the start ones before the main menu
  [@test] tests/e2e/repository/title-bar.spec.ts (`should put the window buttons where the desktop's layout does, in its look`, `should split the buttons of a split layout, the start ones before the main menu`)

### Requirement: Linux: A press drags the window only once the mouse moved 4px

A press on the empty parts MUST drag the window only once the mouse moved 4px, as GTK's header bar does
(`installTitleBarMouse`): Tauri's drag script asks the window manager to move the window on every press, and the grab
of the move took the second click of a double click.

#### Scenario: A press, a double click and a drag
- **WHEN** the user presses on an empty part of the row without a move, or double-clicks there
- **THEN** nothing drags the window, and the double click reaches the page and maximizes
- **AND** a press that moves 4px or more drags the window
  [@test] tests/e2e/repository/title-bar.spec.ts (`should drag only once the mouse moved, so a double click reaches the page and maximizes`)

### Requirement: Linux: The window is rounded as the desktop's windows are

With a compositor the window MUST be rounded as the desktop's windows are (libadwaita's 12px on GNOME, 8px on KDE,
`corner_radius`): the window is transparent (RGBA), GTK keeps drawing its decorations with the same radius (the shadow,
the resize edges in it, the frame extents; `round_corners` gives them the radius and no window background), and the
page clips its canvas to the radius (`data-window-rounded`; GTK3 doesn't clip the webview).

#### Scenario: The radius of the desktop
- **WHEN** the window opens on GNOME, or on KDE
- **THEN** its radius is 12px, or 8px
  [@test] src-tauri/src/title_bar_test.rs (`rounds_the_window_as_the_desktop_does`)
  [@test] tests/e2e/repository/title-bar.spec.ts (`should round the window as the desktop does, square while maximized or tiled`)

### Requirement: Linux: The window is square while maximized, fullscreen or tiled

The window MUST be square while maximized, fullscreen or tiled (GTK's window state, `window-tiled`), and without a
compositor.

#### Scenario: Maximized or tiled
- **WHEN** GTK says the window is tiled, or the window is maximized
- **THEN** it is square, and rounded again once it is not
  [@test] tests/e2e/repository/title-bar.spec.ts (`should round the window as the desktop does, square while maximized or tiled`)

#### Scenario: Where the system rounds it or can't
- **WHEN** the system rounds the window itself, or there is no compositor
- **THEN** the page does not round it
  [@test] tests/e2e/repository/title-bar.spec.ts (`should not round a window where the system does it or can't`)

### Requirement: Linux: The buttons look as IntelliJ draws them

The buttons MUST look as IntelliJ draws them (its icons, `platform/icons/src/linux/theme`): GNOME's (Adwaita) for an
icon theme starting with `adwaita` or `yaru`, KDE's (Breeze) for `breeze`, else KDE's on a KDE desktop and GNOME's
elsewhere.

#### Scenario: The look by the icon theme, then the desktop
- **WHEN** the icon theme starts with `adwaita` or `yaru`, is `breeze`, or is another one
- **THEN** the buttons are GNOME's, KDE's, or KDE's on a KDE desktop and GNOME's elsewhere
  [@test] src-tauri/src/title_bar_test.rs (`picks_the_look_of_the_buttons_by_the_icon_theme_then_the_desktop`)

#### Scenario: KDE's buttons
- **WHEN** the look is KDE's
- **THEN** the page draws KDE's buttons
  [@test] tests/e2e/repository/title-bar.spec.ts (`should draw KDE's buttons, dim them in an inactive window, and leave fullscreen with Restore`)

### Requirement: Windows and Linux: The main menu replaces the native menu bar

The main menu MUST replace the native menu bar: `configure_app_menu` sets no menu for a custom title bar (no menu bar
above the title bar, no accelerator table), the page's main menu holds its items (`mainMenus`) and the page handles
their keys ([Main menu](#main-menu)).

#### Scenario: A custom title bar
- **WHEN** a window has a custom title bar
- **THEN** it has no menu bar above the title bar and no accelerator table
- **AND** the page's main menu holds the items, and the page handles their keys

### Requirement: Windows and Linux: The system title bar keeps the native menu bar

A window with the system title bar MUST keep the native menu bar, with Check for Updates… in its Help menu.

#### Scenario: The system title bar
- **WHEN** a window has the system title bar
- **THEN** it keeps the native menu bar, whose Help menu has Check for Updates…

### Requirement: Windows and Linux: The window buttons are not in the tab order

The buttons MUST NOT be in the tab order, as native window buttons aren't; each MUST have an `aria-label` and a
tooltip.

#### Scenario: Tabbing through the row
- **WHEN** the user tabs through the row of a custom title bar
- **THEN** the focus skips the window buttons
- **AND** each button has an `aria-label` and a tooltip
  [@test] tests/e2e/repository/title-bar.spec.ts (`should put the window buttons where the desktop's layout does, in its look`)

### Requirement: Windows and Linux: Maximize is Restore while maximized or fullscreen

Maximize MUST be Restore while the window is maximized or fullscreen, and leave fullscreen there.

#### Scenario: Restore leaves fullscreen
- **GIVEN** a window in fullscreen
- **WHEN** the user clicks Restore
- **THEN** the window leaves fullscreen, not toggling maximized
  [@test] tests/e2e/repository/title-bar.spec.ts (`should draw KDE's buttons, dim them in an inactive window, and leave fullscreen with Restore`)

### Requirement: Windows and Linux: The buttons of an inactive window dim

The buttons of an inactive window MUST dim (`data-window-inactive`, the focus of the window).

#### Scenario: The window loses its focus
- **WHEN** the window loses its focus, and gets it back
- **THEN** its buttons dim, and no longer do
  [@test] tests/e2e/repository/title-bar.spec.ts (`should draw KDE's buttons, dim them in an inactive window, and leave fullscreen with Restore`)

### Requirement: Windows and Linux: A right click on the empty row opens the window menu

A right click on the empty parts of the row MUST open the window manager's window menu, as a native title bar does
(`show_window_menu` of app-api, on the main thread, through `MenuHost` of `src-tauri/src/menu.rs`); a control MUST keep
its own right click.

#### Scenario: A right click of the empty row and of a control
- **WHEN** the user right-clicks an empty part of the row, then a control
- **THEN** the first opens the window menu, the second no second menu
  [@test] tests/e2e/repository/title-bar.spec.ts (`should do what the system's double click does, and open the window menu on a right click of the empty row`)

#### Scenario: The system menu on Windows
- **GIVEN** Windows
- **WHEN** the window menu opens
- **THEN** it is the system menu with its items enabled by the state of the window, Restore while maximized or
  minimized, Move, Size and Maximize while not, as `DefWindowProc` would

#### Scenario: GDK's window menu on Linux
- **GIVEN** Linux
- **WHEN** the window menu opens
- **THEN** it is GDK's `show_window_menu` with the right button press of the webview of the last two seconds, whose
  serial Wayland checks, forgotten with its window

### Requirement: Main menu: ☰ is the first control of the main window's row

☰ ("Main Menu", Alt+\ in its tooltip) MUST be the first control of the main window's row, after the start buttons, as
IntelliJ's `MainMenuWithButton`. Sub-windows MUST have none.

#### Scenario: ☰ in the main window
- **WHEN** the main window shows with a custom title bar
- **THEN** ☰ is the first control of its row, after the start buttons, its tooltip naming Alt+\
  [@test] tests/e2e/repository/title-bar.spec.ts (`should split the buttons of a split layout, the start ones before the main menu`)

#### Scenario: A sub-window
- **WHEN** a sub-window shows with a custom title bar
- **THEN** its row has no ☰

### Requirement: Main menu: A click on ☰ expands the menu bar in place

A click on ☰ MUST expand the menu bar File, View, Help in place, over the start of the row, which hides meanwhile, with
File open; ←/→ go through the menus; Esc, a click outside or a chosen item MUST collapse it back to ☰ (IntelliJ's
`ExpandableMenu`).

#### Scenario: Expand, go through the menus, collapse
- **WHEN** the user clicks ☰, presses ←/→, then Esc
- **THEN** the menu bar expands over the start of the row with File open, the arrows go through the menus
- **AND** Esc collapses it back to ☰
  [@test] tests/e2e/repository/title-bar.spec.ts (`should expand the menu bar in place of ☰, go through the menus, and collapse on Escape`)

#### Scenario: A click outside or a chosen item
- **WHEN** the user clicks outside the expanded menu bar, or chooses an item
- **THEN** it collapses back to ☰

### Requirement: Main menu: The items are those of the native menu

The items MUST be those of the native menu (`src/lib/main-menu.ts`), in File, View and Help. There MUST be no Edit
menu: the webview's Ctrl+C/V/X/A stay.

#### Scenario: File
- **WHEN** the user opens File
- **THEN** it holds Sync Branches (Ctrl+R, disabled with the reason of the Sync button), Update Project (Ctrl+T),
  Auto-sync on Focus (checked), Settings… (Ctrl+,)

#### Scenario: View
- **WHEN** the user opens View
- **THEN** it holds Command Palette… (Ctrl+K, [Command Palette](../command-palette/spec.md)), Branches (Ctrl+1),
  Reviews (Ctrl+2) and Landing (Ctrl+3) ([Main Views](../main-views/spec.md)), Actual Size (Ctrl+0), Zoom In
  (Ctrl+=), Zoom Out (Ctrl+-), Activity ([Activity](../activity/spec.md))

#### Scenario: Help
- **WHEN** the user opens Help
- **THEN** it holds Getting Started, Install Agent CLI…, Check for Updates… (a build with the `auto-update` feature,
  as the native menu: `checkForUpdates` of the title bar), GitHub Repository

#### Scenario: No Edit menu
- **WHEN** the user presses Ctrl+C/V/X/A in the webview
- **THEN** they do what the webview does, as no Edit menu takes them

### Requirement: Main menu: Each item runs the command its native item runs

Each item MUST run the command the native item runs (`runAppCommand`, registered by the window that runs it with
`useAppCommand`).

#### Scenario: An item of the main menu
- **WHEN** the user chooses an item of the main menu
- **THEN** the command of the native item runs, in the window that registered it

### Requirement: Main menu: The native menu sends its commands to the main window

The native menu MUST send a command to the main window as an event named as it (`sync_branches`, `update_project`,
`check_for_updates`, `install_agent_cli`, `zoom` with its step, `open_settings`): the main window opens Settings
([Settings](../settings/spec.md)), as it has the current settings to start it from.

#### Scenario: Settings… of the native menu
- **WHEN** the user chooses Settings… in the native menu
- **THEN** the main window gets the event `open_settings` and opens Settings from its current settings

### Requirement: Main menu: Check for Updates… says what it found

Check for Updates… MUST say that the app is up to date, with its version, or offer the update the check downloaded
(Install and Restart, a toast that stays until the user acts); a failed check or install MUST say why ("Cannot check
for updates", "Cannot install the update").

#### Scenario: Up to date, an update, a failed install
- **WHEN** the user chooses Check for Updates…
- **THEN** it says the app is up to date with its version, or offers the downloaded update with Install and Restart
- **AND** a failed install says "Cannot install the update"
  [@test] tests/e2e/repository/title-bar.spec.ts (`should say the app is up to date, offer the downloaded update, and say why its install failed`)

#### Scenario: A failed check
- **WHEN** the check fails
- **THEN** it says why, "Cannot check for updates"

### Requirement: Main menu: Check for Updates… joins the silent check of the start

The main window MUST also check silently a few seconds after its start; Check for Updates… during a download of that
check MUST join it, its progress toast saying "Downloading version 1.3.0…" at once.

#### Scenario: Check for Updates… while the silent check downloads
- **GIVEN** the silent check of the start downloads version 1.3.0
- **WHEN** the user chooses Check for Updates…
- **THEN** its progress toast says "Downloading version 1.3.0…" at once, then offers the update
  [@test] src/hooks/useAutoUpdate.test.ts (`says at once that the silent check running meanwhile downloads its version, then offers it`)
  [@test] src-tauri/src/auto_update.rs (`a_download_names_its_version_in_the_status_meanwhile`)

### Requirement: Main menu: The keys open the menus

Alt+\ and F10 MUST open File, Alt+F, Alt+V and Alt+H their menu (the physical keys), the mnemonics underlined.

#### Scenario: The keys of the menus
- **WHEN** the user presses Alt+\ or F10, or Alt+F, Alt+V or Alt+H
- **THEN** File opens, or that menu, with the mnemonics underlined, and its items run
  [@test] tests/e2e/repository/title-bar.spec.ts (`should open from the keyboard and run the items`)

### Requirement: Main menu: A lone Alt tap on Windows highlights File

On Windows a lone Alt tap MUST highlight File, unless a key, a click or leaving the window came between Alt down and up
(IntelliJ's `WinAltKeyProcessor`); a held Alt that repeats MUST still be a tap on its release.

#### Scenario: A lone Alt tap, and one after another key
- **GIVEN** Windows
- **WHEN** the user taps Alt alone, or presses another key between Alt down and up
- **THEN** File is highlighted, or nothing is
  [@test] tests/e2e/repository/title-bar.spec.ts (`should highlight File on a lone Alt tap on Windows, not after another key`)

### Requirement: Main menu: The page handles the keys of the native menu where it is gone

The page MUST handle the keys of the native menu where it is gone, in every window with a custom title bar
(`useMainMenuKeys`, the physical key, as the native accelerators go by the virtual key: a Cyrillic layout's Ctrl+R
syncs too). They MUST be `preventDefault`ed (WebView2 would reload on Ctrl+R).

#### Scenario: Ctrl+R and Ctrl+,
- **WHEN** the user presses Ctrl+R, or Ctrl+, in the main window of a custom title bar
- **THEN** it syncs, or opens the settings
  [@test] tests/e2e/repository/title-bar.spec.ts (`should take the keys of the native menu: Ctrl+R syncs, Ctrl+, opens the settings`)

#### Scenario: Ctrl+K
- **WHEN** the user presses Ctrl+K
- **THEN** the command palette opens
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should open from Ctrl+K and from View → Command Palette… of ☰, and run Update Project and Check for Updates…`)

#### Scenario: The zoom keys
- **WHEN** the user presses a zoom key
- **THEN** all windows zoom one step, once per press
  [@test] tests/e2e/repository/zoom.spec.ts (`should step once per press with the keys the page handles in a custom title bar`)

### Requirement: Main menu: A sub-window sends the keys to the main window

Ctrl+R and Ctrl+T MUST run in the main window (a sub-window sends them there), Ctrl+, MUST open Settings and Ctrl+K the
command palette, also through the main window.

#### Scenario: Sync and Settings from a sub-window
- **WHEN** the user presses Ctrl+R or Ctrl+, in a sub-window
- **THEN** the sub-window sends them to the main window, which syncs or opens Settings
  [@test] tests/e2e/repository/title-bar.spec.ts (`should send Sync and Settings from a sub-window to the main window`)

### Requirement: Main menu: A window with a native menu leaves the keys to it

A window with a native menu MUST leave the keys to it.

#### Scenario: The keys under a native menu
- **GIVEN** a window with the native menu
- **WHEN** the user presses a key of the menu
- **THEN** the page does not handle it
  [@test] tests/e2e/repository/title-bar.spec.ts (`should leave the keys to the native menu where the window has one`)

### Requirement: Setting: The user can ask for the system title bar

Settings → Appearance → Title bar (Windows, Linux) MUST offer "Use the system title bar" (`appSettings.systemTitleBar`,
off): the system title bar and menu bar instead of the page's (IntelliJ's "Merge main menu with window title",
inverted).

#### Scenario: The switch in Settings → Appearance
- **WHEN** the user opens Settings → Appearance on Windows or Linux
- **THEN** Title bar offers "Use the system title bar", off
  [@test] tests/e2e/repository/title-bar.spec.ts (`should offer the system title bar in Settings → Appearance, applied after a restart`)

### Requirement: Setting: A change applies after a restart

The backend MUST read the setting from `settings.json` at startup, so a change MUST say "Applies after a restart" with
Restart (`restart_app`, which writes the setting to `settings.json` itself before it restarts: the main window saves
the settings debounced).

#### Scenario: The backend reads the stored setting
- **WHEN** the app starts
- **THEN** the backend reads the setting from `settings.json`
  [@test] src-tauri/src/title_bar_test.rs (`reads_the_stored_setting`)

#### Scenario: A change of the switch
- **WHEN** the user turns the switch
- **THEN** it says "Applies after a restart" with Restart, which writes the setting before it restarts
  [@test] tests/e2e/repository/title-bar.spec.ts (`should offer the system title bar in Settings → Appearance, applied after a restart`)

### Requirement: Setting: A fixed system title bar shows in the switch with its reason

Where the system title bar is fixed (WSL, a tiling window manager, an unknown desktop, the environment variable) the
switch MUST be on, disabled, and say why.

#### Scenario: A fixed system title bar
- **GIVEN** the system title bar is fixed by a reason that is not the setting
- **WHEN** the user opens Settings → Appearance
- **THEN** the switch is on, disabled, and says why
  [@test] tests/e2e/repository/title-bar.spec.ts (`should show a fixed system title bar in the settings with its reason`)

### Requirement: Every platform: The switcher of the views follows the repository

After the repository the row MUST hold the switcher of the views, Branches · Reviews · Landing
([Main Views](../main-views/spec.md)): where the user is, as the left side of the row says; its names MUST give way to
the icons on a narrow window.

#### Scenario: The view the user is in
- **WHEN** the user switches the view, from the switcher or the native View menu
- **THEN** the switcher after the repository says the view the user is in
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

#### Scenario: A narrow window
- **WHEN** the window is narrow
- **THEN** the names of the views give way to their icons

### Requirement: Every platform: The empty parts of the row drag the window

The empty parts of the row MUST drag the window (`data-tauri-drag-region="deep"`); the controls in it MUST keep their
clicks.

#### Scenario: The drag region of the row
- **WHEN** the main window shows with an overlay or a custom title bar
- **THEN** its row is a drag region, `data-tauri-drag-region="deep"`, and its controls keep their clicks
  [@test] tests/e2e/repository/title-bar.spec.ts (`should start the toolbar after the traffic lights of an overlay title bar, and not in fullscreen`)

### Requirement: Every platform: A double click does what the system's title bar does

A double click on the empty parts of the row MUST do what the system's title bar does there (macOS
`AppleActionOnDoubleClick`, GTK's `gtk-titlebar-double-click`: zoom or maximize, minimize, nothing;
`installTitleBarMouse` takes the second click before Tauri's drag script, which always maximizes).

#### Scenario: The double-click setting of the system
- **WHEN** the system's setting says to zoom or maximize, to minimize, or nothing
- **THEN** a double click on the empty row does that
  [@test] src-tauri/src/title_bar_test.rs (`maps_the_double_click_setting_of_the_system`)

#### Scenario: A double click where the system minimizes
- **GIVEN** the system's double click minimizes
- **WHEN** the user double-clicks the empty row
- **THEN** the window minimizes and does not maximize
  [@test] tests/e2e/repository/title-bar.spec.ts (`should do what the system's double click does, and open the window menu on a right click of the empty row`)

### Requirement: Every platform: Every window has the permissions of the title bar

Every window MUST have the permissions of the title bar (one capability, `title-bar.json`: the window state, dragging,
the double click, the window buttons, the title, the minimum size); a test checks that every sub-window label has it.

#### Scenario: Every sub-window label in the capability
- **WHEN** the capabilities are read
- **THEN** `title-bar.json` names the main window and every sub-window label, with the permissions of the title bar
  [@test] src-tauri/src/title_bar_test.rs (`every_window_gets_the_permissions_of_the_title_bar`)

### Requirement: Every platform: The window state restores no decorations and no visibility

The window-state plugin MUST restore every state it saves, as the size, the position and maximized, but never the
decorations (a saved `decorated` would bring the native caption back on Windows) nor the visibility (a window shows once its title bar is set and its page settled its
content, `show_when_settled`).

#### Scenario: A window restored by the window-state plugin
- **WHEN** the window-state plugin restores a window
- **THEN** it restores its size, its position and the rest of its state, not its decorations nor its visibility
  [@test] src-tauri/src/title_bar_test.rs (`the_window_state_restores_neither_the_decorations_nor_the_visibility`)

### Requirement: Every platform: The toolbar keeps its size at any page zoom

The toolbar MUST keep its size at any page zoom, as window chrome does ([Zoom](../zoom/spec.md)).

#### Scenario: A zoom of the page
- **WHEN** the user zooms the page
- **THEN** the islands zoom, but not the toolbar, the title bar of the window
  [@test] tests/e2e/repository/zoom.spec.ts (`should zoom the islands but not the toolbar, the title bar of the window`)

### Requirement: Every platform: Every control stays on the one 40px line at every width

Every control MUST stay on the one 40px line, apart from the others, at every width of the window down to its 960px
minimum, beside the traffic lights, between ☰ and the window buttons, or under the system title bar.

#### Scenario: Beside the traffic lights
- **GIVEN** an overlay title bar
- **WHEN** the window is any width from 960px up
- **THEN** the controls of the toolbar stay apart on one line
  [@test] tests/e2e/repository/title-bar.spec.ts (`should start the toolbar after the traffic lights of an overlay title bar, and not in fullscreen`)
  [@test] tests/e2e/repository/title-bar.spec.ts (`should keep the toolbar beside the traffic lights on one line at every width up to 1200px, the names of the views gone at the minimum`, `should keep the toolbar beside the traffic lights on one line at every width above 1200px, the names of the views where the repository keeps its room`)

#### Scenario: Between the main menu and the window buttons
- **GIVEN** a custom title bar, of GNOME's buttons or of Windows
- **WHEN** the window is any width from 960px up
- **THEN** the controls stay apart on one line between ☰ and the window buttons
  [@test] tests/e2e/repository/title-bar.spec.ts (`should keep the toolbar between the main menu and the window buttons on one line at every width up to 1200px, the names of the views gone at the minimum`, `should keep the toolbar between the main menu and the window buttons on one line at every width above 1200px, the names of the views where the repository keeps its room`, `should keep the toolbar between the main menu and the window buttons of Windows on one line at every width up to 1200px, the names of the views gone at the minimum`, `should keep the toolbar between the main menu and the window buttons of Windows on one line at every width above 1200px, the names of the views where the repository keeps its room`)

#### Scenario: Under the system title bar
- **GIVEN** the system title bar
- **WHEN** the window is any width from 960px up
- **THEN** the controls stay apart on one line
  [@test] tests/e2e/repository/title-bar.spec.ts (`should keep the toolbar under the system title bar on one line at every width up to 1200px, the names of the views gone at the minimum`, `should keep the toolbar under the system title bar on one line at every width above 1200px, the names of the views where the repository keeps its room`)

#### Scenario: The minimum window width
- **WHEN** the window is at its 960px minimum
- **THEN** the controls of the toolbar stay apart on one line
  [@test] tests/e2e/repository/header.spec.ts (`should keep its controls apart on one line at the minimum window width`)

### Requirement: Every platform: The repository, views and count give way in order

The repository, the switcher of the views and the count of the branches MUST share the room the rest of the row leaves
them (a container of its own, `ConfigurationHeader`) and give way in this order: the repository selector down to its
room beside the names, then the names of the views and the count together (below `@md`, 28rem, of that room), then the
selector down to its minimum; the actions, the time of the last sync among them, MUST keep their place.

#### Scenario: A narrowing row
- **WHEN** the window narrows
- **THEN** the repository selector gives way first, down to its room beside the names
- **AND** the names of the views and the count go together where that room is below `@md`
- **AND** the selector then gives way down to its minimum, while the actions keep their place
- **AND** so ☰ and the window buttons of a custom title bar take their room first
  [@test] tests/e2e/repository/title-bar.spec.ts (`should keep the toolbar beside the traffic lights on one line at every width up to 1200px, the names of the views gone at the minimum`, `should keep the toolbar beside the traffic lights on one line at every width above 1200px, the names of the views where the repository keeps its room`, `should keep the toolbar between the main menu and the window buttons on one line at every width up to 1200px, the names of the views gone at the minimum`, `should keep the toolbar between the main menu and the window buttons on one line at every width above 1200px, the names of the views where the repository keeps its room`, `should keep the toolbar between the main menu and the window buttons of Windows on one line at every width up to 1200px, the names of the views gone at the minimum`, `should keep the toolbar between the main menu and the window buttons of Windows on one line at every width above 1200px, the names of the views where the repository keeps its room`, `should keep the toolbar under the system title bar on one line at every width up to 1200px, the names of the views gone at the minimum`, `should keep the toolbar under the system title bar on one line at every width above 1200px, the names of the views where the repository keeps its room`)

### Requirement: Every platform: The repository selector keeps its path and width

The repository selector MUST keep the selected path and its width in the toolbar; its pop-up grows downward. With
linked repositories it MUST separate them from Recent under their group labels; without links, the recent-project
list MUST remain unlabelled.

#### Scenario: Linked repositories
- **GIVEN** the selected repository has linked repositories
- **WHEN** the user opens the selector
- **THEN** its pop-up grows downward and shows them apart from Recent, under their group labels

#### Scenario: No links
- **GIVEN** the selected repository has no linked repositories
- **WHEN** the user opens the selector
- **THEN** the recent-project list remains unlabelled
  [@test] tests/e2e/repository/header.spec.ts (`should show the unlabelled recent-project list when there are no companions`)

### Requirement: Every platform: An empty repository selector explains how to open a repository

The repository selector MUST be named "Repository". With no options, its popup MUST show "No recent repositories" and
"Use the folder button to open a Git repository. Repositories you open appear here.", with padded, wrapping,
nonselectable text that describes the open selector. Without a folder dialog it MUST instead say "No repositories
available" and "Open a Git repository in your IDE project." The popup MUST support keyboard opening and Escape with
focus returned to the selector.

#### Scenario: An empty selector opened
- **GIVEN** the welcome card is visible and there are no repository choices
- **WHEN** the user opens the repository selector by click or Arrow Down
- **THEN** its explanation is readable over the card, is not a repository option, and Escape closes it and returns focus
  [@test] tests/e2e/repository/header.spec.ts (`should explain the empty repository selector opened by ${activation} and return focus on Escape`)

#### Scenario: A repository opened from the folder button
- **GIVEN** the selector showed its empty explanation
- **WHEN** the user opens a repository through the folder button and opens the selector again
- **THEN** the repository is listed and the empty explanation is gone
  [@test] tests/e2e/repository/header.spec.ts (`should replace the empty selector with the repository opened through the folder button`)

#### Scenario: The host has no folder dialog
- **WHEN** the user opens an empty selector on an IDE host without a folder dialog
- **THEN** it explains how to open a Git repository in the IDE project and offers no folder button
  [@test] tests/e2e/repository/header.spec.ts (`should explain an empty repository selector without offering a folder dialog on an IDE host`)

### Requirement: Every platform: The first sync moves no control of the toolbar

The toolbar of the main window MUST keep the room of the time of the last sync from the selection of a repository on,
empty before its first sync, and Update MUST come on the left of the controls it would move: the toolbar stands at its
end, so what the first sync brings moves neither Activity nor the landing indicator from under a still pointer.

#### Scenario: The pointer on Activity during the first sync
- **GIVEN** a repository selected and never synced, the pointer on Activity, its tooltip open
- **WHEN** the first sync brings Update and "Last synced just now"
- **THEN** Activity and the landing indicator keep their boxes, and the tooltip of Activity stays open, in both engines
  [@test] tests/e2e/repository/header.spec.ts (`should keep Activity under the pointer, its tooltip open, as the first sync brings Update and the time of the last sync`)

### Requirement: Every platform: A window whose page fails says so

When a render throws an error that nothing in the page catches, or the page does not load, the window MUST show the
failure at once, not a blank canvas: a title, the message of the error, Reload and Close Window. The log of the app
MUST hold every error of the page, caught or not; a ResizeObserver loop of the observers of a library as a warning, one
that repeats and one of the page's own observers as an error.

#### Scenario: A render throws
- **WHEN** a render of the page throws an error that nothing catches
- **THEN** the window shows "Cannot show the window" with the message of the error, at once, not by a deadline
- **AND** the log of the app has the error with its stack and the components it was thrown in
- **AND** where the page draws the title bar, the top row of the failure drags the window
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the main window on the failure of a render that threw, log it, and reload or close the window`)

#### Scenario: The page does not load
- **WHEN** the window has a path that no page has, or the chunk of its page does not load
- **THEN** the window shows the same failure, titled "Cannot open the page of the window"
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the failure of a page that did not load`)

#### Scenario: Reload and Close Window
- **WHEN** the user presses Close Window on the failure
- **THEN** the window closes
- **WHEN** the user presses Reload
- **THEN** the page loads again, and shows itself when the failure does not come again
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the main window on the failure of a render that threw, log it, and reload or close the window`)

#### Scenario: An error outside a render
- **WHEN** a module of the page throws as it loads, a handler throws, or a promise that nobody awaits fails
- **THEN** the log of the app has the error
  [@test] tests/e2e/repository/first-frame.spec.ts (`should log an error of a script and a rejection that nobody awaits`)

#### Scenario: An error the page catches
- **WHEN** the page catches an error of a command or of its session (a failed subscription, sync or read)
- **THEN** the log of the app has it, not only the console of the page
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says that a repository without commits has nothing to sync yet`)

#### Scenario: A ResizeObserver loop
- **WHEN** the engine reports a ResizeObserver loop, whose notifications come in the next frame
- **THEN** the log of the app has a warning of a loop of the observers of a library; three within five seconds are an error, a loop that repeats however slowly
- **AND** a loop that left an element of an observer of the page pending is an error at once: one laid out at another size than reported in the box it observes (the content box unless the border box), a scroll bar that takes room in the border box and not in the content box, as the engine tells it
- **AND** the engine's own boxes tell the elements a loop left pending, from the update after it: at a zoom of the app other than 100% or a fractional device scale WebKit lays out borders, paddings and bars on a grid the computed style does not say, and an element laid out as reported is none; a loop is weighed at the time it came
- **AND** an element that changed or left the page between the loop and that update (a render of a task, a remount) is not named: a pending one that changed again is no error, rather than an error of one that was not pending
  [@test] src/lib/observe-size.test.ts (`tells nothing at a loop of an element a task resized before the probe reported, pending or not`, `tells nothing at a loop of an element that left the page before the probe reported, as in a remount`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should take a loop for a library's when a task resizes an element of the app after it, at a zoom of 110% @webkit`, `should take a loop for a library's when an element of the app leaves the page after it until the next update, at a zoom of 110% @webkit`)
  [@test] src/lib/observe-size.test.ts (`names no element at a loop that the engine lays out as reported, where the arithmetic lays it out otherwise, as at a zoom of the app`, `names no element at a loop whose border box, which its observation observes, the engine lays out as reported`, `names no element at a loop whose content box is empty, its border box laid out by the engine as reported`, `names an element of ours at a loop that has no box any more, which the probe does not report`, `tells the loops in their order: a loop of a library alone after one that waits for the probe waits as well`)
  [@test] src/lib/page-errors.test.ts (`weighs a loop by the time it came, however late the observers of the page tell its elements`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should take a loop for a library's when the engine lays an element of the app out as reported, at a device scale of 125% @webkit`, `should take a loop for a library's when the engine lays an element of the app out as reported, at a device scale of 150% @webkit`, `should take a loop for a library's when the engine lays an element of the app out as reported, at a zoom of 110% @webkit`)
- **AND** the line names each element it left pending by its test id or slot, else by its tag and role in the nearest ancestor that has one (`div[treeitem] in review-sidebar`)
- **AND** each kind of line comes once in ten seconds at most, with the count of the loops it left out
- **AND** the page takes the event of the engine as its own: no console reports the loop as an uncaught error
  [@test] src/lib/page-errors.test.ts (`logs a loop of the observers of a library as a warning, and three within five seconds as an error, however slowly they repeat`, `logs loops five seconds apart as warnings, one in ten seconds at most with the count of those it left out`, `logs a loop that left an element of the observers of the page pending as an error at once, then once in ten seconds with the count`)
  [@test] src/lib/observe-size.test.ts (`names at a loop the elements of ours resized since their report, or observed with a size and never reported`, `names at a loop an element with no test id or slot by its tag and role in the nearest ancestor that has one`, `names at a loop an element whose content box changed inside the same border box, as the engine compares it`, `compares the box an observation observes alone, as the engine does`)
  [@test] src/lib/laid-out.test.ts (`reads the content box of a scroller without its scroll bar, as its observer reports it, in either box sizing`, `reads no scroll bar of an overlay bar or of none`, `reads no scroll bar of the pixel the rounding of the client box leaves`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should read the boxes of a scroller whose scroll bar takes room as its observer reports them @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should attribute a loop to the observer of the app or of a library whose element it left pending`)

#### Scenario: No observer of the page makes a loop
- **WHEN** the page changes in a delivery of an observer (of the page or of a library), from its report to the next frame or the next task, whichever comes first
- **THEN** a change in the callback or in the chain of 256 microtasks after it (past the flushSync with which floating-ui places a popup, 42 to 87 microtasks after the callback of its observer) lets go every element of the observers of the page no deeper than the elements of the latest report, as it may have resized it (the layout is the page's: a block above a view that grows shrinks the view, a sibling that grows shrinks it in a flex row, a view hidden around a table empties its cells); a change past the chain (a render in a task, a microtask of the update later still) lets go the elements it is in; each is observed anew as the delivery ends
- **AND** an element deeper than them stays observed, delivered at its depth in that frame, and one let go comes back at once when a report of shallower elements shows it deeper
- **AND** a change after the delivery lets nothing go: the next update delivers it in its own frame
- **AND** an element first observed in the delivery, no deeper than the elements of the latest report (a block a flushSync in a callback mounted, observed by its ref; the cells of a table mounted again; an element in place an effect observes), is observed only as it ends; one its observer observes again there leaves the engine's observer until then
- **AND** a report of an observer of a library opens the delivery at its depth as one of ours does; the observer of the library is the engine's class, prototype and methods, with the same entries in the same frames
  [@test] src/lib/observe-size.test.ts (`lets an element go at once on a change of the delivery, and observes it anew as the delivery ends`, `observes an element the delivery let go anew at the next frame, when that frame comes before the next task`, `leaves an element observed that a change after the delivery resizes: the next update delivers it in its own frame`, `leaves an element deeper than the elements of the latest report observed: the update delivers it at its depth`, `observes an element its report let go anew as a report of shallower elements shows it deeper than they are`, `leaves an element it no longer observes alone`)
  [@test] src/lib/observe-size.test.ts (`lets the view go on a change a commit of React makes in a microtask right after the callback`, `lets an element its observer observes again in the delivery leave the engine's observer until the delivery ends`, `keeps an element observed first in the delivery waiting when a report shallower than the delivery, but not than it, follows`)
  [@test] src/lib/observe-size.test.ts (`lets the view go that a sibling grown three promises after the callback shrinks, though the change is not in it`, `lets the view go that a sibling grown a hundred and twenty promises after the callback of a library shrinks, as floating-ui places a popup`, `lets the view go that a sibling grown 255 promises after the callback shrinks, in the last microtask of the chain`, `leaves the view observed that a sibling grown 256 promises after the callback shrinks, the first past the chain`, `makes the chain as long again from a callback that begins in the chain of another`)
  [@test] src/lib/observe-size.test.ts (`lets go in a task before the delivery ends only the elements a change is in, and observes an element first observed there as it ends`, `observes a row a callback inserted and first observed only as the delivery ends`, `observes a row a commit of React inserted and first observed in a microtask after the callback only as the delivery ends`, `observes an element first observed microtasks after the callback, with no change, only as the delivery ends`)
  [@test] src/lib/observe-size.test.ts (`lets every element of ours go that a change anywhere in the delivery can resize, and leaves one deeper than the report observed`, `keeps an element let go above a report of deeper elements let go when a report shallower than they are, but not than it, follows`)
  [@test] src/lib/observe-size.test.ts (`observes an element first observed in the delivery no deeper than its report only as it ends, with no change to tell it`, `observes the cells of a table mounted again in the delivery only as it ends`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should read a large step by its groups: their files flat by their directory in the module, their tests last, a new module marked, the wiring folded at the end`)
  [@test] src/lib/observe-size.test.ts (`lets an element of ours go that the callback of a library resizes in its delivery, in a frame without a report of ours`, `takes the depth of a report of a library deeper than ours as the depth of the delivery`, `gives a library the engine's observer: its class, prototype and methods, and its callback the same entries and observer`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should give the observer of a library the engine's class and the same entries in the same frames @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app in the review of a pull request as its overview, its commits and their cards come and go @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app as the menu of a row and its submenu open and close @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app as a sibling grown 3 promises after the callback of a library shrinks the room of the repository path @webkit`, `should make no loop of the app as a sibling grown 120 promises after the callback of a library shrinks the room of the repository path @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should make no loop of the app as the toasts of two checks for updates stack and close @webkit`)
  [@test] tests/e2e/resize-loops/resize-loops.spec.ts (`should paint no frame of a review with its overview over its first file as it opens, on a slow CPU`)

#### Scenario: A cycle of an observer of the page
- **WHEN** an element is let go so in ten of twenty frames (in every frame, in every other) after its report there, resized by a change of a callback of the page (its own observer's or another's)
- **THEN** the log of the app has an error, once while the cycle goes on, however slowly the window renders its frames
- **AND** an element let go three frames apart, one resized between frames (a drag), one let go before its own report in the delivery, or one let go by a change after the update, is no cycle
- **AND** the release that would make a cycle is measured against the layout of the element as reported, one arithmetic for both, so a drag is no cycle at any zoom or scale; a bar estimated a pixel apart is the same bar, one that came or went a change in the box observed
  [@test] src/lib/observe-size.test.ts (`logs no cycle of an element a drag resizes between frames where the arithmetic lays it out otherwise than the engine`)
  [@test] src/lib/laid-out.test.ts (`takes a bar the rounding of whole pixels estimates a pixel apart for the same bar`, `tells a bar that came or went, and a box of another size, in the box observed`)
  [@test] src/lib/observe-size.test.ts (`logs a cycle once when its own callback resizes an element in its delivery in every frame`, `logs a cycle that resizes an element in the delivery of every other frame`, `logs no cycle of an element let go three frames apart, nor of one whose releases a pause of twenty frames parts`, `counts an element its observers let go in one delivery once`, `logs no cycle of an element resized between frames, nor of one let go by a change after the update`, `logs no cycle of an element another callback lets go before its own report in every frame, as a drag resized it`)

### Requirement: Every platform: A failed call of a command is its error answer

A command MUST answer every failure of its call as its error, never as a rejection of the page (`commandError` of
`src/lib/api-error.ts` in the `typedError` of the bindings): the `ApiError` it answered as is; a refusal of Tauri (a
string) and a failure of the IPC (an `Error`) as its `internal` error in their words, with one warning of the log,
"Cannot run <command>", as the backend logged nothing of it.

#### Scenario: The call of a command fails
- **WHEN** Tauri refuses the call, or its IPC fails: the read of the Safe Push statistics, the check of a typed branch name, the stop of the watch of an Air session
- **THEN** the command answers its `internal` error in the words of the failure, the log has one warning "Cannot run <command>" (none for an error the command answered), the read ends with it, the submit of a typed name stays open and asks the check again; one that fails again is refused, and the field says why; and the page has no unhandled rejection
  [@test] src/lib/api-error.test.ts (`resolves a refusal of Tauri (a string: an unknown command, an ACL refusal, invalid arguments) to an internal error`, `resolves a failure of the IPC (an Error: a fetch of the E2E bridge, the Air shim) to an internal error, never a rejection`, `resolves to the ApiError it answered, which the backend logged: no warning of the page`, `answers a failure of no words with words of its own, never an empty message`)
  [@test] src/stores/robot-stats.test.ts (`ends a read whose IPC failed with its error, never a rejection of the page: the entry stops loading and says it`)
  [@test] src/presenters/branch-name.test.ts (`says a check whose call failed and marks the name invalid, so no form submits a name nobody checked`)
  [@test] src/lib/debounced-answers.test.ts (`shows a failed answer without keeping it: no pending one, and asked again, a later answer of the same input kept`)
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should ask the check of a typed name again on the submit when its call failed, and rename once it answers`)
  [@test] src/lib/air-launch.test.ts (`a stop whose call fails (the IPC, a refusal of Tauri) is a line of the log, never a rejection of the page`)

### Requirement: Every platform: Every command answers a Result

Every command MUST answer a Result in the bindings, the value of a body that cannot fail (`plain`, `now`) as its `ok`
answer (`src-tauri/src/api.rs`), so that the failed call of any command is its error answer.

#### Scenario: A command whose body cannot fail
- **WHEN** the bindings are exported
- **THEN** every command of them answers a Result (`typedError`), a body that cannot fail too (`plain`, `now`), so no call of the page rejects
  [@test] src-tauri/src/bindings_test.rs (`every_command_of_the_bindings_answers_a_result`)

### Requirement: Main window: The main window shows on its first frame with content

The main window MUST be built hidden and show on its first frame with content, the way a sub-window does (one
`SubWindowState`, `show_when_settled`, `sub_window_ready`), never as a bare canvas nor as a title row over an empty
body. Its first frame MUST be the title row with the first rows of the window's sync (`branchesGrouped`), or with a
state that has no rows (`BranchCreator`, `markWindowContentReady`).

#### Scenario: The first rows of the sync
- **WHEN** the main window opens on a repository whose sync gives rows
- **THEN** it shows on the title row with the first rows of the window's sync (`branchesGrouped`)
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the main window on the first rows of its sync, the table loading where they come until then`)

#### Scenario: A state that has no rows
- **WHEN** the window has no rows to wait for: no repository, Welcome, an error, a sync that ended without rows (none
  found)
- **THEN** it shows on the title row with that state
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the main window without rows where the window has none to wait for`)

#### Scenario: Ready to Sync
- **WHEN** the window opens on Ready to Sync, which waits for the user, not for a sync
- **THEN** it shows on the title row with Ready to Sync
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the main window on Ready to Sync, which waits for the user, not for a sync`)

#### Scenario: A first sync deferred by git
- **WHEN** git defers the first sync, so it ends without rows
- **THEN** the window shows without a table that loads for ever
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the main window when git defers its first sync, without a table that loads for ever`)

### Requirement: Main window: Its open is its build, and its load hands it no settings

The open of the main window MUST be its build (`__SUB_WINDOW_OPEN__` of its init script); its load
(`sub_window_loaded`) MUST hand it no settings, it asks for them itself (`get_settings_changes`).

#### Scenario: A change of the settings while the page loads
- **GIVEN** the settings change while the page of the main window loads
- **WHEN** the page loads (`sub_window_loaded`)
- **THEN** the load hands it no settings, and the page asks for them itself
- **AND** its first rows show it once
  [@test] crates/app-api/src/windows_test.rs (`the_main_window_waits_for_its_first_rows_longer_and_takes_no_settings_with_its_load`)

### Requirement: Main window: It shows at most 1 s after its page committed its shell

The main window MUST show at most 1 s after its page committed its shell (`MAIN_CONTENT_BUDGET`): the title row and the
loading table, which the first render of the title row says (`markWindowShellDrawn`, `sub_window_drawn`), so a slow
sync shows them, never a bare canvas, also when the page took long from its load to its first render.

#### Scenario: A slow sync
- **WHEN** the first rows do not come within the budget after the shell
- **THEN** the window shows the title row and the loading table once the budget passed
- **AND** the budget counts from the shell, not from the load
  [@test] crates/app-api/src/windows_test.rs (`the_budget_of_the_content_counts_from_the_shell_not_from_the_load`)

#### Scenario: The first rows within the budget
- **WHEN** the first rows come within the budget
- **THEN** they show the window, and the budget then does nothing
  [@test] crates/app-api/src/windows_test.rs (`the_first_rows_within_the_budget_show_the_main_window_and_the_budget_then_does_nothing`)

### Requirement: Main window: Each stage of the page sets the deadline of the next

Each stage of the page MUST set the deadline of the next, which holds only until the page gets there: the build waits
3 s for the load (`sub_window_loaded`), the load 3 s for the shell, the shell the budget for the content.

#### Scenario: A page that loaded and draws no shell
- **WHEN** a page loaded and draws no shell
- **THEN** the window shows once the 3 s deadline of its shell passed
  [@test] crates/app-api/src/windows_test.rs (`a_page_that_loaded_and_draws_no_shell_shows_once_the_fallback_of_its_shell_passed`)

#### Scenario: A page that never loads
- **WHEN** a page never loads
- **THEN** the window shows once the 3 s deadline of its build passed
  [@test] crates/app-api/src/windows_test.rs (`a_page_that_never_loads_shows_once_the_fallback_of_its_build_passed`)

### Requirement: Main window: A page that loaded is never shown by the deadline of its load

A page that loaded MUST NOT be shown by the deadline of its load; a broken page MUST still show.

#### Scenario: The page loaded before the deadline of its load
- **WHEN** the page loaded, and the deadline of its load passes
- **THEN** that deadline does not show the window, nor that of its shell once the shell came; the budget of its
  content after the shell does
  [@test] crates/app-api/src/windows_test.rs (`a_page_that_loaded_is_never_shown_by_the_fallback_of_its_load`)

### Requirement: Main window: The loading table stands where the rows come

The loading table MUST stand where the rows come: the filter bar's room where the last sync had enough branches for
it, the column heads and as many rows as the last sync had (`lastBranchCount`), "Syncing branches…" after its delay;
the rows MUST take its room, nothing below moves.

#### Scenario: The rows replace the loading table
- **WHEN** the window shows the loading table before the first rows
- **THEN** it has the filter bar's room, the column heads and as many rows as the last sync had
- **AND** "Syncing branches…" shows after its delay
- **AND** the rows take its room, nothing below moves
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the main window on the first rows of its sync, the table loading where they come until then`)

### Requirement: Main window: The deadlines run on the backend's clock

The deadlines MUST run on the backend's clock (`wait_for_deadline`), never on a timer of the page, which WebKit
throttles while the window is hidden: a page that says nothing after its shell still shows.

#### Scenario: A page that says nothing after its shell
- **WHEN** a page says nothing after its shell
- **THEN** the backend shows the main window once its budget passed
  [@test] crates/app-api/src/windows_test.rs (`the_backend_shows_a_main_window_whose_page_says_nothing_after_its_shell_once_its_budget_passed`)

### Requirement: Main window: The first frame of a window is its page as it is then

The first frame of a window that shows MUST be what its page has in its DOM then, never the layers of an earlier
frame: a window due to show (its content settled, or a deadline passed) MUST show once its webview presented the page
as it is (`show_window`, `next_presentation`), at most 1 s (`PRESENTATION_BUDGET`).

#### Scenario: macOS waits for the presentation of the webview
- **GIVEN** macOS, where WebKit paints a hidden page and commits its layers to the app's process as it does for a
  visible one
- **WHEN** a window is due to show
- **THEN** it waits until `-[WKWebView _doAfterNextPresentationUpdate:]` answers that the layers of the next rendering
  update are there
- **AND** the app flushes them to the render server before it orders the window in
  [@test] src-tauri/src/presentation_test.rs (`a_window_due_to_show_waits_for_the_frame_its_webview_presents`)

#### Scenario: A webview that presents nothing
- **WHEN** a webview presents nothing within 1 s
- **THEN** its window shows as it is and the backend logs it
  [@test] src-tauri/src/presentation_test.rs (`a_webview_that_presents_nothing_shows_its_window_once_the_budget_passed`)

#### Scenario: A page that never loaded, and the other platforms
- **WHEN** a page never loaded, or the window is not on macOS
- **THEN** the window shows at once, with no earlier frame to show
  [@test] src-tauri/src/presentation_test.rs (`a_webview_that_tells_no_presentation_shows_its_window_at_once`)

### Requirement: Main window: The first rows wait for no commit of a branch they don't show

The first rows MUST wait for no commit of a branch they don't show: the listing of the sync MUST read refs only, never
the subjects or the dates of the archived and the remote branches under the prefix; the archive step MUST read the
subjects and the committer times of the local tips with their notes, after the rows, and keep the archived branches
newest tip first.

#### Scenario: The listing reads refs only
- **WHEN** a sync lists the branches under the prefix
- **THEN** it reads no commit of the branches it lists
  [@test] crates/sync-core/src/repo_facts_test.rs (`the_listing_reads_no_commit_of_the_branches_it_lists`)

#### Scenario: The archive step after the rows
- **WHEN** the archive step runs after the rows
- **THEN** it reads the subjects and the committer times of the local tips with their notes
- **AND** the archived branches come newest tip first
  [@test] crates/sync-core/src/detection_archive_step_test.rs (`the_archived_branches_come_newest_tip_first_with_their_subjects_from_the_log_of_their_notes`)

### Requirement: Main window: The rows of a sync wait for no timer of the page

While the window never showed, the rows of a sync MUST wait for no timer of the page, which WebKit throttles in a
hidden page to a second: the session merges its patches on its own 16 ms clock in Rust, and the page applies each as
its channel delivers it.

#### Scenario: A window not shown yet
- **GIVEN** a window not shown yet, whose timers WebKit throttles
- **WHEN** the channel delivers a patch of the session
- **THEN** the page applies it at once
  [@test] src/model/store.test.ts (`applies a patch as its channel delivers it, on no timer of the page: a window not shown yet, whose timers WebKit throttles, holds it at once`)

### Requirement: Main window: The page and the backend log the first frame and its cause

The page MUST log when its first frame with content came, from the build, with its load and its shell ("main open 812
ms: loaded 240, shell 300"), which is not when the window showed: the backend MUST log what showed it, the page or a
deadline, with how long the webview took to present the page (`presented_ms`).

#### Scenario: The page showed the window
- **WHEN** the page settled its content and the window shows
- **THEN** the page logs its first frame with content, as "main open 812 ms: loaded 240, shell 300"
- **AND** the backend logs "the page settled its content, showing the window" with `presented_ms`

#### Scenario: A deadline showed the window
- **WHEN** a deadline shows the window
- **THEN** the backend logs "the page settled no content within its budget, showing the window", or the warnings "the
  page loaded and drew no shell" and "the page of the window did not load"

### Requirement: Sub-windows: A sub-window is its row, its islands and its footer

A sub-window (`WindowPage`) MUST be its row, then its islands, then its footer (the position and the keys). The row
MUST say where the user is on the left and hold the view and the actions on the right; the native title MUST stay set
(`useWindowTitle`) for the Window menu, Mission Control and screen readers.

#### Scenario: The row of a review window beside the traffic lights
- **GIVEN** an overlay title bar
- **WHEN** a review window opens
- **THEN** its row starts after the traffic lights with where the user is, its empty parts dragging the window
- **AND** the view and the actions stand at the other end, on one 40px line
  [@test] tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`)

#### Scenario: The row of a review window in a custom title bar
- **GIVEN** a custom title bar
- **WHEN** a review window opens
- **THEN** its row ends with the window buttons, and Close closes the window
  [@test] tests/e2e/repository/title-bar.spec.ts (`should end the row of a review window with the window buttons of a custom title bar`)

### Requirement: Sub-windows: The row of `review` says its subject

The row of `review` MUST hold Back while it has a review it left, then what its subject, a branch, a list of commits or
a pull request, puts there, and the native title MUST name the subject.

#### Scenario: A review it left
- **WHEN** the review window has a review it left
- **THEN** its row starts with Back

#### Scenario: The review of a branch
- **WHEN** the review window shows a branch
- **THEN** its row holds the stack bar, Inline/Unified/Split and the actions on the branch
- **AND** the native title is "Review login", "Review login · a1b2c3d" when it opened at a commit
  ([Branch Review](../../branch/branch-review/spec.md))
  [@test] tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`)

#### Scenario: The review of a list of commits
- **WHEN** the review window shows a list of commits
- **THEN** its row says "Unassigned › 11346cdf", the list and the commit (the hash in `font-mono`), and holds
  Inline/Unified/Split ([Commit Review](../../commit/commit-review/spec.md),
  [Review Window](../../review/review-window/spec.md#one-window))

#### Scenario: The review of a pull request
- **WHEN** the review window shows a pull request
- **THEN** its row holds ↑ ↓ and "Your Turn › 2 of 5", GitHub, "⋯" and the decisions on the pull request, Approve last
- **AND** its Inline/Unified/Split stands at the end of its tabs, the header of the changes it switches
  ([Review Requests](../review-requests/spec.md#review-window))
  [@test] tests/e2e/repository/title-bar.spec.ts (`should hold the decisions of a pull request review on its one line, and its diff view at the end of the tabs`)

### Requirement: Sub-windows: The row of a conflict window stays above its islands

The row of `conflicting-files` and `missing-commits` MUST say where the user is, as "bug-fix › Conflicting files", with
the islands scrolling under the row.

#### Scenario: The islands scroll under the row
- **WHEN** the conflicting files of "bug-fix" open in a window and its islands scroll to their end
- **THEN** its row says "bug-fix › Conflicting files" and stays at the top
  [@test] tests/e2e/repository/title-bar.spec.ts (`should keep the row of a conflict window above the islands, which scroll under it`)

### Requirement: Sub-windows: The row of `settings` holds its title

The row of `settings` MUST say "Settings", with the sidebar of its pages and the page below it
([Settings](../settings/spec.md)).

#### Scenario: The Settings window
- **WHEN** the Settings window opens
- **THEN** its row says "Settings", and the sidebar of its pages and the page stand below it

### Requirement: Sub-windows: A sub-window is at least 800×500, so its row fits one line

A sub-window MUST be at least 800×500 (wider above a 125% zoom: 640 CSS px for the review, `sub_window_min_size`), so
its row fits one line.

#### Scenario: A zoom above 125%
- **WHEN** the zoom is above 125%
- **THEN** the minimum of a sub-window widens, so the review keeps 640 CSS px
  [@test] src-tauri/src/zoom_test.rs (`a_zoom_above_125_percent_widens_the_minimum_sub_window`)

### Requirement: Sub-windows: A narrow row shows the icons of its actions

An action with an icon MUST show only the icon in a row narrower than 48rem (`ActionButton`), a decision of the pull
request review its icon or its label without the key below 56rem (`MenuItemButton narrow`).

#### Scenario: A narrow row of a pull request review
- **WHEN** the review of a pull request is at the minimum size of a sub-window
- **THEN** its decisions drop their keys, and Request Changes… is its icon, named for the reader
  [@test] tests/e2e/repository/title-bar.spec.ts (`should hold the decisions of a pull request review on its one line, and its diff view at the end of the tabs`)

#### Scenario: An action with an icon in a narrow row
- **WHEN** a row is narrower than 48rem
- **THEN** an action with an icon shows only the icon

### Requirement: Sub-windows: Settings is at least 600×420

Settings, whose row holds only its title, MUST be at least 600×420 (wider above 107%: 560 CSS px for a page).

#### Scenario: A zoom above 107%
- **WHEN** the zoom is above 107%
- **THEN** the minimum of the Settings window widens, so a page keeps 560 CSS px
  [@test] src-tauri/src/zoom_test.rs (`the_settings_window_is_smaller_and_widens_above_107_percent`)

### Requirement: Sub-windows: One window per kind, reused by every open

There MUST be one window per kind, reused: an open of an open kind MUST retitle it, bring it to the front and hand its
page the new data.

#### Scenario: An open of a kind already open
- **WHEN** a kind is opened while its window is loaded, hidden or on screen
- **THEN** its page takes the new data: a hidden window shows for it, one on screen comes to the front at once
  [@test] crates/app-api/src/windows_test.rs (`a_loaded_hidden_page_takes_an_open_and_shows_for_it`)

### Requirement: Sub-windows: A new window shows on the content of the latest open

A new window MUST be built hidden, on the canvas colour of the theme (not on Linux, where a rounded window is
transparent), and show once its page settled the content of the latest open (`sub_window_ready`: a page on its render,
`WindowPage`; a review once its first diffs are prepared), so it never shows a blank frame nor the subject of an
earlier open; its first frame is that content, as the main window's ([Main window](#main-window)).

#### Scenario: A review shows once its first diffs are prepared
- **WHEN** a review window opens
- **THEN** it shows the files while the patch is read, and settles with the diffs
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the files while the patch is read, settle with the diffs, and step to a commit read beside it`)

#### Scenario: The content of an earlier open settles first
- **WHEN** the content of the build settles before the content of a later open
- **THEN** the window waits for the content of the latest open and shows on it, once
  [@test] crates/app-api/src/windows_test.rs (`an_open_while_the_page_loads_waits_for_the_page`)
  [@test] crates/app-api/src/windows_test.rs (`a_reopen_while_the_page_loads_shows_the_window_on_the_content_of_the_reopen`)

### Requirement: Sub-windows: A new window shows by its deadlines at the latest

A new window MUST show at most 300 ms after its page committed its shell, its row (`sub_window_drawn`), 3 s after the
load without a shell and 3 s after the build without a load.

#### Scenario: The budget counts from the shell
- **WHEN** the loaded page says its shell
- **THEN** its content has 300 ms from then
- **AND** a shell said before the load, a second title bar or a remount sets no budget
  [@test] crates/app-api/src/windows_test.rs (`a_shell_counts_once_per_load_and_only_after_it`)

#### Scenario: A page that never loads or draws no shell
- **WHEN** a page never loads, or loads and draws no shell
- **THEN** the window shows 3 s after its build, or 3 s after its load
  [@test] crates/app-api/src/windows_test.rs (`an_open_while_the_page_loads_waits_for_the_page`)

#### Scenario: The deadline of a closed window
- **WHEN** a window closes and is built again before the deadline of the closed one passes
- **THEN** that deadline leaves the new window hidden, and a deadline shows a window once
  [@test] crates/app-api/src/windows_test.rs (`a_deadline_shows_the_window_once_and_only_its_own_build`)

### Requirement: Sub-windows: No animation frame is waited for

The show of a new window MUST wait for no animation frame: a hidden window runs none.

#### Scenario: A hidden page settles its content
- **WHEN** a hidden page loads and settles its content
- **THEN** it says so without an animation frame, once per open
  [@test] src/lib/sub-window.test.ts (`says the page loaded and its content settled without an animation frame, once per open`)

### Requirement: Sub-windows: An open while the page loads is handed over at its load

An open while its page still loads MUST be handed over at its load, the latest one winning, with the changes of the
settings made meanwhile ([Settings](../settings/spec.md)).

#### Scenario: Opens while the page loads
- **WHEN** a kind is opened again, once or twice, while its page loads
- **THEN** the load hands over the latest open, once
  [@test] crates/app-api/src/windows_test.rs (`an_open_while_the_page_loads_waits_for_the_page`, `a_reopen_while_the_page_loads_shows_the_window_on_the_content_of_the_reopen`)

#### Scenario: A window built again after its close
- **WHEN** a window closed and is built again, and an open comes while its page loads
- **THEN** the open waits for the new page and is handed over at its load
  [@test] crates/app-api/src/windows_test.rs (`a_window_built_again_after_its_close_waits_for_its_page_again`)

#### Scenario: The settings changed while the page loaded
- **WHEN** the settings change while the page loads
- **THEN** the page takes those changes once it listens, before it says its shell and its content
  [@test] src/lib/sub-window.test.ts (`takes the settings changed while the page loaded once it listens, before it says its shell and its content`)

### Requirement: Sub-windows: The page logs the phases of each open

The page MUST log the phases of each open ("review open 96 ms: loaded 41, files 44, patch 47, engine 70, primed 88").

#### Scenario: The phases of an open
- **WHEN** an opener asked for an open and its page goes through its phases
- **THEN** the page logs the first time of each phase after the ask, once
  [@test] src/lib/open-timing.test.ts (`tells the phases of an open after its opener asked, the first time of each, once`)

### Requirement: Sub-windows: The opens are serialized

The opens MUST be serialized: two quick opens build one window.

#### Scenario: Two quick opens
- **WHEN** a kind is opened twice in quick succession
- **THEN** one window is built, and the later open wins

### Requirement: Sub-windows: A new window opens centred on the monitor of the main window

A new window MUST open centred on the monitor of the main window, then where it was (the window state plugin, without
the visibility); it MUST have a taskbar entry (Windows, Linux).

#### Scenario: The first and a later open of a window
- **WHEN** a window of a kind opens for the first time, and later again
- **THEN** it opens centred on the monitor of the main window, then where it was
- **AND** on Windows and Linux it has a taskbar entry

### Requirement: Sub-windows: Escape and ⌘W close a sub-window

Escape that nothing in the page handled and ⌘W MUST close a sub-window.

#### Scenario: Escape nothing handled
- **WHEN** the user presses Escape that nothing in the page handles, or ⌘W
- **THEN** the sub-window closes

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
  maximized, a test fires `tauri://resize`, `__e2eWindowCalls` records minimize, toggle_maximize and close. The window
  menu is a command of the app: the desktop of the test server records it (`getWindowMenus`). The first frame: the
  harness builds the window of each page on the test server before it loads (`registerWindowBuild`, the main window in
  `common-setup.ts`, a sub-window in `openLinkedSubWindow`), so its stages set the deadlines there as in the app, and
  records what the page had in its DOM when it said a stage (`observeWindowStages`: `sub_window_drawn`,
  `sub_window_ready`, on `page.route`).
- By hand: `pnpm install-app` on macOS (the traffic lights, dragging, double click, entering and leaving fullscreen, in
  the main window and in each sub-window); an Ubuntu VM with GNOME on Wayland and on Xorg (the frame, resizing from
  every edge, snapping, the layouts by `gsettings set org.gnome.desktop.wm.preferences button-layout`, the double-click
  setting, the window menu, the main menu and its keys, `BRANCH_DECK_NATIVE_TITLE_BAR=1`); a
  Windows build for the window buttons (a Windows on ARM VM works, see "Windows on ARM" in `DEVELOPMENT.md`; UI
  Automation finds the buttons by their `aria-label` and invokes them).

## Open Questions / Risks

- E2E runs in Chromium: it checks the layout and the calls, not the native frame. The traffic light position, the
  Windows frame and the GTK frame (shadow, resize, snapping, the window menu on Wayland) are checked by hand.
- The window-state plugin saves the size GTK reports with the frame extents of client-side decorations; a window that
  grows by its shadow on every start would need the extents subtracted.

## References

- [UI Theme](../../../../docs/design/ui-theme.md): the canvas and the islands.
- [Zoom](../zoom/spec.md): the page zoom, which leaves the toolbar alone.
- [Activity](../activity/spec.md), [Update Project](../update-project/spec.md), [Outgoing Commits](../outgoing/spec.md): the
  controls of the toolbar.
