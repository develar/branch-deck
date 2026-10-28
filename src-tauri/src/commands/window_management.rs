use crate::api::{Settings, tauri_error};
use crate::presentation::{Presentation, next_presentation};
use crate::settings::init_script as settings_init_script;
use crate::title_bar::{prepare_title_bar, title_bar_script, with_title_bar};
use crate::zoom::{apply_zoom, sub_window_min_size};
use app_api::operation::operation;
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
/// How long a loaded page may take to draw its shell (`sub_window_drawn`, the title row) before its window shows as it
/// is: a page that loaded and draws nothing (its script failed) still shows
const DRAW_FALLBACK: Duration = Duration::from_secs(3);
/// How long the page of a sub-window may take from its shell to its content (`sub_window_ready`) before its window
/// shows with what it has: a slow git shows the tree and the headers, not nothing
const CONTENT_BUDGET: Duration = Duration::from_millis(300);
/// How long the page of the main window may take from its shell to its first rows (`sub_window_ready`) before the
/// window shows its title row and the loading table: in the release app on the IntelliJ monorepo the rows of the first
/// sync came about 0.6 s after the page loaded, so a sync that takes longer is slow (a cold cache), not one to wait for
const MAIN_CONTENT_BUDGET: Duration = Duration::from_secs(1);
/// How long a window that is due to show waits at most for the webview to present the page as it is (`presentation`):
/// in the check copy of the release app the hidden webview presented the settled rows 15 to 80 ms after the page said
/// them (and when the page was busy for 0.6 s after it, right after that); a webview that presents nothing in a second
/// shows the window as it is
const PRESENTATION_BUDGET: Duration = Duration::from_secs(1);

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
  /// after its shell
  Sub { settings_version: u32 },
  /// The main window: its page catches up with the settings itself (`get_settings_changes`, which a reload asks again)
  /// and settles on the first rows of a sync, within `MAIN_CONTENT_BUDGET` after its shell
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

/// How far the page of a window built hidden got: each stage has a deadline, after which the window shows as it is
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
  /// The page has not loaded yet (`sub_window_loaded`)
  Built,
  /// The page loaded and listens to the settings: an open reaches it by `eval`; its shell is not drawn yet
  Loaded,
  /// The page committed its shell (`sub_window_drawn`): the title row, the main window's with its loading table
  Drawn,
}

/// What a deadline of a window built hidden waits for: the window shows without it once the deadline passed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Awaited {
  /// The load of the page, `SHOW_FALLBACK` after the build: a broken page still shows
  Load,
  /// The shell of a loaded page, `DRAW_FALLBACK` after the load: a page that draws nothing still shows
  Shell,
  /// The content of a page that drew its shell, its content budget after the shell: a slow git shows the shell
  Content,
}

/// A deadline of a window built hidden: when it passes, the window shows as it is, unless it showed or its page got a
/// stage further meanwhile (that stage has its own deadline)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
  /// The open that built the window: a deadline of an earlier window of the label leaves this one alone
  build: u32,
  /// The stages the page had reached when the deadline was set
  step: u32,
  pub awaited: Awaited,
  pub after: Duration,
}

struct SubWindowEntry {
  build: u32,
  /// How its page starts: the settings it takes when it loads, the budget of its content
  page: BuiltPage,
  stage: Stage,
  /// Counts the stages the page reached, a reload's load again: a deadline holds only while it is the same
  step: u32,
  /// The latest open that came while the page still loaded: an `eval` before the load is lost, the page takes it
  pending: Option<SubWindowUpdate>,
  /// The open whose settled content shows the window: the latest one
  showing_for: u32,
  shown: bool,
}

impl SubWindowEntry {
  /// The page reached the stage `stage`: the deadline after which the window shows without the next one
  const fn reach(&mut self, stage: Stage) -> Deadline {
    self.stage = stage;
    self.step += 1;
    let (awaited, after) = match stage {
      Stage::Built => (Awaited::Load, SHOW_FALLBACK),
      Stage::Loaded => (Awaited::Shell, DRAW_FALLBACK),
      Stage::Drawn => (Awaited::Content, self.page.content_budget()),
    };
    Deadline {
      build: self.build,
      step: self.step,
      awaited,
      after,
    }
  }
}

/// The windows built hidden, by label: every sub-window and the main window. A window shows once its page settled the
/// content of the latest open (no blank frame, never the subject of an earlier open), else once the deadline of the
/// stage its page reached passed: `SHOW_FALLBACK` after the build for a page that never loads, `DRAW_FALLBACK` after the
/// load for one that never draws its shell, the content budget of the page after its shell; and then only once its
/// webview presented the page as it is (`show_window`). An open of a window whose page still loads is kept for the page.
/// The main window has one open, its build at the start of the app.
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

  /// A window of `label` is built for `open`, its page starting as `page` says: it loads with the data of the build. The
  /// deadline of its load
  pub fn built(&self, label: &str, open: SubWindowOpen, page: BuiltPage) -> Deadline {
    let mut entry = SubWindowEntry {
      build: open.id,
      page,
      stage: Stage::Built,
      step: 0,
      pending: None,
      showing_for: open.id,
      shown: false,
    };
    let deadline = entry.reach(Stage::Built);
    self.windows().insert(label.to_string(), entry);
    deadline
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
    if entry.stage == Stage::Built {
      entry.pending = Some(update);
      Reopen::Pending
    } else {
      Reopen::Hidden
    }
  }

  /// The page of `label` loaded: the open that came while it loaded, if any, and the changes of `settings` after the
  /// snapshot of the build that its window gets (none for the main window, which asks for them itself); with the
  /// deadline of its shell. A page loaded again (a reload) takes them again from the same snapshot, which its init script
  /// sets again
  pub fn loaded(&self, label: &str, settings: &SettingsService) -> (SubWindowLoaded, Option<Deadline>) {
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
    let deadline = entry.reach(Stage::Loaded);
    let update = entry.pending.take();
    let page = entry.page;
    drop(windows);
    let settings = match page {
      BuiltPage::Sub { settings_version } => window_changes_since(settings, label, settings_version),
      BuiltPage::Main => Vec::new(),
    };
    (SubWindowLoaded { update, settings }, Some(deadline))
  }

  /// The loaded page of `label` committed its shell, its title row: the deadline of its content, the content budget of
  /// the page from now. None for a page that drew it before (a second title bar, a remount) or did not say its load
  pub fn drawn(&self, label: &str) -> Option<Deadline> {
    match self.windows().get_mut(label) {
      Some(entry) if entry.stage == Stage::Loaded => Some(entry.reach(Stage::Drawn)),
      _ => None,
    }
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

  /// The deadline `deadline` of the window of `label` passed: true when it shows now as it is (still hidden, the same
  /// window, its page no stage further)
  pub fn deadline_passed(&self, label: &str, deadline: &Deadline) -> bool {
    match self.windows().get_mut(label) {
      Some(entry) if entry.build == deadline.build && entry.step == deadline.step && !entry.shown => {
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
pub async fn open_sub_window(app_handle: AppHandle, state: State<'_, SubWindowState>, settings: Settings<'_>, params: OpenSubWindowParams) -> Result<(), ApiError> {
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
    let deadline = state.built(&params.window_id, open, BuiltPage::Sub { settings_version });
    let window = with_title_bar(with_canvas(builder, &app_handle)).build().map_err(tauri_error)?;
    apply_zoom(&window, zoom);
    // the empty GTK titlebar of a custom title bar on Linux, before the page paints and the window shows
    prepare_title_bar(&window);
    show_when_settled(app_handle, params.window_id, deadline);
    Ok(())
  })
  .await
}

/// The page of the calling window loaded and listens to the settings: a sub-window gets the data of an open that came
/// while it loaded and the changes of the settings after the snapshot its window was built with. The window shows once
/// the page settled its content, at most `DRAW_FALLBACK` from now when the page draws no shell (`sub_window_drawn`)
#[tauri::command]
#[instrument(skip(app_handle, webview, state, settings))]
#[specta::specta]
pub async fn sub_window_loaded(app_handle: AppHandle, webview: Webview, state: State<'_, SubWindowState>, settings: Settings<'_>) -> Result<SubWindowLoaded, ApiError> {
  operation(async move {
    let label = webview.label().to_string();
    let (loaded, deadline) = state.loaded(&label, &settings);
    if let Some(deadline) = deadline {
      show_when_settled(app_handle, label, deadline);
    }
    Ok(loaded)
  })
  .await
}

/// The loaded page of the calling window committed its shell, the title row (the main window's with its loading table):
/// the window shows once the page settled its content, at most the content budget of the page from now
/// (`CONTENT_BUDGET`, the main window's `MAIN_CONTENT_BUDGET`), so a slow page shows its shell, never a bare canvas
#[tauri::command]
#[instrument(skip(app_handle, webview, state))]
#[specta::specta]
pub async fn sub_window_drawn(app_handle: AppHandle, webview: Webview, state: State<'_, SubWindowState>) -> Result<(), ApiError> {
  operation(async move {
    let label = webview.label().to_string();
    if let Some(deadline) = state.drawn(&label) {
      show_when_settled(app_handle, label, deadline);
    }
    Ok(())
  })
  .await
}

/// The page of the calling window settled the content of the open `open_id` (the main window: drew its first rows or
/// the state without rows): the window shows if that is the latest open and the window is still hidden, once its
/// webview presented that content (`show_window`)
#[tauri::command]
#[instrument(skip(app_handle, webview, state))]
#[specta::specta]
pub async fn sub_window_ready(app_handle: AppHandle, webview: Webview, state: State<'_, SubWindowState>, open_id: u32) -> Result<(), ApiError> {
  operation(async move {
    if state.ready(webview.label(), open_id) {
      tauri::async_runtime::spawn(show_window(app_handle, webview.label().to_owned(), Shown::Settled { open_id }).in_current_span());
    }
    Ok(())
  })
  .await
}

/// Shows the window of `label`, built hidden, once its page settled its content (`sub_window_ready`), else once
/// `deadline` passed without the stage after the one it was set for: each stage of the page (the build, the load, the
/// shell) sets the deadline of the next. The main window as a sub-window: `prepare_title_bar` first, then this
pub fn show_when_settled(app_handle: AppHandle, label: String, deadline: Deadline) {
  tauri::async_runtime::spawn(
    async move {
      if wait_for_deadline(&app_handle.state::<SubWindowState>(), &label, deadline).await {
        show_window(app_handle, label, Shown::Deadline(deadline)).await;
      }
    }
    .in_current_span(),
  );
}

/// Waits for `deadline` on the backend's clock: true when the window of `label` is due to show then, as it is (it did
/// not show by then, its page got no stage further). Never a timer of the page, which `WebKit` throttles while the
/// window is hidden: a page that says nothing after its load still shows
pub async fn wait_for_deadline(state: &SubWindowState, label: &str, deadline: Deadline) -> bool {
  tokio::time::sleep(deadline.after).await;
  state.deadline_passed(label, &deadline)
}

/// What shows a window built hidden
#[derive(Debug, Clone, Copy)]
enum Shown {
  /// Its page settled the content of the open `open_id`
  Settled { open_id: u32 },
  /// A deadline passed: the window shows as it is
  Deadline(Deadline),
}

/// Shows and focuses the window of `label`, due to show for `shown`, once its webview presented the page as it is now
/// (`next_presentation`, at most `PRESENTATION_BUDGET`): its first frame is the content the page settled, or the shell
/// it drew, never the layers of an earlier frame. A page that never loaded shows at once: nothing of it is to come
async fn show_window(app_handle: AppHandle, label: String, shown: Shown) {
  let Some(window) = app_handle.get_webview_window(&label) else {
    return;
  };
  let started = tokio::time::Instant::now();
  let presentation = match shown {
    Shown::Deadline(Deadline { awaited: Awaited::Load, .. }) => Presentation::Unknown,
    _ => next_presentation(&window, PRESENTATION_BUDGET).await,
  };
  let presented_ms = started.elapsed().as_millis();
  match shown {
    Shown::Settled { open_id } => info!(label, open_id, presented_ms, "the page settled its content, showing the window"),
    Shown::Deadline(Deadline {
      awaited: Awaited::Content, after, ..
    }) => {
      info!(
        label,
        budget_ms = after.as_millis(),
        presented_ms,
        "the page settled no content within its budget, showing the window"
      );
    }
    Shown::Deadline(Deadline { awaited: Awaited::Shell, .. }) => warn!(label, presented_ms, "the page loaded and drew no shell, showing the window"),
    Shown::Deadline(Deadline { awaited: Awaited::Load, .. }) => warn!(label, "the page of the window did not load, showing it"),
  }
  if presentation == Presentation::TimedOut {
    warn!(
      label,
      budget_ms = PRESENTATION_BUDGET.as_millis(),
      "the webview presented no frame within its budget, the window shows as it is"
    );
  }
  if let Err(e) = window.show().and_then(|()| window.set_focus()) {
    warn!(label, error = %e, "cannot show the window");
  }
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
