//! The windows of the app (spec/repository/title-bar.spec.md, "The first frame"): every window is built hidden and shows
//! once its page settled the content of its latest open, else once the deadline of the stage its page reached passed
//! (`SubWindowState`, one per app); an open of a window whose page still loads is kept for the page. The commands:
//! `open_sub_window` (the build or the reopen of a sub-window), the stages its page says (`sub_window_loaded`,
//! `sub_window_drawn`, `sub_window_ready`) and the window manager's menu of a window (`show_window_menu`). What only the
//! host can do (build, title, hand a page its update, show) is its `WindowHost`; the deadlines run here, on the clock of
//! the backend (`show_when_settled`), never on a timer of the page, which `WebKit` throttles while the window is hidden.

use crate::context::AppContext;
use crate::operation::{operation, operation_now};
use crate::transport::Call;
use app_settings::service::SettingsService;
use app_settings::windows::{window_changes_since, window_snapshot};
use domain_types::error::ApiError;
use domain_types::settings::SettingsChange;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{Instrument, instrument};

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
  pub(crate) fn script(&self) -> Result<String, ApiError> {
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
/// webview presented the page as it is (`WindowHost::show`). An open of a window whose page still loads is kept for the page.
/// The main window has one open, its build at the start of the app.
#[derive(Default)]
pub struct SubWindowState {
  /// one open at a time: two opens of a label never both build it (`open_sub_window` is async, opens run in parallel).
  /// Held across the build alone, which awaits nothing
  open_lock: Mutex<()>,
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
#[instrument(skip_all, fields(window_id = %params.window_id, url = %params.url))]
pub async fn open_sub_window(ctx: &AppContext, call: &Call, params: OpenSubWindowParams) -> Result<(), ApiError> {
  operation(async move {
    let (state, host) = (&ctx.sub_windows, &ctx.desktop.windows);
    let _open = state.open_lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let open = state.next_open(params.opened_at);
    if host.is_open(&params.window_id) {
      let update = SubWindowUpdate { data: params.data.clone(), open };
      return match state.reopened(&params.window_id, update.clone()) {
        Reopen::Pending => host.reopen(&call.window, &params, None, false),
        Reopen::Hidden => host.reopen(&call.window, &params, Some(&update.script()?), false),
        Reopen::Shown => host.reopen(&call.window, &params, Some(&update.script()?), true),
      };
    }
    // the settings, the window data and the open, before the page's own scripts
    let (settings_script, settings_version) = settings_script(&ctx.settings, &params.window_id);
    let page_script = format!(
      "
      {settings_script}
      window.__INIT_DATA__ = {};
      window.__SUB_WINDOW_OPEN__ = {};
      ",
      params.data,
      to_json(&open)?
    );
    // known before the page can load and ask for the changes after its snapshot
    let deadline = state.built(&params.window_id, open, BuiltPage::Sub { settings_version });
    host.build(&call.window, &params, &page_script, ctx.settings.app().zoom)?;
    show_when_settled(ctx, params.window_id, deadline);
    Ok(())
  })
  .await
}

/// The page of the calling window loaded and listens to the settings: a sub-window gets the data of an open that came
/// while it loaded and the changes of the settings after the snapshot its window was built with. The window shows once
/// the page settled its content, at most `DRAW_FALLBACK` from now when the page draws no shell (`sub_window_drawn`)
#[instrument(skip_all, fields(window = %call.window))]
pub async fn sub_window_loaded(ctx: &AppContext, call: &Call) -> Result<SubWindowLoaded, ApiError> {
  operation(async move {
    let label = call.window.as_str();
    let (loaded, deadline) = ctx.sub_windows.loaded(label, &ctx.settings);
    if let Some(deadline) = deadline {
      show_when_settled(ctx, label.to_owned(), deadline);
    }
    Ok(loaded)
  })
  .await
}

/// The loaded page of the calling window committed its shell, the title row (the main window's with its loading table):
/// the window shows once the page settled its content, at most the content budget of the page from now
/// (`CONTENT_BUDGET`, the main window's `MAIN_CONTENT_BUDGET`), so a slow page shows its shell, never a bare canvas
#[instrument(skip_all, fields(window = %call.window))]
pub async fn sub_window_drawn(ctx: &AppContext, call: &Call) -> Result<(), ApiError> {
  operation(async move {
    if let Some(deadline) = ctx.sub_windows.drawn(call.window.as_str()) {
      show_when_settled(ctx, call.window.as_str().to_owned(), deadline);
    }
    Ok(())
  })
  .await
}

/// The page of the calling window settled the content of the open `open_id` (the main window: drew its first rows or
/// the state without rows): the window shows if that is the latest open and the window is still hidden, once its
/// webview presented that content (`show_window`)
#[instrument(skip_all, fields(window = %call.window, open_id = open_id))]
pub async fn sub_window_ready(ctx: &AppContext, call: &Call, open_id: u32) -> Result<(), ApiError> {
  operation(async move {
    if ctx.sub_windows.ready(call.window.as_str(), open_id) {
      let (host, label) = (Arc::clone(&ctx.desktop.windows), call.window.as_str().to_owned());
      ctx.runtime.spawn(async move { host.show(&label, Shown::Settled { open_id }).await }.in_current_span());
    }
    Ok(())
  })
  .await
}

/// The window manager's menu of the window (Move, Resize, Always on Top, Close…), as a right click on a native title bar
/// opens it: the page calls it for a right click on the empty parts of its title bar (`WindowTitleBar`). A sync command,
/// so it runs on the main thread, where GDK and Win32 menus live.
#[instrument(skip_all, fields(window = %call.window))]
pub fn show_window_menu(ctx: &AppContext, call: &Call) -> Result<(), ApiError> {
  operation_now(|| ctx.desktop.menu.show_window_menu(&call.window))
}

/// Shows the window of `label`, built hidden, once its page settled its content (`sub_window_ready`), else once
/// `deadline` passed without the stage after the one it was set for: each stage of the page (the build, the load, the
/// shell) sets the deadline of the next. The main window as a sub-window: the app builds it and calls this
pub fn show_when_settled(ctx: &AppContext, label: String, deadline: Deadline) {
  let (state, host) = (Arc::clone(&ctx.sub_windows), Arc::clone(&ctx.desktop.windows));
  ctx.runtime.spawn(
    async move {
      if wait_for_deadline(&state, &label, deadline).await {
        host.show(&label, Shown::Deadline(deadline)).await;
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
pub enum Shown {
  /// Its page settled the content of the open `open_id`
  Settled { open_id: u32 },
  /// A deadline passed: the window shows as it is
  Deadline(Deadline),
}

/// The part of the init script of the window `label` that hands it the settings (`window.__TAURI_STORE__`, the values
/// by key, and `window.__TAURI_STORE_VERSION__`, the version they were taken at), before the page's own scripts, and
/// the version of that snapshot, after which a sub-window's page takes the changes when it loads (`sub_window_loaded`).
/// The values are serialized JSON, so the script always parses, a settings file cut within a write too (its snapshot is
/// the defaults)
pub fn settings_script(settings: &SettingsService, label: &str) -> (String, u32) {
  let (version, values) = window_snapshot(settings, label);
  (format!("window.__TAURI_STORE__ = {values}; window.__TAURI_STORE_VERSION__ = {version};"), version)
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
