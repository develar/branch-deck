//! The commands of app-api as Tauri commands: one `#[tauri::command]` wrapper per entry of
//! `app_api::for_each_command!` (`generated`), with the docs and the argument names of the entry, which the bindings
//! show (`specta`), and the `AppContext` the app manages for the body. `lib.rs` registers them with the handwritten
//! commands that have no entry yet.

use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use std::sync::Arc;

/// A failure of Tauri itself (a window, an event, the store of the settings): `internal` with its words
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(tauri_error)`")]
pub fn tauri_error(error: tauri::Error) -> ApiError {
  ApiError::internal(error.to_string())
}

/// The writer of the settings, as a handwritten command takes it to resolve the prefix (`PrefixInputs::new`)
pub type Settings<'a> = tauri::State<'a, Arc<SettingsService>>;

/// The memo of the git prefixes, as a handwritten command takes it to resolve the prefix (`PrefixInputs::new`)
pub type Prefixes<'a> = tauri::State<'a, BranchPrefixes>;

/// The callback of `for_each_command!`: a wrapper of each command
macro_rules! tauri_commands {
  ($($(#[doc = $doc:literal])* $class:ident $kind:ident $name:ident($($arg:ident: $type:ty),*) -> $output:ty = $body:path;)*) => {
    $(tauri_command!($(#[doc = $doc])* $kind $name($($arg: $type),*) -> $output = $body);)*
  };
}

/// The wrapper of one command by its kind: `json` async with its `Result`, `plain` on the main thread with its value,
/// `raw` async with the bytes as a `tauri::ipc::Response`, which the typed bindings cannot say (`raw_invoke_handler`)
macro_rules! tauri_command {
  ($(#[doc = $doc:literal])* json $name:ident($($arg:ident: $type:ty),*) -> $output:ty = $body:path) => {
    $(#[doc = $doc])*
    #[tauri::command]
    #[specta::specta]
    pub async fn $name(ctx: tauri::State<'_, app_api::context::AppContext>, $($arg: $type),*) -> Result<$output, domain_types::error::ApiError> {
      $body(&ctx, $($arg),*).await
    }
  };
  ($(#[doc = $doc:literal])* plain $name:ident($($arg:ident: $type:ty),*) -> $output:ty = $body:path) => {
    $(#[doc = $doc])*
    #[tauri::command]
    #[specta::specta]
    pub fn $name($($arg: $type),*) -> $output {
      $body($($arg),*)
    }
  };
  ($(#[doc = $doc:literal])* raw $name:ident($($arg:ident: $type:ty),*) -> $output:ty = $body:path) => {
    $(#[doc = $doc])*
    #[tauri::command]
    pub async fn $name(ctx: tauri::State<'_, app_api::context::AppContext>, $($arg: $type),*) -> Result<tauri::ipc::Response, domain_types::error::ApiError> {
      $body(&ctx, $($arg),*).await.map(tauri::ipc::Response::new)
    }
  };
}

/// The wrappers of the commands of app-api, by the names of the commands
pub mod generated {
  app_api::for_each_command!(tauri_commands);
}
