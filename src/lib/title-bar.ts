/**
 * How every window draws its title bar, decided once per process by the backend (`src-tauri/src/title_bar.rs`) and set
 * before the page loads; the top row of the page (`WindowTitleBar`) is the title bar unless it is `native`:
 * - `overlay` on macOS: beside the traffic lights;
 * - `custom` on Windows and Linux: the row draws the window buttons, in the order and on the sides of the desktop
 *   (`controls`), and the main menu (☰) replaces the native menu bar;
 * - `native`: the system title bar above the row, on Linux where a custom one doesn't fit (WSL, a tiling window
 *   manager, an unknown desktop) and where the user asked for it (`reason`); in E2E without one.
 */
export type TitleBar =
  | { readonly kind: "overlay"; readonly doubleClick: DoubleClickAction }
  | {
      readonly kind: "custom"
      readonly controls: WindowControlsLayout
      readonly doubleClick: DoubleClickAction
      /** The build updates itself: the main menu offers Check for Updates…, as the native menu would */
      readonly checkForUpdates: boolean
      /**
       * The page rounds the corners of the window as the desktop's windows are (px, 0 for none): Linux with a compositor,
       * whose GTK3 rounds its decorations but doesn't clip the webview; square while maximized, tiled or fullscreen
       */
      readonly cornerRadius: number
    }
  | { readonly kind: "native"; readonly reason?: NativeTitleBarReason }

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

export const titleBar: TitleBar = window.__BD_TITLE_BAR__ ?? { kind: "native" }

/** The page draws the title bar: its empty parts drag the window */
export const drawsTitleBar = titleBar.kind !== "native"

/** The page draws the window buttons and the main menu, and handles the keys of the menu: there is no native menu bar */
export const hasCustomTitleBar = titleBar.kind === "custom"
