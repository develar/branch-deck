//! The services of the app (`app_api::app::services`, the same as `brd mcp` of an IDE runs with): their context is the
//! `AppContext` Tauri manages (`TauriContext`), which the app builds once its services exist (`lib.rs`).

use app_api::app::services::ContextSource;
use app_api::context::AppContext;
use tauri::{AppHandle, Manager};

/// The `AppContext` the app manages
pub struct TauriContext(pub AppHandle);

impl ContextSource for TauriContext {
  fn context(&self) -> &AppContext {
    self.0.state::<AppContext>().inner()
  }
}
