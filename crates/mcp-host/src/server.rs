//! The connection of `brd mcp` with air-ide (openspec/specs/repository/ide-embedding/spec.md;
//! docs/design/ide-embedding.md): MCP (`initialize`, `tools/list`, `tools/call`, `resources/read`, `roots/list`) with
//! the vocabulary `x-air-app` (the mirrors of the views, the events, the life of the places).
//!
//! - **Tools**: `bd_invoke {command, args}` runs one command of app-api by its name (`app_api::dispatch`) for the window
//!   of the instance of the call, the events of its channel as `x-air-app/callEvent {callId, event}` (the id of the
//!   call), each before the answer; `bd_bootstrap {view}` gives what the page takes before its main. Both are only for
//!   the app, and a call of the origin `agent` is refused. A cancel stops a read and leaves no answer; a write runs to
//!   its end, its answer left out.
//! - **Mirrors** (`mirror.rs`): `x-air-app/subscribe` of a repository subscribes its session once for every view of it,
//!   under a window of its own (`mirror:<path>`), so the close of one view ends no other's mirror; the last
//!   `x-air-app/unsubscribe` ends it.
//! - **Places**: `x-air-app/viewShown` records the params of an instance, `x-air-app/viewClosed` forgets its page and its
//!   window (`AppContext::forget_window`); the end of a WebSocket never reaches brd.
//! - **Roots**: the repositories are the Git roots of the project (`roots/list`, asked at the start and on
//!   `notifications/roots/list_changed`); the main view shows the first.
//!
//! Every message brd writes goes through the one queue of `Link`, in the order it was sent: a command that changes the
//! view sends its patches (from the session, through the mirror) and its events before its answer.

use crate::hosts::{Instances, MAIN_VIEW, McpEvents, desktop, label_of, view_of};
use crate::link::{INTERNAL_ERROR, INVALID_PARAMS, Link, METHOD_NOT_FOUND, NO_CONSENT, Reply};
use crate::mirror::MirrorView;
use crate::resources;
use app_api::context::AppContext;
use app_api::dispatch::{Answer, prepare};
use app_api::registry::{Command, CommandClass};
use app_api::transport::{AppEvents, Desktop, EventSink, SinkGone, WindowId};
use app_settings::windows::{MAIN_WINDOW, window_snapshot};
use base64::Engine;
use domain_types::error::ApiError;
use domain_types::session::{SubscribeViewParams, ViewPatch};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use tracing::Instrument;

/// The version of the vocabulary x-air-app brd speaks
pub const X_AIR_APP_VERSION: &str = "1.0.0";
/// The version of MCP brd answers
const PROTOCOL_VERSION: &str = "2025-06-18";

/// The title bar of a page in the IDE: the place of the view is the IDE's, the page draws none of its own
const TITLE_BAR: &str = r#"{"kind":"native","checkForUpdates":false}"#;

/// One connection of air-ide with brd: what its messages go through, and the instances of its views, which the hosts of
/// the context it serves share (`events`, `desktop`)
pub struct Connection {
  link: Arc<Link>,
  instances: Arc<Instances>,
}

impl Connection {
  /// A connection whose messages come out of the receiver, one JSON object each, in the order brd sent them
  pub fn new() -> (Self, mpsc::UnboundedReceiver<String>) {
    let (link, out) = Link::new();
    let connection = Self {
      link: Arc::new(link),
      instances: Arc::default(),
    };
    (connection, out)
  }

  /// The events of the app, which the context the connection serves sends
  pub fn events(&self) -> Arc<dyn AppEvents> {
    Arc::new(McpEvents {
      link: Arc::clone(&self.link),
      instances: Arc::clone(&self.instances),
    })
  }

  /// The desktop of the app in the IDE, which the context the connection serves does on
  pub fn desktop(&self) -> Desktop {
    desktop(&self.link, &self.instances)
  }

  /// Serves the messages of `incoming`, each one JSON-RPC message, in `ctx`, until it ends; then every window of the
  /// connection is forgotten (`AppContext::forget_windows`): its subscriptions end
  pub async fn serve(self, ctx: Arc<AppContext>, mut incoming: mpsc::UnboundedReceiver<String>) {
    let server = Arc::new(Server {
      ctx: Arc::clone(&ctx),
      link: self.link,
      instances: self.instances,
      calls: Mutex::default(),
      mirrors: Mutex::default(),
      roots: Mutex::default(),
    });
    while let Some(line) = incoming.recv().await {
      if line.trim().is_empty() {
        continue;
      }
      match serde_json::from_str::<Value>(&line) {
        Ok(message) => Arc::clone(&server).handle(message).await,
        Err(error) => {
          tracing::warn!(%error, "a message of air-ide is not JSON");
          server.link.fail(&Value::Null, -32700, "The message is not JSON", "parse");
        }
      }
    }
    server.end();
    ctx.forget_windows();
  }
}

/// A command that runs for a call of a tool
struct Running {
  abort: Option<AbortHandle>,
  /// The command writes nothing: a cancel stops it
  read: bool,
  /// air-ide cancelled the call: nothing of it is sent any more
  cancelled: Arc<AtomicBool>,
}

/// The subscription of the session of a repository that the views of it share
struct Mirror {
  /// The window the subscription is kept under (`mirror:<path>`)
  window: WindowId,
  /// The id of the subscription of the session, which `set_demand` of a view names
  subscription: u32,
  instances: HashSet<String>,
  slot: Arc<Mutex<Slot>>,
}

/// The view of a mirror as its patches come
enum Slot {
  /// The subscription has not answered yet: the patches that came meanwhile
  Waiting(Vec<ViewPatch>),
  Live(Box<MirrorView>),
  /// The last view left the mirror, or the subscription failed
  Ended,
}

struct Server {
  ctx: Arc<AppContext>,
  link: Arc<Link>,
  instances: Arc<Instances>,
  /// The commands that run for calls of tools, by the id of the call
  calls: Mutex<HashMap<String, Running>>,
  /// The mirrors, by the path of their repository
  mirrors: Mutex<HashMap<String, Mirror>>,
  /// The Git roots of the project, as `roots/list` answered last
  roots: Mutex<Vec<String>>,
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
  mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Server {
  async fn handle(self: Arc<Self>, message: Value) {
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
      // an answer to a request of brd
      if let Some(id) = id {
        let reply: Reply = message
          .get("error")
          .map_or_else(|| Ok(message.get("result").cloned().unwrap_or(Value::Null)), |error| Err(error.clone()));
        self.link.answered(&id, reply);
      }
      return;
    };
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    match id {
      Some(id) => self.request(id, method, params).await,
      None => self.notification(method, &params),
    }
  }

  async fn request(self: Arc<Self>, id: Value, method: &str, params: Value) {
    match method {
      "initialize" => self.link.respond(
        &id,
        json!({
          "protocolVersion": PROTOCOL_VERSION,
          "capabilities": { "tools": {}, "resources": {}, "experimental": { "x-air-app": { "version": X_AIR_APP_VERSION } } },
          "serverInfo": { "name": "branch-deck", "version": env!("CARGO_PKG_VERSION") },
        }),
      ),
      "ping" => self.link.respond(&id, json!({})),
      "tools/list" => self.link.respond(&id, json!({ "tools": tools() })),
      "tools/call" => self.call_tool(id, &params),
      "resources/read" => {
        let uri = params["uri"].as_str().unwrap_or_default();
        match resources::read(uri) {
          Some(contents) => self.link.respond(&id, json!({ "contents": [contents] })),
          None => self.link.fail(&id, INVALID_PARAMS, &format!("No resource of Branch Deck is {uri}"), "badParams"),
        }
      }
      "x-air-app/subscribe" => self.subscribe(id, &params).await,
      "x-air-app/unsubscribe" => {
        self.leave(params["instance"].as_str().unwrap_or_default());
        self.link.respond(&id, json!({}));
      }
      // the page sends its intents through bd_invoke, as in the desktop app
      "x-air-app/dispatchAction" => self.link.fail(
        &id,
        INVALID_PARAMS,
        "The view of Branch Deck takes no action: the page sends its intents with bd_invoke",
        "unknownAction",
      ),
      _ => self.link.fail(&id, METHOD_NOT_FOUND, &format!("brd mcp has no method {method}"), "unknownMethod"),
    }
  }

  fn notification(self: &Arc<Self>, method: &str, params: &Value) {
    match method {
      "notifications/initialized" | "notifications/roots/list_changed" => self.read_roots(),
      "notifications/cancelled" => self.cancel(&params["requestId"]),
      "x-air-app/viewShown" => {
        let instance = params["instance"].as_str().unwrap_or_default();
        self.instances.shown(instance, params.get("params").cloned().unwrap_or_else(|| json!({})));
      }
      "x-air-app/viewClosed" => {
        let instance = params["instance"].as_str().unwrap_or_default();
        self.leave(instance);
        self.instances.closed(instance);
        self.ctx.forget_window(&WindowId::new(label_of(instance)));
      }
      "x-air-app/viewHidden" => {}
      _ => tracing::debug!(method, "a notification brd does not know"),
    }
  }

  /// Asks the IDE for the roots of the project, and keeps its Git roots
  fn read_roots(self: &Arc<Self>) {
    let answer = self.link.request("roots/list", None);
    let server = Arc::clone(self);
    tokio::spawn(
      async move {
        let Ok(Ok(result)) = answer.await else {
          tracing::warn!("the IDE named no roots");
          return;
        };
        let roots: Vec<String> = result["roots"]
          .as_array()
          .into_iter()
          .flatten()
          .filter_map(|root| root["uri"].as_str())
          .filter_map(path_of_uri)
          .filter(|path| is_git_root(path))
          .collect();
        let changed = {
          let mut kept = locked(&server.roots);
          let changed = *kept != roots;
          kept.clone_from(&roots);
          changed
        };
        if changed {
          server.ctx.events.emit_to(&WindowId::main(), "roots-changed", json!({ "roots": roots }));
        }
      }
      .in_current_span(),
    );
  }

  /// The repository the main view shows: the first Git root of the project
  fn main_repository(&self) -> Option<String> {
    locked(&self.roots).first().cloned()
  }

  fn call_tool(self: &Arc<Self>, id: Value, params: &Value) {
    let meta = &params["_meta"]["x-air-app"];
    let origin = meta["origin"].as_str();
    if !matches!(origin, Some("click" | "action")) {
      return self.link.fail(&id, NO_CONSENT, "Branch Deck takes no call of an agent: an agent runs brd", "agentOrigin");
    }
    let instance = meta["instance"].as_str().map(str::to_owned);
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    match params["name"].as_str() {
      Some("bd_invoke") => self.invoke(id, &arguments, instance.as_deref()),
      Some("bd_bootstrap") => {
        let server = Arc::clone(self);
        let view = arguments["view"].as_str().unwrap_or(MAIN_VIEW).to_owned();
        tokio::spawn(
          async move {
            let result = server.bootstrap(&view, instance.as_deref()).await;
            server.link.respond(&id, json!({ "content": [], "structuredContent": result }));
          }
          .in_current_span(),
        );
      }
      // the action Sync Virtual Branches of the app file: a sync of the repository of the main view
      Some("sync") if origin == Some("action") => {
        let repository_path = self.main_repository().unwrap_or_default();
        let sync = json!({ "command": "sync", "args": { "params": { "repositoryPath": repository_path, "reason": "manual" } } });
        self.invoke(id, &sync, instance.as_deref());
      }
      name => self
        .link
        .fail(&id, INVALID_PARAMS, &format!("Branch Deck has no tool {}", name.unwrap_or_default()), "unknownTool"),
    }
  }

  /// Runs the command of `arguments` for the window of `instance`, the events of its channel as call events of `id`
  fn invoke(self: &Arc<Self>, id: Value, arguments: &Value, instance: Option<&str>) {
    let command = arguments["command"].as_str().unwrap_or_default();
    let args = arguments.get("args").cloned().unwrap_or(Value::Null);
    if let Some(value) = self.host_command(command, &args) {
      return self.link.respond(&id, json!({ "content": [], "structuredContent": { "value": value } }));
    }
    let args = self.with_subscription(command, args, instance);
    let prepared = match prepare(command, args) {
      Ok(prepared) => prepared,
      Err(error) => return self.link.respond(&id, tool_error(&error)),
    };
    let call = self.ctx.call(&instance.map_or_else(|| MAIN_WINDOW.to_owned(), label_of));
    let key = id.to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    locked(&self.calls).insert(
      key.clone(),
      Running {
        abort: None,
        read: prepared.spec.class == CommandClass::Read,
        cancelled: Arc::clone(&cancelled),
      },
    );
    let events = Arc::new(CallEvents {
      link: Arc::clone(&self.link),
      call_id: id.clone(),
      cancelled: Arc::clone(&cancelled),
    });
    let command = tokio::spawn(prepared.run(Arc::clone(&self.ctx), call, events).in_current_span());
    if let Some(running) = locked(&self.calls).get_mut(&key) {
      running.abort = Some(command.abort_handle());
    }
    let server = Arc::clone(self);
    tokio::spawn(
      async move {
        let answer = command.await;
        locked(&server.calls).remove(&key);
        if cancelled.load(Ordering::SeqCst) {
          return;
        }
        match answer {
          Ok(Ok(Answer::Json(value))) => server.link.respond(&id, json!({ "content": [], "structuredContent": { "value": value } })),
          Ok(Ok(Answer::Bytes(bytes))) => {
            let bytes = base64::engine::general_purpose::STANDARD.encode(bytes);
            server.link.respond(&id, json!({ "content": [], "structuredContent": { "bytes": bytes } }));
          }
          Ok(Err(error)) => server.link.respond(&id, tool_error(&error)),
          Err(failure) => {
            tracing::error!("A command panicked: {failure}");
            server.link.fail(&id, INTERNAL_ERROR, "The command failed in brd", "badResponse");
          }
        }
      }
      .in_current_span(),
    );
  }

  /// What the desktop does beside the commands, which the page asks through `bd_invoke` by the plugin call of Tauri it
  /// makes in the desktop app: the badge and the progress of the Dock mark the tool window, a notification of the OS is
  /// one of the IDE. None for a command of app-api
  fn host_command(&self, command: &str, args: &Value) -> Option<Value> {
    match command {
      "plugin:window|set_badge_count" => {
        let count = args["value"].as_u64().filter(|count| *count > 0);
        self.link.ask("x-air-app/setBadge", json!({ "view": MAIN_VIEW, "count": count }));
      }
      "plugin:window|set_progress_bar" => {
        let value = &args["value"];
        let fraction = (value["status"].as_str() == Some("normal")).then(|| value["progress"].as_f64().unwrap_or(0.0).clamp(0.0, 100.0) / 100.0);
        self.link.ask("x-air-app/setProgress", json!({ "view": MAIN_VIEW, "fraction": fraction }));
      }
      "plugin:notification|notify" => {
        let options = &args["options"];
        let mut notification = json!({ "title": options["title"].as_str().unwrap_or("Branch Deck") });
        if let Some(body) = options["body"].as_str() {
          notification["body"] = Value::String(body.to_owned());
        }
        if let Some(actions) = options.get("actions").filter(|actions| actions.is_array()) {
          notification["actions"] = actions.clone();
        }
        self.link.ask("x-air-app/notify", notification);
      }
      _ => return None,
    }
    Some(Value::Null)
  }

  /// `set_demand` of a view names the subscription of its mirror, which the page does not know: the mirror of the
  /// instance gives it
  fn with_subscription(&self, command: &str, mut args: Value, instance: Option<&str>) -> Value {
    if command != "set_demand" {
      return args;
    }
    let subscription = instance.and_then(|instance| {
      locked(&self.mirrors)
        .values()
        .find(|mirror| mirror.instances.contains(instance))
        .map(|mirror| mirror.subscription)
    });
    if let Some(subscription) = subscription {
      args["params"]["subscription"] = json!(subscription);
    }
    args
  }

  /// air-ide cancelled the call `request_id`: a read stops, and no call gives an answer or an event after it
  fn cancel(&self, request_id: &Value) {
    let Some(running) = locked(&self.calls).remove(&request_id.to_string()) else {
      return;
    };
    running.cancelled.store(true, Ordering::SeqCst);
    if running.read
      && let Some(abort) = running.abort
    {
      abort.abort();
    }
  }

  /// What the page takes before its main (`bd_bootstrap {view}`): the settings of its window as the desktop app hands
  /// them in its init script, the warm frame of the repository of the main view, the init data of the view and the
  /// title bar
  async fn bootstrap(&self, view: &str, instance: Option<&str>) -> Value {
    let label = instance.map_or_else(|| MAIN_WINDOW.to_owned(), label_of);
    let (version, values) = window_snapshot(&self.ctx.settings, &label);
    let title_bar: Value = serde_json::from_str(TITLE_BAR).unwrap_or(Value::Null);
    let shown = instance.and_then(|instance| self.instances.params(instance)).unwrap_or_else(|| json!({}));
    let (warm_frame, init_data) = if view == MAIN_VIEW || instance.is_some_and(|instance| view_of(instance) == MAIN_VIEW) {
      let repository = self.main_repository();
      let roots = locked(&self.roots).clone();
      let frame = match &repository {
        Some(path) => app_api::app::warm_start::seed(&self.ctx, path).await,
        None => None,
      };
      (json!(frame), json!({ "repositoryPath": repository, "roots": roots, "params": shown }))
    } else {
      (Value::Null, json!({ "data": shown.get("data").cloned().unwrap_or(Value::Null), "params": shown }))
    };
    json!({
      "settings": { "values": values, "version": version },
      "warmFrame": warm_frame,
      "initData": init_data,
      "titleBar": title_bar,
    })
  }

  /// Subscribes `instance` to the mirror of the repository its params name: the session subscribes once for every view
  /// of the repository. Answered in the order of the messages, so a second view of a repository joins the mirror the
  /// first opened
  async fn subscribe(&self, id: Value, params: &Value) {
    let instance = params["instance"].as_str().unwrap_or_default().to_owned();
    let Some(path) = params["params"]["repositoryPath"].as_str().map(str::to_owned) else {
      return self
        .link
        .fail(&id, INVALID_PARAMS, "A view of Branch Deck subscribes to a repository: params.repositoryPath", "badParams");
    };
    {
      let mut mirrors = locked(&self.mirrors);
      if let Some(mirror) = mirrors.get_mut(&path) {
        mirror.instances.insert(instance.clone());
        let slot = locked(&mirror.slot);
        if let Slot::Live(view) = &*slot {
          self.link.respond(&id, json!({ "mirror": path, "state": view.state() }));
          return;
        }
      }
    }
    let window = WindowId::new(format!("mirror:{path}"));
    let slot = Arc::new(Mutex::new(Slot::Waiting(Vec::new())));
    let patches = Arc::new(MirrorPatches {
      mirror: path.clone(),
      slot: Arc::clone(&slot),
      link: Arc::clone(&self.link),
    });
    let call = self.ctx.call(window.as_str());
    let args = app_api::commands::subscribe_view::Args {
      params: SubscribeViewParams {
        repository_path: path.clone(),
        have: None,
      },
    };
    let subscribed = app_api::commands::subscribe_view::Command::call(&self.ctx, &call, args, patches).await;
    let subscription = match subscribed {
      Ok(subscription) => subscription,
      Err(error) => {
        *locked(&slot) = Slot::Ended;
        return self.link.send(&json!({
          "jsonrpc": "2.0",
          "id": id,
          "error": { "code": INTERNAL_ERROR, "message": error.message, "data": { "reason": "badResponse", "error": error } },
        }));
      }
    };
    let Some(view) = subscription.view else {
      *locked(&slot) = Slot::Ended;
      return self.link.fail(&id, INTERNAL_ERROR, "The session gave no view", "badResponse");
    };
    {
      // the answer before any change of the mirror: the patches that came meanwhile are in its state
      let mut held = locked(&slot);
      let mut mirror = MirrorView::new(view);
      if let Slot::Waiting(patches) = std::mem::replace(&mut *held, Slot::Ended) {
        for patch in patches {
          mirror.catch_up(patch);
        }
      }
      self.link.respond(&id, json!({ "mirror": path, "state": mirror.state() }));
      *held = Slot::Live(Box::new(mirror));
    }
    locked(&self.mirrors).insert(
      path,
      Mirror {
        window,
        subscription: subscription.id,
        instances: HashSet::from([instance]),
        slot,
      },
    );
  }

  /// `instance` leaves its mirror; the last view of a mirror ends its subscription
  fn leave(&self, instance: &str) {
    let ended: Vec<Mirror> = {
      let mut mirrors = locked(&self.mirrors);
      for mirror in mirrors.values_mut() {
        mirror.instances.remove(instance);
      }
      let empty: Vec<String> = mirrors.iter().filter(|(_, mirror)| mirror.instances.is_empty()).map(|(path, _)| path.clone()).collect();
      empty.iter().filter_map(|path| mirrors.remove(path)).collect()
    };
    for mirror in ended {
      *locked(&mirror.slot) = Slot::Ended;
      self.ctx.forget_page(&mirror.window);
    }
  }

  /// The connection ended: every mirror with it, and no command answers any more
  fn end(&self) {
    let mirrors: Vec<Mirror> = locked(&self.mirrors).drain().map(|(_, mirror)| mirror).collect();
    for mirror in mirrors {
      *locked(&mirror.slot) = Slot::Ended;
      self.ctx.forget_page(&mirror.window);
    }
    for (_, running) in locked(&self.calls).drain() {
      running.cancelled.store(true, Ordering::SeqCst);
      if let Some(abort) = running.abort {
        abort.abort();
      }
    }
  }
}

/// The two tools of Branch Deck, both only for the app (`_meta.ui.visibility`): no agent sees them
fn tools() -> Value {
  json!([
    {
      "name": "bd_invoke",
      "description": "Runs one command of Branch Deck by its name, as the page of the desktop app invokes it.",
      "inputSchema": { "type": "object", "properties": { "command": { "type": "string" }, "args": { "type": "object" } }, "required": ["command"] },
      "_meta": { "ui": { "visibility": ["app"] } },
    },
    {
      "name": "bd_bootstrap",
      "description": "What the page of a view takes before its main: the settings, the warm frame, the init data of the view and the title bar.",
      "inputSchema": { "type": "object", "properties": { "view": { "type": "string" } }, "required": ["view"] },
      "_meta": { "ui": { "visibility": ["app"] } },
    },
  ])
}

/// The result of a call whose command failed: its `ApiError`, which the page acts on by its code
fn tool_error(error: &ApiError) -> Value {
  json!({ "content": [{ "type": "text", "text": error.message }], "isError": true, "structuredContent": { "error": error } })
}

/// The path of a `file://` URI of a root
fn path_of_uri(uri: &str) -> Option<String> {
  let path = uri.strip_prefix("file://")?;
  // a Windows root is `file:///C:/…`
  let path = if path.len() > 2 && path.as_bytes()[2] == b':' { &path[1..] } else { path };
  Some(percent_decoded(path))
}

fn percent_decoded(text: &str) -> String {
  let bytes = text.as_bytes();
  let mut decoded = Vec::with_capacity(bytes.len());
  let mut index = 0;
  while index < bytes.len() {
    if bytes[index] == b'%'
      && let Some(byte) = text.get(index + 1..index + 3).and_then(|hex| u8::from_str_radix(hex, 16).ok())
    {
      decoded.push(byte);
      index += 3;
      continue;
    }
    decoded.push(bytes[index]);
    index += 1;
  }
  String::from_utf8_lossy(&decoded).into_owned()
}

/// A root is a Git root when it has a `.git` of its own (a directory, or the file of a linked worktree)
fn is_git_root(path: &str) -> bool {
  Path::new(path).join(".git").exists()
}

/// The channel of a command as the call events of its call
struct CallEvents {
  link: Arc<Link>,
  call_id: Value,
  cancelled: Arc<AtomicBool>,
}

impl EventSink<Value> for CallEvents {
  fn send(&self, event: Value) -> Result<(), SinkGone> {
    // a cancelled call has no page to hear it; a write runs to its end all the same
    if !self.cancelled.load(Ordering::SeqCst) {
      self.link.notify("x-air-app/callEvent", json!({ "callId": self.call_id, "event": event }));
    }
    Ok(())
  }
}

/// The patches of the session of a mirror, as its changes
struct MirrorPatches {
  mirror: String,
  slot: Arc<Mutex<Slot>>,
  link: Arc<Link>,
}

impl EventSink<ViewPatch> for MirrorPatches {
  fn send(&self, patch: ViewPatch) -> Result<(), SinkGone> {
    // the change is written while the view is held, so the changes go out in the order of the patches
    let mut slot = locked(&self.slot);
    match &mut *slot {
      Slot::Waiting(patches) => patches.push(patch),
      Slot::Live(view) => match view.apply(patch) {
        Ok(ops) if ops.is_empty() => {}
        Ok(ops) => self.link.notify("x-air-app/viewChanged", json!({ "mirror": self.mirror, "ops": ops })),
        Err(error) => tracing::warn!(%error, mirror = self.mirror, "a patch of the session does not apply to its mirror"),
      },
      Slot::Ended => return Err(SinkGone),
    }
    Ok(())
  }
}

/// The session dropped the subscription while a view showed the mirror: the session closed
impl Drop for MirrorPatches {
  fn drop(&mut self) {
    let mut slot = locked(&self.slot);
    if matches!(&*slot, Slot::Live(_)) {
      *slot = Slot::Ended;
      self.link.notify("x-air-app/viewEnded", json!({ "mirror": self.mirror, "reason": "sessionClosed" }));
    }
  }
}

#[cfg(test)]
#[path = "server_test.rs"]
mod server_test;
