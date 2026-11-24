//! The connection of the bridge `branch-deck ide` with air-ide (openspec/specs/repository/ide-embedding/spec.md;
//! docs/design/ide-embedding.md): MCP (`initialize`, `tools/list`, `tools/call`, `resources/read`, `roots/list`) with the
//! vocabulary `x-air-app` on one side, the protocol of the backend (docs/design/backend-protocol.md) on the other: the
//! bridge is a client of the one backend of the user (`backend.rs`), the desktop app over its socket, or a backend in
//! its own process (`fallback.rs`), and keeps what is the IDE's alone.
//!
//! - **Tools**: `bd_invoke {command, args}` is the root command `x-branch-deck/<command>` for the window of the instance
//!   of the call, its call events as `x-air-app/callEvent {callId, event}` (the id of the call), each before the answer;
//!   `bd_bootstrap {view, titleBar}` is `x-branch-deck/bootstrap` with what the IDE adds; `sync` and
//!   `open_command_palette` are the actions of the app file (Sync Virtual Branches, Branch Deck Command Palette), and
//!   `reveal_branch` the action of a notification of the bridge (`notices.rs`). Each is only for the app, and a call of
//!   the origin `agent` is refused. A cancel stops a read (`x-branch-deck/cancel`) and leaves no answer; a write runs to
//!   its end, its answer left out.
//! - **Mirrors** (`mirror.rs`): `x-air-app/subscribe` of a repository subscribes the repository channel once for every
//!   view of it, under a window of its own (`mirror:<path>`), so the close of one view ends no other's mirror; each
//!   `x-branch-deck/viewPatched` of that window is one `x-air-app/viewChanged`, a resync one change of every part. The
//!   last `x-air-app/unsubscribe` ends it, and the window gets the union of the demands of its views. A subscription again
//!   to a mirror the bridge holds, by another view or by `bd_invoke` of `subscribe_view` (a frame holds one view, so the
//!   page asks it so), follows the prefix the git config names now (`x-branch-deck/followPrefix`): a channel of another
//!   prefix moves to the session of the new one, a resync of its window. A patch into another epoch of its session (a
//!   slot of a lease pool leased again, which opens the session anew) reads the view anew (`get_snapshot`), one change of
//!   every part. The news a patch brings are told by the IDE's notifications (`notices.rs`).
//! - **Places**: `x-air-app/viewShown`, `viewHidden` and `viewClosed` are `x-branch-deck/windowShown`, `windowHidden` and
//!   `windowClosed` of the window of the instance; a `bd_bootstrap` is `x-branch-deck/pageLoaded` of it; the end of a
//!   WebSocket never reaches the bridge.
//! - **Host requests**: `x-branch-deck/openWindow` is `x-air-app/openView`, answered with the instance the IDE gives.
//! - **Roots**: the repositories are the Git roots of the project (`roots/list`, asked at the start and on
//!   `notifications/roots/list_changed`), which the views see as their recent projects; the main view shows the root
//!   picked last for these roots, else the first (`projects.rs`).
//!
//! Every message the bridge writes to the IDE goes through the one queue of `Link`, in the order it was sent, and the
//! frames of the backend are handled in their order on its reader (`BackendSource::run`): a command that changes the
//! view sends its patches and its events before its answer, as the backend sent them.

use crate::backend::{BackendLink, BackendSource, Reply};
use crate::hosts::{Instances, MAIN_VIEW, label_of, open_view_of, view_of};
use crate::link::{INTERNAL_ERROR, INVALID_PARAMS, Link, METHOD_NOT_FOUND, NO_CONSENT};
use crate::mirror::MirrorView;
use crate::notices::{Notices, REVEAL_BRANCH_EVENT, REVEAL_BRANCH_TOOL};
use crate::projects::{Projects, RECENT_PROJECTS_KEY, merged};
use crate::resources;
use crate::settings::SettingsMirror;
use app_api::dispatch;
use app_api::events::SETTINGS_CHANGED_EVENT;
use app_api::registry::CommandClass;
use app_settings::file::IDE_ROOT_PICKS_KEY;
use app_settings::ide_roots;
use app_settings::windows::MAIN_WINDOW;
use domain_types::error::ApiError;
use domain_types::session::{Demand, RepositoryView, ViewPatch};
use domain_types::settings::SettingsPatch;
use domain_types::title_bar::{DoubleClick, PageTitleBar, TitleBar};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;

/// The version of the vocabulary x-air-app the bridge speaks
pub const X_AIR_APP_VERSION: &str = "1.0.0";
/// The version of MCP the bridge answers
const PROTOCOL_VERSION: &str = "2025-06-18";

/// The window of the bridge's own writes of the settings (the root picked, the recent projects of the roots): no page
/// of the IDE, so every view takes the change as another's
const BRIDGE_WINDOW: &str = "bridge";

/// How long the end of the input waits for the backend to end the connection
const END_WAIT: Duration = Duration::from_secs(2);

/// The backend the bridge serves the IDE with: the writing side of its connection and its reading side, initialized
pub struct Backend {
  pub link: Arc<BackendLink>,
  pub source: BackendSource,
}

/// Which side ended the connection of the bridge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
  /// The input of the IDE ended: the IDE closed the project
  Input,
  /// The backend ended the connection first: an unexpected end, after which the IDE starts the bridge again
  Backend,
}

/// One connection of air-ide with the bridge: what its messages go through, the instances of its views and the roots of
/// its project
pub struct Connection {
  link: Arc<Link>,
  instances: Arc<Instances>,
  projects: Arc<Projects>,
}

/// The methods the bridge answers before its backend is ready: they need none
fn answers_alone(message: &Value) -> bool {
  match message.get("method").and_then(Value::as_str) {
    None => true,
    Some(method) => matches!(method, "initialize" | "ping" | "tools/list" | "resources/read"),
  }
}

impl Connection {
  /// A connection whose messages come out of the receiver, one JSON object each, in the order the bridge sent them
  pub fn new() -> (Self, mpsc::UnboundedReceiver<String>) {
    let (link, out) = Link::new();
    let connection = Self {
      link: Arc::new(link),
      instances: Arc::default(),
      projects: Arc::default(),
    };
    (connection, out)
  }

  /// Serves the messages of `incoming`, each one JSON-RPC message, with the backend `backend` gives, until either ends:
  /// which one ended. Before the backend is ready (the app starts) the bridge answers what needs none (`initialize`,
  /// `ping`, `tools/list`, `resources/read`) and keeps every other message, in its order, for it
  pub async fn serve(self, backend: impl Future<Output = Backend>, mut incoming: mpsc::UnboundedReceiver<String>) -> Ended {
    let mut kept = VecDeque::new();
    tokio::pin!(backend);
    let backend = loop {
      tokio::select! {
        backend = &mut backend => break backend,
        line = incoming.recv() => {
          let Some(line) = line else {
            return Ended::Input;
          };
          match serde_json::from_str::<Value>(&line) {
            Ok(message) if answers_alone(&message) => alone(&self.link, &message),
            Ok(message) => kept.push_back(message),
            Err(_) if line.trim().is_empty() => {}
            Err(error) => {
              tracing::warn!(%error, "a message of air-ide is not JSON");
              self.link.fail(&Value::Null, -32700, "The message is not JSON", "parse");
            }
          }
        }
      }
    };
    let Backend { link: backend, source } = backend;
    let server = Server::new(Arc::clone(&backend), self.link, self.instances, self.projects);
    let mut reader = {
      let server = Arc::clone(&server);
      tokio::spawn(
        async move {
          let link = Arc::clone(&server.backend);
          source.run(&link, |message| server.from_backend(&message)).await;
        }
        .in_current_span(),
      )
    };
    server.read_settings().await;
    for message in kept {
      Arc::clone(&server).handle(message).await;
    }
    let ended = loop {
      tokio::select! {
        _ = &mut reader => break Ended::Backend,
        line = incoming.recv() => {
          let Some(line) = line else {
            break Ended::Input;
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
      }
    };
    server.end();
    if ended == Ended::Input {
      // the IDE closed the project: the backend ends the connection, which closes every window of the bridge
      backend.close();
      if tokio::time::timeout(END_WAIT, &mut reader).await.is_err() {
        tracing::warn!("the backend did not end the connection of the bridge");
        reader.abort();
      }
    }
    ended
  }
}

/// The answer of a message the bridge answers without its backend
fn alone(link: &Link, message: &Value) {
  let Some(method) = message.get("method").and_then(Value::as_str) else {
    if let Some(id) = message.get("id") {
      let reply = message
        .get("error")
        .map_or_else(|| Ok(message.get("result").cloned().unwrap_or(Value::Null)), |error| Err(error.clone()));
      link.answered(id, reply);
    }
    return;
  };
  let id = message.get("id").cloned().unwrap_or(Value::Null);
  match method {
    "initialize" => link.respond(
      &id,
      json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {}, "resources": {}, "experimental": { "x-air-app": { "version": X_AIR_APP_VERSION } } },
        "serverInfo": { "name": "branch-deck", "version": env!("CARGO_PKG_VERSION") },
      }),
    ),
    "ping" => link.respond(&id, json!({})),
    "tools/list" => link.respond(&id, json!({ "tools": tools() })),
    "resources/read" => {
      let uri = message["params"]["uri"].as_str().unwrap_or_default();
      match resources::read(uri) {
        Some(contents) => link.respond(&id, json!({ "contents": [contents] })),
        None => link.fail(&id, INVALID_PARAMS, &format!("No resource of Branch Deck is {uri}"), "badParams"),
      }
    }
    _ => {}
  }
}

/// A command that runs for a call of a tool
struct Running {
  /// The id of its request to the backend
  request: u64,
  /// The command writes nothing: a cancel stops it
  read: bool,
  /// air-ide cancelled the call: nothing of it is sent any more
  cancelled: Arc<AtomicBool>,
}

/// The view of a mirror
enum View {
  /// Its subscription has not answered yet
  Opening,
  Live(Box<MirrorView>),
  /// A patch into another epoch came: the view is read anew (`get_snapshot`), the patches after it wait for it
  Reading(Box<MirrorView>, Vec<ViewPatch>),
  /// The session ended it, or it never opened
  Ended,
}

/// The subscription of a repository channel that the views of the repository share
struct Mirror {
  /// The window of the bridge the subscription is of (`mirror:<path>`)
  window: String,
  /// The URI of the repository channel
  channel: String,
  instances: HashSet<String>,
  view: View,
  /// The demand of each view of the mirror, by its instance: the views share one subscription, so the window of the
  /// mirror gets the union of them (`Demand::union`), which the session of another prefix takes when it moves
  demands: HashMap<String, Demand>,
}

impl Mirror {
  /// The demand of the views of the mirror as one; none before a view set one
  fn demand(&self) -> Option<Demand> {
    (!self.demands.is_empty()).then(|| Demand::union(self.demands.values()))
  }

  /// `instance` set `demand`: the demand of the mirror now, the union of its views
  fn set_demand(&mut self, instance: &str, demand: Demand) -> Demand {
    self.demands.insert(instance.to_owned(), demand);
    Demand::union(self.demands.values())
  }

  /// `instance` leaves the mirror: the demand of the views that stay, when `instance` had set one and a view stays
  fn leave(&mut self, instance: &str) -> Option<Demand> {
    self.instances.remove(instance);
    let had = self.demands.remove(instance).is_some();
    (had && !self.instances.is_empty()).then(|| self.demand().unwrap_or_default())
  }

  /// The state the views of the mirror hold: none before its view, or once it ended
  fn state(&self) -> Option<Value> {
    match &self.view {
      View::Live(view) | View::Reading(view, _) => Some(view.state()),
      View::Opening | View::Ended => None,
    }
  }

  /// Its window subscribes to the channel
  const fn subscribed(&self) -> bool {
    matches!(self.view, View::Live(_) | View::Reading(..))
  }
}

struct Server {
  backend: Arc<BackendLink>,
  link: Arc<Link>,
  instances: Arc<Instances>,
  /// The notifications of the news of the mirrors
  notices: Arc<Notices>,
  /// The Git roots of the project and the one the main view shows
  projects: Arc<Projects>,
  /// The settings as the backend told them
  settings: Arc<SettingsMirror>,
  /// A page took the settings (`bd_bootstrap`): a change of the roots reaches it as a change of the settings
  bootstrapped: AtomicBool,
  /// The commands that run for calls of tools, by the id of the call
  calls: Mutex<HashMap<String, Running>>,
  /// The call of each request of a command to the backend, whose call events it names: the id of the call of air-ide
  /// and its cancel
  streams: Mutex<HashMap<u64, (Value, Arc<AtomicBool>)>>,
  /// The mirrors, by the path of their repository
  mirrors: Mutex<HashMap<String, Mirror>>,
}

impl Server {
  fn new(backend: Arc<BackendLink>, link: Arc<Link>, instances: Arc<Instances>, projects: Arc<Projects>) -> Arc<Self> {
    let settings = Arc::new(SettingsMirror::default());
    let notices = Arc::new(Notices::new(Arc::clone(&link), Arc::clone(&backend), Arc::clone(&instances), Arc::clone(&settings)));
    Arc::new(Self {
      backend,
      link,
      instances,
      notices,
      projects,
      settings,
      bootstrapped: AtomicBool::new(false),
      calls: Mutex::default(),
      streams: Mutex::default(),
      mirrors: Mutex::default(),
    })
  }
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
  mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The `ApiError` of a refusal of the backend: the one in its data (`data.apiError`), else an internal one with its
/// message
fn api_error_of(error: &Value) -> Value {
  match error["data"].get("apiError") {
    Some(api_error) if api_error.is_object() => api_error.clone(),
    _ => {
      let message = error["message"].as_str().unwrap_or("The backend of Branch Deck refused the call");
      serde_json::to_value(ApiError::internal(message)).unwrap_or(Value::Null)
    }
  }
}

/// The refusal of a request whose connection to the backend ended before its answer
fn gone() -> Value {
  json!({ "code": INTERNAL_ERROR, "message": "The backend of Branch Deck went away" })
}

impl Server {
  async fn handle(self: Arc<Self>, message: Value) {
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
      return alone(&self.link, &message);
    };
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    match id {
      Some(id) => self.request(id, method, params).await,
      None => self.notification(method, &params),
    }
  }

  async fn request(self: Arc<Self>, id: Value, method: &str, params: Value) {
    match method {
      "initialize" | "ping" | "tools/list" | "resources/read" => {
        alone(&self.link, &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
      }
      "tools/call" => self.call_tool(id, &params).await,
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
      _ => self.link.fail(&id, METHOD_NOT_FOUND, &format!("Branch Deck has no method {method}"), "unknownMethod"),
    }
  }

  fn notification(self: &Arc<Self>, method: &str, params: &Value) {
    let instance = params["instance"].as_str().unwrap_or_default();
    match method {
      "notifications/initialized" | "notifications/roots/list_changed" => self.read_roots(),
      "notifications/cancelled" => self.cancel(&params["requestId"]),
      "x-air-app/viewShown" => {
        let shown = params.get("params").cloned().unwrap_or_else(|| json!({}));
        self.instances.shown(instance, shown.clone());
        self.backend.notify(
          "x-branch-deck/windowShown",
          json!({ "window": label_of(instance), "view": view_of(instance), "params": shown }),
        );
      }
      "x-air-app/viewClosed" => {
        self.leave(instance);
        self.instances.closed(instance);
        self.backend.notify("x-branch-deck/windowClosed", json!({ "window": label_of(instance) }));
      }
      "x-air-app/viewHidden" => {
        self.instances.hidden(instance);
        self.backend.notify("x-branch-deck/windowHidden", json!({ "window": label_of(instance) }));
      }
      _ => tracing::debug!(method, "a notification the bridge does not know"),
    }
  }

  /// A frame of the backend that is no answer of a request of the bridge, in the order of the frames
  fn from_backend(self: &Arc<Self>, message: &Value) {
    let method = message["method"].as_str().unwrap_or_default();
    let params = &message["params"];
    if let Some(id) = message.get("id") {
      return self.host_request(id.clone(), method, params);
    }
    match method {
      "action" => self.action(params),
      "x-branch-deck/resync" => self.resync(params),
      "x-branch-deck/subscriptionEnded" => self.subscription_ended(params),
      "x-branch-deck/callEvent" => {
        let call = params["requestId"].as_u64().and_then(|request| locked(&self.streams).get(&request).cloned());
        if let Some((call_id, cancelled)) = call
          && !cancelled.load(Ordering::SeqCst)
        {
          self.link.notify("x-air-app/callEvent", json!({ "callId": call_id, "event": params["event"] }));
        }
      }
      "x-branch-deck/event" => self.event(params),
      _ => tracing::debug!(method, "a notification of the backend the bridge does not know"),
    }
  }

  /// An event of app-api to the views: to every view, or with `window` to the instance of that window; a change of the
  /// settings as the views see the recent projects (`Projects::seen_change`)
  fn event(&self, params: &Value) {
    let event = params["event"].as_str().unwrap_or_default();
    let mut payload = params.get("payload").cloned().unwrap_or(Value::Null);
    if event == SETTINGS_CHANGED_EVENT {
      self.settings.changed(&payload);
      self.projects.seen_change(&mut payload);
    }
    self.tell_views(params["window"].as_str(), event, &payload);
  }

  /// `x-air-app/event` of `event` to every view, or with `window` to the instance of that window; a window that is not
  /// open hears nothing, as in Tauri
  fn tell_views(&self, window: Option<&str>, event: &str, payload: &Value) {
    match window {
      None => self.link.notify("x-air-app/event", json!({ "event": event, "payload": payload })),
      Some(label) => {
        if let Some(instance) = self.instances.instance_of(label) {
          self.link.notify("x-air-app/event", json!({ "instance": instance, "event": event, "payload": payload }));
        }
      }
    }
  }

  /// A host request of the backend: the open of a window is `x-air-app/openView`
  fn host_request(self: &Arc<Self>, id: Value, method: &str, params: &Value) {
    match method {
      "x-branch-deck/openWindow" => self.open_window(id, params),
      // the IDE shows a view itself
      "x-branch-deck/showWindow" => self.backend.respond(&id, &json!({})),
      _ => self
        .backend
        .fail(&id, METHOD_NOT_FOUND, &format!("The bridge has no host request {method}"), "unknownMethod"),
    }
  }

  /// `x-branch-deck/openWindow {opener, params}`: asks the IDE to open the view of the URL of `params`, keyed by its
  /// target, and answers the instance the IDE gives as the label of the window; a view it showed already takes the new
  /// data (`init-data-updated`) and its title
  fn open_window(&self, id: Value, params: &Value) {
    let opener = params["opener"].as_str().and_then(|label| self.instances.instance_of(label));
    let open = open_view_of(&params["params"], opener);
    let answer = self.link.request("x-air-app/openView", Some(open.request));
    let (link, backend, instances) = (Arc::clone(&self.link), Arc::clone(&self.backend), Arc::clone(&self.instances));
    tokio::spawn(
      async move {
        let instance = match answer.await {
          Ok(Ok(result)) => result["instance"].as_str().map(str::to_owned),
          Ok(Err(error)) => {
            tracing::warn!(%error, "the IDE opens no view");
            None
          }
          Err(_) => None,
        };
        let Some(instance) = instance else {
          return backend.fail(&id, INTERNAL_ERROR, "The IDE opened no view", "openView");
        };
        if instances.opened(&open.window_id, &instance, open.params) {
          link.notify(
            "x-air-app/event",
            json!({ "instance": instance, "event": "init-data-updated", "payload": { "data": open.data } }),
          );
          link.ask("x-air-app/setViewTitle", json!({ "instance": instance, "title": open.title }));
        }
        backend.respond(&id, &json!({ "window": instance }));
      }
      .in_current_span(),
    );
  }

  /// The settings the backend has now, as the snapshot of the main window: the bridge reads its keys from it and from
  /// each change after it
  async fn read_settings(&self) {
    if let Ok(Ok(answer)) = self
      .backend
      .request("x-branch-deck/bootstrap", json!({ "window": MAIN_WINDOW, "view": BRIDGE_WINDOW }))
      .await
    {
      self.settings.snapshot(&answer["settings"]["values"], answer["settings"]["version"].as_u64().unwrap_or(0));
    } else {
      tracing::warn!("the backend told the bridge no settings");
    }
  }

  /// Writes `patch` through the backend, as a change of the bridge
  fn write_setting(&self, patch: &SettingsPatch) {
    let Ok(patch) = serde_json::to_value(patch) else {
      return;
    };
    self.backend.request_then(
      "x-branch-deck/set_setting",
      json!({ "window": BRIDGE_WINDOW, "args": { "params": { "patch": patch } } }),
      |reply| {
        if let Ok(Some(error)) = reply.as_ref().map(|answer| answer.get("error")) {
          tracing::warn!(%error, "the backend refused a change of the settings of the bridge");
        }
      },
    );
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
        let remembered = ide_roots::picked_of(&server.settings.value(IDE_ROOT_PICKS_KEY), &roots);
        if server.projects.set_roots(roots.clone(), remembered) {
          if let Some(instance) = server.instances.instance_of(MAIN_WINDOW) {
            server
              .link
              .notify("x-air-app/event", json!({ "instance": instance, "event": "roots-changed", "payload": { "roots": roots } }));
          }
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
    self.write_setting(&SettingsPatch::Value {
      key: RECENT_PROJECTS_KEY.to_owned(),
      value: self.settings.value(RECENT_PROJECTS_KEY),
    });
  }

  /// A write of the recent projects by the page of the IDE, whose list is the roots of the project: its first is the
  /// root the main view shows now, which the bridge remembers for this set of roots, and what it changed of each root
  /// goes into the recent projects of the settings, in their order, which stays the desktop app's
  fn with_recent_projects(&self, command: &str, mut args: Value) -> Value {
    if command != "set_setting" || args["params"]["patch"]["kind"] != "value" || args["params"]["patch"]["key"] != RECENT_PROJECTS_KEY {
      return args;
    }
    let seen = args["params"]["patch"]["value"].take();
    if let Some(picked) = seen[0]["path"].as_str()
      && self.projects.pick(picked)
    {
      self.write_setting(&ide_roots::pick_patch(&self.projects.roots(), picked));
    }
    args["params"]["patch"]["value"] = merged(&self.settings.value(RECENT_PROJECTS_KEY), &seen);
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
        let title_bar = title_bar_of(&arguments["titleBar"]);
        tokio::spawn(
          async move {
            let result = server.bootstrap(&view, instance.as_deref(), &title_bar).await;
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
        if let Some(main) = self.instances.instance_of(MAIN_WINDOW) {
          self
            .link
            .notify("x-air-app/event", json!({ "instance": main, "event": COMMAND_PALETTE_TOOL, "payload": null }));
        }
        self.link.respond(&id, json!({ "content": [], "structuredContent": { "value": null } }));
      }
      // Show in Branch Deck of a notification of the bridge: the main view shows, and reveals the branch
      Some(REVEAL_BRANCH_TOOL) => {
        let payload = json!({ "repositoryPath": arguments["repositoryPath"], "branchName": arguments["branchName"] });
        self.show_view(MAIN_VIEW, Some((REVEAL_BRANCH_EVENT.to_owned(), payload)));
        self.link.respond(&id, json!({ "content": [], "structuredContent": { "value": null } }));
      }
      name => self
        .link
        .fail(&id, METHOD_NOT_FOUND, &format!("Branch Deck has no tool {}", name.unwrap_or_default()), "unknownTool"),
    }
  }

  /// Asks the IDE to show `view`, then tells the instance it answered `event` with its payload, when there is one: Show
  /// in Branch Deck of a notification shows the main view, which reveals the branch (`reveal-branch`), and the focus of
  /// the main window shows the tool window. The bridge tells the event at once: the IDE holds an event for an instance
  /// until its page sent `ui/notifications/initialized`, which the page sends once it listens
  fn show_view(&self, view: &str, event: Option<(String, Value)>) {
    let answer = self.link.request("x-air-app/openView", Some(json!({ "view": view })));
    let link = Arc::clone(&self.link);
    let view = view.to_owned();
    tokio::spawn(
      async move {
        match answer.await {
          Ok(Ok(result)) => {
            if let (Some(instance), Some((event, payload))) = (result["instance"].as_str(), event) {
              link.notify("x-air-app/event", json!({ "instance": instance, "event": event, "payload": payload }));
            }
          }
          Ok(Err(error)) => tracing::warn!(%error, view, "the IDE shows no view"),
          Err(_) => {}
        }
      }
      .in_current_span(),
    );
  }

  /// Runs the command of `arguments` for the window of `instance` as a root command of the backend, the events of its
  /// call as call events of `id`
  fn invoke(self: &Arc<Self>, id: Value, arguments: &Value, instance: Option<&str>) {
    let command = arguments["command"].as_str().unwrap_or_default().to_owned();
    let args = arguments.get("args").cloned().unwrap_or(Value::Null);
    if self.host_command(&id, &command, &args, instance) {
      return;
    }
    if command == "set_demand" {
      return self.set_demand(&id, &args, instance);
    }
    let args = self.with_recent_projects(&command, args);
    let window = instance.map_or_else(|| MAIN_WINDOW.to_owned(), label_of);
    let read = dispatch::spec(&command).is_some_and(|spec| spec.class == CommandClass::Read);
    let request = self.backend.next_id();
    let cancelled = Arc::new(AtomicBool::new(false));
    locked(&self.calls).insert(
      id.to_string(),
      Running {
        request,
        read,
        cancelled: Arc::clone(&cancelled),
      },
    );
    locked(&self.streams).insert(request, (id.clone(), Arc::clone(&cancelled)));
    let server = Arc::clone(self);
    let method = format!("x-branch-deck/{command}");
    self.backend.request_as(request, &method, json!({ "window": window, "args": args }), move |reply| {
      locked(&server.streams).remove(&request);
      locked(&server.calls).remove(&id.to_string());
      if !cancelled.load(Ordering::SeqCst) {
        server.link.respond(&id, server.tool_result(&command, reply));
      }
    });
  }

  /// The result of the call of `command` of the answer `reply` of the backend: `{value}`, `{bytes}` (base64) or its
  /// `ApiError`, which the page acts on by its code
  fn tool_result(&self, command: &str, reply: Reply) -> Value {
    match reply {
      Ok(answer) if answer.get("error").is_some() => tool_error(&answer["error"]),
      Ok(answer) if answer.get("bytes").is_some() => json!({ "content": [], "structuredContent": { "bytes": answer["bytes"] } }),
      Ok(mut answer) => {
        let mut value = answer["value"].take();
        if SETTINGS_COMMANDS.contains(&command) {
          self.projects.seen_changes(&mut value);
        }
        json!({ "content": [], "structuredContent": { "value": value } })
      }
      Err(error) => tool_error(&api_error_of(&error)),
    }
  }

  /// What the desktop does beside the commands, which the page asks through `bd_invoke` by the plugin call of Tauri it
  /// makes in the desktop app: an event the page emits goes to the views of the IDE (to one window with a target), the
  /// badge and the progress of the Dock mark the tool window, a notification of the OS is one of the IDE (none of a
  /// news told already, `Notices::tell_page_notification`), and the close of the window of `instance` (Escape in a
  /// review or a dialog) closes its place. False for a command of app-api; true once it answered `id`
  fn host_command(&self, id: &Value, command: &str, args: &Value, instance: Option<&str>) -> bool {
    match command {
      // the window of the page comes to the front (a reveal that Activity or a review asked, the palette): the IDE shows
      // the tool window of the main view; the place of another view is the IDE's to show
      "plugin:window|set_focus" => {
        if instance.is_some_and(|instance| view_of(instance) == MAIN_VIEW) {
          self.show_view(MAIN_VIEW, None);
        }
      }
      // an event of a page stays in its connection: the views of this IDE hear it
      "plugin:event|emit" => {
        let (event, payload) = (args["event"].as_str().unwrap_or_default(), args.get("payload").cloned().unwrap_or(Value::Null));
        self.tell_views(args["target"].as_str(), event, &payload);
      }
      "plugin:window|set_badge_count" => {
        let count = args["value"].as_u64().filter(|count| *count > 0);
        self.link.ask("x-air-app/setBadge", json!({ "view": MAIN_VIEW, "count": count }));
      }
      "plugin:window|set_progress_bar" => self.link.ask("x-air-app/setProgress", progress_of(&args["value"])),
      "plugin:notification|notify" => {
        let (link, id) = (Arc::clone(&self.link), id.clone());
        self.notices.tell_page_notification(&args["options"], move || {
          link.respond(&id, json!({ "content": [], "structuredContent": { "value": null } }));
        });
        return true;
      }
      "plugin:window|close" | "plugin:window|destroy" => {
        if let Some(instance) = instance {
          self.link.ask("x-air-app/closeView", json!({ "instance": instance }));
        }
      }
      // the shim of the page ends the subscription of its frame itself (`x-air-app/unsubscribe`)
      "unsubscribe_view" => {}
      _ => return false,
    }
    self.link.respond(id, json!({ "content": [], "structuredContent": { "value": null } }));
    true
  }

  /// `set_demand` of a view: the views of a mirror share its subscription (the main view, a review, Activity), so the
  /// mirror keeps the demand of each view and its window gets the union of them (`x-branch-deck/setDemand`); a mirror
  /// whose view has not answered yet takes it once it does
  fn set_demand(self: &Arc<Self>, id: &Value, args: &Value, instance: Option<&str>) {
    let answered = json!({ "content": [], "structuredContent": { "value": null } });
    let Ok(demand) = serde_json::from_value::<Demand>(args["params"]["demand"].clone()) else {
      let error = serde_json::to_value(ApiError::internal("set_demand names a Demand")).unwrap_or(Value::Null);
      return self.link.respond(id, tool_error(&error));
    };
    let sent = instance.and_then(|instance| {
      let mut mirrors = locked(&self.mirrors);
      let mirror = mirrors.values_mut().find(|mirror| mirror.instances.contains(instance))?;
      let union = mirror.set_demand(instance, demand);
      mirror.subscribed().then(|| (mirror.channel.clone(), mirror.window.clone(), union))
    });
    let Some((channel, window, union)) = sent else {
      return self.link.respond(id, answered);
    };
    let (link, id) = (Arc::clone(&self.link), id.clone());
    self.backend.request_then(
      "x-branch-deck/setDemand",
      json!({ "channel": channel, "window": window, "demand": union }),
      move |reply| match reply {
        Err(error) if error["data"]["reason"] != "notSubscribed" => link.respond(&id, tool_error(&api_error_of(&error))),
        _ => link.respond(&id, answered),
      },
    );
  }

  /// The demand of `mirror` to its window, when a view set one
  fn send_demand(&self, mirror: &Mirror) {
    if let Some(demand) = mirror.demand() {
      self.backend.request_then(
        "x-branch-deck/setDemand",
        json!({ "channel": mirror.channel, "window": mirror.window, "demand": demand }),
        |_| {},
      );
    }
  }

  /// air-ide cancelled the call `request_id`: a read stops, and no call gives an answer or an event after it
  fn cancel(&self, request_id: &Value) {
    let Some(running) = locked(&self.calls).remove(&request_id.to_string()) else {
      return;
    };
    running.cancelled.store(true, Ordering::SeqCst);
    if running.read {
      self.backend.notify("x-branch-deck/cancel", json!({ "requestId": running.request }));
    }
  }

  /// What the page takes before its main (`bd_bootstrap {view, titleBar}`): the settings of its window and, for the
  /// main view, the warm frame of the repository it shows, both of `x-branch-deck/bootstrap` (the page loaded,
  /// `x-branch-deck/pageLoaded`); the init data of the view, the title bar (`title_bar_of`) and the home directory of
  /// this process, which the page shows as `~` (`homeDir` of `@tauri-apps/api/path` in the desktop app, which reads the
  /// same `dirs::home_dir`)
  async fn bootstrap(&self, view: &str, instance: Option<&str>, title_bar: &TitleBar) -> Value {
    let label = instance.map_or_else(|| MAIN_WINDOW.to_owned(), label_of);
    self.bootstrapped.store(true, Ordering::SeqCst);
    self.backend.notify("x-branch-deck/pageLoaded", json!({ "window": label }));
    let main = view == MAIN_VIEW || instance.is_some_and(|instance| view_of(instance) == MAIN_VIEW);
    let repository = if main { self.main_repository() } else { None };
    let mut asked = json!({ "window": label, "view": if main { MAIN_VIEW } else { view } });
    if let Some(path) = &repository {
      asked["repositoryPath"] = Value::String(path.clone());
    }
    let answer = match self.backend.request("x-branch-deck/bootstrap", asked).await {
      Ok(Ok(answer)) => answer,
      _ => json!({ "settings": { "values": {}, "version": 0 }, "warmFrame": null }),
    };
    let version = answer["settings"]["version"].clone();
    let mut values = answer["settings"]["values"].clone();
    self.settings.snapshot(&values, version.as_u64().unwrap_or(0));
    if let Some(recent) = values.get_mut(RECENT_PROJECTS_KEY) {
      *recent = self.projects.seen(recent);
    } else if let Value::Object(values) = &mut values {
      values.insert(RECENT_PROJECTS_KEY.to_owned(), self.projects.seen(&Value::Null));
    }
    // the app updates with the desktop app, the roots of the project name the repositories where the host has no
    // folder dialog, and the title of the tool window names the progress by a text (`progressText`)
    let title_bar = PageTitleBar {
      title_bar,
      check_for_updates: false,
      folder_dialog: false,
      progress_text: true,
    };
    let shown = instance.and_then(|instance| self.instances.params(instance)).unwrap_or_else(|| json!({}));
    let (warm_frame, init_data) = if main {
      let frame = if repository.is_some() { answer["warmFrame"].clone() } else { Value::Null };
      (frame, json!({ "repositoryPath": repository, "roots": self.projects.roots(), "params": shown }))
    } else {
      (Value::Null, json!({ "data": shown.get("data").cloned().unwrap_or(Value::Null), "params": shown }))
    };
    json!({
      "settings": { "values": values, "version": version },
      "warmFrame": warm_frame,
      "initData": init_data,
      "titleBar": title_bar,
      "homeDir": dirs::home_dir().map(|home| home.to_string_lossy().into_owned()),
    })
  }

  /// The channel and the window of the mirror of `path` while its window subscribes
  fn subscribed_mirror(&self, path: &str) -> Option<(String, String)> {
    locked(&self.mirrors)
      .get(path)
      .filter(|mirror| mirror.subscribed())
      .map(|mirror| (mirror.channel.clone(), mirror.window.clone()))
  }

  /// The channel of the mirror of `path` takes the session of the prefix the git config names now
  /// (`x-branch-deck/followPrefix`): a channel of another prefix moves to the session of the new one, a resync of the
  /// window of the mirror, one change of every part, before the answer. Nothing for a mirror whose window subscribes not
  async fn follow_prefix(&self, path: &str) -> Result<(), Value> {
    let Some((channel, window)) = self.subscribed_mirror(path) else {
      return Ok(());
    };
    match self.backend.request("x-branch-deck/followPrefix", json!({ "channel": channel, "window": window })).await {
      Ok(Ok(_)) => Ok(()),
      // the window ended meanwhile: it opens again with the next subscription
      Ok(Err(error)) if error["data"]["reason"] == "notSubscribed" => Ok(()),
      Ok(Err(error)) => Err(error),
      Err(_) => Err(gone()),
    }
  }

  /// Subscribes `instance` to the mirror of the repository its params name: the repository channel is subscribed once
  /// for every view of the repository. Answered in the order of the messages, so a second view of a repository joins the
  /// mirror the first opened, which takes the session of the prefix the git config names now first (`follow_prefix`)
  async fn subscribe(self: &Arc<Self>, id: Value, params: &Value) {
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
      // the state and the answer under the lock of the mirrors, so the changes after it follow it
      let mut mirrors = locked(&self.mirrors);
      if let Some(mirror) = mirrors.get_mut(&path) {
        mirror.instances.insert(instance.clone());
        if let Some(state) = mirror.state() {
          self.link.respond(&id, json!({ "mirror": path, "state": state }));
          return;
        }
      }
    }
    self.open_mirror(id, path, instance).await;
  }

  /// Opens the mirror of `path` for `instance`: the window of the mirror subscribes to the repository channel, whose
  /// snapshot is the answer of `id`; the actions of the channel follow it
  async fn open_mirror(self: &Arc<Self>, id: Value, path: String, instance: String) {
    let channel = match self.backend.request("x-branch-deck/repositoryChannel", json!({ "repositoryPath": path })).await {
      Ok(Ok(answer)) => answer["channel"].as_str().unwrap_or_default().to_owned(),
      Ok(Err(error)) => return self.fail_subscription(&id, &error),
      Err(_) => return self.fail_subscription(&id, &gone()),
    };
    let window = mirror_window(&path);
    {
      let mut mirrors = locked(&self.mirrors);
      let mirror = mirrors.entry(path.clone()).or_insert_with(|| Mirror {
        window: window.clone(),
        channel: channel.clone(),
        instances: HashSet::new(),
        view: View::Opening,
        demands: HashMap::new(),
      });
      mirror.channel.clone_from(&channel);
      mirror.view = View::Opening;
      mirror.instances.insert(instance);
    }
    let (done, opened) = oneshot::channel();
    let server = Arc::clone(self);
    self.backend.request_then(
      "subscribe",
      json!({ "channel": channel, "_meta": { "x-branch-deck": { "window": window } } }),
      move |reply| {
        server.opened(&id, &path, reply);
        let _ = done.send(());
      },
    );
    let _ = opened.await;
  }

  /// The subscription of the window of the mirror of `path` answered: its snapshot is the view of the mirror and the
  /// answer of `id`, before every change of it
  fn opened(&self, id: &Value, path: &str, reply: Reply) {
    let view = reply
      .as_ref()
      .ok()
      .and_then(|answer| serde_json::from_value::<RepositoryView>(answer["snapshot"]["state"].clone()).ok());
    let mut mirrors = locked(&self.mirrors);
    let Some(mirror) = mirrors.get_mut(path) else {
      return;
    };
    if let Some(view) = view {
      let view = MirrorView::new(view);
      self.link.respond(id, json!({ "mirror": path, "state": view.state() }));
      mirror.view = View::Live(Box::new(view));
      self.send_demand(mirror);
    } else {
      mirror.view = View::Ended;
      let error = reply.err().unwrap_or_else(|| json!({ "code": INTERNAL_ERROR, "message": "The session gave no view" }));
      self.fail_subscription(id, &error);
    }
  }

  /// `bd_invoke` of `subscribe_view`: the page subscribes again to the repository whose mirror its frame holds (the
  /// prefix that names its branches may have changed, `resubscribeSession` of `src/model/store.ts`), and its frame holds
  /// one view, so the page shares the subscription it has and asks the bridge. Answered after the change that moves the
  /// mirror to the session of the prefix the git config names now; the error of that session, such as
  /// `no_branch_prefix`, as the answer of a command
  async fn subscribe_again(&self, id: Value, arguments: &Value) {
    let path = arguments["args"]["params"]["repositoryPath"].as_str().unwrap_or_default();
    match self.follow_prefix(path).await {
      Ok(()) => self.link.respond(&id, json!({ "content": [], "structuredContent": { "value": null } })),
      Err(error) => self.link.respond(&id, tool_error(&api_error_of(&error))),
    }
  }

  /// The mirror of the window `label` of the bridge (`mirror:<path>`), with its path
  fn mirror_of_window<'a>(mirrors: &'a mut HashMap<String, Mirror>, label: &'a str) -> Option<(&'a str, &'a mut Mirror)> {
    let path = label.strip_prefix(MIRROR_PREFIX)?;
    mirrors.get_mut(path).map(|mirror| (path, mirror)).filter(|(_, mirror)| mirror.window == label)
  }

  /// `action {channel, action: viewPatched, window}`: the patch of the session as one change of the mirror of the
  /// window, and the news it brings as the notifications of the IDE; a patch into another epoch reads the view anew
  fn action(self: &Arc<Self>, params: &Value) {
    if params["action"]["type"] != "x-branch-deck/viewPatched" {
      return;
    }
    let Ok(patch) = serde_json::from_value::<ViewPatch>(params["action"]["patch"].clone()) else {
      return tracing::warn!("a patch of the backend is no ViewPatch");
    };
    let label = params["window"].as_str().unwrap_or_default();
    let mut mirrors = locked(&self.mirrors);
    let Some(path) = label.strip_prefix(MIRROR_PREFIX).map(str::to_owned) else {
      return;
    };
    let Some(mirror) = mirrors.get_mut(&path).filter(|mirror| mirror.window == label) else {
      return;
    };
    // a patch of another epoch, or one past a patch the mirror lacks: the view is read anew, as the page asks a snapshot
    let behind = matches!(&mirror.view, View::Live(view) if {
      let held = view.view().version;
      patch.base.epoch != held.epoch || patch.next.epoch != held.epoch || patch.base.seq > held.seq
    });
    if behind {
      return self.read_anew(mirror, &path, patch);
    }
    match &mut mirror.view {
      View::Live(view) => {
        let news = patch.changes.news.clone();
        match view.apply(patch) {
          Ok(ops) => {
            if !ops.is_empty() {
              self.link.notify("x-air-app/viewChanged", json!({ "mirror": path, "ops": ops }));
            }
            self.notices.tell(view.view(), &path, &news);
          }
          Err(error) => tracing::warn!(%error, mirror = path, "a patch of the session does not apply to its mirror"),
        }
      }
      View::Reading(_, waiting) => waiting.push(patch),
      View::Opening | View::Ended => {}
    }
  }

  /// A patch into another epoch reached `mirror`: the session opened anew (a slot of a lease pool leased again), and no
  /// patch of it applies to the view of the mirror (or a patch past one the mirror lacks). The mirror reads the view of the session (`get_snapshot` of its
  /// window), one change of every part, a `version` of another epoch, after which the patches that came meanwhile
  /// follow; a session that answers no view ends the mirror
  fn read_anew(self: &Arc<Self>, mirror: &mut Mirror, path: &str, patch: ViewPatch) {
    let View::Live(view) = std::mem::replace(&mut mirror.view, View::Ended) else {
      return;
    };
    mirror.view = View::Reading(view, vec![patch]);
    let (server, path_of) = (Arc::clone(self), path.to_owned());
    self.backend.request_then(
      "x-branch-deck/get_snapshot",
      json!({ "window": mirror.window, "args": { "params": { "repositoryPath": path } } }),
      move |reply| server.read(&path_of, reply),
    );
  }

  /// The view of the session the mirror of `path` read anew (`read_anew`)
  fn read(&self, path: &str, reply: Reply) {
    let view = reply.ok().and_then(|answer| serde_json::from_value::<RepositoryView>(answer["value"].clone()).ok());
    let mut mirrors = locked(&self.mirrors);
    let Some(mirror) = mirrors.get_mut(path) else {
      return;
    };
    let View::Reading(_, waiting) = std::mem::replace(&mut mirror.view, View::Ended) else {
      // a resync came first, or the mirror ended
      return;
    };
    let Some(view) = view else {
      tracing::warn!(mirror = path, "the mirror cannot read the view of its session anew");
      return self.link.notify("x-air-app/viewEnded", json!({ "mirror": path, "reason": "sessionClosed" }));
    };
    let mut view = MirrorView::new(view);
    for patch in waiting {
      view.catch_up(patch);
    }
    self.link.notify("x-air-app/viewChanged", json!({ "mirror": path, "ops": view.parts() }));
    mirror.view = View::Live(Box::new(view));
  }

  /// `x-branch-deck/resync {channel, window, snapshot}`: the channel moved to another session (a new prefix), or the
  /// window read too slowly: the mirror takes the view of the snapshot in one change of every part, a `version` of
  /// another epoch last, and the session the demand of the mirror
  fn resync(&self, params: &Value) {
    let Ok(view) = serde_json::from_value::<RepositoryView>(params["snapshot"]["state"].clone()) else {
      return tracing::warn!("a resync of the backend has no view");
    };
    let mut mirrors = locked(&self.mirrors);
    let Some((path, mirror)) = Self::mirror_of_window(&mut mirrors, params["window"].as_str().unwrap_or_default()) else {
      return;
    };
    let view = MirrorView::new(view);
    self.link.notify("x-air-app/viewChanged", json!({ "mirror": path, "ops": view.parts() }));
    mirror.view = View::Live(Box::new(view));
    self.send_demand(mirror);
  }

  /// `x-branch-deck/subscriptionEnded`: the session went while a view showed the mirror (the repository is gone, the
  /// session closed): `x-air-app/viewEnded {reason: "sessionClosed"}`, the one reason of the contract
  fn subscription_ended(&self, params: &Value) {
    let mut mirrors = locked(&self.mirrors);
    let Some((path, mirror)) = Self::mirror_of_window(&mut mirrors, params["window"].as_str().unwrap_or_default()) else {
      return;
    };
    if mirror.subscribed() {
      mirror.view = View::Ended;
      self.link.notify("x-air-app/viewEnded", json!({ "mirror": path, "reason": "sessionClosed" }));
    }
  }

  /// The subscription `id` failed with `error`, a refusal of the backend: its `ApiError` in the data of the error, for
  /// air-ide to pass on
  fn fail_subscription(&self, id: &Value, error: &Value) {
    let api_error = api_error_of(error);
    self.link.send(&json!({
      "jsonrpc": "2.0",
      "id": id,
      "error": { "code": INTERNAL_ERROR, "message": api_error["message"], "data": { "reason": "badResponse", "error": api_error } },
    }));
  }

  /// `instance` leaves its mirror; the last view of a mirror ends its subscription, and the window of a mirror that
  /// other views keep gets the union of their demands without the one of `instance`
  fn leave(&self, instance: &str) {
    let mut kept = Vec::new();
    let ended: Vec<Mirror> = {
      let mut mirrors = locked(&self.mirrors);
      for mirror in mirrors.values_mut() {
        if let Some(demand) = mirror.leave(instance)
          && mirror.subscribed()
        {
          kept.push((mirror.channel.clone(), mirror.window.clone(), demand));
        }
      }
      let empty: Vec<String> = mirrors.iter().filter(|(_, mirror)| mirror.instances.is_empty()).map(|(path, _)| path.clone()).collect();
      empty.iter().filter_map(|path| mirrors.remove(path)).collect()
    };
    for (channel, window, demand) in kept {
      self
        .backend
        .request_then("x-branch-deck/setDemand", json!({ "channel": channel, "window": window, "demand": demand }), |_| {});
    }
    for mirror in ended.into_iter().filter(Mirror::subscribed) {
      self.backend.request_then(
        "unsubscribe",
        json!({ "channel": mirror.channel, "_meta": { "x-branch-deck": { "window": mirror.window } } }),
        |_| {},
      );
    }
  }

  /// The connection ended: every mirror with it, and no command answers any more
  fn end(&self) {
    locked(&self.mirrors).clear();
    for (_, running) in locked(&self.calls).drain() {
      running.cancelled.store(true, Ordering::SeqCst);
    }
  }
}

/// The window of the bridge whose subscription the mirror of a path is
const MIRROR_PREFIX: &str = "mirror:";

/// The window the subscription of the mirror of `path` is of
fn mirror_window(path: &str) -> String {
  format!("{MIRROR_PREFIX}{path}")
}

/// The commands whose answer holds changes of the settings, which the views see with the recent projects of the roots
const SETTINGS_COMMANDS: [&str; 3] = ["set_setting", "get_settings_changes", "sub_window_loaded"];

/// The tool of the action Sync Virtual Branches of the app file
const SYNC_TOOL: &str = "sync";
/// The tool of the action Branch Deck Command Palette of the app file, and the event it sends the main view
const COMMAND_PALETTE_TOOL: &str = "open_command_palette";

/// The title bar of a view of the IDE from the title bar its host context names (`hostContext["x-air-app"].titleBar`,
/// which the page passes to `bd_bootstrap`): `overlay` where the IDE merges the row of the page into its title bar (a
/// window on macOS), so the row starts after the traffic lights and the IDE drags the window and does its double click
/// (`DoubleClick::Host`); else `native`, where the place of the IDE draws its own title (a tool window, an editor tab, a
/// dialog, a host that names none)
fn title_bar_of(host: &Value) -> TitleBar {
  if host["kind"] != "overlay" {
    return TitleBar::Native { reason: None };
  }
  TitleBar::Overlay {
    double_click: DoubleClick::Host,
    height: host["height"].as_f64().filter(|height| (1.0..=200.0).contains(height)).map(row_height),
  }
}

/// The height of the row in whole px: the IDE names its title bar in points, 1 to 200, maybe with a fraction
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a height of 1 to 200, rounded")]
const fn row_height(points: f64) -> u32 {
  points.round() as u32
}

/// The tools of Branch Deck, each only for the app (`_meta.ui.visibility`), so no agent sees one: `bd_invoke` and
/// `bd_bootstrap` for the page, the tool of each action of the app file for the IDE, and the tool of the action of a
/// notification of the bridge
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
      "inputSchema": { "type": "object", "properties": { "view": { "type": "string" }, "titleBar": { "type": "object" } }, "required": ["view"] },
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

/// `x-air-app/setProgress` of the main view for the progress bar of the Dock (`ProgressBarState` of Tauri with the
/// `text` of what runs, which the page in the IDE adds, `setDockProgress`): a normal one is its fraction with the text,
/// an indeterminate one with a text is the text without a fraction, a progress without a percent; anything else, an
/// indeterminate one without a text too, clears the progress (`fraction: null`)
fn progress_of(value: &Value) -> Value {
  let text = value["text"].as_str().filter(|text| !text.is_empty());
  let mut params = match value["status"].as_str() {
    Some("normal") => json!({ "view": MAIN_VIEW, "fraction": value["progress"].as_f64().unwrap_or(0.0).clamp(0.0, 100.0) / 100.0 }),
    Some("indeterminate") if text.is_some() => json!({ "view": MAIN_VIEW }),
    _ => return json!({ "view": MAIN_VIEW, "fraction": null }),
  };
  if let Some(text) = text {
    params["text"] = json!(text);
  }
  params
}

/// The result of a call whose command failed: its `ApiError`, which the page acts on by its code
fn tool_error(error: &Value) -> Value {
  json!({ "content": [{ "type": "text", "text": error["message"] }], "isError": true, "structuredContent": { "error": error } })
}

/// The path of a `file://` URI of a root, without a trailing separator (`file:///x/repo/` is `/x/repo`, the spelling of
/// the desktop app, which keys the session of the repository), but the root of a file system (`/`, `C:/`)
fn path_of_uri(uri: &str) -> Option<String> {
  let path = uri.strip_prefix("file://")?;
  // a Windows root is `file:///C:/…`
  let path = if path.len() > 2 && path.as_bytes()[2] == b':' { &path[1..] } else { path };
  let mut path = percent_decoded(path);
  let kept = path.trim_end_matches('/').len();
  let is_root = kept == 0 || (kept == 2 && path.as_bytes()[1] == b':');
  path.truncate(if is_root { (kept + 1).min(path.len()) } else { kept });
  Some(path)
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

#[cfg(test)]
#[path = "server_test.rs"]
mod server_test;
