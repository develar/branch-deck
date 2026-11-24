//! A connection of a client (docs/design/backend-protocol.md, "The wire", "The windows of a connection"): what it
//! read of each line, its `initialize` with the build and the capabilities of the client, the life of its windows, its
//! host requests, and its end, which closes each of its windows and none of another connection. Every frame to the
//! client goes through its queue (`Outbox`), which a transport writes (`next_frame`).

use crate::client::{Capabilities, ClientWindows, client_hosts};
use crate::host::{Host, lock};
use crate::outbox::{Next, Outbox};
use crate::wire::{EXTENSION, Failure, Frame, Incoming, VOCABULARY_VERSION, negotiate, notification, read, result, semver};
use app_api::transport::{ConnectionId, WindowId};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tokio::sync::oneshot;
use tracing::Instrument;

/// Whose connection it is
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
  /// The app's own windows in the process (the Tauri shell): `ConnectionId::APP`, the desktop and the events of the app
  App,
  /// A client of its own: an id of `Connections`, the desktop of its host requests and capabilities, the events it
  /// hears on the connection
  Client,
}

/// The answer of a host request: its result, or its error object
pub(crate) type HostAnswer = Result<Value, Value>;

pub struct Connection {
  pub(crate) host: Arc<Host>,
  key: u64,
  kind: Kind,
  outbox: Outbox,
  id: OnceLock<ConnectionId>,
  client_id: OnceLock<String>,
  ended: AtomicBool,
  /// The windows the client shows and the labels it gave the windows the host opened
  pub(crate) windows: Arc<ClientWindows>,
  last_request: AtomicU64,
  requests: Mutex<HashMap<u64, oneshot::Sender<HostAnswer>>>,
  /// The calls that run, by the JSON of their request id: the flag their cancel sets
  pub(crate) calls: Mutex<HashMap<String, Arc<AtomicBool>>>,
  pub(crate) me: Weak<Self>,
}

impl Connection {
  pub(crate) fn new(host: Arc<Host>, key: u64, kind: Kind) -> Arc<Self> {
    Arc::new_cyclic(|me| Self {
      host,
      key,
      kind,
      outbox: Outbox::default(),
      id: OnceLock::new(),
      client_id: OnceLock::new(),
      ended: AtomicBool::new(false),
      windows: Arc::default(),
      last_request: AtomicU64::new(0),
      requests: Mutex::default(),
      calls: Mutex::default(),
      me: Weak::clone(me),
    })
  }

  /// The id of the connection in app-api, once it initialized
  pub fn id(&self) -> Option<ConnectionId> {
    self.id.get().copied()
  }

  pub(crate) const fn outbox(&self) -> &Outbox {
    &self.outbox
  }

  pub(crate) fn client_id(&self) -> String {
    self.client_id.get().cloned().unwrap_or_default()
  }

  pub(crate) fn me(&self) -> Arc<Self> {
    self.me.upgrade().expect("a connection lives while it reads")
  }

  /// Queues the result `value` of the request `id`
  pub(crate) fn answer(&self, id: &Value, value: Value) {
    self.outbox.push(Frame::of(&result(id, value)));
  }

  /// Queues `failure` as the answer of the request `id`
  pub(crate) fn fail(&self, id: &Value, failure: &Failure) {
    self.outbox.push(Frame::of(&failure.answer(id)));
  }

  /// Queues the notification `method`
  pub(crate) fn notify(&self, method: &str, params: Value) {
    self.outbox.push(Frame::of(&notification(method, params)));
  }

  /// Sends the host request `method` to the client: its answer, none when the connection ended first
  pub(crate) fn request(&self, method: &str, params: &Value) -> oneshot::Receiver<HostAnswer> {
    let (reply, answer) = oneshot::channel();
    let id = self.last_request.fetch_add(1, Ordering::SeqCst) + 1;
    lock(&self.requests).insert(id, reply);
    let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    if !self.outbox.push(Frame::of(&request)) {
      lock(&self.requests).remove(&id);
    }
    answer
  }

  /// The next frame to write to the client, none once the connection ended and its queue is empty; a stale window of
  /// the connection is resynced as its queue drains
  pub async fn next_frame(&self) -> Option<Frame> {
    loop {
      match self.outbox.next().await? {
        Next::Frame(frame) => return Some(frame),
        Next::Resync(windows) => {
          let (host, me) = (Arc::clone(&self.host), self.me());
          self.host.ctx.runtime.spawn(async move { host.resync_stale(&me, windows).await }.in_current_span());
        }
      }
    }
  }

  /// One line of the client
  pub fn receive(&self, line: &str) {
    if self.ended.load(Ordering::SeqCst) {
      return;
    }
    match read(line) {
      Ok(Incoming::Request { id, method, params }) => self.request_of(id, &method, &params),
      Ok(Incoming::Notification { method, params }) => self.notification_of(&method, &params),
      Ok(Incoming::Response { id, answer }) => {
        let reply = id.as_u64().and_then(|id| lock(&self.requests).remove(&id));
        if let Some(reply) = reply {
          let _ = reply.send(answer);
        }
      }
      Err((id, failure)) => self.fail(&id, &failure),
    }
  }

  fn request_of(&self, id: Value, method: &str, params: &Value) {
    match method {
      "ping" => return self.answer(&id, json!({})),
      "initialize" => return self.initialize(&id, params),
      _ => {}
    }
    if self.id().is_none() {
      return self.fail(&id, &Failure::invalid("notInitialized", "The first request of a connection is initialize"));
    }
    let host = &self.host;
    match method {
      "subscribe" => host.subscribe(&self.me(), id, params),
      "unsubscribe" => host.unsubscribe(self, &id, params),
      "x-branch-deck/repositoryChannel" => host.repository_channel(self, &id, params),
      "x-branch-deck/setDemand" => host.set_demand(self, &id, params),
      "x-branch-deck/followPrefix" => host.follow_prefix(&self.me(), id, params),
      "x-branch-deck/claimNews" => match params["repositoryPath"].as_str() {
        Some(path) => {
          let first = host.claim_news(path, &params["news"]);
          self.answer(&id, json!({ "first": first }));
        }
        None => self.fail(&id, &Failure::bad_params("claimNews names its repositoryPath")),
      },
      "x-branch-deck/showMain" => {
        (host.hooks.show_main)();
        self.answer(&id, json!({}));
      }
      "x-branch-deck/bootstrap" => self.bootstrap(id, params),
      _ => match method.strip_prefix(EXTENSION) {
        Some(command) => self.root_command(id, command, params),
        None => self.fail(&id, &Failure::unknown_method(method)),
      },
    }
  }

  fn notification_of(&self, method: &str, params: &Value) {
    let Some(of) = self.id() else {
      return;
    };
    let label = params["window"].as_str().filter(|label| !label.is_empty());
    match (method, label) {
      ("dispatchAction", _) => self.host.dispatch_action(self, params),
      ("x-branch-deck/cancel", _) => {
        if let Some(canceled) = lock(&self.calls).get(&params["requestId"].to_string()) {
          canceled.store(true, Ordering::SeqCst);
        }
      }
      ("x-branch-deck/windowShown", Some(label)) => self.windows.shown(label),
      ("x-branch-deck/windowClosed", Some(label)) => {
        if self.windows.closed(label) {
          self.host.end_windows(of, Some(label));
          self.host.ctx.forget_window(&WindowId::of(of, label));
        }
      }
      ("x-branch-deck/pageLoaded", Some(label)) => {
        self.host.end_windows(of, Some(label));
        self.host.ctx.forget_page(&WindowId::of(of, label));
      }
      _ => {}
    }
  }

  /// `initialize`: the AHP version, the build of the client, its vocabulary and its capabilities
  fn initialize(&self, id: &Value, params: &Value) {
    if self.id().is_some() {
      return self.fail(id, &Failure::invalid("alreadyInitialized", "The connection initialized already"));
    }
    let (Some(offered), Some(client_id), Some(build), Some(vocabulary)) = (
      params["protocolVersions"].as_array(),
      params["clientId"].as_str(),
      params["clientInfo"]["build"].as_str(),
      params["_meta"]["x-branch-deck"]["version"].as_str(),
    ) else {
      return self.fail(
        id,
        &Failure::bad_params("initialize names protocolVersions, clientId, clientInfo.build and _meta[\"x-branch-deck\"].version"),
      );
    };
    let host = &self.host;
    if build != host.build {
      let stale = (host.hooks.stale)();
      let exits = stale && !(host.hooks.has_window)();
      self.fail(
        id,
        &Failure::new(-32005, "otherBuild", format!("The backend is of the build {}", host.build))
          .with("build", Value::String(host.build.clone()))
          .with("exits", Value::Bool(exits)),
      );
      if exits {
        self.exit_after_answer();
      } else if stale {
        (host.hooks.restart_to_update)();
      }
      return;
    }
    let Some(version) = negotiate(offered) else {
      return self.fail(id, &Failure::new(-32005, "ahpVersion", "No offered version of AHP is one the backend serves"));
    };
    if semver(vocabulary).map(|(major, _, _)| major) != semver(VOCABULARY_VERSION).map(|(major, _, _)| major) {
      return self.fail(
        id,
        &Failure::new(-32005, "vocabularyVersion", format!("The vocabulary of the backend is {VOCABULARY_VERSION}")),
      );
    }
    let capabilities = Capabilities::of(&params["_meta"]["x-branch-deck"]["capabilities"]);
    let of = match self.kind {
      Kind::App => ConnectionId::APP,
      Kind::Client => host.ctx.connections.open(client_hosts(&self.me(), &capabilities, &host.ctx.app_desktop)),
    };
    let _ = self.client_id.set(client_id.to_owned());
    let _ = self.id.set(of);
    if self.kind == Kind::Client {
      host.count_client(true);
    }
    self.answer(
      id,
      json!({
        "protocolVersion": version,
        "serverSeq": host.server_seq(),
        "snapshots": [],
        "serverInfo": { "name": "branch-deck", "build": host.build },
        "_meta": { "x-branch-deck": { "version": VOCABULARY_VERSION } },
      }),
    );
  }

  /// A stale backend without a window exits once the answer of `otherBuild` was written
  fn exit_after_answer(&self) {
    let host = Arc::clone(&self.host);
    self.host.ctx.runtime.spawn(
      async move {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        (host.hooks.exit)();
      }
      .in_current_span(),
    );
  }

  /// `x-branch-deck/bootstrap {window, view, repositoryPath?}`: the settings of the window as the app hands them in its
  /// init script, and for the main view the warm frame of the repository it names, else of the selected project (the
  /// bridge names the root of the project its main view shows)
  fn bootstrap(&self, id: Value, params: &Value) {
    let Some(label) = params["window"].as_str().map(str::to_owned) else {
      return self.fail(&id, &Failure::bad_params("bootstrap names its window"));
    };
    let main = params["view"].as_str() == Some("main");
    let named = params["repositoryPath"].as_str().filter(|path| !path.is_empty()).map(str::to_owned);
    let me = self.me();
    self.host.ctx.runtime.spawn(
      async move {
        let ctx = &me.host.ctx;
        let (version, values) = app_settings::windows::window_snapshot(&ctx.settings, &label);
        let repository = named.or_else(|| app_settings::projects::selected_project(&ctx.settings));
        let warm_frame = match repository.filter(|_| main) {
          Some(path) => app_api::app::warm_start::seed(ctx, &path).await,
          None => None,
        };
        me.answer(&id, json!({ "settings": { "values": values, "version": version }, "warmFrame": warm_frame }));
      }
      .in_current_span(),
    );
  }

  /// The connection ended: each of its windows closes, its calls are canceled, its host requests get no answer
  pub fn close(&self) {
    if self.ended.swap(true, Ordering::SeqCst) {
      return;
    }
    self.outbox.close();
    for canceled in lock(&self.calls).values() {
      canceled.store(true, Ordering::SeqCst);
    }
    lock(&self.requests).clear();
    if let Some(of) = self.id() {
      self.host.end_windows(of, None);
      self.host.ctx.close_connection(of);
      if self.kind == Kind::Client {
        self.host.count_client(false);
      }
    }
    self.host.forget_connection(self.key);
  }
}

impl Drop for Connection {
  fn drop(&mut self) {
    self.close();
  }
}

#[cfg(test)]
#[path = "connection_test.rs"]
mod connection_test;
