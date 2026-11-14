//! The connection of `brd mcp` with air-ide (openspec/specs/repository/ide-embedding/spec.md;
//! docs/design/ide-embedding.md): MCP (`initialize`, `tools/list`, `tools/call`, `resources/read`, `roots/list`) with
//! the vocabulary `x-air-app` (the mirrors of the views, the events, the life of the places).
//!
//! - **Tools**: `bd_invoke {command, args}` runs one command of app-api by its name (`app_api::dispatch`) for the window
//!   of the instance of the call, the events of its channel as `x-air-app/callEvent {callId, event}` (the id of the
//!   call), each before the answer; `bd_bootstrap {view}` gives what the page takes before its main; `sync` and
//!   `open_command_palette` are the actions of the app file (Sync Virtual Branches, Branch Deck Command Palette), and
//!   `reveal_branch` the action of a notification of brd (`notices.rs`). Each is only for the app, and a call of the
//!   origin `agent` is refused. A cancel stops a read and leaves no answer; a write
//!   runs to its end, its answer left out.
//! - **Mirrors** (`mirror.rs`): `x-air-app/subscribe` of a repository subscribes its session once for every view of it,
//!   under a window of its own (`mirror:<path>`), so the close of one view ends no other's mirror; the last
//!   `x-air-app/unsubscribe` ends it. A subscription again to a mirror brd holds, by another view or by `bd_invoke` of
//!   `subscribe_view` (a frame holds one view, so the page asks it so), follows the prefix the git config names now: a
//!   mirror of another prefix moves to the session of the new one in one change of every part. A patch into another
//!   epoch of its session (a slot of a lease pool leased again, which opens the session anew) moves the mirror the same
//!   way, as no patch of another epoch applies to its view. The news a patch brings are told by the IDE's notifications
//!   (`notices.rs`).
//! - **Places**: `x-air-app/viewShown` records the params of an instance, `x-air-app/viewHidden` that the IDE hides it,
//!   `x-air-app/viewClosed` forgets its page and its window (`AppContext::forget_window`); the end of a WebSocket never reaches brd.
//! - **Roots**: the repositories are the Git roots of the project (`roots/list`, asked at the start and on
//!   `notifications/roots/list_changed`), which the views see as their recent projects; the main view shows the root
//!   picked last for these roots, else the first (`projects.rs`).
//!
//! Every message brd writes goes through the one queue of `Link`, in the order it was sent: a command that changes the
//! view sends its patches (from the session, through the mirror) and its events before its answer.

use crate::hosts::{Instances, MAIN_VIEW, McpEvents, desktop, label_of, view_of};
use crate::link::{INTERNAL_ERROR, INVALID_PARAMS, Link, METHOD_NOT_FOUND, NO_CONSENT, Reply};
use crate::mirror::MirrorView;
use crate::notices::{Notices, REVEAL_BRANCH_EVENT, REVEAL_BRANCH_TOOL};
use crate::projects::{Projects, RECENT_PROJECTS_KEY, merged};
use crate::resources;
use app_api::context::AppContext;
use app_api::dispatch::{Answer, prepare};
use app_api::events::settings_changed;
use app_api::registry::{Command, CommandClass};
use app_api::transport::{AppEvents, Desktop, EventSink, SinkGone, WindowId};
use app_settings::ide_roots;
use app_settings::windows::{MAIN_WINDOW, window_snapshot};
use base64::Engine;
use domain_types::error::ApiError;
use domain_types::session::{Demand, RepositoryView, SubscribeViewParams, ViewPatch};
use domain_types::settings::SettingsPatch;
use repository_model::patch::PatchRejected;
use repository_model::session::SubscriptionId;
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

/// One connection of air-ide with brd: what its messages go through, and the instances of its views, which the hosts of
/// the context it serves share (`events`, `desktop`)
pub struct Connection {
  link: Arc<Link>,
  instances: Arc<Instances>,
  projects: Arc<Projects>,
}

impl Connection {
  /// A connection whose messages come out of the receiver, one JSON object each, in the order brd sent them
  pub fn new() -> (Self, mpsc::UnboundedReceiver<String>) {
    let (link, out) = Link::new();
    let connection = Self {
      link: Arc::new(link),
      instances: Arc::default(),
      projects: Arc::default(),
    };
    (connection, out)
  }

  /// The events of the app, which the context the connection serves sends
  pub fn events(&self) -> Arc<dyn AppEvents> {
    Arc::new(McpEvents {
      link: Arc::clone(&self.link),
      instances: Arc::clone(&self.instances),
      projects: Arc::clone(&self.projects),
    })
  }

  /// The desktop of the app in the IDE, which the context the connection serves does on
  pub fn desktop(&self) -> Desktop {
    desktop(&self.link, &self.instances)
  }

  /// Serves the messages of `incoming`, each one JSON-RPC message, in `ctx`, until it ends; then every window of the
  /// connection is forgotten (`AppContext::forget_windows`): its subscriptions end
  pub async fn serve(self, ctx: Arc<AppContext>, mut incoming: mpsc::UnboundedReceiver<String>) {
    let notices = Arc::new(Notices::new(Arc::clone(&self.link), Arc::clone(&self.instances), Arc::clone(&ctx.settings)));
    let (resyncs, mut resynced) = mpsc::unbounded_channel();
    let server = Arc::new(Server {
      ctx: Arc::clone(&ctx),
      link: self.link,
      instances: self.instances,
      notices,
      projects: self.projects,
      bootstrapped: AtomicBool::new(false),
      calls: Mutex::default(),
      mirrors: Mutex::default(),
      resyncs,
    });
    loop {
      let line = tokio::select! {
        line = incoming.recv() => line,
        Some(resync) = resynced.recv() => {
          server.resync(&resync).await;
          continue;
        }
      };
      let Some(line) = line else {
        break;
      };
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
  /// The demand a view set last, which the session of another prefix takes when the mirror moves to it
  demand: Option<Demand>,
}

/// The subscription of a session that answered its view, before the view goes live in its slot
struct Opened {
  subscription: u32,
  slot: Arc<Mutex<Slot>>,
  view: RepositoryView,
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
  /// The notifications of the news of the mirrors
  notices: Arc<Notices>,
  /// The commands that run for calls of tools, by the id of the call
  calls: Mutex<HashMap<String, Running>>,
  /// The mirrors, by the path of their repository
  mirrors: Mutex<HashMap<String, Mirror>>,
  /// The Git roots of the project and the one the main view shows
  projects: Arc<Projects>,
  /// A page took the settings (`bd_bootstrap`): a change of the roots reaches it as a change of the settings
  bootstrapped: AtomicBool,
  /// The mirrors whose session sent a patch into another epoch, each with the slot that got it: served between the
  /// messages of air-ide, so a move of a mirror never runs beside a subscription or another move
  resyncs: mpsc::UnboundedSender<Resync>,
}

/// A patch into another epoch of the session of a mirror reached its slot (`MirrorPatches`): the mirror reads the view of
/// the session anew (`Server::resync`)
struct Resync {
  mirror: String,
  slot: Arc<Mutex<Slot>>,
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
      "tools/call" => self.call_tool(id, &params).await,
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
      "x-air-app/viewHidden" => self.instances.hidden(params["instance"].as_str().unwrap_or_default()),
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
        let remembered = ide_roots::picked(&server.ctx.settings, &roots);
        if server.projects.set_roots(roots.clone(), remembered) {
          server.ctx.events.emit_to(&WindowId::main(), "roots-changed", json!({ "roots": roots }));
          server.tell_recent_projects();
        }
      }
      .in_current_span(),
    );
  }

  /// The repository the main view shows: the Git root of the project the user picked last, else the first
  fn main_repository(&self) -> Option<String> {
    self.projects.shown()
  }

  /// The roots changed after a page took the settings: the views get the recent projects anew, as a change of the
  /// settings that writes them as they are, so the version of the change is newer than every one a view has
  fn tell_recent_projects(&self) {
    if !self.bootstrapped.load(Ordering::SeqCst) {
      return;
    }
    let recent = self.ctx.settings.value(RECENT_PROJECTS_KEY);
    let patch = SettingsPatch::Value {
      key: RECENT_PROJECTS_KEY.to_owned(),
      value: recent,
    };
    match self.ctx.settings.set(patch, "rust") {
      Ok(change) => settings_changed(&*self.ctx.events, &change),
      Err(error) => tracing::warn!(%error, "the views do not hear the roots of the project"),
    }
  }

  /// A write of the recent projects by the page of the IDE, whose list is the roots of the project: its first is the
  /// root the main view shows now, which brd remembers for this set of roots, and what it changed of each root goes into
  /// the recent projects of the settings, in their order, which stays the desktop app's
  fn with_recent_projects(&self, command: &str, mut args: Value) -> Value {
    if command != "set_setting" || args["params"]["patch"]["kind"] != "value" || args["params"]["patch"]["key"] != RECENT_PROJECTS_KEY {
      return args;
    }
    let seen = args["params"]["patch"]["value"].take();
    if let Some(picked) = seen[0]["path"].as_str()
      && self.projects.pick(picked)
      && let Err(error) = ide_roots::remember(&self.ctx.settings, &self.projects.roots(), picked)
    {
      tracing::warn!(%error, "the root the main view shows is not remembered");
    }
    args["params"]["patch"]["value"] = merged(&self.ctx.settings.value(RECENT_PROJECTS_KEY), &seen);
    args
  }

  async fn call_tool(self: &Arc<Self>, id: Value, params: &Value) {
    let meta = &params["_meta"]["x-air-app"];
    let origin = meta["origin"].as_str();
    if !matches!(origin, Some("click" | "action")) {
      return self.link.fail(&id, NO_CONSENT, "Branch Deck takes no call of an agent: an agent runs brd", "agentOrigin");
    }
    let instance = meta["instance"].as_str().map(str::to_owned);
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    match params["name"].as_str() {
      // the page subscribes again to the repository whose mirror its frame holds: answered in the order of the messages,
      // as a subscription is, after the change that moves the mirror
      Some("bd_invoke") if arguments["command"] == "subscribe_view" => self.subscribe_again(id, &arguments).await,
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
      Some(SYNC_TOOL) => {
        let repository_path = self.main_repository().unwrap_or_default();
        let sync = json!({ "command": "sync", "args": { "params": { "repositoryPath": repository_path, "reason": "manual" } } });
        self.invoke(id, &sync, instance.as_deref());
      }
      // the action Branch Deck Command Palette: the palette of the main view, as the menu of the desktop app opens it
      Some(COMMAND_PALETTE_TOOL) => {
        self.ctx.events.emit_to(&WindowId::main(), COMMAND_PALETTE_TOOL, Value::Null);
        self.link.respond(&id, json!({ "content": [], "structuredContent": { "value": null } }));
      }
      // Show in Branch Deck of a notification of brd: the main view shows, and reveals the branch
      Some(REVEAL_BRANCH_TOOL) => {
        self.reveal_branch(&arguments);
        self.link.respond(&id, json!({ "content": [], "structuredContent": { "value": null } }));
      }
      name => self
        .link
        .fail(&id, INVALID_PARAMS, &format!("Branch Deck has no tool {}", name.unwrap_or_default()), "unknownTool"),
    }
  }

  /// Asks the IDE to show the main view, then tells the instance it answered to reveal the branch of `arguments`
  /// (`{repositoryPath, branchName}`), which the page does when it shows that repository
  fn reveal_branch(&self, arguments: &Value) {
    let payload = json!({ "repositoryPath": arguments["repositoryPath"], "branchName": arguments["branchName"] });
    let answer = self.link.request("x-air-app/openView", Some(json!({ "view": MAIN_VIEW })));
    let link = Arc::clone(&self.link);
    tokio::spawn(
      async move {
        match answer.await {
          Ok(Ok(result)) => {
            if let Some(instance) = result["instance"].as_str() {
              link.notify("x-air-app/event", json!({ "instance": instance, "event": REVEAL_BRANCH_EVENT, "payload": payload }));
            }
          }
          Ok(Err(error)) => tracing::warn!(%error, "the IDE shows no main view"),
          Err(_) => {}
        }
      }
      .in_current_span(),
    );
  }

  /// Runs the command of `arguments` for the window of `instance`, the events of its channel as call events of `id`
  fn invoke(self: &Arc<Self>, id: Value, arguments: &Value, instance: Option<&str>) {
    let command = arguments["command"].as_str().unwrap_or_default();
    let args = arguments.get("args").cloned().unwrap_or(Value::Null);
    if let Some(value) = self.host_command(command, &args, instance) {
      return self.link.respond(&id, json!({ "content": [], "structuredContent": { "value": value } }));
    }
    let args = self.with_subscription(command, args, instance);
    let args = self.with_recent_projects(command, args);
    let changes_settings = SETTINGS_COMMANDS.contains(&command);
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
          Ok(Ok(Answer::Json(mut value))) => {
            if changes_settings {
              server.projects.seen_changes(&mut value);
            }
            server.link.respond(&id, json!({ "content": [], "structuredContent": { "value": value } }));
          }
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
  /// makes in the desktop app: an event the page emits goes to the views of the app (to one window with a target), the
  /// badge and the progress of the Dock mark the tool window, a notification of the OS is one of the IDE (none of a news
  /// brd told itself, `Notices::page_notification`), and the close
  /// of the window of `instance` (Escape in a review or a dialog) closes its place. None for a command of app-api
  fn host_command(&self, command: &str, args: &Value, instance: Option<&str>) -> Option<Value> {
    match command {
      "plugin:event|emit" => {
        let (event, payload) = (args["event"].as_str().unwrap_or_default(), args.get("payload").cloned().unwrap_or(Value::Null));
        match args["target"].as_str() {
          Some(label) => self.ctx.events.emit_to(&WindowId::new(label), event, payload),
          None => self.ctx.events.emit(event, payload),
        }
      }
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
        if let Some(notification) = self.notices.page_notification(&args["options"]) {
          self.link.ask("x-air-app/notify", notification);
        }
      }
      "plugin:window|close" | "plugin:window|destroy" => {
        if let Some(instance) = instance {
          self.link.ask("x-air-app/closeView", json!({ "instance": instance }));
        }
      }
      _ => return None,
    }
    Some(Value::Null)
  }

  /// `set_demand` of a view names the subscription of its mirror, which the page does not know: the mirror of the
  /// instance gives it, and keeps the demand for the session it moves to
  fn with_subscription(&self, command: &str, mut args: Value, instance: Option<&str>) -> Value {
    if command != "set_demand" {
      return args;
    }
    let demand = serde_json::from_value::<Demand>(args["params"]["demand"].clone()).ok();
    let subscription = instance.and_then(|instance| {
      let mut mirrors = locked(&self.mirrors);
      let mirror = mirrors.values_mut().find(|mirror| mirror.instances.contains(instance))?;
      mirror.demand.clone_from(&demand);
      Some(mirror.subscription)
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
    self.bootstrapped.store(true, Ordering::SeqCst);
    let (version, mut values) = window_snapshot(&self.ctx.settings, &label);
    if let Some(recent) = values.get_mut(RECENT_PROJECTS_KEY) {
      *recent = self.projects.seen(recent);
    } else if let Value::Object(values) = &mut values {
      values.insert(RECENT_PROJECTS_KEY.to_owned(), self.projects.seen(&Value::Null));
    }
    // the place of the view is the IDE's, the page draws no title bar of its own; the app updates with the desktop app,
    // and the roots of the project name the repositories where the host has no folder dialog
    let title_bar = json!({ "kind": "native", "checkForUpdates": false, "folderDialog": self.ctx.desktop.dialog.has_folder_dialog() });
    let shown = instance.and_then(|instance| self.instances.params(instance)).unwrap_or_else(|| json!({}));
    let (warm_frame, init_data) = if view == MAIN_VIEW || instance.is_some_and(|instance| view_of(instance) == MAIN_VIEW) {
      let repository = self.main_repository();
      let roots = self.projects.roots();
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
  /// first opened, which takes the session of the prefix the git config names now first (`follow_prefix`)
  async fn subscribe(&self, id: Value, params: &Value) {
    let instance = params["instance"].as_str().unwrap_or_default().to_owned();
    let Some(path) = params["params"]["repositoryPath"].as_str().map(str::to_owned) else {
      return self
        .link
        .fail(&id, INVALID_PARAMS, "A view of Branch Deck subscribes to a repository: params.repositoryPath", "badParams");
    };
    if let Err(error) = self.follow_prefix(&path).await {
      return self.fail_subscription(&id, &error);
    }
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
    let opened = match self.open_session(&path).await {
      Ok(opened) => opened,
      Err(error) => return self.fail_subscription(&id, &error),
    };
    // the answer before any change of the mirror
    go_live(&opened.slot, opened.view, |mirror| {
      self.link.respond(&id, json!({ "mirror": path, "state": mirror.state() }));
    });
    locked(&self.mirrors).insert(
      path.clone(),
      Mirror {
        window: mirror_window(&path),
        subscription: opened.subscription,
        instances: HashSet::from([instance]),
        slot: opened.slot,
        demand: None,
      },
    );
  }

  /// `bd_invoke` of `subscribe_view`: the page subscribes again to the repository whose mirror its frame holds (the
  /// prefix that names its branches may have changed, `resubscribeSession` of `src/model/store.ts`), and its frame holds
  /// one view, so the page shares the subscription it has and asks brd. Answered after the change that moves the mirror
  /// to the session of the prefix the git config names now; the error of that session, such as `no_branch_prefix`, as
  /// the answer of a command
  async fn subscribe_again(&self, id: Value, arguments: &Value) {
    let path = arguments["args"]["params"]["repositoryPath"].as_str().unwrap_or_default();
    match self.follow_prefix(path).await {
      Ok(()) => self.link.respond(&id, json!({ "content": [], "structuredContent": { "value": null } })),
      Err(error) => self.link.respond(&id, tool_error(&error)),
    }
  }

  /// The mirror of `path` takes the session of the prefix the git config names now (`AppContext::session_key_now`), as
  /// a window of the desktop app that subscribes again does: a mirror of another prefix moves to the session of the new
  /// one, one change that replaces every part with its view, a `version` of another epoch, after which no patch of the
  /// old session reaches it; the session of the new prefix takes the demand of the mirror, the old one loses it. Nothing
  /// for a mirror of that prefix, or no mirror that shows a view
  ///
  /// # Errors
  /// No session of the repository now: the repository is not accessible, or it has no prefix
  async fn follow_prefix(&self, path: &str) -> Result<(), ApiError> {
    let held = {
      let mirrors = locked(&self.mirrors);
      mirrors.get(path).and_then(|mirror| match &*locked(&mirror.slot) {
        Slot::Live(view) => Some(view.view().header.prefix.clone()),
        _ => None,
      })
    };
    let Some(prefix) = held else {
      return Ok(());
    };
    if self.ctx.session_key_now(path).await?.prefix == prefix {
      return Ok(());
    }
    self.move_mirror(path).await
  }

  /// A patch into another epoch reached the slot of `resync`, which still is the slot of its mirror: the session opened
  /// anew (a slot of a lease pool leased again), and no patch of it applies to the view of the mirror. The mirror moves to
  /// the session of the repository now, as at a change of the prefix (`move_mirror`): the view its subscription answers,
  /// one change of every part, a `version` of another epoch. A session that answers no view ends the mirror, as a session
  /// that closed does. Nothing for a mirror that moved or ended meanwhile: its slot was replaced
  async fn resync(&self, resync: &Resync) {
    let held = locked(&self.mirrors).get(&resync.mirror).is_some_and(|mirror| Arc::ptr_eq(&mirror.slot, &resync.slot));
    if !held {
      return;
    }
    if let Err(error) = self.move_mirror(&resync.mirror).await {
      tracing::warn!(%error, mirror = resync.mirror, "the mirror cannot read the view of its session anew");
      let mut slot = locked(&resync.slot);
      if matches!(&*slot, Slot::Live(_)) {
        *slot = Slot::Ended;
        self.link.notify("x-air-app/viewEnded", json!({ "mirror": resync.mirror, "reason": "sessionClosed" }));
      }
    }
  }

  /// The mirror of `path` moves to the session of the repository the git config names now: a new subscription of that
  /// session, whose view goes live in one change of every part, after which no patch of the subscription before reaches
  /// the mirror; the session takes the demand of the mirror, the subscription before loses it. Nothing for a mirror that
  /// no view holds any more
  ///
  /// # Errors
  /// No session of the repository now: the repository is not accessible, or it has no prefix
  async fn move_mirror(&self, path: &str) -> Result<(), ApiError> {
    let opened = self.open_session(path).await?;
    let moved = locked(&self.mirrors).get_mut(path).map(|mirror| {
      let left = std::mem::replace(&mut mirror.slot, Arc::clone(&opened.slot));
      let old = std::mem::replace(&mut mirror.subscription, opened.subscription);
      (left, old, mirror.demand.clone())
    });
    let Some((left, old, demand)) = moved else {
      // the views left the mirror meanwhile
      *locked(&opened.slot) = Slot::Ended;
      self.drop_session_subscription(path, opened.subscription);
      return Ok(());
    };
    // the old session's patches and its end reach no view any more
    *locked(&left) = Slot::Ended;
    let mirror = path.to_owned();
    go_live(&opened.slot, opened.view, |view| {
      self.link.notify("x-air-app/viewChanged", json!({ "mirror": mirror, "ops": view.parts() }));
    });
    self.drop_session_subscription(path, old);
    if let Some(demand) = demand {
      self.ctx.sessions.set_demand(SubscriptionId(opened.subscription), &demand);
    }
    Ok(())
  }

  /// Subscribes the session of `path` the git config names now under the window of its mirror; its patches wait in the
  /// slot until its view goes live (`go_live`)
  async fn open_session(&self, path: &str) -> Result<Opened, ApiError> {
    let slot = Arc::new(Mutex::new(Slot::Waiting(Vec::new())));
    let patches = Arc::new(MirrorPatches {
      mirror: path.to_owned(),
      slot: Arc::clone(&slot),
      link: Arc::clone(&self.link),
      notices: Arc::clone(&self.notices),
      resyncs: self.resyncs.clone(),
    });
    let call = self.ctx.call(mirror_window(path).as_str());
    let args = app_api::commands::subscribe_view::Args {
      params: SubscribeViewParams {
        repository_path: path.to_owned(),
        have: None,
      },
    };
    let subscribed = app_api::commands::subscribe_view::Command::call(&self.ctx, &call, args, patches).await;
    let subscription = subscribed.inspect_err(|_| *locked(&slot) = Slot::Ended)?;
    let Some(view) = subscription.view else {
      *locked(&slot) = Slot::Ended;
      return Err(ApiError::internal("The session gave no view"));
    };
    Ok(Opened {
      subscription: subscription.id,
      slot,
      view,
    })
  }

  /// The subscription `id` of the session of `path` leaves the window of its mirror and the session
  fn drop_session_subscription(&self, path: &str, id: u32) {
    let call = self.ctx.call(mirror_window(path).as_str());
    self.ctx.windows.drop_subscription(&call, SubscriptionId(id));
    self.ctx.sessions.unsubscribe(SubscriptionId(id));
  }

  /// The subscription `id` failed with `error`: its `ApiError` in the data of the error, for air-ide to pass on
  fn fail_subscription(&self, id: &Value, error: &ApiError) {
    self.link.send(&json!({
      "jsonrpc": "2.0",
      "id": id,
      "error": { "code": INTERNAL_ERROR, "message": error.message, "data": { "reason": "badResponse", "error": error } },
    }));
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

/// The window the subscription of the mirror of `path` is kept under
fn mirror_window(path: &str) -> WindowId {
  WindowId::new(format!("mirror:{path}"))
}

/// The view of a subscription that answered goes live in its slot: the patches that came meanwhile are in it, and
/// `first` writes what comes before every change of it (the answer of the subscription, or the change that moves the
/// mirror), while the slot is held, so the changes after it follow it
fn go_live(slot: &Mutex<Slot>, view: RepositoryView, first: impl FnOnce(&MirrorView)) {
  let mut held = locked(slot);
  let mut mirror = MirrorView::new(view);
  if let Slot::Waiting(patches) = std::mem::replace(&mut *held, Slot::Ended) {
    for patch in patches {
      mirror.catch_up(patch);
    }
  }
  first(&mirror);
  *held = Slot::Live(Box::new(mirror));
}

/// The commands whose answer holds changes of the settings, which the views see with the recent projects of the roots
const SETTINGS_COMMANDS: [&str; 3] = ["set_setting", "get_settings_changes", "sub_window_loaded"];

/// The tool of the action Sync Virtual Branches of the app file
const SYNC_TOOL: &str = "sync";
/// The tool of the action Branch Deck Command Palette of the app file, and the event it sends the main view
const COMMAND_PALETTE_TOOL: &str = "open_command_palette";

/// The tools of Branch Deck, each only for the app (`_meta.ui.visibility`), so no agent sees one: `bd_invoke` and
/// `bd_bootstrap` for the page, the tool of each action of the app file for the IDE, and the tool of the action of a
/// notification of brd
fn tools() -> Value {
  let app_only = json!({ "ui": { "visibility": ["app"] } });
  let no_arguments = json!({ "type": "object", "properties": {} });
  json!([
    {
      "name": "bd_invoke",
      "description": "Runs one command of Branch Deck by its name, as the page of the desktop app invokes it.",
      "inputSchema": { "type": "object", "properties": { "command": { "type": "string" }, "args": { "type": "object" } }, "required": ["command"] },
      "_meta": app_only,
    },
    {
      "name": "bd_bootstrap",
      "description": "What the page of a view takes before its main: the settings, the warm frame, the init data of the view and the title bar.",
      "inputSchema": { "type": "object", "properties": { "view": { "type": "string" } }, "required": ["view"] },
      "_meta": app_only,
    },
    {
      "name": SYNC_TOOL,
      "description": "Syncs the virtual branches of the repository the main view shows (Sync Virtual Branches).",
      "inputSchema": no_arguments,
      "_meta": app_only,
    },
    {
      "name": COMMAND_PALETTE_TOOL,
      "description": "Opens the command palette of the main view (Branch Deck Command Palette).",
      "inputSchema": no_arguments,
      "_meta": app_only,
    },
    {
      "name": REVEAL_BRANCH_TOOL,
      "description": "Shows the main view and reveals a branch of its repository in it (Show in Branch Deck of a notification).",
      "inputSchema": {
        "type": "object",
        "properties": { "repositoryPath": { "type": "string" }, "branchName": { "type": "string" } },
        "required": ["repositoryPath", "branchName"],
      },
      "_meta": app_only,
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

/// The patches of the session of a mirror, as its changes, and the news they bring, as the notifications of the IDE
struct MirrorPatches {
  mirror: String,
  slot: Arc<Mutex<Slot>>,
  link: Arc<Link>,
  notices: Arc<Notices>,
  /// Where a patch into another epoch asks the server to read the view of the session anew (`Server::resync`)
  resyncs: mpsc::UnboundedSender<Resync>,
}

impl EventSink<ViewPatch> for MirrorPatches {
  fn send(&self, patch: ViewPatch) -> Result<(), SinkGone> {
    // the change is written while the view is held, so the changes go out in the order of the patches
    let mut slot = locked(&self.slot);
    match &mut *slot {
      Slot::Waiting(patches) => patches.push(patch),
      Slot::Live(view) => {
        // the news of a patch before the answer of the subscription are in its first view: none is told
        let news = patch.changes.news.clone();
        match view.apply(patch) {
          Ok(ops) => {
            if !ops.is_empty() {
              self.link.notify("x-air-app/viewChanged", json!({ "mirror": self.mirror, "ops": ops }));
            }
            self.notices.tell(view.view(), &self.mirror, &news);
          }
          Err(PatchRejected::ForeignEpoch { .. }) => {
            // the session opened anew: the mirror reads its view, and the patches of the new epoch until then go with
            // this subscription
            let resync = Resync {
              mirror: self.mirror.clone(),
              slot: Arc::clone(&self.slot),
            };
            let _ = self.resyncs.send(resync);
          }
          Err(error) => tracing::warn!(%error, mirror = self.mirror, "a patch of the session does not apply to its mirror"),
        }
      }
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
