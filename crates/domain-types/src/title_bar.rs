use serde::Serialize;

/// How every window draws its title bar, as the page reads it before it loads (`window.__BD_TITLE_BAR__`,
/// `src/lib/title-bar.ts`). The desktop app decides it once per process (`src-tauri/src/title_bar.rs`), `brd mcp` for
/// each view of the IDE (`bd_bootstrap`):
/// - `Overlay` on macOS: the page's top row sits in an overlay title bar beside the traffic lights;
/// - `Custom` on Windows and Linux: the row is the title bar and draws the window buttons (Windows: no decorations;
///   Linux: GTK client-side decorations with an empty titlebar, so GTK still draws the shadow and the resize edges);
/// - `Native` where the system title bar stays above the row: on Linux where a custom one doesn't fit (WSL, a tiling
///   window manager, an unknown desktop), wherever the user asked for it (the setting, the environment variable), and
///   in a place of the IDE that draws its own title (no reason).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TitleBar {
  /// `height`: the height of the row that the host gives its title bar (a window of the IDE), none for the 40 px of the
  /// desktop app
  Overlay {
    double_click: DoubleClick,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
  },
  /// `corner_radius`: the page rounds the corners of the window as the desktop's windows are (Linux with a compositor:
  /// GTK3 rounds its decorations but doesn't clip the webview, so the window is transparent and the page clips its
  /// canvas), 0 where the system does it (Windows 11) or can't (no compositor)
  Custom {
    controls: WindowControls,
    double_click: DoubleClick,
    corner_radius: u32,
  },
  /// `reason`: why the desktop app keeps the system title bar; none in the IDE
  Native {
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<NativeReason>,
  },
}

/// Why the system title bar stays, for the log and for the setting, which says it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeReason {
  /// `BRANCH_DECK_NATIVE_TITLE_BAR=1`
  Env,
  /// `WSLg` draws its own frame
  Wsl,
  /// No `XDG_CURRENT_DESKTOP`: nothing tells how the desktop decorates its windows
  NoDesktop,
  /// A tiling window manager draws the titles itself
  TilingWm,
  /// "Use the system title bar" (`appSettings.systemTitleBar`)
  Setting,
}

/// The window buttons of a custom title bar, in the order and on the sides the desktop shows them
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowControls {
  pub start: Vec<WindowButton>,
  pub end: Vec<WindowButton>,
  pub style: ButtonStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowButton {
  Minimize,
  Maximize,
  Close,
}

/// How the buttons look: the Windows caption buttons, GNOME's (Adwaita) or KDE's (Breeze), as IntelliJ draws them
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonStyle {
  Windows,
  Gnome,
  Kde,
}

/// What a double click on the empty parts of the title bar does: as the system setting says, or what the host does
/// (`Host`: the page tells the IDE of the click, and the IDE does the double click of the system)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DoubleClick {
  ToggleMaximize,
  Minimize,
  None,
  Host,
}

/// What the page reads before it loads: how it draws the title bar, and with every kind whether the build updates
/// itself (the `auto-update` feature: ☰ and the command palette offer Check for Updates… as the native menu does),
/// whether the host has a folder dialog and whether it shows a text beside the progress of the Dock
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageTitleBar<'a> {
  #[serde(flatten)]
  pub title_bar: &'a TitleBar,
  pub check_for_updates: bool,
  /// The host has a folder dialog (`DialogHost::has_folder_dialog`): the page offers Open Repository… and Add
  /// Companion…; the desktop app always has it
  pub folder_dialog: bool,
  /// The host shows the progress of the Dock with a text before its percent (the title of the tool window in the
  /// IDE); absent in the desktop app
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  pub progress_text: bool,
}
