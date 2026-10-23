use crate::operation::{operation, tauri_error};
use crate::title_bar::{prepare_title_bar, title_bar_script, with_title_bar};
use crate::zoom::{apply_zoom, stored_zoom, sub_window_min_size};
use domain_types::error::ApiError;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, State, Webview, WebviewUrl, WebviewWindowBuilder};
use tracing::{Instrument, instrument, warn};

/// How long a new sub-window stays hidden at most when its page never loads (a broken page still shows)
const SHOW_FALLBACK: Duration = Duration::from_secs(3);
/// How long a loaded page may take to settle its content (`sub_window_ready`) before its window shows with what it has:
/// a slow git shows the tree and the headers, not nothing
const CONTENT_BUDGET: Duration = Duration::from_millis(300);

/// The canvas of the light and the dark theme (`--canvas` of `src/index.css` in sRGB): the colour behind the first frame
/// that the webview paints after `show()`, never white
const LIGHT_CANVAS: tauri::window::Color = tauri::window::Color(233, 234, 238, 255);
const DARK_CANVAS: tauri::window::Color = tauri::window::Color(38, 40, 44, 255);

/// An open of a sub-window, as its page gets it (`window.__SUB_WINDOW_OPEN__`)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubWindowOpen {
  /// Counts the opens of all windows: the page names the open whose content it settled (`sub_window_ready`)
  pub id: u32,
  /// When the opener asked (`Date.now()`), for the timing line of the page
  pub opened_at: f64,
}

/// The data of an open of a sub-window, as `open_sub_window` got it: `__INIT_DATA__`, `__TAURI_STORE__` and
/// `__SUB_WINDOW_OPEN__` of its page
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubWindowUpdate {
  pub data: String,
  pub store_cache: String,
  pub open: SubWindowOpen,
}

impl SubWindowUpdate {
  /// The script that hands the update to a loaded page
  fn script(&self) -> Result<String, ApiError> {
    let open = to_json(&self.open)?;
    Ok(format!(
      "
      window.__TAURI_STORE__ = {};
      window.__INIT_DATA__ = {};
      window.__SUB_WINDOW_OPEN__ = {open};
      window.dispatchEvent(new CustomEvent('init-data-updated'));
      ",
      self.store_cache, self.data
    ))
  }
}

/// What an open of an existing window does
#[derive(Debug, PartialEq, Eq)]
pub enum Reopen {
  /// The page still loads: it takes the open from `sub_window_loaded`
  Pending,
  /// The page is loaded, the window not shown yet: it takes the open now and shows once that content settled
  Hidden,
  /// The window is on screen: it takes the open and comes to the front
  Shown,
}

struct SubWindowEntry {
  /// The open that built the window: a timer of an earlier window of the label leaves this one alone
  build: u32,
  /// The page loaded (`sub_window_loaded`): an open reaches it by `eval`
  loaded: bool,
  /// The latest open that came while the page still loaded: an `eval` before the load is lost, the page takes it
  pending: Option<SubWindowUpdate>,
  /// The open whose settled content shows the window: the latest one
  showing_for: u32,
  shown: bool,
}

/// The sub-windows by label. A window is built hidden and shows once its page settled the content of the latest open
/// (no blank frame, never the subject of an earlier open), at most `CONTENT_BUDGET` after its page loaded and
/// `SHOW_FALLBACK` after the build; an open of a window whose page still loads is kept for the page.
#[derive(Default)]
pub struct SubWindowState {
  /// one open at a time: two opens of a label never both build it (`open_sub_window` is async, opens run in parallel)
  open_lock: tauri::async_runtime::Mutex<()>,
  windows: Mutex<HashMap<String, SubWindowEntry>>,
  opens: AtomicU32,
}

impl SubWindowState {
  fn windows(&self) -> std::sync::MutexGuard<'_, HashMap<String, SubWindowEntry>> {
    self.windows.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  /// A new open, numbered
  pub fn next_open(&self, opened_at: f64) -> SubWindowOpen {
    SubWindowOpen {
      id: self.opens.fetch_add(1, Ordering::Relaxed) + 1,
      opened_at,
    }
  }

  /// A window of `label` was built for `open`: its page loads with the data of the build
  pub fn built(&self, label: &str, open: SubWindowOpen) {
    self.windows().insert(
      label.to_string(),
      SubWindowEntry {
        build: open.id,
        loaded: false,
        pending: None,
        showing_for: open.id,
        shown: false,
      },
    );
  }

  /// An open of the existing window of `label`
  pub fn reopened(&self, label: &str, update: SubWindowUpdate) -> Reopen {
    let mut windows = self.windows();
    // a window this state never built (none today): its page is long loaded and on screen
    let Some(entry) = windows.get_mut(label) else {
      return Reopen::Shown;
    };
    if entry.shown {
      return Reopen::Shown;
    }
    entry.showing_for = update.open.id;
    if entry.loaded {
      Reopen::Hidden
    } else {
      entry.pending = Some(update);
      Reopen::Pending
    }
  }

  /// The page of `label` loaded: the open that came while it loaded, if any, and the build the content budget is for
  pub fn loaded(&self, label: &str) -> (Option<SubWindowUpdate>, Option<u32>) {
    let mut windows = self.windows();
    let Some(entry) = windows.get_mut(label) else {
      return (None, None);
    };
    entry.loaded = true;
    (entry.pending.take(), Some(entry.build))
  }

  /// The page of `label` settled the content of the open `open_id`: true when the window shows now (the latest open,
  /// not shown yet)
  pub fn ready(&self, label: &str, open_id: u32) -> bool {
    match self.windows().get_mut(label) {
      Some(entry) if !entry.shown && entry.showing_for == open_id => {
        entry.shown = true;
        true
      }
      _ => false,
    }
  }

  /// A budget of the window `build` of `label` passed: true when it shows now with what it has
  pub fn budget_passed(&self, label: &str, build: u32) -> bool {
    match self.windows().get_mut(label) {
      Some(entry) if entry.build == build && !entry.shown => {
        entry.shown = true;
        true
      }
      _ => false,
    }
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
  /// When the opener asked (`Date.now()`)
  pub opened_at: f64,
}

/// Opens a sub-window, or brings the existing one of `window_id` to the front with new data. A new one is built hidden
/// on the monitor of the main window (the window state plugin restores where it was since) and shows once its page
/// settled its content (`sub_window_ready`).
#[tauri::command]
#[instrument(skip(app_handle, state, params), fields(window_id = %params.window_id, url = %params.url))]
#[specta::specta]
pub async fn open_sub_window(app_handle: AppHandle, state: State<'_, SubWindowState>, params: OpenSubWindowParams) -> Result<(), ApiError> {
  operation(async move {
    let _open = state.open_lock.lock().await;
    let open = state.next_open(params.opened_at);
    if let Some(existing_window) = app_handle.get_webview_window(&params.window_id) {
      // titled for what it shows now
      existing_window.set_title(&params.title).map_err(tauri_error)?;
      let update = SubWindowUpdate {
        data: params.data,
        store_cache: params.store_cache,
        open,
      };
      match state.reopened(&params.window_id, update.clone()) {
        Reopen::Pending => {}
        Reopen::Hidden => existing_window.eval(update.script()?).map_err(tauri_error)?,
        Reopen::Shown => {
          existing_window.eval(update.script()?).map_err(tauri_error)?;
          existing_window.show().map_err(tauri_error)?;
          existing_window.unminimize().map_err(tauri_error)?;
          existing_window.set_focus().map_err(tauri_error)?;
        }
      }
      return Ok(());
    }

    // the store cache, the window data, the open and how the page draws the title bar, before the page's own scripts
    let init_script = format!(
      "
      window.__TAURI_STORE__ = {};
      window.__INIT_DATA__ = {};
      window.__SUB_WINDOW_OPEN__ = {};
      {}
      ",
      params.store_cache,
      params.data,
      to_json(&open)?,
      title_bar_script()
    );

    // Create new window: its page draws the title bar, as the main window's does
    let zoom = stored_zoom(&params.store_cache);
    let (min_width, min_height) = sub_window_min_size(&params.window_id, zoom);
    let width = params.width.unwrap_or(1400.0).max(min_width);
    let height = params.height.unwrap_or(900.0).max(min_height);
    let builder = WebviewWindowBuilder::new(&app_handle, &params.window_id, WebviewUrl::App(params.url.into()))
      .title(params.title)
      .inner_size(width, height)
      .min_inner_size(min_width, min_height)
      .visible(false)
      .resizable(true)
      .initialization_script(&init_script)
      // the native file drop handler would swallow HTML5 drag and drop (on Windows)
      .disable_drag_drop_handler();
    let builder = match centered_on_main_monitor(&app_handle, width, height) {
      Some((x, y)) => builder.position(x, y),
      None => builder.center(),
    };
    let window = with_title_bar(with_canvas(builder, &app_handle)).build().map_err(tauri_error)?;
    apply_zoom(&window, zoom);
    // the empty GTK titlebar of a custom title bar on Linux, before the page paints and the window shows
    prepare_title_bar(&window);
    state.built(&params.window_id, open);
    show_after(
      app_handle,
      params.window_id,
      open.id,
      SHOW_FALLBACK,
      Some("the page of the sub-window did not load, showing it"),
    );
    Ok(())
  })
  .await
}

/// The page of the calling sub-window loaded: it gets the data of an open that came while it loaded, and the window
/// shows once the page settled its content, at most `CONTENT_BUDGET` from now
#[tauri::command]
#[instrument(skip(app_handle, webview, state))]
#[specta::specta]
pub async fn sub_window_loaded(app_handle: AppHandle, webview: Webview, state: State<'_, SubWindowState>) -> Result<Option<SubWindowUpdate>, ApiError> {
  operation(async move {
    let label = webview.label().to_string();
    let (update, build) = state.loaded(&label);
    if let Some(build) = build {
      show_after(app_handle, label, build, CONTENT_BUDGET, None);
    }
    Ok(update)
  })
  .await
}

/// The page of the calling sub-window settled the content of the open `open_id`: the window shows if that is the latest
/// open and the window is still hidden
#[tauri::command]
#[instrument(skip(webview, state))]
#[specta::specta]
pub async fn sub_window_ready(webview: Webview, state: State<'_, SubWindowState>, open_id: u32) -> Result<(), ApiError> {
  operation(async move {
    if state.ready(webview.label(), open_id) {
      let window = webview.window();
      window.show().map_err(tauri_error)?;
      window.set_focus().map_err(tauri_error)?;
    }
    Ok(())
  })
  .await
}

/// Shows the window `build` of `label` after `delay` unless it showed by then, logging `warning` if it does
fn show_after(app_handle: AppHandle, label: String, build: u32, delay: Duration, warning: Option<&'static str>) {
  tauri::async_runtime::spawn(
    async move {
      tokio::time::sleep(delay).await;
      if !app_handle.state::<SubWindowState>().budget_passed(&label, build) {
        return;
      }
      if let Some(warning) = warning {
        warn!(label, "{warning}");
      }
      if let Some(window) = app_handle.get_webview_window(&label)
        && let Err(e) = window.show().and_then(|()| window.set_focus())
      {
        warn!(label, error = %e, "cannot show the sub-window");
      }
    }
    .in_current_span(),
  );
}

fn to_json(open: &SubWindowOpen) -> Result<String, ApiError> {
  serde_json::to_string(open).map_err(|e| ApiError::internal(e.to_string()))
}

/// The window on the canvas of the theme the main window shows. Not on Linux: a rounded window is transparent there,
/// the page clips its canvas to the radius (`round_corners`)
fn with_canvas<'a>(builder: WebviewWindowBuilder<'a, tauri::Wry, AppHandle>, app_handle: &AppHandle) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
  if cfg!(target_os = "linux") {
    return builder;
  }
  let theme = app_handle.get_webview_window("main").and_then(|window| window.theme().ok());
  builder.background_color(if theme == Some(tauri::Theme::Dark) { DARK_CANVAS } else { LIGHT_CANVAS })
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
