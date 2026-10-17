use crate::command_error::CommandError;

/// The window manager's menu of the window (Move, Resize, Always on Top, Close…), as a right click on a native title bar
/// opens it: the page calls it for a right click on the empty parts of its title bar (`WindowTitleBar`). A sync command,
/// so it runs on the main thread, where GDK and Win32 menus live.
#[tauri::command]
#[specta::specta]
#[expect(clippy::needless_pass_by_value, reason = "tauri passes the window of a command by value")]
pub fn show_window_menu(window: tauri::WebviewWindow) -> Result<(), CommandError> {
  show(&window)
}

/// Windows: the system menu at the pointer, its choice posted as `WM_SYSCOMMAND`, as `DefWindowProc` does for a caption
#[cfg(target_os = "windows")]
#[expect(unsafe_code, reason = "Win32: the system menu of the window")]
fn show(window: &tauri::WebviewWindow) -> Result<(), CommandError> {
  use windows::Win32::Foundation::{LPARAM, POINT, WPARAM};
  use windows::Win32::UI::WindowsAndMessaging::{
    EnableMenuItem, GetCursorPos, GetSystemMenu, IsIconic, IsZoomed, MF_BYCOMMAND, MF_ENABLED, MF_GRAYED, PostMessageW, SC_MAXIMIZE, SC_MINIMIZE, SC_MOVE, SC_RESTORE, SC_SIZE,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_SYSCOMMAND,
  };
  let hwnd = window.hwnd().map_err(|e| CommandError::new(e.to_string()))?;
  // SAFETY: `hwnd` is the live window of this webview, and the calls run on its thread (a sync command runs on the main
  // thread); `point` outlives the call that writes it
  unsafe {
    let menu = GetSystemMenu(hwnd, false);
    // `DefWindowProc` updates the items only when it opens the menu itself: by the state of the window, as a caption
    let maximized = IsZoomed(hwnd).as_bool();
    let minimized = IsIconic(hwnd).as_bool();
    let state = |enabled: bool| MF_BYCOMMAND | if enabled { MF_ENABLED } else { MF_GRAYED };
    for (command, enabled) in [
      (SC_RESTORE, maximized || minimized),
      (SC_MOVE, !maximized),
      (SC_SIZE, !maximized),
      (SC_MINIMIZE, !minimized),
      (SC_MAXIMIZE, !maximized),
    ] {
      let _ = EnableMenuItem(menu, command, state(enabled));
    }
    let mut point = POINT::default();
    GetCursorPos(&raw mut point).map_err(|e| CommandError::new(e.to_string()))?;
    let command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, point.x, point.y, None, hwnd, None);
    if command.0 != 0 {
      #[expect(clippy::cast_sign_loss, reason = "a menu command id is a positive WORD")]
      let command = command.0 as usize;
      PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM(command), LPARAM(0)).map_err(|e| CommandError::new(e.to_string()))?;
    }
  }
  Ok(())
}

/// Linux: GDK asks the window manager for the menu with the right button press that started it (`last_right_press`):
/// Wayland takes the serial of a real input event, a made-up event would be refused
#[cfg(target_os = "linux")]
fn show(window: &tauri::WebviewWindow) -> Result<(), CommandError> {
  use gtk::prelude::WidgetExt;
  let Some(mut event) = crate::title_bar::last_right_press(window.label()) else {
    tracing::warn!(window = window.label(), "no right button press of the webview for the window menu");
    return Ok(());
  };
  let gtk_window = window.gtk_window().map_err(|e| CommandError::new(e.to_string()))?;
  if let Some(gdk_window) = gtk_window.window()
    && !gdk_window.show_window_menu(&mut event)
  {
    tracing::warn!(window = window.label(), "the window manager has no window menu");
  }
  Ok(())
}

/// macOS has no window menu of the title bar
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
#[expect(clippy::unnecessary_wraps, reason = "the command has one signature on every platform")]
const fn show(_window: &tauri::WebviewWindow) -> Result<(), CommandError> {
  Ok(())
}
