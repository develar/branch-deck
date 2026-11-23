//! The root commands (docs/design/backend-protocol.md, "Root commands"): every command of `for_each_command!` is
//! `x-branch-deck/<name>`, its name of the registry verbatim, run by `app_api::dispatch` for the window the params name
//! in the connection, or for the connection itself. It answers `{value}`, `{bytes}` or `{error}`; each event of its
//! channel is `x-branch-deck/callEvent {requestId, event}` before its answer; `x-branch-deck/cancel` ends its channel. An
//! answer or an event that names a version of a repository goes after the actions of its channel up to that version to
//! each window of the connection (the fence, `Host::fence`).

use crate::connection::Connection;
use crate::host::lock;
use crate::wire::{Failure, Frame, ROOT, notification, result};
use app_api::dispatch::{self, Answer};
use app_api::transport::{Call, EventSink, SinkGone, WindowId};
use domain_types::session::Version;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::mpsc;
use tracing::Instrument;

/// The commands the client maps to the methods of a channel (`subscribe`, `unsubscribe`, `x-branch-deck/setDemand`)
const CHANNEL_COMMANDS: &[&str] = &["subscribe_view", "unsubscribe_view", "set_demand"];

/// The methods of the protocol of one word that no command of the registry may take (Key decision 4)
pub const PROTOCOL_METHODS: &[&str] = &[
  "repositoryChannel",
  "setDemand",
  "followPrefix",
  "claimNews",
  "bootstrap",
  "showMain",
  "openWindow",
  "showWindow",
  "cancel",
  "windowShown",
  "windowHidden",
  "windowClosed",
  "pageLoaded",
  "callEvent",
  "event",
  "resync",
  "subscriptionEnded",
];

/// The channel of a call: each event queued at once, in its order, but one that names a version and every event after
/// it, which wait in the forwarder of the call for the fence; none once the call was canceled
struct CallSink {
  connection: Weak<Connection>,
  request: Value,
  command: String,
  args: Value,
  canceled: Arc<AtomicBool>,
  /// The events in the forwarder
  waiting: Arc<Mutex<usize>>,
  forward: mpsc::UnboundedSender<Value>,
}

impl EventSink<Value> for CallSink {
  fn send(&self, event: Value) -> Result<(), SinkGone> {
    if self.canceled.load(Ordering::SeqCst) {
      return Err(SinkGone);
    }
    let connection = self.connection.upgrade().ok_or(SinkGone)?;
    let mut waiting = lock(&self.waiting);
    if *waiting == 0 && version_of_event(&self.command, &self.args, &event).is_none() {
      return if connection.outbox().push(call_event(&self.request, &event)) {
        Ok(())
      } else {
        Err(SinkGone)
      };
    }
    *waiting += 1;
    self.forward.send(event).map_err(|_closed| SinkGone)
  }
}

/// The frame of `event` of the call `request`
fn call_event(request: &Value, event: &Value) -> Frame {
  Frame::of(&notification("x-branch-deck/callEvent", json!({ "channel": ROOT, "requestId": request, "event": event })))
}

/// The version of a repository that the answer `value` of `command` names, with the path of its repository
fn version_of_answer(command: &str, args: &Value, value: &Value) -> Option<(String, Version)> {
  let version = match command {
    "sync" | "set_window_focus" => &value["version"],
    "refresh_facts" => value,
    "execute" => &value["versionAfter"],
    _ => return None,
  };
  named(args, version)
}

/// The version of a repository that the event `event` of `command` names: the `done` of an operation
fn version_of_event(command: &str, args: &Value, event: &Value) -> Option<(String, Version)> {
  (command == "execute" && event["kind"] == "done").then(|| named(args, &event["outcome"]["versionAfter"]))?
}

fn named(args: &Value, version: &Value) -> Option<(String, Version)> {
  let path = args["params"]["repositoryPath"].as_str()?.to_owned();
  let version = serde_json::from_value(version.clone()).ok()?;
  Some((path, version))
}

impl Connection {
  /// The root command `name` of the request `id`
  pub(crate) fn root_command(&self, id: Value, name: &str, params: &Value) {
    let Some(spec) = dispatch::spec(name) else {
      return self.fail(&id, &Failure::unknown_method(&format!("x-branch-deck/{name}")));
    };
    if CHANNEL_COMMANDS.contains(&name) {
      return self.fail(
        &id,
        &Failure::new(
          -32601,
          "channelCommand",
          format!("{name} is subscribe, unsubscribe or setDemand of a window on its repository channel"),
        ),
      );
    }
    let Some(of) = self.id() else {
      return self.fail(&id, &Failure::invalid("notInitialized", "initialize first"));
    };
    let window = params["window"].as_str().filter(|window| !window.is_empty());
    if spec.window && window.is_none() {
      return self.fail(&id, &Failure::bad_params(format!("{name} is a command of a window: its params name the window")));
    }
    let args = params.get("args").cloned().unwrap_or(Value::Null);
    let prepared = match dispatch::prepare(name, args.clone()) {
      Ok(prepared) => prepared,
      Err(error) => {
        return self.fail(
          &id,
          &Failure::bad_params(error.message.clone()).with("apiError", serde_json::to_value(&error).unwrap_or(Value::Null)),
        );
      }
    };
    let ctx = Arc::clone(&self.host.ctx);
    let call = window.map_or_else(|| Call::of_connection(of), |window| ctx.call_of(&WindowId::of(of, window)));
    let canceled = Arc::new(AtomicBool::new(false));
    let call_key = id.to_string();
    lock(&self.calls).insert(call_key.clone(), Arc::clone(&canceled));
    let (forward, mut received) = mpsc::unbounded_channel();
    let waiting = Arc::new(Mutex::new(0));
    let command = name.to_owned();
    let sink = Arc::new(CallSink {
      connection: Weak::clone(&self.me),
      request: id.clone(),
      command: command.clone(),
      args: args.clone(),
      canceled: Arc::clone(&canceled),
      waiting: Arc::clone(&waiting),
      forward,
    });
    let (connection, host) = (self.me(), Arc::clone(&self.host));
    self.host.ctx.runtime.spawn(
      async move {
        // a task of its own, as Tauri runs a command: a command that panics fails its call (`-32603 panic`) instead of
        // leaving it without an answer
        let answered = ctx.runtime.spawn(prepared.run(Arc::clone(&ctx), call, sink).in_current_span());
        tokio::pin!(answered);
        let forwarded = |event: Value| {
          let (connection, host, command, args, id, canceled, waiting) = (&connection, &host, &command, &args, &id, &canceled, &waiting);
          async move {
            if let Some((path, version)) = version_of_event(command, args, &event) {
              host.fence(of, &path, version).await;
            }
            if !canceled.load(Ordering::SeqCst) {
              connection.outbox().push(call_event(id, &event));
            }
            *lock(waiting) -= 1;
          }
        };
        let answer = loop {
          tokio::select! {
            biased;
            Some(event) = received.recv() => forwarded(event).await,
            answer = &mut answered => break answer,
          }
        };
        while let Ok(event) = received.try_recv() {
          forwarded(event).await;
        }
        lock(&connection.calls).remove(&call_key);
        let frame = match answer {
          Ok(Ok(Answer::Json(value))) => {
            if let Some((path, version)) = version_of_answer(&command, &args, &value) {
              host.fence(of, &path, version).await;
            }
            Frame::of(&result(&id, json!({ "value": value })))
          }
          Ok(Ok(Answer::Bytes(bytes))) => Frame::Bytes { id, bytes },
          Ok(Err(error)) => Frame::of(&result(&id, json!({ "error": error }))),
          Err(failure) => {
            tracing::error!(%failure, command, "a command panicked");
            Frame::of(&Failure::new(-32603, "panic", format!("{command} panicked")).answer(&id))
          }
        };
        connection.outbox().push(frame);
      }
      .in_current_span(),
    );
  }
}

#[cfg(test)]
#[path = "call_test.rs"]
mod call_test;
