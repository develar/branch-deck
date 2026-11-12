//! A command of the registry by its name, with its arguments and its channel as JSON: what a host that gets a command
//! as a message runs (the e2e server, `crates/test-server/src/invoke.rs`; `brd mcp`, `crates/mcp-host`), over
//! `for_each_command!`, so no host keeps a list of its own. The app does not use it: Tauri calls the typed wrapper of
//! each command (`src-tauri/src/api.rs`).
//!
//! The arguments are one object of the argument names in camelCase, as the page's `invoke` sends them (`{ params }`,
//! `{ repositoryPath, branchName }`); `null` for a command without arguments. The output is the JSON of the command, or
//! the bytes of a raw one; the events of its channel go to one sink of JSON values (`JsonSink`), each the JSON of one
//! event, in the order the command sends them.

use crate::commands::COMMANDS;
use crate::context::AppContext;
use crate::registry::{Command, CommandSpec};
use crate::transport::{Call, EventSink, Sink, SinkGone};
use domain_types::error::ApiError;
use serde::Serialize;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

/// What a command answers: its output as JSON, or the bytes of a raw command (`get_blob_syntax`)
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
  Json(serde_json::Value),
  Bytes(Vec<u8>),
}

/// The channel of a command as JSON: each event of its channel, serialized
pub type JsonSink = Sink<serde_json::Value>;

/// The answer of a command, on the heap: one type for every command, so a host spawns each as the same kind of task
pub type Dispatched = Pin<Box<dyn Future<Output = Result<Answer, ApiError>> + Send>>;

/// A command whose name and arguments were read, to run in a context for a call
pub struct Prepared {
  pub spec: &'static CommandSpec,
  run: Box<dyn FnOnce(Arc<AppContext>, Call, JsonSink) -> Dispatched + Send>,
}

impl Prepared {
  /// The answer of the command in `ctx` for `call`, the events of its channel to `sink` (a command without a channel
  /// sends nothing to it)
  pub fn run(self, ctx: Arc<AppContext>, call: Call, sink: JsonSink) -> Dispatched {
    (self.run)(ctx, call, sink)
  }
}

/// The spec of the command `name`, none for a name no command has
#[must_use]
pub fn spec(name: &str) -> Option<&'static CommandSpec> {
  COMMANDS.iter().find(|spec| spec.name == name)
}

/// The command `name` with `args`, the object of its arguments by their camelCase names (`null` or `{}` for none)
///
/// # Errors
/// No command has the name, or the arguments are not the command's
pub fn prepare(name: &str, args: serde_json::Value) -> Result<Prepared, ApiError> {
  let args = if args.is_null() { serde_json::Value::Object(serde_json::Map::new()) } else { args };
  prepare_named(name, args)
}

/// A sink that takes nothing: the channel of a call that has none, whose sends fail as those to a page that went away
#[must_use]
pub fn no_channel() -> JsonSink {
  Arc::new(NoChannel)
}

struct NoChannel;

impl EventSink<serde_json::Value> for NoChannel {
  fn send(&self, _event: serde_json::Value) -> Result<(), SinkGone> {
    Err(SinkGone)
  }
}

/// The channel argument of a command made of a sink of JSON: none for a command without one
pub trait ChannelOf {
  fn of(sink: JsonSink) -> Self;
}

impl ChannelOf for () {
  fn of(_sink: JsonSink) -> Self {}
}

impl<T: Serialize + Send + Sync + 'static> ChannelOf for Sink<T> {
  fn of(sink: JsonSink) -> Self {
    Arc::new(Typed { sink, event: PhantomData })
  }
}

/// A typed channel over a sink of JSON: each event serialized as the page reads it
struct Typed<T> {
  sink: JsonSink,
  event: PhantomData<fn(T)>,
}

impl<T: Serialize + Send + Sync> EventSink<T> for Typed<T> {
  fn send(&self, event: T) -> Result<(), SinkGone> {
    match serde_json::to_value(&event) {
      Ok(value) => self.sink.send(value),
      Err(error) => {
        tracing::error!(%error, "an event of a channel is not JSON");
        Ok(())
      }
    }
  }
}

/// A command that answers JSON
fn json<C>(args: serde_json::Value) -> Result<Prepared, ApiError>
where
  C: Command<Args: 'static, Output: Serialize + 'static, Channel: ChannelOf + 'static> + 'static,
{
  let args = arguments::<C>(args)?;
  Ok(Prepared {
    spec: &C::SPEC,
    run: Box::new(move |ctx, call, sink| {
      Box::pin(async move {
        let output = C::call(&ctx, &call, args, C::Channel::of(sink)).await?;
        serde_json::to_value(output)
          .map(Answer::Json)
          .map_err(|error| ApiError::internal(format!("The answer of {} is not JSON: {error}", C::SPEC.name)))
      })
    }),
  })
}

/// A command that answers bytes
fn raw<C>(args: serde_json::Value) -> Result<Prepared, ApiError>
where
  C: Command<Args: 'static, Output = Vec<u8>, Channel = ()> + 'static,
{
  let args = arguments::<C>(args)?;
  Ok(Prepared {
    spec: &C::SPEC,
    run: Box::new(move |ctx, call, _sink| Box::pin(async move { C::call(&ctx, &call, args, ()).await.map(Answer::Bytes) })),
  })
}

fn arguments<C: Command>(args: serde_json::Value) -> Result<C::Args, ApiError> {
  serde_json::from_value(args).map_err(|error| ApiError::internal(format!("Invalid arguments of {}: {error}", C::SPEC.name)))
}

/// The callback of `for_each_command!`: the command of a name, by its kind
macro_rules! dispatch_commands {
  ($($(#[doc = $doc:literal])* $class:ident $(($reason:ident))? $kind:ident $(($($field:tt)*))? $name:ident($($arg:ident: $type:ty),*) $([channel $channel:ident: $event:ty $(, $open:ident)?])? $(-> $output:ty)? = $body:path;)*) => {
    fn prepare_named(name: &str, args: serde_json::Value) -> Result<Prepared, ApiError> {
      match name {
        $(stringify!($name) => dispatch_command!($kind $name, args),)*
        _ => Err(ApiError::internal(format!("No command is named {name}"))),
      }
    }
  };
}

macro_rules! dispatch_command {
  (raw $name:ident, $args:ident) => {
    raw::<crate::commands::$name::Command>($args)
  };
  ($kind:ident $name:ident, $args:ident) => {
    json::<crate::commands::$name::Command>($args)
  };
}

crate::for_each_command!(dispatch_commands);

#[cfg(test)]
#[path = "dispatch_test.rs"]
mod dispatch_test;
