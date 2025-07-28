use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Debug, Clone, Serialize, Type)]
pub struct WindowError {
  message: String,
}

impl WindowError {
  fn new(message: String) -> Self {
    Self { message }
  }
}

impl From<tauri::Error> for WindowError {
  fn from(err: tauri::Error) -> Self {
    WindowError::new(format!("Failed to create window: {err}"))
  }
}

impl From<serde_json::Error> for WindowError {
  fn from(err: serde_json::Error) -> Self {
    WindowError::new(format!("Failed to serialize data: {err}"))
  }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenSubWindowParams {
  pub window_id: String,
  pub url: String,
  pub title: String,
  pub width: Option<f64>,
  pub height: Option<f64>,
  pub data: String, // JSON string
}

#[tauri::command]
#[specta::specta]
pub async fn open_sub_window(app_handle: tauri::AppHandle, params: OpenSubWindowParams) -> Result<(), WindowError> {
  // Check if window already exists
  if let Some(existing_window) = app_handle.get_webview_window(&params.window_id) {
    // Window exists, bring it to front
    existing_window.show()?;
    existing_window.unminimize()?;
    existing_window.set_focus()?;

    // Update __INIT_DATA__ directly by evaluating JavaScript
    let update_script = format!(
      r#"
      window.__INIT_DATA__ = {};
      // Trigger a custom event to notify the app that data has been updated
      window.dispatchEvent(new CustomEvent('init-data-updated'));
      "#,
      params.data
    );

    existing_window.eval(&update_script)?;

    return Ok(());
  }

  // Use the data directly since it's already a JSON string
  // Create initialization script that sets the data on window object
  let init_script = format!(
    r#"
      window.__INIT_DATA__ = {};
      "#,
    params.data
  );

  // Create new window
  WebviewWindowBuilder::new(&app_handle, &params.window_id, WebviewUrl::App(params.url.into()))
    .title(params.title)
    .inner_size(params.width.unwrap_or(1400.0), params.height.unwrap_or(900.0))
    .center()
    .resizable(true)
    .skip_taskbar(true)
    .initialization_script(&init_script)
    .build()?;

  Ok(())
}
