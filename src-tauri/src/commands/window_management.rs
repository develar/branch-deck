use crate::command_error::CommandError;
use crate::title_bar::{title_bar_script, with_title_bar};
use crate::zoom::{MIN_SUB_WINDOW_HEIGHT, apply_zoom, min_sub_window_width, stored_zoom};
use serde::Deserialize;
use specta::Type;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenSubWindowParams {
  pub window_id: String,
  pub url: String,
  pub title: String,
  pub width: Option<f64>,
  pub height: Option<f64>,
  pub data: String,        // JSON string with window-specific data
  pub store_cache: String, // JSON string with store cache
}

#[tauri::command]
#[specta::specta]
pub async fn open_sub_window(app_handle: tauri::AppHandle, params: OpenSubWindowParams) -> Result<(), CommandError> {
  // Check if window already exists
  if let Some(existing_window) = app_handle.get_webview_window(&params.window_id) {
    // Window exists, bring it to front, titled for what it shows now
    existing_window.set_title(&params.title)?;
    existing_window.show()?;
    existing_window.unminimize()?;
    existing_window.set_focus()?;

    // Update both __TAURI_STORE__ and __INIT_DATA__ directly by evaluating JavaScript
    let update_script = format!(
      "
      window.__TAURI_STORE__ = {};
      window.__INIT_DATA__ = {};
      // Trigger a custom event to notify the app that data has been updated
      window.dispatchEvent(new CustomEvent('init-data-updated'));
      ",
      params.store_cache, params.data
    );

    existing_window.eval(&update_script)?;

    return Ok(());
  }

  // Create initialization script that sets the store cache, the window data and how the page draws the title bar
  let init_script = format!(
    "
      window.__TAURI_STORE__ = {};
      window.__INIT_DATA__ = {};
      {}
      ",
    params.store_cache,
    params.data,
    title_bar_script()
  );

  // Create new window: its page draws the title bar, as the main window's does
  let zoom = stored_zoom(&params.store_cache);
  let min_width = min_sub_window_width(zoom);
  let window = with_title_bar(
    WebviewWindowBuilder::new(&app_handle, &params.window_id, WebviewUrl::App(params.url.into()))
      .title(params.title)
      .inner_size(params.width.unwrap_or(1400.0).max(min_width), params.height.unwrap_or(900.0))
      .min_inner_size(min_width, MIN_SUB_WINDOW_HEIGHT)
      .center()
      .resizable(true)
      .skip_taskbar(true)
      .initialization_script(&init_script)
      // the native file drop handler would swallow HTML5 drag and drop (on Windows)
      .disable_drag_drop_handler(),
  )
  .build()?;
  apply_zoom(&window, zoom);

  Ok(())
}
