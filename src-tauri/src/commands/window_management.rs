use crate::command_error::CommandError;
use crate::title_bar::{title_bar_script, with_title_bar};
use crate::zoom::{MIN_SUB_WINDOW_HEIGHT, apply_zoom, min_sub_window_width, stored_zoom};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, State, Webview, WebviewUrl, WebviewWindowBuilder};
use tracing::warn;

/// How long a new sub-window stays hidden at most when its page never says it painted (a broken page still shows)
const SHOW_FALLBACK: Duration = Duration::from_secs(3);

/// The data of an open of a sub-window, as `open_sub_window` got it: `__INIT_DATA__` and `__TAURI_STORE__` of its page
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubWindowUpdate {
  pub data: String,
  pub store_cache: String,
}

#[derive(Default)]
struct SubWindowEntry {
  /// the page loaded and painted (`sub_window_ready`): an open reaches it by `eval`
  ready: bool,
  /// the latest open that came while the page still loaded: an `eval` before the load is lost, the page takes it
  pending: Option<SubWindowUpdate>,
}

/// The sub-windows by label. A window is built hidden and shows once its page painted (no blank frame); an open of a
/// window whose page still loads is kept for the page, so two quick opens show the second subject, not the first.
#[derive(Default)]
pub struct SubWindowState {
  /// one open at a time: two opens of a label never both build it (`open_sub_window` is async, opens run in parallel)
  open_lock: tauri::async_runtime::Mutex<()>,
  windows: Mutex<HashMap<String, SubWindowEntry>>,
}

impl SubWindowState {
  fn windows(&self) -> std::sync::MutexGuard<'_, HashMap<String, SubWindowEntry>> {
    self.windows.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  /// A window of `label` was built: its page loads with the data of the build
  pub fn built(&self, label: &str) {
    self.windows().insert(label.to_string(), SubWindowEntry::default());
  }

  /// An open of the existing window of `label`: true when its page has loaded and takes `update` now; false when the
  /// page still loads and takes it when it is ready
  pub fn reopened(&self, label: &str, update: SubWindowUpdate) -> bool {
    match self.windows().get_mut(label) {
      // a window this state never built (none today): its page is long loaded
      None => true,
      Some(entry) if entry.ready => true,
      Some(entry) => {
        entry.pending = Some(update);
        false
      }
    }
  }

  /// The page of `label` painted: the data of an open that came while it loaded, if any
  pub fn ready(&self, label: &str) -> Option<SubWindowUpdate> {
    let mut windows = self.windows();
    let entry = windows.entry(label.to_string()).or_default();
    entry.ready = true;
    entry.pending.take()
  }

  /// The window went
  pub fn forget(&self, label: &str) {
    self.windows().remove(label);
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
  pub data: String,        // JSON string with window-specific data
  pub store_cache: String, // JSON string with store cache
}

/// Opens a sub-window, or brings the existing one of `window_id` to the front with new data. A new one is built hidden
/// on the monitor of the main window (the window state plugin restores where it was since) and shows once its page
/// painted (`sub_window_ready`).
#[tauri::command]
#[specta::specta]
pub async fn open_sub_window(app_handle: AppHandle, state: State<'_, SubWindowState>, params: OpenSubWindowParams) -> Result<(), CommandError> {
  let _open = state.open_lock.lock().await;
  if let Some(existing_window) = app_handle.get_webview_window(&params.window_id) {
    // titled for what it shows now
    existing_window.set_title(&params.title)?;
    let update = SubWindowUpdate {
      data: params.data,
      store_cache: params.store_cache,
    };
    // a page that still loads is hidden yet: it takes the data and shows itself
    if state.reopened(&params.window_id, update.clone()) {
      let update_script = format!(
        "
        window.__TAURI_STORE__ = {};
        window.__INIT_DATA__ = {};
        // Trigger a custom event to notify the app that data has been updated
        window.dispatchEvent(new CustomEvent('init-data-updated'));
        ",
        update.store_cache, update.data
      );
      existing_window.eval(&update_script)?;
      existing_window.show()?;
      existing_window.unminimize()?;
      existing_window.set_focus()?;
    }
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
  let width = params.width.unwrap_or(1400.0).max(min_width);
  let height = params.height.unwrap_or(900.0);
  let builder = WebviewWindowBuilder::new(&app_handle, &params.window_id, WebviewUrl::App(params.url.into()))
    .title(params.title)
    .inner_size(width, height)
    .min_inner_size(min_width, MIN_SUB_WINDOW_HEIGHT)
    .visible(false)
    .resizable(true)
    .initialization_script(&init_script)
    // the native file drop handler would swallow HTML5 drag and drop (on Windows)
    .disable_drag_drop_handler();
  let builder = match centered_on_main_monitor(&app_handle, width, height) {
    Some((x, y)) => builder.position(x, y),
    None => builder.center(),
  };
  let window = with_title_bar(builder).build()?;
  apply_zoom(&window, zoom);
  state.built(&params.window_id);

  let label = params.window_id;
  tauri::async_runtime::spawn(async move {
    tokio::time::sleep(SHOW_FALLBACK).await;
    if let Some(window) = app_handle.get_webview_window(&label)
      && !window.is_visible().unwrap_or(true)
    {
      warn!(label, "the page of the sub-window did not say it painted, showing it");
      if let Err(e) = window.show().and_then(|()| window.set_focus()) {
        warn!(label, error = %e, "cannot show the sub-window");
      }
    }
  });

  Ok(())
}

/// The page of the calling sub-window painted: the window shows (it was built hidden) and the page gets the data of an
/// open that came while it loaded
#[tauri::command]
#[specta::specta]
pub async fn sub_window_ready(webview: Webview, state: State<'_, SubWindowState>) -> Result<Option<SubWindowUpdate>, CommandError> {
  let update = state.ready(webview.label());
  let window = webview.window();
  window.show()?;
  window.set_focus()?;
  Ok(update)
}

/// Where a sub-window of `width` x `height` (logical) is centred on the monitor of the main window: a new window opens
/// where the user works, not on the primary monitor
fn centered_on_main_monitor(app_handle: &AppHandle, width: f64, height: f64) -> Option<(f64, f64)> {
  let monitor = app_handle.get_webview_window("main")?.current_monitor().ok()??;
  let scale = monitor.scale_factor();
  let position = monitor.position().to_logical::<f64>(scale);
  let size = monitor.size().to_logical::<f64>(scale);
  Some((position.x + ((size.width - width) / 2.0).max(0.0), position.y + ((size.height - height) / 2.0).max(0.0)))
}
