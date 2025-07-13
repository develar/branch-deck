use serde::Serialize;
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

#[tauri::command]
#[specta::specta]
pub async fn open_sub_window(
  app_handle: tauri::AppHandle,
  window_id: String,
  url: String,
  title: String,
  width: Option<f64>,
  height: Option<f64>,
  data: String, // JSON string
) -> Result<(), WindowError> {
  // Check if window already exists
  if let Some(existing_window) = app_handle.get_webview_window(&window_id) {
    // Window exists, bring it to front
    existing_window.show()?;
    existing_window.unminimize()?;
    existing_window.set_focus()?;

    // Update __INIT_DATA__ directly by evaluating JavaScript
    let update_script = format!(
      r#"
      window.__INIT_DATA__ = {data};
      // Trigger a custom event to notify the app that data has been updated
      window.dispatchEvent(new CustomEvent('init-data-updated'));
      "#
    );

    existing_window.eval(&update_script)?;

    return Ok(());
  }

  // Use the data directly since it's already a JSON string
  // Create initialization script that sets the data on window object
  let init_script = format!(
    r#"
      window.__INIT_DATA__ = {data};
      "#
  );

  // Create new window
  WebviewWindowBuilder::new(&app_handle, &window_id, WebviewUrl::App(url.into()))
    .title(title)
    .inner_size(width.unwrap_or(1400.0), height.unwrap_or(900.0))
    .center()
    .resizable(true)
    .skip_taskbar(true)
    .initialization_script(&init_script)
    .build()?;

  Ok(())
}
