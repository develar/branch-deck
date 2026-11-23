//! The pages of the app's own windows over the in-process connection (docs/design/backend-protocol.md, "The pages of
//! the app"): the Tauri shell and the page mock of the e2e server hand each `invoke` of a page here, by the name of its
//! command, its arguments as the page sends them and the channel it gave, and answer it as Tauri answers an `invoke`,
//! so the page keeps its bindings while every command goes through the host, in one connection for every window
//! (`InProcess::app`, `ConnectionId::APP`).
//!
//! - A command is the root command of its window (`x-branch-deck/<name>` with the label of the window); each call event
//!   goes to the channel of its call, and a page that cannot read its channel any more cancels the call.
//! - `subscribe_view`, `unsubscribe_view` and `set_demand` are `subscribe`, `unsubscribe` and `x-branch-deck/setDemand`
//!   of the window on the repository channel of its path, under an id of the subscription that the page keeps; each
//!   `viewPatched` of the window goes to the channel of its subscription. A window holds one subscription of a channel:
//!   a subscribe of a window that holds one (the page subscribes again before it ends the subscription it replaces)
//!   ends that one first, so the later unsubscribe of the replaced id ends nothing.
//! - A resync (the channel moved to the session of another prefix, a window past its cap) reaches the page as a patch
//!   from and to the version of the snapshot, which no view applies: the page reads the snapshot (`get_snapshot`) as it
//!   does after a gap, and takes the view of the other epoch (`installAnother`).
//! - An answer, a patch and a call event reach the page as the text of their JSON, cut out of the frame, never a tree
//!   built again; the bytes of a raw command as they are.

use crate::host::{Host, lock};
use crate::in_process::InProcess;
use crate::wire::{EXTENSION, Frame, ROOT};
use domain_types::error::ApiError;
use domain_types::session::{SetDemandParams, SubscribeViewParams, Version};
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use tokio::sync::oneshot;
use tracing::Instrument;

/// What a command of a page answers: the text of the JSON of its output, or the bytes of a raw command
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
  Json(String),
  Bytes(Vec<u8>),
}

/// Why a command of a page has no answer
#[derive(Debug, PartialEq, Eq)]
pub enum Failed {
  /// It failed with its `ApiError`, the text of its JSON
  Error(String),
  /// It panicked
  Panicked,
}

/// The answer of a command of a page
pub type Reply = Result<Answer, Failed>;

/// A channel a page gave a command: the patches of its subscription, the events of its call
pub trait PageChannel: Send + Sync {
  /// Sends the message whose JSON is `json`: false once the page cannot read it any more
  fn send(&self, json: String) -> bool;
}

/// The window of a channel: the URI of the channel and the label of the window
type WindowKey = (String, String);

/// The pages of the app's own windows, over their connection to the host
pub struct Pages {
  host: Arc<Host>,
  client: InProcess,
  state: Mutex<State>,
}

#[derive(Default)]
struct State {
  last_request: u64,
  /// The requests sent, by their id: what each answers
  waiting: HashMap<u64, Waiting>,
  /// The subscription each window holds of each channel
  subscriptions: HashMap<WindowKey, Subscription>,
  /// The window of each subscription id the page holds
  ids: HashMap<u32, WindowKey>,
  last_id: u32,
}

struct Subscription {
  id: u32,
  patches: Arc<dyn PageChannel>,
}

struct Waiting {
  reply: oneshot::Sender<Reply>,
  expect: Expect,
  /// The channel of a call: its events
  events: Option<Arc<dyn PageChannel>>,
}

/// What a request answers the page
enum Expect {
  /// A root command: its `{value}`, `{bytes}` or `{error}`
  Command,
  /// A subscribe: its snapshot as the `ViewSubscription` of the id
  Subscription(u32),
  /// An unsubscribe, a demand: `null`, whatever the host answered
  Nothing,
}

/// The answer of a request, or the reply the page gets at once
enum Pending {
  Now(Reply),
  Later(oneshot::Receiver<Reply>),
}

impl State {
  /// A new request that answers by `expect`: its id and the receiver of its reply
  fn wait(&mut self, expect: Expect, events: Option<Arc<dyn PageChannel>>) -> (u64, oneshot::Receiver<Reply>) {
    self.last_request += 1;
    let (reply, answer) = oneshot::channel();
    self.waiting.insert(self.last_request, Waiting { reply, expect, events });
    (self.last_request, answer)
  }

  /// The subscription of `key` ends, when it is `id`
  fn end(&mut self, key: &WindowKey, id: u32) -> bool {
    if self.subscriptions.get(key).is_some_and(|subscription| subscription.id == id) {
      self.subscriptions.remove(key);
      self.ids.remove(&id);
      return true;
    }
    false
  }
}

/// A frame of the host as far as the pages read it: the parts they hand on stay the text of their JSON
#[derive(Deserialize)]
struct Message<'a> {
  #[serde(default, borrow)]
  id: Option<&'a RawValue>,
  #[serde(default, borrow)]
  method: Option<Cow<'a, str>>,
  #[serde(default, borrow)]
  params: Option<&'a RawValue>,
  #[serde(default, borrow)]
  result: Option<&'a RawValue>,
  #[serde(default, borrow)]
  error: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct ActionParams<'a> {
  #[serde(borrow)]
  channel: Cow<'a, str>,
  #[serde(borrow)]
  window: Cow<'a, str>,
  #[serde(borrow)]
  action: &'a RawValue,
}

#[derive(Deserialize)]
struct ViewPatched<'a> {
  #[serde(rename = "type", borrow)]
  kind: Cow<'a, str>,
  #[serde(borrow)]
  patch: &'a RawValue,
}

#[derive(Deserialize)]
struct WindowParams<'a> {
  #[serde(borrow)]
  channel: Cow<'a, str>,
  #[serde(borrow)]
  window: Cow<'a, str>,
  #[serde(default, borrow)]
  snapshot: Option<Snapshot<'a>>,
}

#[derive(Deserialize)]
struct Snapshot<'a> {
  #[serde(default, borrow)]
  state: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct Subscribed<'a> {
  #[serde(borrow)]
  snapshot: Snapshot<'a>,
}

/// The version of a view, the rest of it passed over
#[derive(Deserialize)]
struct VersionOf {
  version: Version,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallEvent<'a> {
  request_id: u64,
  #[serde(borrow)]
  event: &'a RawValue,
}

#[derive(Deserialize)]
struct CommandResult<'a> {
  #[serde(default, borrow)]
  value: Option<&'a RawValue>,
  #[serde(default, borrow)]
  error: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct RpcError<'a> {
  #[serde(default, borrow)]
  message: Cow<'a, str>,
  #[serde(default, borrow)]
  data: Option<RpcData<'a>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcData<'a> {
  #[serde(default, borrow)]
  reason: Cow<'a, str>,
  #[serde(default, borrow)]
  api_error: Option<&'a RawValue>,
}

/// The text of the JSON of `error`
fn error_text(error: &ApiError) -> String {
  serde_json::to_string(error).unwrap_or_else(|_| r#"{"code":"internal","message":"An error that is not JSON"}"#.to_owned())
}

fn failed(error: &ApiError) -> Reply {
  Err(Failed::Error(error_text(error)))
}

/// The request `method` with `params` and the id `id`
fn request(id: u64, method: &str, params: &Value) -> Value {
  json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

/// The `_meta` of a channel command of `window`, with `have` when the window holds a view
fn meta(window: &str, have: Option<Version>) -> Value {
  let mut meta = json!({ "window": window });
  if let Some(have) = have {
    meta["have"] = json!(have);
  }
  json!({ "x-branch-deck": meta })
}

impl Pages {
  /// The pages of the app on `host`: its connection of the app, initialized before the first command, and the reader of
  /// its frames
  pub fn new(host: &Arc<Host>) -> Arc<Self> {
    let pages = Arc::new(Self {
      host: Arc::clone(host),
      client: InProcess::app(host),
      state: Mutex::default(),
    });
    pages.client.send(&json!({ "jsonrpc": "2.0", "id": "initialize", "method": "initialize", "params": {
      "channel": ROOT,
      "protocolVersions": ["1.0.0"],
      "clientId": "app",
      "clientInfo": { "name": "branch-deck-app", "build": host.build() },
      "_meta": { "x-branch-deck": { "version": crate::wire::VOCABULARY_VERSION, "capabilities": {
        "folderDialog": true, "updater": true, "restart": true, "agentCli": true, "nativeMenu": true,
      } } },
    } }));
    let connection = Arc::clone(pages.client.connection());
    let reader = Arc::downgrade(&pages);
    host.context().runtime.spawn(
      async move {
        while let Some(frame) = connection.next_frame().await {
          let Some(pages) = Weak::upgrade(&reader) else {
            break;
          };
          pages.read(frame);
        }
      }
      .in_current_span(),
    );
    pages
  }

  pub const fn host(&self) -> &Arc<Host> {
    &self.host
  }

  fn state(&self) -> MutexGuard<'_, State> {
    lock(&self.state)
  }

  /// The command `command` of the page of `window`, with `args` as the page sends them and the channel it gave
  /// (`channel`): its reply as Tauri answers the `invoke`. The command is sent before this returns, so the commands of
  /// a page reach the host in the order it invoked them
  pub fn invoke(&self, window: &str, command: &str, args: &Value, channel: Option<Arc<dyn PageChannel>>) -> impl Future<Output = Reply> + Send + 'static {
    let pending = match command {
      "subscribe_view" => self.subscribe(window, args, channel),
      "unsubscribe_view" => self.unsubscribe(args),
      "set_demand" => self.set_demand(args),
      _ => self.call(window, command, args, channel),
    };
    async move {
      match pending {
        Pending::Now(reply) => reply,
        Pending::Later(answer) => answer.await.unwrap_or_else(|_| failed(&ApiError::internal("The backend of the app went away"))),
      }
    }
  }

  /// The page of `window` loads again: what its page held ends (`x-branch-deck/pageLoaded`)
  pub fn page_loaded(&self, window: &str) {
    self.window_went("x-branch-deck/pageLoaded", window);
  }

  /// `window` closed: it is forgotten (`x-branch-deck/windowClosed`)
  pub fn window_closed(&self, window: &str) {
    self.window_went("x-branch-deck/windowClosed", window);
  }

  fn window_went(&self, method: &str, window: &str) {
    let mut state = self.state();
    state.subscriptions.retain(|(_, label), _| label != window);
    let kept: std::collections::HashSet<u32> = state.subscriptions.values().map(|subscription| subscription.id).collect();
    state.ids.retain(|id, _| kept.contains(id));
    self
      .client
      .send(&json!({ "jsonrpc": "2.0", "method": method, "params": { "channel": ROOT, "window": window } }));
  }

  /// A root command of the window
  fn call(&self, window: &str, command: &str, args: &Value, events: Option<Arc<dyn PageChannel>>) -> Pending {
    let (id, answer) = self.state().wait(Expect::Command, events);
    let params = json!({ "channel": ROOT, "window": window, "args": args });
    self.client.send(&request(id, &format!("{EXTENSION}{command}"), &params));
    Pending::Later(answer)
  }

  /// `subscribe_view`: `subscribe` of the window on the channel of the path, ending the subscription the window holds of
  /// it
  fn subscribe(&self, window: &str, args: &Value, patches: Option<Arc<dyn PageChannel>>) -> Pending {
    let params: SubscribeViewParams = match serde_json::from_value(args["params"].clone()) {
      Ok(params) => params,
      Err(error) => return Pending::Now(failed(&ApiError::internal(format!("Invalid arguments of subscribe_view: {error}")))),
    };
    let Some(patches) = patches else {
      return Pending::Now(failed(&ApiError::internal("subscribe_view takes the channel of its patches")));
    };
    let uri = self.host.channel_of_path(&params.repository_path).uri.clone();
    let key = (uri.clone(), window.to_owned());
    let mut state = self.state();
    // under the lock to the end: two subscribes of a window reach the host in the order of their ids
    if let Some(replaced) = state.subscriptions.remove(&key) {
      state.ids.remove(&replaced.id);
      let (id, _) = state.wait(Expect::Nothing, None);
      self.client.send(&request(id, "unsubscribe", &json!({ "channel": uri, "_meta": meta(window, None) })));
    }
    state.last_id += 1;
    let subscription = state.last_id;
    state.subscriptions.insert(key.clone(), Subscription { id: subscription, patches });
    state.ids.insert(subscription, key);
    let (id, answer) = state.wait(Expect::Subscription(subscription), None);
    self.client.send(&request(id, "subscribe", &json!({ "channel": uri, "_meta": meta(window, params.have) })));
    Pending::Later(answer)
  }

  /// `unsubscribe_view`: `unsubscribe` of the window of the id, nothing for an id that ended already
  fn unsubscribe(&self, args: &Value) -> Pending {
    let Some(id) = args["params"]["id"].as_u64().and_then(|id| u32::try_from(id).ok()) else {
      return Pending::Now(failed(&ApiError::internal("unsubscribe_view names the id of its subscription")));
    };
    let mut state = self.state();
    let Some(key) = state.ids.get(&id).cloned() else {
      return Pending::Now(Ok(Answer::Json("null".to_owned())));
    };
    state.end(&key, id);
    let (request_id, answer) = state.wait(Expect::Nothing, None);
    self
      .client
      .send(&request(request_id, "unsubscribe", &json!({ "channel": key.0, "_meta": meta(&key.1, None) })));
    Pending::Later(answer)
  }

  /// `set_demand`: `x-branch-deck/setDemand` of the window of the subscription, nothing for one that ended
  fn set_demand(&self, args: &Value) -> Pending {
    let params: SetDemandParams = match serde_json::from_value(args["params"].clone()) {
      Ok(params) => params,
      Err(error) => return Pending::Now(failed(&ApiError::internal(format!("Invalid arguments of set_demand: {error}")))),
    };
    let mut state = self.state();
    let Some((channel, window)) = state.ids.get(&params.subscription).cloned() else {
      return Pending::Now(Ok(Answer::Json("null".to_owned())));
    };
    let (id, answer) = state.wait(Expect::Nothing, None);
    let params = json!({ "channel": channel, "window": window, "demand": params.demand });
    self.client.send(&request(id, "x-branch-deck/setDemand", &params));
    Pending::Later(answer)
  }

  /// One frame of the host
  fn read(&self, frame: Frame) {
    match frame {
      Frame::Bytes { id, bytes } => {
        if let Some(waiting) = id.as_u64().and_then(|id| self.state().waiting.remove(&id)) {
          let _ = waiting.reply.send(Ok(Answer::Bytes(bytes)));
        }
      }
      Frame::Text(text) => match serde_json::from_str::<Message<'_>>(&text) {
        Ok(message) => self.read_message(&message),
        Err(error) => tracing::error!(%error, "a frame of the host is not JSON-RPC"),
      },
    }
  }

  fn read_message(&self, message: &Message<'_>) {
    match (message.method.as_deref(), message.params) {
      (Some("action"), Some(params)) => {
        if let Ok(action) = serde_json::from_str::<ActionParams<'_>>(params.get())
          && let Ok(patched) = serde_json::from_str::<ViewPatched<'_>>(action.action.get())
          && patched.kind == "x-branch-deck/viewPatched"
        {
          self.patch(&(action.channel.into_owned(), action.window.into_owned()), patched.patch.get().to_owned());
        }
      }
      (Some("x-branch-deck/resync"), Some(params)) => {
        let Ok(resync) = serde_json::from_str::<WindowParams<'_>>(params.get()) else {
          return;
        };
        let version = resync
          .snapshot
          .and_then(|snapshot| snapshot.state)
          .and_then(|state| serde_json::from_str::<VersionOf>(state.get()).ok());
        if let Some(VersionOf { version }) = version {
          // a patch no view applies: the page reads the snapshot of the session
          let patch = json!({ "base": version, "next": version, "changes": {} }).to_string();
          self.patch(&(resync.channel.into_owned(), resync.window.into_owned()), patch);
        }
      }
      (Some("x-branch-deck/subscriptionEnded"), Some(params)) => {
        if let Ok(ended) = serde_json::from_str::<WindowParams<'_>>(params.get()) {
          let key = (ended.channel.into_owned(), ended.window.into_owned());
          let mut state = self.state();
          if let Some(id) = state.subscriptions.get(&key).map(|subscription| subscription.id) {
            state.end(&key, id);
          }
        }
      }
      (Some("x-branch-deck/callEvent"), Some(params)) => {
        if let Ok(event) = serde_json::from_str::<CallEvent<'_>>(params.get()) {
          self.call_event(event.request_id, event.event.get().to_owned());
        }
      }
      // the events of the app reach its windows through its own `AppEvents`, and the host asks the app nothing
      (Some(_), _) => {}
      (None, _) => self.answered(message),
    }
  }

  /// A patch of the session to the window of `key`: to the channel of its subscription; a page that cannot read it any
  /// more ends the subscription
  fn patch(&self, key: &WindowKey, patch: String) {
    let Some((id, patches)) = self.state().subscriptions.get(key).map(|subscription| (subscription.id, Arc::clone(&subscription.patches))) else {
      return;
    };
    if patches.send(patch) {
      return;
    }
    let mut state = self.state();
    if state.end(key, id) {
      let (request_id, _) = state.wait(Expect::Nothing, None);
      self
        .client
        .send(&request(request_id, "unsubscribe", &json!({ "channel": key.0, "_meta": meta(&key.1, None) })));
    }
  }

  /// An event of the call `request`: to the channel of the call; a page that cannot read it any more cancels the call
  fn call_event(&self, request: u64, event: String) {
    let events = self.state().waiting.get(&request).and_then(|waiting| waiting.events.clone());
    if let Some(events) = events
      && !events.send(event)
    {
      self
        .client
        .send(&json!({ "jsonrpc": "2.0", "method": "x-branch-deck/cancel", "params": { "channel": ROOT, "requestId": request } }));
    }
  }

  /// The answer of a request of the pages: the reply of its page
  fn answered(&self, message: &Message<'_>) {
    let Some(id) = message.id.and_then(|id| id.get().parse::<u64>().ok()) else {
      return;
    };
    let Some(waiting) = self.state().waiting.remove(&id) else {
      return;
    };
    let reply = match (message.result, message.error) {
      (_, Some(error)) => {
        let reply = rpc_failure(error);
        if let Expect::Subscription(subscription) = waiting.expect {
          let mut state = self.state();
          if let Some(key) = state.ids.get(&subscription).cloned() {
            state.end(&key, subscription);
          }
        }
        reply
      }
      (Some(result), None) => match waiting.expect {
        Expect::Command => command_reply(result),
        Expect::Subscription(subscription) => {
          let view = serde_json::from_str::<Subscribed<'_>>(result.get())
            .ok()
            .and_then(|subscribed| subscribed.snapshot.state.map(|state| state.get().to_owned()));
          let view = view.as_deref().unwrap_or("null");
          Ok(Answer::Json(format!(r#"{{"id":{subscription},"view":{view},"catchUp":null}}"#)))
        }
        Expect::Nothing => Ok(Answer::Json("null".to_owned())),
      },
      (None, None) => Ok(Answer::Json("null".to_owned())),
    };
    let _ = waiting.reply.send(reply);
  }
}

/// The reply of the result `{value}` or `{error}` of a root command
fn command_reply(result: &RawValue) -> Reply {
  match serde_json::from_str::<CommandResult<'_>>(result.get()) {
    Ok(CommandResult { error: Some(error), .. }) => Err(Failed::Error(error.get().to_owned())),
    Ok(CommandResult { value: Some(value), .. }) => Ok(Answer::Json(value.get().to_owned())),
    Ok(CommandResult { value: None, error: None }) => Ok(Answer::Json("null".to_owned())),
    Err(error) => failed(&ApiError::internal(format!("The answer of the backend is no result of a command: {error}"))),
  }
}

/// The reply of a JSON-RPC error: the `ApiError` of its data, a panic, or a failure of the protocol as `internal`
fn rpc_failure(error: &RawValue) -> Reply {
  let Ok(error) = serde_json::from_str::<RpcError<'_>>(error.get()) else {
    return failed(&ApiError::internal("The backend failed the command"));
  };
  match error.data {
    Some(RpcData { api_error: Some(api_error), .. }) => Err(Failed::Error(api_error.get().to_owned())),
    Some(RpcData { reason, .. }) if reason == "panic" => Err(Failed::Panicked),
    _ => failed(&ApiError::internal(error.message.into_owned())),
  }
}

#[cfg(test)]
#[path = "page_test.rs"]
mod page_test;
