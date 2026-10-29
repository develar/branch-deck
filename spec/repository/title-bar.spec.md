---
name: Title Bar
description: Requirements for the top row of every window as its title bar, as in the IntelliJ new UI: the toolbar of the main window and the row of each sub-window, beside the macOS traffic lights, with its own window buttons and main menu on Windows and Linux, the system title bar where a custom one doesn't fit or the user asks for it.
style: plain-1
targets:
  - ../../src-tauri/src/lib.rs
  - ../../src-tauri/src/title_bar.rs
  - ../../crates/app-api/src/app/windows.rs
  - ../../src-tauri/src/desktop.rs
  - ../../src-tauri/src/menu.rs
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
  - ../../src/components/repository/MainMenuButton.tsx
  - ../../src/lib/title-bar-mouse.ts
  - ../../src/lib/main-menu.ts
  - ../../src/lib/app-commands.ts
  - ../../src/hooks/useAppCommand.ts
  - ../../src/hooks/useMainMenuKeys.ts
  - ../../src/components/settings/AppearancePage.tsx
  - ../../src-tauri/capabilities/settings.json
  - ../../src/pages/IndexPage.tsx
  - ../../src/components/branch/BranchCreator.tsx
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../crates/sync-core/src/repo_facts.rs
  - ../../crates/branch-integration/src/common.rs
  - ../../tests/e2e/setup/tauri-mocks.ts
  - ../../src/model/store.ts
  - ../../crates/repository-model/src/runtime.rs
---

# Title Bar

Status: Implemented
Date: 2026-10-15, sub-windows and Linux 2026-10-17, the first frame of the main window 2026-10-24

## Summary

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

## Requirements

### Kind

- Every window is built the same way (`with_title_bar` in `src-tauri/src/title_bar.rs`: the main window and
  `open_sub_window`), and its initialization script tells the page before it loads how the title bar is drawn
  (`title_bar_script`, `window.__BD_TITLE_BAR__`, `titleBar` in `src/lib/title-bar.ts`), one JSON object decided once
  per process (`init_title_bar`, in `setup` before the menu and the first window):
  `{"kind":"overlay","doubleClick":…}` on macOS, `{"kind":"custom","controls":{"start":[…],"end":[…],"style":…},"doubleClick":…,"cornerRadius":…}`
  on Windows and Linux, `{"kind":"native","reason":…}` where the system title bar stays, each with
  `"checkForUpdates":…` (`PageTitleBar`): whether the build updates itself (the `auto-update` feature), whatever draws
  the title bar, for ☰ and the command palette (`updatesItself`). The page does not guess from the user agent.
  [@test] ../../src-tauri/src/title_bar_test.rs (`the_page_gets_one_json_object`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should keep the native title bar where the page doesn't draw one`)
- The system title bar stays (`native`), in this order of reasons: `BRANCH_DECK_NATIVE_TITLE_BAR=1` (`env`); on Linux
  WSL (`WSL_DISTRO_NAME` or `WSL_INTEROP`), no `XDG_CURRENT_DESKTOP` (`no-desktop`), a tiling window manager
  (IntelliJ's list plus niri, matched against each `:` part of `XDG_CURRENT_DESKTOP`; on Wayland too, unlike IntelliJ:
  an empty titlebar would take sway's title away); then the setting "Use the system title bar" (`setting`, Windows and
  Linux). The decision is logged with its reason.
  [@test] ../../src-tauri/src/title_bar_test.rs (`linux_keeps_the_system_title_bar_where_intellij_does_and_where_the_user_asked`, `windows_keeps_the_system_title_bar_only_when_asked`)

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

### Linux

- The window keeps its decorations, and gets an empty GTK titlebar before GTK realizes it (`prepare_title_bar`): GTK then
  draws client-side decorations without a title, as GNOME apps with a header bar have: the shadow, the resize edges in
  it, the tiled, maximized and fullscreen states and the frame extents snapping needs. Every window is built hidden,
  the window-state plugin doesn't show it (no `StateFlags::VISIBLE`), and the main thread sets the titlebar before the
  window shows (`prepare_title_bar`), then the window shows once its page settled its content (`sub_window_ready`,
  [Main window](#main-window)).
  [@test] ../../src-tauri/src/title_bar_test.rs (`the_window_state_restores_neither_the_decorations_nor_the_visibility`)
- The window buttons are those of GTK's `gtk-decoration-layout` (GNOME's `button-layout` through XSETTINGS or the
  portal, KDE's through kde-gtk-config), the start before `:` and the end after it, by GTK's rules: `icon`, `menu`,
  `appmenu`, `spacer` and unknown names dropped, a layout with a button twice refused; a missing or refused one is
  `:minimize,maximize,close`. GNOME's default `appmenu:close` gives Close alone.
  [@test] ../../src-tauri/src/title_bar_test.rs (`reads_the_buttons_of_a_gtk_decoration_layout_by_its_rules`, `a_missing_or_refused_layout_is_the_default_of_gtk`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should put the window buttons where the desktop's layout does, in its look`, `should split the buttons of a split layout, the start ones before the main menu`)
- A press on the empty parts drags the window only once the mouse moved 4px, as GTK's header bar does
  (`installTitleBarMouse`): Tauri's drag script asks the window manager to move the window on every press, and the
  grab of the move took the second click of a double click.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should drag only once the mouse moved, so a double click reaches the page and maximizes`)
- The window is rounded as the desktop's windows are (libadwaita's 12px on GNOME, 8px on KDE, `corner_radius`), with a
  compositor: the window is transparent (RGBA), GTK keeps drawing its decorations with the same radius (the shadow, the
  resize edges in it, the frame extents; `round_corners` gives them the radius and no window background), and the page
  clips its canvas to the radius (`data-window-rounded`; GTK3 doesn't clip the webview). Square while maximized,
  fullscreen or tiled (GTK's window state, `window-tiled`), and without a compositor. This is what Tauri apps on Linux do
  by hand ([dbx](https://github.com/t8y2/dbx/pull/11142), [Hatoba](https://github.com/scarletkc/Hatoba/pull/107):
  undecorated, their own shadow and resize handles); keeping GTK's decorations leaves the shadow, resizing and snapping
  to GTK.
  [@test] ../../src-tauri/src/title_bar_test.rs (`rounds_the_window_as_the_desktop_does`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should round the window as the desktop does, square while maximized or tiled`, `should not round a window where the system does it or can't`)
- They look as IntelliJ draws them (its icons, `platform/icons/src/linux/theme`): GNOME's (Adwaita) for an icon theme
  starting with `adwaita` or `yaru`, KDE's (Breeze) for `breeze`, else KDE's on a KDE desktop and GNOME's elsewhere.
  [@test] ../../src-tauri/src/title_bar_test.rs (`picks_the_look_of_the_buttons_by_the_icon_theme_then_the_desktop`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should draw KDE's buttons, dim them in an inactive window, and leave fullscreen with Restore`)

### Windows and Linux

- The main menu replaces the native menu bar: `configure_app_menu` sets no menu for a custom title bar (no menu bar
  above the title bar, no accelerator table), the page's main menu holds its items (`mainMenus`) and the page handles
  their keys ([Main menu](#main-menu)). A window with the system title bar keeps the native menu bar, with Check for
  Updates… in its Help menu.
- The buttons are not in the tab order, as native window buttons aren't; each has an `aria-label` and a tooltip.
  Maximize is Restore while the window is maximized or fullscreen, and leaves fullscreen there. The buttons of an
  inactive window dim (`data-window-inactive`, the focus of the window).
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should draw KDE's buttons, dim them in an inactive window, and leave fullscreen with Restore`)
- A right click on the empty parts of the row opens the window manager's window menu, as a native title bar does
  (`show_window_menu` of app-api, on the main thread, through `MenuHost` of `src-tauri/src/menu.rs`: the system menu on
  Windows with its items enabled by the state of the window, Restore while
  maximized or minimized, Move, Size and Maximize while not, as `DefWindowProc` would; GDK's `show_window_menu` on Linux
  with the right button press of the webview of the last two seconds, whose serial Wayland checks, forgotten with its
  window); a control keeps its own right click.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should do what the system's double click does, and open the window menu on a right click of the empty row`)

### Main menu

- ☰ ("Main Menu", Alt+\ in its tooltip) is the first control of the main window's row, after the start buttons, as
  IntelliJ's `MainMenuWithButton`. A click expands the menu bar File, View, Help in place, over the start of the row,
  which hides meanwhile, with File open; ←/→ go through the menus; Esc, a click outside or a chosen item collapse it
  back to ☰ (IntelliJ's `ExpandableMenu`). Sub-windows have none.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should expand the menu bar in place of ☰, go through the menus, and collapse on Escape`)
- The items are those of the native menu (`src/lib/main-menu.ts`): File: Sync Branches (Ctrl+R, disabled with the
  reason of the Sync button), Update Project (Ctrl+T), Auto-sync on Focus (checked), Settings… (Ctrl+,); View: Command
  Palette… (Ctrl+K, [Command Palette](command-palette.spec.md)), Branches (Ctrl+1), Reviews (Ctrl+2) and Landing (Ctrl+3)
  ([Main Views](main-views.spec.md)), Actual Size (Ctrl+0), Zoom In (Ctrl+=), Zoom Out
  (Ctrl+-), Activity ([Activity](activity.spec.md)); Help: Getting Started, Install Agent CLI…, Check for
  Updates… (a build with the `auto-update` feature, as the native menu: `checkForUpdates` of the title bar), GitHub
  Repository. No Edit menu: the webview's Ctrl+C/V/X/A stay. Each item runs the
  command the native item runs (`runAppCommand`, registered by the window that runs it with `useAppCommand`). The
  native menu sends a command to the main window as an event named as it (`sync_branches`, `update_project`,
  `check_for_updates`, `install_agent_cli`, `zoom` with its step, `open_settings`): the main window opens Settings
  ([Settings](settings.spec.md)), as it has the current settings to start it from.
- Check for Updates… says that the app is up to date, with its version, or offers the update the check downloaded
  (Install and Restart, a toast that stays until the user acts); a failed check or install says why ("Cannot check for
  updates", "Cannot install the update"). The main window also checks silently a few seconds after its start.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should say the app is up to date, offer the downloaded update, and say why its install failed`)
- Alt+\ and F10 open File, Alt+F, Alt+V and Alt+H their menu (the physical keys), the mnemonics underlined; on Windows a
  lone Alt tap highlights File, unless a key, a click or leaving the window came between Alt down and up (IntelliJ's
  `WinAltKeyProcessor`); a held Alt that repeats is still a tap on its release.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should open from the keyboard and run the items`, `should highlight File on a lone Alt tap on Windows, not after another key`)
- The page handles the keys of the native menu where it is gone, in every window with a custom title bar
  (`useMainMenuKeys`, the physical key, as the native accelerators go by the virtual key: a Cyrillic layout's Ctrl+R
  syncs too): Ctrl+R and Ctrl+T run in the main window
  (a sub-window sends them there), Ctrl+, opens Settings and Ctrl+K the command palette, also through the main window,
  the zoom keys zoom all windows one step. They are `preventDefault`ed (WebView2 would reload on Ctrl+R). A window with a native menu leaves them to it.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should take the keys of the native menu: Ctrl+R syncs, Ctrl+, opens the settings`, `should leave the keys to the native menu where the window has one`, `should send Sync and Settings from a sub-window to the main window`)
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should open from Ctrl+K and from View → Command Palette… of ☰, and run Update Project and Check for Updates…`)
  [@test] ../../tests/e2e/repository/zoom.spec.ts (`should step once per press with the keys the page handles in a custom title bar`)

### Setting

- Settings → Appearance → Title bar (Windows, Linux): "Use the system title bar" (`appSettings.systemTitleBar`, off), the system title
  bar and menu bar instead of the page's (IntelliJ's "Merge main menu with window title", inverted). The backend reads
  it from `settings.json` at startup, so a change says "Applies after a restart" with Restart (`restart_app`, which
  writes the setting to `settings.json` itself before it restarts: the main window saves the settings debounced). Where the
  system title bar is fixed (WSL, a tiling window manager, an unknown desktop, the environment variable) the switch is
  on, disabled, and says why.
  [@test] ../../src-tauri/src/title_bar_test.rs (`reads_the_stored_setting`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should offer the system title bar in Settings → Appearance, applied after a restart`, `should show a fixed system title bar in the settings with its reason`)

### Every platform

- After the repository the switcher of the views, Branches · Reviews · Landing ([Main Views](main-views.spec.md)): where the
  user is, as the left side of the row says; its names give way to the icons on a narrow window.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)
- The empty parts of the row drag the window (`data-tauri-drag-region="deep"`); a double click does what the system's
  title bar does there (macOS `AppleActionOnDoubleClick`, GTK's `gtk-titlebar-double-click`: zoom or maximize,
  minimize, nothing; `installTitleBarMouse` takes the second click before Tauri's drag script, which always maximizes).
  The controls in it keep their clicks.
  [@test] ../../src-tauri/src/title_bar_test.rs (`maps_the_double_click_setting_of_the_system`)
- Every window has the permissions of the title bar (one capability, `title-bar.json`: the window state, dragging, the
  double click, the window buttons, the title, the minimum size); a test checks that every sub-window label has it.
  [@test] ../../src-tauri/src/title_bar_test.rs (`every_window_gets_the_permissions_of_the_title_bar`)
- The window-state plugin restores sizes and positions, never the decorations (a saved `decorated` would bring the
  native caption back on Windows) nor the visibility (a window shows once its title bar is set and its page settled
  its content, `show_when_settled`).
- The toolbar keeps its size at any page zoom, as window chrome does ([Zoom](zoom.spec.md)).
  [@test] ../../tests/e2e/repository/zoom.spec.ts (`should zoom the islands but not the toolbar, the title bar of the window`)
- Every control stays on the one 40px line, apart from the others, at every width of the window down to its 960px
  minimum, beside the traffic lights, between ☰ and the window buttons, or under the system title bar. The
  repository, the switcher of the views and the count of the branches share the room the rest of the row leaves them
  (a container of its own, `ConfigurationHeader`) and give way in this order: the repository selector down to its room
  beside the names, then the names of the views and the count together (below `@md`, 28rem, of that room), then the
  selector down to its minimum; the actions, the time of the last sync among them, keep their place. So ☰ and the
  window buttons of a custom title bar take their room first. Without it GNOME's 1077px maximized window put the names
  and the count over the repository selector and Update: from 1032 to 1080px, and to 1152px once the time of the last
  sync kept the room of its widest text and Activity stayed in an empty log.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should start the toolbar after the traffic lights of an overlay title bar, and not in fullscreen`)

### Windows

- The window has no decorations; `shadow(true)` gives it the 1px border and, on Windows 11, the rounded corners.
- The toolbar ends with Minimize, Maximize (Restore while maximized) and Close, flush with the right edge: 46×40 as
  the native caption buttons, the Segoe Fluent Icons glyphs, `bg-muted` on hover and `bg-destructive` for Close.
  Each has an `aria-label` and a tooltip. A failed call is a "Cannot minimize the window" error.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should end a custom title bar with the window buttons`)

### Linux

- The window keeps its decorations, and gets an empty GTK titlebar before GTK realizes it (`prepare_title_bar`): GTK then
  draws client-side decorations without a title, as GNOME apps with a header bar have: the shadow, the resize edges in
  it, the tiled, maximized and fullscreen states and the frame extents snapping needs. Every window is built hidden,
  the window-state plugin doesn't show it (no `StateFlags::VISIBLE`), and the main thread sets the titlebar before the
  window shows (`prepare_title_bar`), then the window shows once its page settled its content (`sub_window_ready`,
  [Main window](#main-window)).
  [@test] ../../src-tauri/src/title_bar_test.rs (`the_window_state_restores_neither_the_decorations_nor_the_visibility`)
- The window buttons are those of GTK's `gtk-decoration-layout` (GNOME's `button-layout` through XSETTINGS or the
  portal, KDE's through kde-gtk-config), the start before `:` and the end after it, by GTK's rules: `icon`, `menu`,
  `appmenu`, `spacer` and unknown names dropped, a layout with a button twice refused; a missing or refused one is
  `:minimize,maximize,close`. GNOME's default `appmenu:close` gives Close alone.
  [@test] ../../src-tauri/src/title_bar_test.rs (`reads_the_buttons_of_a_gtk_decoration_layout_by_its_rules`, `a_missing_or_refused_layout_is_the_default_of_gtk`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should put the window buttons where the desktop's layout does, in its look`, `should split the buttons of a split layout, the start ones before the main menu`)
- A press on the empty parts drags the window only once the mouse moved 4px, as GTK's header bar does
  (`installTitleBarMouse`): Tauri's drag script asks the window manager to move the window on every press, and the
  grab of the move took the second click of a double click.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should drag only once the mouse moved, so a double click reaches the page and maximizes`)
- The window is rounded as the desktop's windows are (libadwaita's 12px on GNOME, 8px on KDE, `corner_radius`), with a
  compositor: the window is transparent (RGBA), GTK keeps drawing its decorations with the same radius (the shadow, the
  resize edges in it, the frame extents; `round_corners` gives them the radius and no window background), and the page
  clips its canvas to the radius (`data-window-rounded`; GTK3 doesn't clip the webview). Square while maximized,
  fullscreen or tiled (GTK's window state, `window-tiled`), and without a compositor. This is what Tauri apps on Linux do
  by hand ([dbx](https://github.com/t8y2/dbx/pull/11142), [Hatoba](https://github.com/scarletkc/Hatoba/pull/107):
  undecorated, their own shadow and resize handles); keeping GTK's decorations leaves the shadow, resizing and snapping
  to GTK.
  [@test] ../../src-tauri/src/title_bar_test.rs (`rounds_the_window_as_the_desktop_does`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should round the window as the desktop does, square while maximized or tiled`, `should not round a window where the system does it or can't`)
- They look as IntelliJ draws them (its icons, `platform/icons/src/linux/theme`): GNOME's (Adwaita) for an icon theme
  starting with `adwaita` or `yaru`, KDE's (Breeze) for `breeze`, else KDE's on a KDE desktop and GNOME's elsewhere.
  [@test] ../../src-tauri/src/title_bar_test.rs (`picks_the_look_of_the_buttons_by_the_icon_theme_then_the_desktop`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should draw KDE's buttons, dim them in an inactive window, and leave fullscreen with Restore`)

### Windows and Linux

- The main menu replaces the native menu bar: `configure_app_menu` sets no menu for a custom title bar (no menu bar
  above the title bar, no accelerator table), the page's main menu holds its items (`mainMenus`) and the page handles
  their keys ([Main menu](#main-menu)). A window with the system title bar keeps the native menu bar, with Check for
  Updates… in its Help menu.
- The buttons are not in the tab order, as native window buttons aren't; each has an `aria-label` and a tooltip.
  Maximize is Restore while the window is maximized or fullscreen, and leaves fullscreen there. The buttons of an
  inactive window dim (`data-window-inactive`, the focus of the window).
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should draw KDE's buttons, dim them in an inactive window, and leave fullscreen with Restore`)
- A right click on the empty parts of the row opens the window manager's window menu, as a native title bar does
  (`show_window_menu` of app-api, on the main thread, through `MenuHost` of `src-tauri/src/menu.rs`: the system menu on
  Windows with its items enabled by the state of the window, Restore while
  maximized or minimized, Move, Size and Maximize while not, as `DefWindowProc` would; GDK's `show_window_menu` on Linux
  with the right button press of the webview of the last two seconds, whose serial Wayland checks, forgotten with its
  window); a control keeps its own right click.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should do what the system's double click does, and open the window menu on a right click of the empty row`)

### Main menu

- ☰ ("Main Menu", Alt+\ in its tooltip) is the first control of the main window's row, after the start buttons, as
  IntelliJ's `MainMenuWithButton`. A click expands the menu bar File, View, Help in place, over the start of the row,
  which hides meanwhile, with File open; ←/→ go through the menus; Esc, a click outside or a chosen item collapse it
  back to ☰ (IntelliJ's `ExpandableMenu`). Sub-windows have none.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should expand the menu bar in place of ☰, go through the menus, and collapse on Escape`)
- The items are those of the native menu (`src/lib/main-menu.ts`): File: Sync Branches (Ctrl+R, disabled with the
  reason of the Sync button), Update Project (Ctrl+T), Auto-sync on Focus (checked), Settings… (Ctrl+,); View: Command
  Palette… (Ctrl+K, [Command Palette](command-palette.spec.md)), Branches (Ctrl+1), Reviews (Ctrl+2) and Landing (Ctrl+3)
  ([Main Views](main-views.spec.md)), Actual Size (Ctrl+0), Zoom In (Ctrl+=), Zoom Out
  (Ctrl+-), Activity ([Activity](activity.spec.md)); Help: Getting Started, Install Agent CLI…, Check for
  Updates… (a build with the `auto-update` feature, as the native menu: `checkForUpdates` of the title bar), GitHub
  Repository. No Edit menu: the webview's Ctrl+C/V/X/A stay. Each item runs the
  command the native item runs (`runAppCommand`, registered by the window that runs it with `useAppCommand`). The
  native menu sends a command to the main window as an event named as it (`sync_branches`, `update_project`,
  `check_for_updates`, `install_agent_cli`, `zoom` with its step, `open_settings`): the main window opens Settings
  ([Settings](settings.spec.md)), as it has the current settings to start it from.
- Check for Updates… says that the app is up to date, with its version, or offers the update the check downloaded
  (Install and Restart, a toast that stays until the user acts); a failed check or install says why ("Cannot check for
  updates", "Cannot install the update"). The main window also checks silently a few seconds after its start.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should say the app is up to date, offer the downloaded update, and say why its install failed`)
- Alt+\ and F10 open File, Alt+F, Alt+V and Alt+H their menu (the physical keys), the mnemonics underlined; on Windows a
  lone Alt tap highlights File, unless a key, a click or leaving the window came between Alt down and up (IntelliJ's
  `WinAltKeyProcessor`); a held Alt that repeats is still a tap on its release.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should open from the keyboard and run the items`, `should highlight File on a lone Alt tap on Windows, not after another key`)
- The page handles the keys of the native menu where it is gone, in every window with a custom title bar
  (`useMainMenuKeys`, the physical key, as the native accelerators go by the virtual key: a Cyrillic layout's Ctrl+R
  syncs too): Ctrl+R and Ctrl+T run in the main window
  (a sub-window sends them there), Ctrl+, opens Settings and Ctrl+K the command palette, also through the main window,
  the zoom keys zoom all windows one step. They are `preventDefault`ed (WebView2 would reload on Ctrl+R). A window with a native menu leaves them to it.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should take the keys of the native menu: Ctrl+R syncs, Ctrl+, opens the settings`, `should leave the keys to the native menu where the window has one`, `should send Sync and Settings from a sub-window to the main window`)
  [@test] ../../tests/e2e/command-palette/command-palette.spec.ts (`should open from Ctrl+K and from View → Command Palette… of ☰, and run Update Project and Check for Updates…`)
  [@test] ../../tests/e2e/repository/zoom.spec.ts (`should step once per press with the keys the page handles in a custom title bar`)

### Setting

- Settings → Appearance → Title bar (Windows, Linux): "Use the system title bar" (`appSettings.systemTitleBar`, off), the system title
  bar and menu bar instead of the page's (IntelliJ's "Merge main menu with window title", inverted). The backend reads
  it from `settings.json` at startup, so a change says "Applies after a restart" with Restart (`restart_app`, which
  writes the setting to `settings.json` itself before it restarts: the main window saves the settings debounced). Where the
  system title bar is fixed (WSL, a tiling window manager, an unknown desktop, the environment variable) the switch is
  on, disabled, and says why.
  [@test] ../../src-tauri/src/title_bar_test.rs (`reads_the_stored_setting`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should offer the system title bar in Settings → Appearance, applied after a restart`, `should show a fixed system title bar in the settings with its reason`)

### Every platform

- After the repository the switcher of the views, Branches · Reviews · Landing ([Main Views](main-views.spec.md)): where the
  user is, as the left side of the row says; its names give way to the icons on a narrow window.
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)
- The empty parts of the row drag the window (`data-tauri-drag-region="deep"`); a double click does what the system's
  title bar does there (macOS `AppleActionOnDoubleClick`, GTK's `gtk-titlebar-double-click`: zoom or maximize,
  minimize, nothing; `installTitleBarMouse` takes the second click before Tauri's drag script, which always maximizes).
  The controls in it keep their clicks.
  [@test] ../../src-tauri/src/title_bar_test.rs (`maps_the_double_click_setting_of_the_system`)
- Every window has the permissions of the title bar (one capability, `title-bar.json`: the window state, dragging, the
  double click, the window buttons, the title, the minimum size); a test checks that every sub-window label has it.
  [@test] ../../src-tauri/src/title_bar_test.rs (`every_window_gets_the_permissions_of_the_title_bar`)
- The window-state plugin restores sizes and positions, never the decorations (a saved `decorated` would bring the
  native caption back on Windows) nor the visibility (a window shows once its title bar is set and its page settled
  its content, `show_when_settled`).
- The toolbar keeps its size at any page zoom, as window chrome does ([Zoom](zoom.spec.md)).
  [@test] ../../tests/e2e/repository/zoom.spec.ts (`should zoom the islands but not the toolbar, the title bar of the window`)
- Every control stays on the one 40px line, apart from the others, at every width of the window down to its 960px
  minimum, beside the traffic lights, between ☰ and the window buttons, or under the system title bar. The repository, the switcher of the views and
  the count of the branches share the room the rest of the row leaves them (a container of its own,
  `ConfigurationHeader`): the names of the views and the count show where that room is `@md` (28rem) or more, so the
  window buttons and ☰ of a custom title bar take their room first. Without it GNOME's 1077px maximized window put the
  names and the count over the repository selector and Update.
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should start the toolbar after the traffic lights of an overlay title bar, and not in fullscreen`)
  [@test] ../../tests/e2e/repository/header.spec.ts (`should keep its controls apart on one line at the minimum window width`)
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should keep the toolbar beside the traffic lights on one line at every width, the names of the views where the repository keeps its room`, `should keep the toolbar between the main menu and the window buttons on one line at every width, the names of the views where the repository keeps its room`, `should keep the toolbar between the main menu and the window buttons of Windows on one line at every width, the names of the views where the repository keeps its room`, `should keep the toolbar under the system title bar on one line at every width, the names of the views where the repository keeps its room`)

### Main window

- The main window is built hidden and shows on its first frame with content, the way a sub-window does (one
  `SubWindowState`, `show_when_settled`, `sub_window_ready`), never as a bare canvas nor as a title row over an empty
  body. Its first frame is the title row with the first rows of the window's sync (`branchesGrouped`), or with a
  state that has no rows: no repository, Welcome, Ready to Sync, an error, a sync that ended without rows (none found,
  or deferred by git) (`BranchCreator`, `markWindowContentReady`). Its open is its build (`__SUB_WINDOW_OPEN__` of its init script); its
  load (`sub_window_loaded`) hands it no settings, it asks for them itself (`get_settings_changes`).
  [@test] ../../tests/e2e/repository/first-frame.spec.ts (`should show the main window on the first rows of its sync, the table loading where they come until then`)
  [@test] ../../tests/e2e/repository/first-frame.spec.ts (`should show the main window without rows where the window has none to wait for`)
  [@test] ../../tests/e2e/repository/first-frame.spec.ts (`should show the main window on Ready to Sync, which waits for the user, not for a sync`)
  [@test] ../../tests/e2e/repository/first-frame.spec.ts (`should show the main window when git defers its first sync, without a table that loads for ever`)
  [@test] ../../crates/app-api/src/windows_test.rs (`the_main_window_waits_for_its_first_rows_longer_and_takes_no_settings_with_its_load`)
- It shows at most 1 s after its page committed its shell (`MAIN_CONTENT_BUDGET`; the rows of the first sync of the
  IntelliJ monorepo came about 0.6 s after the page loaded): the title row and the loading table, which the first
  render of the title row says (`markWindowShellDrawn`, `sub_window_drawn`), so a slow sync shows them, never a bare
  canvas, also when the page took long from its load to its first render. Each stage of the page sets the deadline of
  the next, which holds only until the page gets there: the build waits 3 s for the load (`sub_window_loaded`), the
  load 3 s for the shell, the shell the budget for the content. A page that loaded is never shown by the deadline of
  its load (on the monorepo at load 90 a page loaded 2.26 s after the build, and that deadline showed the window 0.74 s
  later with no content and the warning that the page did not load); a broken page still shows. The loading table
  stands where the rows come: the filter bar's room where the last sync had enough branches for it, the column heads
  and as many rows as the last sync had (`lastBranchCount`), "Syncing branches…" after its delay; the rows take its
  room, nothing below moves. The deadlines run on the backend's clock (`wait_for_deadline`), never on a timer of the
  page, which WebKit throttles while the window is hidden: a page that says nothing after its shell still shows.
  [@test] ../../crates/app-api/src/windows_test.rs (`a_page_that_loaded_is_never_shown_by_the_fallback_of_its_load`, `the_budget_of_the_content_counts_from_the_shell_not_from_the_load`)
  [@test] ../../crates/app-api/src/windows_test.rs (`a_page_that_loaded_and_draws_no_shell_shows_once_the_fallback_of_its_shell_passed`, `a_page_that_never_loads_shows_once_the_fallback_of_its_build_passed`)
  [@test] ../../crates/app-api/src/windows_test.rs (`the_backend_shows_a_main_window_whose_page_says_nothing_after_its_shell_once_its_budget_passed`)
  [@test] ../../crates/app-api/src/windows_test.rs (`the_first_rows_within_the_budget_show_the_main_window_and_the_budget_then_does_nothing`)
  [@test] ../../tests/e2e/repository/first-frame.spec.ts (`should show the main window on the first rows of its sync, the table loading where they come until then`)
- The first frame of a window that shows is what its page has in its DOM then, never the layers of an earlier frame.
  A page says its content settled from an effect, right after React committed it and before WebKit painted it; a
  window shown then showed the frame before (the shell, or the bare canvas when no frame had the shell) until the next
  rendering update reached the screen, 25 to 80 ms in the check copy of the release app and longer on a loaded
  machine. So a window due to show (its content settled, or a deadline passed) shows once its webview presented the
  page as it is (`show_window`, `next_presentation`): on macOS WebKit paints a hidden page and commits its layers to
  the app's process as it does for a visible one, `-[WKWebView _doAfterNextPresentationUpdate:]` answers once the
  layers of the next rendering update are there, and the app flushes them to the render server before it orders the
  window in. At most 1 s (`PRESENTATION_BUDGET`): a webview that presents nothing shows its window as it is and the
  backend logs it; a page that never loaded shows at once. Elsewhere the window shows at once, with no earlier frame
  to show: WebKitGTK paints nothing while its view is not visible, so a Linux window is transparent until WebKit's
  first frame of the page as it is; WebView2 paints a hidden page, and Windows fades the window in from that frame.
  [@test] ../../src-tauri/src/presentation_test.rs (`a_window_due_to_show_waits_for_the_frame_its_webview_presents`, `a_webview_that_presents_nothing_shows_its_window_once_the_budget_passed`, `a_webview_that_tells_no_presentation_shows_its_window_at_once`)
- The first rows wait for no commit of a branch they don't show: the listing of the sync reads refs only, never the
  subjects or the dates of the archived and the remote branches under the prefix (on the monorepo 452 refs, 1 to 2 s
  from a cold pack before the rows); the archive step reads the subjects and the committer times of the local tips
  with their notes, after the rows, and keeps the archived branches newest tip first.
  [@test] ../../crates/sync-core/src/repo_facts_test.rs (`the_listing_reads_no_commit_of_the_branches_it_lists`)
  [@test] ../../crates/sync-core/src/detection_archive_step_test.rs (`the_archived_branches_come_newest_tip_first_with_their_subjects_from_the_log_of_their_notes`)
- While the window never showed, the rows of a sync wait for no timer of the page, which WebKit throttles in a hidden
  page to a second: the session merges its patches on its own 16 ms clock in Rust, and the page applies each as its
  channel delivers it.
  [@test] ../../src/model/store.test.ts (`applies a patch as its channel delivers it, on no timer of the page: a window not shown yet, whose timers WebKit throttles, holds it at once`)
- The page logs when its first frame with content came, from the build, with its load and its shell ("main open 812
  ms: loaded 240, shell 300"), which is not when the window showed: the backend logs what showed it, the page ("the
  page settled its content, showing the window") or a deadline ("the page settled no content within its budget,
  showing the window"; the warnings "the page loaded and drew no shell" and "the page of the window did not load"),
  with how long the webview took to present the page (`presented_ms`).

### Sub-windows

- A sub-window (`WindowPage`) is its row, then its islands, then its footer (the position and the keys). The row says
  where the user is on the left and holds the view and the actions on the right; the native title stays set
  (`useWindowTitle`) for the Window menu, Mission Control and screen readers.
  - `branch-review`: the stack bar; Split/Unified/Inline and the actions on the branch; the native title "Review login", "Review
    login · a1b2c3d" when it opened at a commit ([Branch Review](../branch/branch-review.spec.md)).
  - `commit-review`: "Unassigned › 11346cdf", the list and the commit (the hash in `font-mono`), of a commit no branch
    review shows; Split/Unified/Inline ([Commit Review](../commit/commit-review.spec.md)).
  - `pull-request-review`: ↑ ↓ and "Your Turn › 2 of 5"; GitHub, "⋯" and the decisions on the pull request, Approve
    last; its Split/Unified/Inline is at the end of its tabs, the header of the changes it switches
    ([Review Requests](review-requests.spec.md#review-window)).
  - `conflicting-files`, `missing-commits`: "bug-fix › Conflicting files", the islands scrolling under the row.
  - `settings`: "Settings", the sidebar of its pages and the page ([Settings](settings.spec.md)).
  [@test] ../../tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`, `should end the row of a review window with the window buttons of a custom title bar`, `should keep the row of a conflict window above the islands, which scroll under it`, `should hold the decisions of a pull request review on its one line, and its diff view at the end of the tabs`)
- A sub-window is at least 800×500 (wider above a 125% zoom: 640 CSS px for the review, `sub_window_min_size`), so
  its row fits one line; an action with an icon shows only the icon in a row narrower than 48rem (`ActionButton`), a
  decision of the pull request review its icon or its label without the key below 56rem (`MenuItemButton narrow`).
  Settings, whose row holds only its title, is at least 600×420 (wider above 107%: 560 CSS px for a page).
  [@test] ../../src-tauri/src/zoom_test.rs (`a_zoom_above_125_percent_widens_the_minimum_sub_window`, `the_settings_window_is_smaller_and_widens_above_107_percent`)
- One window per kind, reused: an open of an open kind retitles it, brings it to the front and hands its page the new
  data. A new window is built hidden, on the canvas colour of the theme (not on Linux, where a rounded window is
  transparent), and shows once its page settled the content of
  the latest open (`sub_window_ready`: a page on its render, `WindowPage`; a review once its first diffs are
  prepared), at most 300 ms after its page committed its shell, its row (`sub_window_drawn`), 3 s after the load
  without a shell and 3 s after the build without a load, so it never shows a blank frame nor the subject of an
  earlier open; its first frame is that content, as the main window's ([Main window](#main-window)); no animation
  frame is waited for, a hidden window runs none. An open while its page still loads is handed over at its load, the latest one winning, with the changes of the
  settings made meanwhile ([Settings](settings.spec.md)). The page logs the
  phases of each open ("commit-review open 96 ms: loaded 41, files 44, patch 47, engine 70, primed 88"). The opens are serialized: two quick opens build one window. A new window opens centred on the
  monitor of the main window, then where it was (the window state plugin, without the visibility); it has a taskbar
  entry (Windows, Linux). Escape that nothing in the page handled and ⌘W close it.
  [@test] ../../crates/app-api/src/windows_test.rs (`an_open_while_the_page_loads_waits_for_the_page`, `a_loaded_hidden_page_takes_an_open_and_shows_for_it`, `a_deadline_shows_the_window_once_and_only_its_own_build`, `a_window_built_again_after_its_close_waits_for_its_page_again`)
  [@test] ../../crates/app-api/src/windows_test.rs (`a_shell_counts_once_per_load_and_only_after_it`, `a_reopen_while_the_page_loads_shows_the_window_on_the_content_of_the_reopen`)
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should show the files while the patch is read, settle with the diffs, and step to a commit read beside it`)
  [@test] ../../src/lib/sub-window.test.ts (`says the page loaded and its content settled without an animation frame, once per open`, `takes the settings changed while the page loaded once it listens, before it says its shell and its content`)
  [@test] ../../src/lib/open-timing.test.ts (`tells the phases of an open after its opener asked, the first time of each, once`)

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

- [UI Theme](../../docs/design/ui-theme.md): the canvas and the islands.
- [Zoom](zoom.spec.md): the page zoom, which leaves the toolbar alone.
- [Activity](activity.spec.md), [Update Project](update-project.spec.md), [Outgoing Commits](outgoing.spec.md): the
  controls of the toolbar.
