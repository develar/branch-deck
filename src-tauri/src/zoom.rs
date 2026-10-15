use tauri::WebviewWindow;

/// The header fits one line from 960 px (`ConfigurationHeader`); narrower, its controls would overlap. It keeps its
/// size at any zoom, as window chrome does.
pub const MIN_WINDOW_WIDTH: f64 = 960.0;
pub const MIN_WINDOW_HEIGHT: f64 = 600.0;
/// The islands below the header need 768 CSS px (`md`); a zoom above 125% needs a wider window for them.
/// Keep in sync with `minWindowWidth` in `src/lib/zoom.ts`.
const MIN_ISLANDS_WIDTH: f64 = 768.0;

/// The zoom level of the pages, saved as `appSettings.zoom` (`src/stores/settings/appSettings.ts`, absent at 100%).
/// `store_json` is the content of `settings.json` that the window gets as `window.__TAURI_STORE__`.
pub fn stored_zoom(store_json: &str) -> f64 {
  serde_json::from_str::<serde_json::Value>(store_json)
    .ok()
    .and_then(|store| store.get("appSettings")?.get("zoom")?.as_f64())
    .filter(|zoom| zoom.is_finite() && *zoom > 0.0)
    .unwrap_or(1.0)
}

/// The minimum width of the main window at `zoom`
pub fn min_window_width(zoom: f64) -> f64 {
  MIN_WINDOW_WIDTH.max(MIN_ISLANDS_WIDTH * zoom)
}

/// Zooms a new window before its page paints; the page keeps it in sync afterwards (`useZoom`)
pub fn apply_zoom(window: &WebviewWindow, zoom: f64) {
  if (zoom - 1.0).abs() > f64::EPSILON
    && let Err(e) = window.set_zoom(zoom)
  {
    tracing::error!(error = ?e, zoom, window = window.label(), "cannot zoom the window");
  }
}
