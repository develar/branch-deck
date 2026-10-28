//! The commands of app-api as Tauri commands: one `#[tauri::command]` wrapper per entry of
//! `app_api::for_each_command!` (`generated`), with the docs and the argument names of the entry, which the bindings
//! show (`specta`), and the `AppContext` the app manages for the body; the call of a window is its `Webview`, the channel
//! of a command its Tauri channel (`ChannelSink`). `lib.rs` registers them with the handwritten commands that have no
//! entry yet.

use app_api::transport::{AppEvents, EventSink, SinkGone};
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::settings::SettingsChange;
use std::sync::Arc;
use tauri::AppHandle;
use tauri::ipc::{Channel, IpcResponse};

/// A failure of Tauri itself (a window, an event, the store of the settings): `internal` with its words
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(tauri_error)`")]
pub fn tauri_error(error: tauri::Error) -> ApiError {
  ApiError::internal(error.to_string())
}

/// The writer of the settings, as a handwritten command takes it to resolve the prefix (`PrefixInputs::new`)
pub type Settings<'a> = tauri::State<'a, Arc<SettingsService>>;

/// The memo of the git prefixes, as a handwritten command takes it to resolve the prefix (`PrefixInputs::new`)
pub type Prefixes<'a> = tauri::State<'a, BranchPrefixes>;

/// A Tauri channel of the page as the channel of a command of app-api: the patches of a subscription, the events of an
/// operation. A channel of a page that went away never fails its send, so a window's subscription ends with its page
/// (`AppContext::forget_page`)
pub struct ChannelSink<T: IpcResponse>(pub Channel<T>);

impl<T: IpcResponse + Send + Sync> EventSink<T> for ChannelSink<T> {
  fn send(&self, event: T) -> Result<(), SinkGone> {
    self.0.send(event).map_err(|_gone| SinkGone)
  }
}

/// The events of the app to its windows
pub struct TauriEvents(pub AppHandle);

impl AppEvents for TauriEvents {
  fn settings_changed(&self, change: &SettingsChange) {
    crate::settings::emit_change(&self.0, change);
  }
}

/// The callback of `for_each_command!`: a wrapper of each command
macro_rules! tauri_commands {
  ($($(#[doc = $doc:literal])* $class:ident $kind:ident $name:ident($($arg:ident: $type:ty),*) $([channel $channel:ident: $event:ty $(, $open:ident)?])? $(-> $output:ty)? = $body:path;)*) => {
    $(tauri_command!($(#[doc = $doc])* $kind $name($($arg: $type),*) [$($channel: $event)?] [$($output)?] = $body);)*
  };
}

/// The wrapper of one command by its kind: `json` async with its `Result`, `plain` on the main thread with its value,
/// `raw` async with the bytes as a `tauri::ipc::Response`, which the typed bindings cannot say (`raw_invoke_handler`);
/// `session` async in the frame of app-api (`Command::call`) with the call of its webview and its channel, `now` on
/// the main thread with the call of its webview
macro_rules! tauri_command {
  ($(#[doc = $doc:literal])* json $name:ident($($arg:ident: $type:ty),*) [] [$output:ty] = $body:path) => {
    $(#[doc = $doc])*
    #[tauri::command]
    #[specta::specta]
    pub async fn $name(ctx: tauri::State<'_, app_api::context::AppContext>, $($arg: $type),*) -> Result<$output, domain_types::error::ApiError> {
      $body(&ctx, $($arg),*).await
    }
  };
  ($(#[doc = $doc:literal])* plain $name:ident($($arg:ident: $type:ty),*) [] [$output:ty] = $body:path) => {
    $(#[doc = $doc])*
    #[tauri::command]
    #[specta::specta]
    pub fn $name($($arg: $type),*) -> $output {
      $body($($arg),*)
    }
  };
  ($(#[doc = $doc:literal])* raw $name:ident($($arg:ident: $type:ty),*) [] [$output:ty] = $body:path) => {
    $(#[doc = $doc])*
    #[tauri::command]
    pub async fn $name(ctx: tauri::State<'_, app_api::context::AppContext>, $($arg: $type),*) -> Result<tauri::ipc::Response, domain_types::error::ApiError> {
      $body(&ctx, $($arg),*).await.map(tauri::ipc::Response::new)
    }
  };
  ($(#[doc = $doc:literal])* session $name:ident($($arg:ident: $type:ty),*) [] [$output:ty] = $body:path) => {
    $(#[doc = $doc])*
    #[tauri::command]
    #[specta::specta]
    pub async fn $name(ctx: tauri::State<'_, app_api::context::AppContext>, webview: tauri::Webview, $($arg: $type),*) -> Result<$output, domain_types::error::ApiError> {
      let call = ctx.call(webview.label());
      let args = app_api::commands::$name::Args { $($arg),* };
      <app_api::commands::$name::Command as app_api::registry::Command>::call(&ctx, &call, args, ()).await
    }
  };
  ($(#[doc = $doc:literal])* session $name:ident($($arg:ident: $type:ty),*) [$channel:ident: $event:ty] [$output:ty] = $body:path) => {
    $(#[doc = $doc])*
    #[tauri::command]
    #[specta::specta]
    pub async fn $name(
      ctx: tauri::State<'_, app_api::context::AppContext>,
      webview: tauri::Webview,
      $($arg: $type,)*
      $channel: tauri::ipc::Channel<$event>,
    ) -> Result<$output, domain_types::error::ApiError> {
      let call = ctx.call(webview.label());
      let args = app_api::commands::$name::Args { $($arg),* };
      let sink: app_api::transport::Sink<$event> = std::sync::Arc::new($crate::api::ChannelSink($channel));
      <app_api::commands::$name::Command as app_api::registry::Command>::call(&ctx, &call, args, sink).await
    }
  };
  ($(#[doc = $doc:literal])* now $name:ident($($arg:ident: $type:ty),*) [] [$output:ty] = $body:path) => {
    $(#[doc = $doc])*
    #[tauri::command]
    #[specta::specta]
    pub fn $name(ctx: tauri::State<'_, app_api::context::AppContext>, webview: tauri::Webview, $($arg: $type),*) -> $output {
      let call = ctx.call(webview.label());
      $body(&ctx, &call, $($arg),*)
    }
  };
  // no output: `void` in the bindings, which an explicit `-> ()` would type `null`
  ($(#[doc = $doc:literal])* now $name:ident($($arg:ident: $type:ty),*) [] [] = $body:path) => {
    $(#[doc = $doc])*
    #[tauri::command]
    #[specta::specta]
    pub fn $name(ctx: tauri::State<'_, app_api::context::AppContext>, webview: tauri::Webview, $($arg: $type),*) {
      let call = ctx.call(webview.label());
      $body(&ctx, &call, $($arg),*);
    }
  };
}

/// The wrappers of the commands of app-api, by the names of the commands
pub mod generated {
  app_api::for_each_command!(tauri_commands);
}
