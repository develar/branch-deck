use tauri::{Manager, Runtime, WebviewWindowBuilder};

/// How every window draws its title bar, passed to the page as `window.__BD_TITLE_BAR__` (`src/lib/title-bar.ts`): on
/// macOS the page's top row sits in an overlay title bar beside the traffic lights, on Windows the window has no
/// decorations and the row draws the window buttons, Linux keeps the native title bar above the row.
#[cfg(target_os = "macos")]
const TITLE_BAR: Option<&str> = Some("overlay");
#[cfg(target_os = "windows")]
const TITLE_BAR: Option<&str> = Some("custom");
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const TITLE_BAR: Option<&str> = None;

/// The line of the initialization script that tells the page how it draws the title bar (`TITLE_BAR`); empty where the
/// native title bar stays
pub fn title_bar_script() -> String {
  TITLE_BAR.map_or_else(String::new, |title_bar| format!("window.__BD_TITLE_BAR__ = \"{title_bar}\";"))
}

/// A window whose page draws the title bar (`TITLE_BAR`): the main window and every sub-window. The native title stays
/// set (`title`, `set_title`): the Window menu, Mission Control and screen readers read it.
pub fn with_title_bar<R: Runtime, M: Manager<R>>(builder: WebviewWindowBuilder<'_, R, M>) -> WebviewWindowBuilder<'_, R, M> {
  // centred in the 40 px top row, 14 px from the left as from the top. wry takes y as the height of the button container
  // above the buttons, not as their top: the buttons keep their offset in the container, so y = 13 left their centre at
  // 11 px, and y = 22 puts it at 20 px
  #[cfg(target_os = "macos")]
  let builder = builder
    .title_bar_style(tauri::TitleBarStyle::Overlay)
    .hidden_title(true)
    .traffic_light_position(tauri::LogicalPosition::new(14.0, 22.0));
  #[cfg(target_os = "windows")]
  let builder = builder.decorations(false).shadow(true);
  builder
}
