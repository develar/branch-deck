//! The warm start of the app (openspec/specs/repository/onboarding/spec.md, "Before the first sync";
//! docs/design/rust-domain-authority.md, "s13-warm-start"): before the main window is built, the seed of the session of
//! the selected project (`app_api::app::warm_start::seed`, the view of its frame) goes into the init script of the
//! window as `window.__BD_MODEL__`, beside `__TAURI_STORE__`. The page shows its rows on its first frame and subscribes
//! as the window that holds the seed (`subscribe_view` with `have`): it gets the patch from the seed to the view of the
//! session, whose first sync runs (a project synced before) while the webview loads.
//!
//! A reload runs the same script: the page installs that seed again, and the session, which keeps it while it lives,
//! catches the window up from it; one opened since (another epoch) answers its snapshot, whose run the page doesn't take
//! for one that ended. The page ends the first sync in that catch-up only while no page of the window saw it end
//! (`src/model/store.ts`, in `sessionStorage`): a channel to a page that went away takes its patches without a failure,
//! so only the page knows what it got.

use app_api::app::warm_start::seed;
use app_api::context::AppContext;
use domain_types::session::CachedModel;
use tracing::{debug, instrument};

/// The part of the init script of the main window that hands its page the seed of the session of `repository_path`
/// (`window.__BD_MODEL__`, `app_api::app::warm_start::seed`); empty without one. Blocking: it waits for the key and for
/// the actor of the session to open
#[instrument(skip(ctx))]
pub fn model_script(ctx: &AppContext, repository_path: &str) -> String {
  tauri::async_runtime::block_on(seed(ctx, repository_path)).map_or_default(|model| script(&model))
}

/// The statement that sets `window.__BD_MODEL__` to `model`: the JSON in a string that `JSON.parse` reads, which a
/// JavaScript engine parses faster than the same object as a literal
#[must_use]
pub fn script(model: &CachedModel) -> String {
  match serde_json::to_string(model) {
    Ok(json) => format!("window.__BD_MODEL__ = JSON.parse('{}');", js_string(&json)),
    Err(error) => {
      debug!("The view of the frame is not encodable: {error}");
      String::new()
    }
  }
}

/// `text` as the content of a single-quoted JavaScript string
fn js_string(text: &str) -> String {
  let mut quoted = String::with_capacity(text.len() + text.len() / 16);
  for char in text.chars() {
    match char {
      '\\' => quoted.push_str("\\\\"),
      '\'' => quoted.push_str("\\'"),
      '\n' => quoted.push_str("\\n"),
      '\r' => quoted.push_str("\\r"),
      '\u{2028}' => quoted.push_str("\\u2028"),
      '\u{2029}' => quoted.push_str("\\u2029"),
      other => quoted.push(other),
    }
  }
  quoted
}
