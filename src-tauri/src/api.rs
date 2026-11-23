//! The commands of app-api in the Tauri shell (docs/design/backend-protocol.md, "The pages of the app"): every `invoke`
//! of a page goes through the in-process connection of `backend-host` (`backend_host::page::Pages`), as the root
//! command of its window (the label of its webview), its channel the Tauri channel the page gave, and the answer goes
//! to the page as the text of its JSON (`InvokeResponseBody::Json`), the bytes of a raw command as they are. So the
//! desktop windows and the bridge of the IDE share one host, one session per repository and one set of events, and the
//! page keeps its bindings.
//!
//! The typed bindings are still exported from one `#[tauri::command]` declaration per entry of
//! `app_api::for_each_command!` (`generated`, `typed_commands`), with the docs and the argument names of the entry,
//! which the bindings show (`specta`); the shell never calls them (`invoke`).

use app_api::transport::{AppEvents, WindowId};
use backend_host::page::{Answer, Failed, PageChannel, Pages};
use domain_types::error::ApiError;
use std::str::FromStr;
use std::sync::Arc;
use tauri::ipc::{Channel, Invoke, InvokeBody, InvokeError, InvokeResponseBody, JavaScriptChannelId};
use tauri::{AppHandle, Emitter, Manager};
use tracing::warn;

/// A failure of Tauri itself (a window, an event, the store of the settings): `internal` with its words
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(tauri_error)`")]
pub fn tauri_error(error: tauri::Error) -> ApiError {
  ApiError::internal(error.to_string())
}

/// A Tauri channel of a page as the channel of a command: the patches of a subscription, the events of a call, each the
/// text of its JSON. A channel of a page that went away never fails its send, so a window's subscription ends with its
/// page (`pageLoaded`)
struct TauriChannel(Channel<InvokeResponseBody>);

impl PageChannel for TauriChannel {
  fn send(&self, json: String) -> bool {
    self.0.send(InvokeResponseBody::Json(json)).is_ok()
  }
}

/// The handler of every command of the pages: the command of the registry by its name, through the in-process
/// connection of the host (`Pages`); false for a name no command has, which Tauri answers as a command it lacks
pub fn invoke(invoke: Invoke<tauri::Wry>) -> bool {
  let command = invoke.message.command();
  let Some(spec) = app_api::dispatch::spec(command) else {
    return false;
  };
  let webview = invoke.message.webview();
  let Some(pages) = webview.try_state::<Arc<Pages>>() else {
    invoke.resolver.invoke_error(rejection(&ApiError::internal("The backend of the app is not started yet")));
    return true;
  };
  let empty = serde_json::Value::Null;
  let args = match invoke.message.payload() {
    InvokeBody::Json(args) => args,
    InvokeBody::Raw(_) => &empty,
  };
  let channel = spec
    .channel
    .as_ref()
    .and_then(|spec| args.get(spec.name))
    .and_then(serde_json::Value::as_str)
    .and_then(|id| {
      let id = JavaScriptChannelId::from_str(id).ok()?;
      Some(Arc::new(TauriChannel(id.channel_on(webview.clone()))) as Arc<dyn PageChannel>)
    });
  let reply = pages.invoke(webview.label(), command, args, channel);
  invoke.resolver.respond_async_serialized(async move {
    match reply.await {
      Ok(Answer::Json(json)) => Ok(InvokeResponseBody::Json(json)),
      Ok(Answer::Bytes(bytes)) => Ok(InvokeResponseBody::Raw(bytes)),
      Err(Failed::Error(json)) => Err(InvokeError(serde_json::from_str(&json).unwrap_or(serde_json::Value::String(json)))),
      Err(Failed::Panicked) => Err(rejection(&ApiError::internal("The command panicked"))),
    }
  });
  true
}

/// The rejection of a command with `error`, as the bindings read it
fn rejection(error: &ApiError) -> InvokeError {
  InvokeError(serde_json::to_value(error).unwrap_or_default())
}

/// The page of the webview `label` loads again: what its page held ends
pub fn page_loaded(app: &AppHandle, label: &str) {
  if let Some(pages) = app.try_state::<Arc<Pages>>() {
    pages.page_loaded(label);
  }
}

/// The window `label` closed: it is forgotten
pub fn window_closed(app: &AppHandle, label: &str) {
  if let Some(pages) = app.try_state::<Arc<Pages>>() {
    pages.window_closed(label);
  }
}

/// The events of the app to its windows (`app_api::events`), by Tauri's `Emitter`: an event to every window reaches the
/// listeners of every webview, one to a window those of the webviews of its label
pub struct TauriEvents(pub AppHandle);

impl AppEvents for TauriEvents {
  /// Every change of the settings comes this way, so the native menu follows it here too (`follow_settings_change`)
  fn emit(&self, event: &str, payload: serde_json::Value) {
    crate::menu::follow_settings_change(&self.0, event, &payload);
    if let Err(error) = self.0.emit(event, payload) {
      warn!(%error, event, "cannot send the event to the windows");
    }
  }

  fn emit_to(&self, window: &WindowId, event: &str, payload: serde_json::Value) {
    crate::menu::follow_settings_change(&self.0, event, &payload);
    if let Err(error) = self.0.emit_to(window.as_str(), event, payload) {
      warn!(%error, event, %window, "cannot send the event to the window");
    }
  }
}

/// The events the app tells beside a command (the sessions, the journals, a change of the settings another process
/// saved, a failed save) to every connection of the backend, through the events of the context once it exists
/// (`Connections`: the app's own windows by Tauri, and each client of the host); to the app's windows alone before, when
/// no client can be connected yet
pub struct ContextEvents {
  context: std::sync::OnceLock<std::sync::Weak<app_api::context::AppContext>>,
  app: TauriEvents,
}

impl ContextEvents {
  pub const fn new(app: AppHandle) -> Self {
    Self {
      context: std::sync::OnceLock::new(),
      app: TauriEvents(app),
    }
  }

  /// The context exists: the events go through its connections from now on
  pub fn bind(&self, ctx: &Arc<app_api::context::AppContext>) {
    let _ = self.context.set(Arc::downgrade(ctx));
  }

  fn context(&self) -> Option<Arc<app_api::context::AppContext>> {
    self.context.get().and_then(std::sync::Weak::upgrade)
  }
}

impl AppEvents for ContextEvents {
  fn emit(&self, event: &str, payload: serde_json::Value) {
    match self.context() {
      Some(ctx) => ctx.events.emit(event, payload),
      None => self.app.emit(event, payload),
    }
  }

  fn emit_to(&self, window: &WindowId, event: &str, payload: serde_json::Value) {
    match self.context() {
      Some(ctx) => ctx.events.emit_to(window, event, payload),
      None => self.app.emit_to(window, event, payload),
    }
  }

  fn emit_to_main(&self, event: &str, payload: serde_json::Value) {
    match self.context() {
      Some(ctx) => ctx.events.emit_to_main(event, payload),
      None => self.app.emit_to_main(event, payload),
    }
  }
}

/// The callback of `for_each_command!`: the declaration of each typed command
macro_rules! tauri_commands {
  ($($(#[doc = $doc:literal])* $class:ident $(($reason:ident))? $kind:ident $(($($field:tt)*))? $name:ident($($arg:ident: $type:ty),*) $([channel $channel:ident: $event:ty $(, $open:ident)?])? $(-> $output:ty)? = $body:path;)*) => {
    $(tauri_command!($(#[doc = $doc])* $kind $name($($arg: $type),*) [$($channel: $event)?] [$($output)?]);)*
  };
}

/// The declaration of one command for the bindings: its arguments, its channel, and its `Result`, of `()` for a command
/// without an output (`null` in the bindings). Every typed command answers a `Result`, so each is a `typedError` of the
/// bindings, whose failed call is its error answer, and none rejects (`every_command_of_the_bindings_answers_a_result`).
/// A `raw` command answers bytes, which the typed bindings cannot say: it has none. The shell answers every command
/// through the host (`invoke`), never by a declaration
macro_rules! tauri_command {
  ($(#[doc = $doc:literal])* raw $name:ident($($arg:ident: $type:ty),*) [] [$output:ty]) => {};
  ($(#[doc = $doc:literal])* $kind:ident $name:ident($($arg:ident: $type:ty),*) [$($channel:ident: $event:ty)?] []) => {
    tauri_command!($(#[doc = $doc])* $kind $name($($arg: $type),*) [$($channel: $event)?] [()]);
  };
  ($(#[doc = $doc:literal])* $kind:ident $name:ident($($arg:ident: $type:ty),*) [$($channel:ident: $event:ty)?] [$output:ty]) => {
    $(#[doc = $doc])*
    #[tauri::command]
    #[specta::specta]
    pub fn $name($($arg: $type,)* $($channel: tauri::ipc::Channel<$event>)?) -> Result<$output, domain_types::error::ApiError> {
      let _ = ($(&$arg,)* $(&$channel)?);
      Err(domain_types::error::ApiError::internal(concat!(stringify!($name), " goes through the host")))
    }
  };
}

/// The declarations of the typed commands of app-api, by the names of the commands
pub mod generated {
  app_api::for_each_command!(tauri_commands);
}

/// The callback of `for_each_command!` that collects the declarations of `generated` for the bindings: the `raw` ones
/// left out (`@split` takes the entries one by one)
macro_rules! tauri_handlers {
  (@split [$($typed:ident)*] raw $name:ident $($rest:ident)*) => {
    tauri_handlers!(@split [$($typed)*] $($rest)*);
  };
  (@split [$($typed:ident)*] $kind:ident $name:ident $($rest:ident)*) => {
    tauri_handlers!(@split [$($typed)* $name] $($rest)*);
  };
  (@split [$($typed:ident)*]) => {
    /// The typed commands, which the bindings are exported from (`specta_builder`)
    pub fn typed_commands() -> tauri_specta::Commands<tauri::Wry> {
      tauri_specta::collect_commands![$(generated::$typed),*]
    }
  };
  ($($(#[doc = $doc:literal])* $class:ident $(($reason:ident))? $kind:ident $(($($field:tt)*))? $name:ident($($arg:ident: $type:ty),*) $([channel $channel:ident: $event:ty $(, $open:ident)?])? $(-> $output:ty)? = $body:path;)*) => {
    tauri_handlers!(@split [] $($kind $name)*);
  };
}

app_api::for_each_command!(tauri_handlers);
