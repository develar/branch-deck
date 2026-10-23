//! The content of `settings.json` that the main window gets as `window.__TAURI_STORE__` at startup

use std::path::Path;
use tracing::warn;

/// The content of the settings file as a JSON value for the init script of the main window: `{}` without the file, and
/// for one that does not parse (the app stopped within a write, which the store plugin does in place): its text in the
/// script would be a syntax error, and the page would start without its settings
pub fn stored_settings(store_path: &Path) -> String {
  let Ok(text) = std::fs::read_to_string(store_path) else {
    return "{}".to_string();
  };
  match serde_json::from_str::<serde_json::Value>(&text) {
    Ok(value) if value.is_object() => text,
    Ok(_) => {
      warn!(file = %store_path.display(), "the settings file is no JSON object, starting with the defaults");
      "{}".to_string()
    }
    Err(e) => {
      warn!(error = %e, file = %store_path.display(), "the settings file does not parse, starting with the defaults");
      "{}".to_string()
    }
  }
}
