//! The grammar of the commands of the app, declared once in `commands.rs` (`for_each_command!`), and what that one
//! declaration gives each host: [`Command`], one type per command in `app_api::commands::<name>`, the e2e server's
//! route of it (`crates/test-server/src/invoke.rs`), and [`COMMANDS`](crate::commands::COMMANDS); the app's
//! `#[tauri::command]` wrapper with the docs and the argument names of the bindings (`src-tauri/src/api.rs`).
//!
//! An entry is the doc comment of the bindings, then
//! `<class> <kind> <name>(<argument>: <type>, …) [channel <name>: <event>, open] -> <output> = <body>;`, the output
//! left out for a `now` command that answers nothing (`void` in the bindings, where `-> ()` would be `null`):
//! - the class says what it does to the app and its repositories: `read` writes nothing, `engine` runs or stops an
//!   operation of a session (`operations::engine`), `app(<reason>)` writes what no session admits, with the reason
//!   ([`AppReason`]); the e2e tests answer no command but a `read` in the page (`command_registry_test.rs` of the e2e
//!   server);
//! - the kind says how it runs and answers: `json` is `async fn(&AppContext, arguments…) -> Result<output, ApiError>`,
//!   `raw` the same with the output bytes the typed bindings cannot say (`Vec<u8>`), `window` the same for the window
//!   that calls it, `async fn(&AppContext, &Call, arguments…)` (a search of its own, which a newer one of the window
//!   supersedes); `plain` a synchronous rule that cannot fail, `fn(arguments…) -> output`, which needs no context (the
//!   grammar of what the user types); `session` a
//!   command of the session of the repository its `params.repository_path` names, `async fn(&AppContext, &Call,
//!   SessionKey, arguments…) -> Result<output, ApiError>`, whose frame runs it as one operation in the span of the
//!   command (`<name>{repository_path}:operation{operation_id}`) once it resolved the key (`AppContext::session_key`);
//!   `now` a synchronous command of a window that cannot fail and answers at once (on the main thread of the app),
//!   `fn(&AppContext, &Call, arguments…) -> output`; `main` the same that can fail, `fn(&AppContext, &Call, arguments…)
//!   -> Result<output, ApiError>`, for what the app does on its main thread (a native menu, the restart);
//! - the arguments are what the page sends, by name (`params`, `repository_path` as `repositoryPath`);
//! - the channel, for a command that tells the page its events while it runs (`[channel events: OperationEvent]`, of a
//!   `json` or a `session` command), is the last argument of the body (`Sink<event>`) and of the bindings
//!   (`Channel<event>`); `open`: it stays open after the answer (the patches of a subscription). The page mock of the e2e
//!   server knows each channel from the table of the registry, `crates/app-api/commands.json` (`commands_json_test.rs`).

use crate::context::AppContext;
use crate::transport::Call;
use domain_types::error::ApiError;
use serde::de::DeserializeOwned;
use std::future::Future;

/// What a command does to the state of the app and its repositories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClass {
  /// Writes nothing: a read of a repository, of a remote service or of the app
  Read,
  /// Runs or stops an operation of a session, which its registry admits (`operations::engine`)
  Engine,
  /// Writes what no session admits: a ticket of a session, the settings, an account, the app itself; the reason says
  /// which, and its one writer (docs/design/rust-domain-authority.md, "Решение 2026-10-28")
  App(AppReason),
}

/// Why a write is a command of the app and not an intent of the engine: no repository is its subject, or what it writes
/// has a writer of its own
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppReason {
  /// The session's own tickets: a sync and a focus ask the session to read, never admitted as an operation
  SessionTickets,
  /// The cancel of a read: it ends the read of this window, which wrote nothing
  CanceledRead,
  /// The page's journal of the app data directory, one writer (`operations::journal`) under its lock
  PageJournal,
  /// A settings writer of no repository's session, serialized per file
  SettingsWriter,
  /// The app itself: its menu, its CLI, its update and its restart
  TheApp,
  /// The CLIs of Settings → Tools, one action of a tool at a time (`cli_tools`)
  Tools,
  /// An account of the app, one sign-in of a provider at a time (`remote_facts::sign_in`)
  Accounts,
}

/// How a command runs and answers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
  /// Async, answers `Result<output, ApiError>` as JSON
  Json,
  /// A synchronous rule that cannot fail, answers its output as JSON; the bindings return it as it is
  Plain,
  /// Async, answers raw bytes: the app registers it beside the typed bindings (`generate_handler!`), and the page calls
  /// it with `invoke` of its own
  Raw,
  /// Async, answers `Result<output, ApiError>` as JSON for the window that calls it
  Window,
  /// Async, a command of the session of a repository: the frame resolves its key, the body runs as one operation
  Session,
  /// A synchronous command of a window that cannot fail: the bindings return its output as it is
  Now,
  /// A synchronous command of a window that can fail, on the main thread of the app
  Main,
}

/// The channel a command tells the page its events on while it runs
#[derive(Debug)]
pub struct ChannelSpec {
  /// The name of its argument (`patches`, `events`)
  pub name: &'static str,
  /// It stays open after the answer: the patches of a subscription come until it ends
  pub open: bool,
}

/// One command of the registry, as a host routes and checks it
#[derive(Debug)]
pub struct CommandSpec {
  pub name: &'static str,
  pub class: CommandClass,
  pub kind: CommandKind,
  /// The names of the arguments the page sends, in their order
  pub args: &'static [&'static str],
  pub channel: Option<ChannelSpec>,
}

/// A command of the registry: its arguments as the page sends them, its channel, and its body in the frame of its kind
pub trait Command {
  const SPEC: CommandSpec;
  /// The arguments as one object of their camelCase names
  type Args: DeserializeOwned + Send;
  /// `Sink<event>` of a command with a channel, `()` without one
  type Channel: Send;
  type Output: Send;
  /// Runs the body for `call`; a `plain` or `now` one answers at once
  fn call(ctx: &AppContext, call: &Call, args: Self::Args, channel: Self::Channel) -> impl Future<Output = Result<Self::Output, ApiError>> + Send;
}

/// The callback of `for_each_command!` that gives each command its [`Command`] (`app_api::commands::<name>::Command`,
/// its arguments `…::Args`) and lists them all in `COMMANDS`
#[macro_export]
macro_rules! api_commands {
  ($($(#[doc = $doc:literal])* $class:ident $(($reason:ident))? $kind:ident $name:ident($($arg:ident: $type:ty),*) $([channel $channel:ident: $event:ty $(, $open:ident)?])? $(-> $output:ty)? = $body:path;)*) => {
    $($crate::api_command!([$class $($reason)?] $kind $name($($arg: $type),*) [$(channel $channel: $event $(, $open)?)?] -> [$($output)?] = $body);)*

    /// Every command of the registry, in the order of `for_each_command!`
    pub const COMMANDS: &[$crate::registry::CommandSpec] = &[$(<$name::Command as $crate::registry::Command>::SPEC),*];
  };
}

/// One command of `api_commands!`: its module, and the frame of its kind around its body (`@call`)
#[doc(hidden)]
#[macro_export]
macro_rules! api_command {
  (@class read) => {
    $crate::registry::CommandClass::Read
  };
  (@class engine) => {
    $crate::registry::CommandClass::Engine
  };
  (@class app $reason:ident) => {
    $crate::registry::CommandClass::App($crate::registry::AppReason::$reason)
  };
  (@kind json) => {
    $crate::registry::CommandKind::Json
  };
  (@kind plain) => {
    $crate::registry::CommandKind::Plain
  };
  (@kind raw) => {
    $crate::registry::CommandKind::Raw
  };
  (@kind window) => {
    $crate::registry::CommandKind::Window
  };
  (@kind session) => {
    $crate::registry::CommandKind::Session
  };
  (@kind now) => {
    $crate::registry::CommandKind::Now
  };
  (@kind main) => {
    $crate::registry::CommandKind::Main
  };
  (@channel_type []) => {
    ()
  };
  (@channel_type [channel $channel:ident: $event:ty $(, $open:ident)?]) => {
    $crate::transport::Sink<$event>
  };
  (@channel_spec []) => {
    None
  };
  (@output []) => {
    ()
  };
  (@output [$output:ty]) => {
    $output
  };
  (@channel_spec [channel $channel:ident: $event:ty]) => {
    Some($crate::registry::ChannelSpec { name: stringify!($channel), open: false })
  };
  (@channel_spec [channel $channel:ident: $event:ty, open]) => {
    Some($crate::registry::ChannelSpec { name: stringify!($channel), open: true })
  };
  (@call plain $name:ident $body:path, $ctx:ident, $call:ident, $args:ident($($arg:ident),*), $sink:ident []) => {{
    let _ = ($ctx, $call, $sink);
    ::std::future::ready(Ok($body($($args.$arg),*)))
  }};
  (@call now $name:ident $body:path, $ctx:ident, $call:ident, $args:ident($($arg:ident),*), $sink:ident []) => {{
    let () = $sink;
    ::std::future::ready(Ok($body($ctx, $call, $($args.$arg),*)))
  }};
  (@call main $name:ident $body:path, $ctx:ident, $call:ident, $args:ident($($arg:ident),*), $sink:ident []) => {{
    let () = $sink;
    ::std::future::ready($body($ctx, $call, $($args.$arg),*))
  }};
  // the span of the command, then its operation: the key resolved in it, so its error names the operation too
  (@call session $name:ident $body:path, $ctx:ident, $call:ident, $args:ident($($arg:ident),*), $sink:ident [$($channel:tt)*]) => {{
    let span = ::tracing::info_span!(stringify!($name), repository_path = %$args.params.repository_path);
    ::tracing::Instrument::instrument(
      $crate::operation::operation(async move {
        let key = $ctx.session_key(&$args.params.repository_path).await?;
        $crate::api_command!(@session_body $body, $ctx, $call, key, $args($($arg),*), $sink [$($channel)*]).await
      }),
      span,
    )
  }};
  (@session_body $body:path, $ctx:ident, $call:ident, $key:ident, $args:ident($($arg:ident),*), $sink:ident []) => {{
    let () = $sink;
    $body($ctx, $call, $key, $($args.$arg),*)
  }};
  (@session_body $body:path, $ctx:ident, $call:ident, $key:ident, $args:ident($($arg:ident),*), $sink:ident [$($channel:tt)+]) => {
    $body($ctx, $call, $key, $($args.$arg,)* $sink)
  };
  (@call window $name:ident $body:path, $ctx:ident, $call:ident, $args:ident($($arg:ident),*), $sink:ident []) => {{
    let () = $sink;
    $body($ctx, $call, $($args.$arg),*)
  }};
  (@call $kind:ident $name:ident $body:path, $ctx:ident, $call:ident, $args:ident($($arg:ident),*), $sink:ident []) => {{
    let (_, ()) = ($call, $sink);
    $body($ctx, $($args.$arg),*)
  }};
  (@call json $name:ident $body:path, $ctx:ident, $call:ident, $args:ident($($arg:ident),*), $sink:ident [$($channel:tt)+]) => {{
    let _ = $call;
    $body($ctx, $($args.$arg,)* $sink)
  }};
  ([$($class:tt)+] $kind:ident $name:ident($($arg:ident: $type:ty),*) [$($channel:tt)*] -> [$($output:ty)?] = $body:path) => {
    pub mod $name {
      pub struct Command;

      #[derive(::serde::Deserialize)]
      #[serde(rename_all = "camelCase")]
      pub struct Args {
        $(pub $arg: $type),*
      }

      impl $crate::registry::Command for Command {
        const SPEC: $crate::registry::CommandSpec = $crate::registry::CommandSpec {
          name: stringify!($name),
          class: $crate::api_command!(@class $($class)+),
          kind: $crate::api_command!(@kind $kind),
          args: &[$(stringify!($arg)),*],
          channel: $crate::api_command!(@channel_spec [$($channel)*]),
        };
        type Args = Args;
        type Channel = $crate::api_command!(@channel_type [$($channel)*]);
        type Output = $crate::api_command!(@output [$($output)?]);

        fn call(
          ctx: &$crate::context::AppContext,
          call: &$crate::transport::Call,
          args: Args,
          channel: Self::Channel,
        ) -> impl ::std::future::Future<Output = Result<Self::Output, ::domain_types::error::ApiError>> + Send {
          // a command without arguments reads none
          let _ = &args;
          $crate::api_command!(@call $kind $name $body, ctx, call, args($($arg),*), channel [$($channel)*])
        }
      }
    }
  };
}
