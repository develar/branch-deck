/**
 * How every window draws its title bar, decided once per process by the backend (`src-tauri/src/title_bar.rs`) and set
 * before the page loads; the top row of the page (`WindowTitleBar`) is the title bar unless it is `native`:
 * - `overlay` on macOS: beside the traffic lights;
 * - `custom` on Windows and Linux: the row draws the window buttons, in the order and on the sides of the desktop
 *   (`controls`), and the main menu (☰) replaces the native menu bar;
 * - `native`: the system title bar above the row, on Linux where a custom one doesn't fit (WSL, a tiling window
 *   manager, an unknown desktop) and where the user asked for it (`reason`); in E2E without one.
 *
 * With every kind the backend says whether the build updates itself (`checkForUpdates`, `PageTitleBar`), whether
 * the host has a folder dialog (`folderDialog`, `DialogHost::has_folder_dialog`), whether the host shows each view of
 * the main window in a tab of its own (`viewTabs`, `brd mcp`) and whether it shows a text beside the progress of the
 * Dock (`progressText`, `brd mcp`)
 */
export type TitleBar = (
  | { readonly kind: "overlay"; readonly doubleClick: DoubleClickAction }
  | {
      readonly kind: "custom"
      readonly controls: WindowControlsLayout
      readonly doubleClick: DoubleClickAction
      /**
       * The page rounds the corners of the window as the desktop's windows are (px, 0 for none): Linux with a compositor,
       * whose GTK3 rounds its decorations but doesn't clip the webview; square while maximized, tiled or fullscreen
       */
      readonly cornerRadius: number
    }
  | { readonly kind: "native"; readonly reason?: NativeTitleBarReason }
) & {
  /** The build updates itself (the `auto-update` feature): ☰ and the command palette offer Check for Updates… */
  readonly checkForUpdates: boolean
  /**
   * The host has a folder dialog: the repository selector, the command palette and Settings → Landing offer Open
   * Repository… and Add Companion…; false in the IDE, whose roots name the repositories. Offered unless false
   */
  readonly folderDialog?: boolean
  /**
   * The host shows Branches, Reviews and Landing each in a tab of its own, a page each (the tool window of the IDE): no
   * page has the switch of the views, and a switch to the view of another tab asks the host for that tab
   * (`showMainView`); absent in the desktop app, whose main window switches its views
   */
  readonly viewTabs?: boolean
  /**
   * The host shows the progress of the Dock with a text before its percent (the tab of the main view in the IDE):
   * `setDockProgress` names what runs; absent in the desktop app, whose Dock shows no text
   */
  readonly progressText?: boolean
}

export type WindowButton = "minimize" | "maximize" | "close"

/** The Windows caption buttons, GNOME's (Adwaita) or KDE's (Breeze), as IntelliJ draws them */
export type WindowButtonStyle = "windows" | "gnome" | "kde"

export interface WindowControlsLayout {
  readonly start: readonly WindowButton[]
  readonly end: readonly WindowButton[]
  readonly style: WindowButtonStyle
}

/** What a double click on the empty parts of the title bar does, as the system setting says */
export type DoubleClickAction = "toggle-maximize" | "minimize" | "none"

/** Why the system title bar stays */
export type NativeTitleBarReason = "env" | "wsl" | "no-desktop" | "tiling-wm" | "setting"

/** As the backend set it; the native one without a page (a unit test in Node, which reads the stores of views) */
export const titleBar: TitleBar = (typeof window === "undefined" ? undefined : window.__BD_TITLE_BAR__) ?? { kind: "native", checkForUpdates: false }

/** The page draws the title bar: its empty parts drag the window */
export const drawsTitleBar = titleBar.kind !== "native"

/** The page draws the window buttons and the main menu, and handles the keys of the menu: there is no native menu bar */
export const hasCustomTitleBar = titleBar.kind === "custom"

/**
 * The build updates itself (the `auto-update` feature, as the backend says, whatever draws the title bar): the main
 * menu (☰) and the command palette offer Check for Updates…, as the native menu does
 */
export const updatesItself = titleBar.checkForUpdates

/** The host has a folder dialog, as the backend says: Open Repository… and Add Companion… pick a folder in it */
export const hasFolderDialog = titleBar.folderDialog !== false

/** The host shows each view of the main window in a tab of its own, as the backend says (`TitleBar.viewTabs`) */
export const hasViewTabs = titleBar.viewTabs === true

/** The host shows a text beside the progress of the Dock, as the backend says (`TitleBar.progressText`) */
export const hasProgressText = titleBar.progressText === true
