//! The identifier the app is built with, as tauri-build reads it: the `identifier` of `src-tauri/tauri.conf.json`, under
//! the overlay of `TAURI_CONFIG` (the JSON a `tauri build --config …` hands every build script of the build). Shared by
//! the build script of this crate, which compiles it into `identity::APP_IDENTIFIER`, and its test.

/// The identifier of the configuration `base` (the text of `tauri.conf.json`) under the overlay `overlay` (the text of
/// `TAURI_CONFIG`); `None` when neither names one
pub(crate) fn identifier_of(base: &str, overlay: Option<&str>) -> Option<String> {
  let identifier = |text: &str| serde_json::from_str::<serde_json::Value>(text).ok()?.get("identifier")?.as_str().map(str::to_owned);
  overlay.and_then(identifier).or_else(|| identifier(base))
}
