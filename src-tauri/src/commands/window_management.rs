use crate::operation::{operation, tauri_error};
use crate::settings::init_script as settings_init_script;
use crate::title_bar::{prepare_title_bar, title_bar_script, with_title_bar};
use crate::zoom::{apply_zoom, sub_window_min_size};
use app_settings::service::SettingsService;
use app_settings::windows::window_changes_since;
use domain_types::error::ApiError;
use domain_types::settings::SettingsChange;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, State, Webview, WebviewUrl, WebviewWindowBuilder};
use tracing::{Instrument, info, instrument, warn};

/// How long a new window stays hidden at most when its page never loads (a broken page still shows)
const SHOW_FALLBACK: Duration = Duration::from_secs(3);
/// How long a loaded page of a sub-window may take to settle its content (`sub_window_ready`) before its window shows
/// with what it has: a slow git shows the tree and the headers, not nothing
const CONTENT_BUDGET: Duration = Duration::from_millis(300);
/// How long the loaded page of the main window may take to draw its first rows (`sub_window_ready`) before the window
/// shows its title row and the loading table: in the release app on the IntelliJ monorepo the rows of the first sync
/// came about 0.6 s after the page loaded, so a sync that takes longer is slow (a cold cache), not one to wait for
const MAIN_CONTENT_BUDGET: Duration = Duration::from_secs(1);

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

/// The data of an open of a sub-window, as `open_sub_window` got it: `__INIT_DATA__` and `__SUB_WINDOW_OPEN__` of its
/// page. Not the settings: a loaded page hears every change of them (`settings_changed`)
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubWindowUpdate {
  pub data: String,
  pub open: SubWindowOpen,
}

impl SubWindowUpdate {
  /// The script that hands the update to a loaded page
  fn script(&self) -> Result<String, ApiError> {
    let open = to_json(&self.open)?;
    Ok(format!(
      "
      window.__INIT_DATA__ = {};
      window.__SUB_WINDOW_OPEN__ = {open};
      window.dispatchEvent(new CustomEvent('init-data-updated'));
      ",
      self.data
    ))
  }
}

/// What the page of a sub-window gets when it loaded (`sub_window_loaded`)
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubWindowLoaded {
  /// The latest open that came while the page loaded, if any
  pub update: Option<SubWindowUpdate>,
  /// The changes of the settings after the snapshot the window was built with, which reached no listener of the page:
  /// the page applies them by their versions before its content settles (a zoom changed while a review window loaded)
  pub settings: Vec<SettingsChange>,
}

/// How the page of a window built hidden starts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltPage {
  /// A sub-window, from the snapshot of the settings at `settings_version`, which its init script has
  /// (`__TAURI_STORE_VERSION__`): its load takes the changes after it, and its content settles within `CONTENT_BUDGET`
  Sub { settings_version: u32 },
  /// The main window: its page catches up with the settings itself (`get_settings_changes`, which a reload asks again)
  /// and settles on the first rows of a sync, within `MAIN_CONTENT_BUDGET`
  Main,
}

impl BuiltPage {
  const fn content_budget(self) -> Duration {
    match self {
      Self::Sub { .. } => CONTENT_BUDGET,
      Self::Main => MAIN_CONTENT_BUDGET,
    }
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
  /// How its page starts: the settings it takes when it loads, the budget of its content
  page: BuiltPage,
  /// The page loaded (`sub_window_loaded`): an open reaches it by `eval`
  loaded: bool,
  /// The latest open that came while the page still loaded: an `eval` before the load is lost, the page takes it
  pending: Option<SubWindowUpdate>,
  /// The open whose settled content shows the window: the latest one
  showing_for: u32,
  shown: bool,
}

/// The windows built hidden, by label: every sub-window and the main window. A window shows once its page settled the
/// content of the latest open (no blank frame, never the subject of an earlier open), at most the content budget of its
/// page after the page loaded and `SHOW_FALLBACK` after the build; an open of a window whose page still loads is kept
/// for the page. The main window has one open, its build at the start of the app.
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

  /// A window of `label` is built for `open`, its page starting as `page` says: it loads with the data of the build
  pub fn built(&self, label: &str, open: SubWindowOpen, page: BuiltPage) {
    self.windows().insert(
      label.to_string(),
      SubWindowEntry {
        build: open.id,
        page,
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

  /// The page of `label` loaded: the open that came while it loaded, if any, and the changes of `settings` after the
  /// snapshot of the build that its window gets (none for the main window, which asks for them itself); with the build
  /// and the content budget its window waits for. A page loaded again (a reload) takes them again from the same
  /// snapshot, which its init script sets again
  pub fn loaded(&self, label: &str, settings: &SettingsService) -> (SubWindowLoaded, Option<(u32, Duration)>) {
    let mut windows = self.windows();
    let Some(entry) = windows.get_mut(label) else {
      return (
        SubWindowLoaded {
          update: None,
          settings: Vec::new(),
        },
        None,
      );
    };
    entry.loaded = true;
    let update = entry.pending.take();
    let (build, page) = (entry.build, entry.page);
    drop(windows);
    let settings = match page {
      BuiltPage::Sub { settings_version } => window_changes_since(settings, label, settings_version),
      BuiltPage::Main => Vec::new(),
    };
    (SubWindowLoaded { update, settings }, Some((build, page.content_budget())))
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
  /// The JSON of the window's data (`__INIT_DATA__`)
  pub data: String,
  /// When the opener asked (`Date.now()`)
  pub opened_at: f64,
}

/// Opens a sub-window, or brings the existing one of `window_id` to the front with new data. A new one is built hidden
/// on the monitor of the main window (the window state plugin restores where it was since) and shows once its page
/// settled its content (`sub_window_ready`). A new window starts from the settings as the writer has them now.
#[tauri::command]
#[instrument(skip(app_handle, state, settings, params), fields(window_id = %params.window_id, url = %params.url))]
#[specta::specta]
pub async fn open_sub_window(app_handle: AppHandle, state: State<'_, SubWindowState>, settings: State<'_, SettingsService>, params: OpenSubWindowParams) -> Result<(), ApiError> {
  operation(async move {
    let _open = state.open_lock.lock().await;
    let open = state.next_open(params.opened_at);
    if let Some(existing_window) = app_handle.get_webview_window(&params.window_id) {
      // titled for what it shows now
      existing_window.set_title(&params.title).map_err(tauri_error)?;
      let update = SubWindowUpdate { data: params.data, open };
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

    // the settings, the window data, the open and how the page draws the title bar, before the page's own scripts
    let (settings_script, settings_version) = settings_init_script(&settings, &params.window_id);
    let init_script = format!(
      "
      {settings_script}
      window.__INIT_DATA__ = {};
      window.__SUB_WINDOW_OPEN__ = {};
      {}
      ",
      params.data,
      to_json(&open)?,
      title_bar_script()
    );

    // Create new window: its page draws the title bar, as the main window's does
    let zoom = settings.app().zoom;
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
    // known before the page can load and ask for the changes after its snapshot
    state.built(&params.window_id, open, BuiltPage::Sub { settings_version });
    let window = with_title_bar(with_canvas(builder, &app_handle)).build().map_err(tauri_error)?;
    apply_zoom(&window, zoom);
    // the empty GTK titlebar of a custom title bar on Linux, before the page paints and the window shows
    prepare_title_bar(&window);
    show_when_settled(app_handle, params.window_id, open.id);
    Ok(())
  })
  .await
}

/// The page of the calling window loaded and listens to the settings: a sub-window gets the data of an open that came
/// while it loaded and the changes of the settings after the snapshot its window was built with, and the window shows
/// once the page settled its content, at most its content budget from now (`CONTENT_BUDGET`, the main window's
/// `MAIN_CONTENT_BUDGET`)
#[tauri::command]
#[instrument(skip(app_handle, webview, state, settings))]
#[specta::specta]
pub async fn sub_window_loaded(
  app_handle: AppHandle,
  webview: Webview,
  state: State<'_, SubWindowState>,
  settings: State<'_, SettingsService>,
) -> Result<SubWindowLoaded, ApiError> {
  operation(async move {
    let label = webview.label().to_string();
    let (loaded, build) = state.loaded(&label, &settings);
    if let Some((build, budget)) = build {
      show_after(app_handle, label, build, budget, None);
    }
    Ok(loaded)
  })
  .await
}

/// The page of the calling window settled the content of the open `open_id` (the main window: drew its first rows or
/// the state without rows): the window shows if that is the latest open and the window is still hidden
#[tauri::command]
#[instrument(skip(webview, state))]
#[specta::specta]
pub async fn sub_window_ready(webview: Webview, state: State<'_, SubWindowState>, open_id: u32) -> Result<(), ApiError> {
  operation(async move {
    if state.ready(webview.label(), open_id) {
      info!(label = webview.label(), open_id, "the page settled its content, showing the window");
      let window = webview.window();
      window.show().map_err(tauri_error)?;
      window.set_focus().map_err(tauri_error)?;
    }
    Ok(())
  })
  .await
}

/// Shows the window `build` of `label`, built hidden, once its page settled its content (`sub_window_ready`), at most
/// the content budget after its page loaded (`sub_window_loaded`) and `SHOW_FALLBACK` from now when its page never
/// loads. The main window as a sub-window: `prepare_title_bar` first, then this
pub fn show_when_settled(app_handle: AppHandle, label: String, build: u32) {
  show_after(app_handle, label, build, SHOW_FALLBACK, Some("the page of the window did not load, showing it"));
}

/// Shows the window `build` of `label` after `delay` unless it showed by then, logging `warning` if it does, else that
/// the budget showed it
fn show_after(app_handle: AppHandle, label: String, build: u32, delay: Duration, warning: Option<&'static str>) {
  tauri::async_runtime::spawn(
    async move {
      let state = app_handle.state::<SubWindowState>();
      show_when_budget_passes(&state, &label, build, delay, || {
        if let Some(warning) = warning {
          warn!(label, "{warning}");
        } else {
          info!(label, budget_ms = delay.as_millis(), "the page settled no content within its budget, showing the window");
        }
        if let Some(window) = app_handle.get_webview_window(&label)
          && let Err(e) = window.show().and_then(|()| window.set_focus())
        {
          warn!(label, error = %e, "cannot show the window");
        }
      })
      .await;
    }
    .in_current_span(),
  );
}

/// Waits `delay` on the backend's clock, then shows the window `build` of `label` with `show` unless it showed by then
/// (its page settled its content, `ready`): true when it showed it. Never a timer of the page, which `WebKit` throttles
/// while the window is hidden: a page that says nothing after its load still shows
pub async fn show_when_budget_passes(state: &SubWindowState, label: &str, build: u32, delay: Duration, show: impl FnOnce()) -> bool {
  tokio::time::sleep(delay).await;
  if !state.budget_passed(label, build) {
    return false;
  }
  show();
  true
}

/// The line of the main window's init script that names its open (`__SUB_WINDOW_OPEN__`, its build): its page settles
/// it when it drew its first rows (`markWindowContentReady`)
pub fn open_script(open: &SubWindowOpen) -> String {
  // two numbers: their JSON is never an error
  format!("window.__SUB_WINDOW_OPEN__ = {};", to_json(open).unwrap_or_else(|_| "undefined".to_owned()))
}

/// Now in ms since the epoch, as the page's `Date.now()`: when the main window was asked for (`SubWindowOpen::opened_at`)
pub fn epoch_millis() -> f64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0)
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
