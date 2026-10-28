//! The windows of the app with Tauri (`WindowHost` of app-api, whose `app::windows` decides when a window shows): a
//! window is built hidden with its title bar (`with_title_bar`) on the canvas of the theme, on the monitor of the main
//! window, zoomed as its settings say, and shows once its webview presented the page as it is (`next_presentation`);
//! the folder dialog (`DialogHost`) and the restart (`RestartHost`).

use crate::api::tauri_error;
use crate::presentation::{Presentation, next_presentation};
use crate::title_bar::{prepare_title_bar, title_bar_script, with_title_bar};
use crate::zoom::{apply_zoom, sub_window_min_size};
use app_api::app::windows::{Awaited, Deadline, OpenSubWindowParams, Shown};
use app_api::transport::{DialogHost, Later, RestartHost, WindowHost, WindowId};
use domain_types::error::ApiError;
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::{info, warn};

/// How long a window that is due to show waits at most for the webview to present the page as it is (`presentation`):
/// in the check copy of the release app the hidden webview presented the settled rows 15 to 80 ms after the page said
/// them (and when the page was busy for 0.6 s after it, right after that); a webview that presents nothing in a second
/// shows the window as it is
const PRESENTATION_BUDGET: Duration = Duration::from_secs(1);

/// The canvas of the light and the dark theme (`--canvas` of `src/index.css` in sRGB): the colour behind the first frame
/// that the webview paints after `show()`, never white
const LIGHT_CANVAS: tauri::window::Color = tauri::window::Color(233, 234, 238, 255);
const DARK_CANVAS: tauri::window::Color = tauri::window::Color(38, 40, 44, 255);

/// The windows, the dialog and the restart of the app
pub struct TauriDesktop(pub AppHandle);

impl WindowHost for TauriDesktop {
  fn is_open(&self, label: &str) -> bool {
    self.0.get_webview_window(label).is_some()
  }

  fn reopen(&self, _opener: &WindowId, params: &OpenSubWindowParams, update: Option<&str>, front: bool) -> Result<(), ApiError> {
    let Some(window) = self.0.get_webview_window(&params.window_id) else {
      return Ok(());
    };
    // titled for what it shows now
    window.set_title(&params.title).map_err(tauri_error)?;
    if let Some(update) = update {
      window.eval(update).map_err(tauri_error)?;
    }
    if front {
      window.show().map_err(tauri_error)?;
      window.unminimize().map_err(tauri_error)?;
      window.set_focus().map_err(tauri_error)?;
    }
    Ok(())
  }

  /// Its page draws the title bar, as the main window's does
  fn build(&self, _opener: &WindowId, params: &OpenSubWindowParams, page_script: &str, zoom: f64) -> Result<(), ApiError> {
    // how the page draws the title bar, before the page's own scripts
    let init_script = format!("{page_script} {}", title_bar_script());
    let (min_width, min_height) = sub_window_min_size(&params.window_id, zoom);
    let width = params.width.unwrap_or(1400.0).max(min_width);
    let height = params.height.unwrap_or(900.0).max(min_height);
    let builder = WebviewWindowBuilder::new(&self.0, &params.window_id, WebviewUrl::App(params.url.clone().into()))
      .title(&params.title)
      .inner_size(width, height)
      .min_inner_size(min_width, min_height)
      .visible(false)
      .resizable(true)
      .initialization_script(&init_script)
      // the native file drop handler would swallow HTML5 drag and drop (on Windows)
      .disable_drag_drop_handler();
    let builder = match centered_on_main_monitor(&self.0, width, height) {
      Some((x, y)) => builder.position(x, y),
      None => builder.center(),
    };
    let window = with_title_bar(with_canvas(builder, &self.0)).build().map_err(tauri_error)?;
    apply_zoom(&window, zoom);
    // the empty GTK titlebar of a custom title bar on Linux, before the page paints and the window shows
    prepare_title_bar(&window);
    Ok(())
  }

  fn show(&self, label: &str, shown: Shown) -> Later<'_, ()> {
    Box::pin(show_window(self.0.clone(), label.to_owned(), shown))
  }
}

impl DialogHost for TauriDesktop {
  fn pick_folder(&self, title: &str) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    self.0.dialog().file().set_title(title).blocking_pick_folder().map(|folder| folder.to_string())
  }
}

impl RestartHost for TauriDesktop {
  fn restart(&self) -> ApiError {
    self.0.restart()
  }
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
