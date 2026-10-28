//! The grammar of the commands of the app, declared once in `commands.rs` (`for_each_command!`), and what that one
//! declaration gives each host: [`Command`], one type per command in `app_api::commands::<name>`, the e2e server's
//! route of it (`crates/test-server/src/invoke.rs`), and [`COMMANDS`](crate::commands::COMMANDS); the app's
//! `#[tauri::command]` wrapper with the docs and the argument names of the bindings (`src-tauri/src/api.rs`).
//!
//! An entry is the doc comment of the bindings, then
//! `<class> <kind> <name>(<argument>: <type>, …) -> <output> = <body>;`:
//! - the class says what it does to the app and its repositories: `read` writes nothing;
//! - the kind says how it runs and answers: `json` is `async fn(&AppContext, arguments…) -> Result<output, ApiError>`,
//!   `raw` the same with the output bytes the typed bindings cannot say (`Vec<u8>`), `plain` a synchronous rule that
//!   cannot fail, `fn(arguments…) -> output`, which needs no context (the grammar of what the user types);
//! - the arguments are what the page sends, by name (`params`, `repository_path` as `repositoryPath`).

use crate::context::AppContext;
use domain_types::error::ApiError;
use serde::de::DeserializeOwned;
use std::future::Future;

/// What a command does to the state of the app and its repositories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClass {
  /// Writes nothing: a read of a repository, of a remote service or of the app
  Read,
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
}

/// One command of the registry, as a host routes and checks it
#[derive(Debug)]
pub struct CommandSpec {
  pub name: &'static str,
  pub class: CommandClass,
  pub kind: CommandKind,
  /// The names of the arguments the page sends, in their order
  pub args: &'static [&'static str],
}

/// A command of the registry: its arguments as the page sends them, and its body
pub trait Command {
  const SPEC: CommandSpec;
  /// The arguments as one object of their camelCase names
  type Args: DeserializeOwned + Send;
  type Output: Send;
  /// Runs the body; a `plain` one answers at once
  fn call(ctx: &AppContext, args: Self::Args) -> impl Future<Output = Result<Self::Output, ApiError>> + Send;
}

/// The callback of `for_each_command!` that gives each command its [`Command`] (`app_api::commands::<name>::Command`,
/// its arguments `…::Args`) and lists them all in `COMMANDS`
#[macro_export]
macro_rules! api_commands {
  ($($(#[doc = $doc:literal])* $class:ident $kind:ident $name:ident($($arg:ident: $type:ty),*) -> $output:ty = $body:path;)*) => {
    $($crate::api_command!($class $kind $name($($arg: $type),*) -> $output = $body);)*

    /// Every command of the registry, in the order of `for_each_command!`
    pub const COMMANDS: &[$crate::registry::CommandSpec] = &[$(<$name::Command as $crate::registry::Command>::SPEC),*];
  };
}

/// One command of `api_commands!`
#[doc(hidden)]
#[macro_export]
macro_rules! api_command {
  (@class read) => {
    $crate::registry::CommandClass::Read
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
  (@call plain $body:path, $ctx:ident, $args:ident($($arg:ident),*)) => {{
    let _ = $ctx;
    ::std::future::ready(Ok($body($($args.$arg),*)))
  }};
  (@call $kind:ident $body:path, $ctx:ident, $args:ident($($arg:ident),*)) => {
    $body($ctx, $($args.$arg),*)
  };
  ($class:ident $kind:ident $name:ident($($arg:ident: $type:ty),*) -> $output:ty = $body:path) => {
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
          class: $crate::api_command!(@class $class),
          kind: $crate::api_command!(@kind $kind),
          args: &[$(stringify!($arg)),*],
        };
        type Args = Args;
        type Output = $output;

        fn call(ctx: &$crate::context::AppContext, args: Args) -> impl ::std::future::Future<Output = Result<$output, ::domain_types::error::ApiError>> + Send {
          $crate::api_command!(@call $kind $body, ctx, args($($arg),*))
        }
      }
    }
  };
}
