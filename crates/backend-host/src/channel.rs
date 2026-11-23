//! The repository channels (docs/design/backend-protocol.md, "The repository channel"): one per repository path for the
//! life of the host, `x-branch-deck:/repository/<n>`. A subscription is a window's: `subscribe` subscribes that window of
//! the connection to the session of the prefix the git config names now, as `subscribe_view` does (through
//! `app_api::session::view::subscribe_view`: the registry, the `have` of the window, `Windows::keep`), and each patch
//! of the session is one `x-branch-deck/viewPatched` action to each subscribed window, in a frame of its own, all with
//! the `serverSeq` of the action. A patch that comes before the answer of its subscribe waits for it; a subscribe of a
//! prefix another than the one the channel follows moves the channel to that session, with a resync to each window
//! subscribed to it; the end of a session ends its subscriptions (`subscriptionEnded`).

use crate::connection::Connection;
use crate::host::{Host, Sequence, lock};
use crate::outbox::{Pushed, WindowOf};
use crate::wire::{Failure, Frame};
use app_api::session::view::subscribe_view;
use app_api::transport::{Call, ConnectionId, EventSink, SinkGone, WindowId};
use domain_types::session::{Demand, RepositoryView, SubscribeViewParams, Version, ViewPatch, ViewSubscription};
use repository_model::session::{SessionKey, SubscriptionId};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;
use tracing::Instrument;

/// The URI of a repository channel, before its number
pub const URI_PREFIX: &str = "x-branch-deck:/repository/";

/// How long an answer that names a version waits at most for the actions up to it (the fence); a wait that long is a
/// bug, logged, and the answer goes
const FENCE_LIMIT: Duration = Duration::from_secs(10);

/// A window of a connection
type WindowKey = (ConnectionId, String);

/// The channel of one repository path
pub struct Channel {
  pub id: u64,
  pub uri: String,
  pub path: String,
  state: Mutex<ChannelState>,
}

#[derive(Default)]
pub(crate) struct ChannelState {
  /// The session the channel follows now: the one its windows subscribed to last
  pub(crate) key: Option<SessionKey>,
  /// The epoch of the views of that session
  pub(crate) epoch: Option<u32>,
  pub(crate) windows: BTreeMap<WindowKey, WindowSubscription>,
  /// The last patch of the session queued, encoded, with its `serverSeq`: the frames of the same patch to the other
  /// windows carry it
  last: Option<Last>,
}

struct Last {
  base: Version,
  next: Version,
  seq: u64,
  action: Arc<str>,
}

/// The subscription of one window
pub(crate) struct WindowSubscription {
  pub(crate) connection: Weak<Connection>,
  /// The subscription of the session, its call: none while the window joins
  pub(crate) subscription: Option<(SubscriptionId, Call)>,
  /// The host ended it (an unsubscribe, a close, a move): its session sends nothing more to the window
  pub(crate) ended: Arc<AtomicBool>,
  /// The version of the view the window holds by the frames queued so far
  pub(crate) delivered: Option<Version>,
  /// The patches that came before the answer of the subscribe, which follow it
  pub(crate) joining: Option<Vec<ViewPatch>>,
}

impl ChannelState {
  /// The `serverSeq` and the action of `patch` when it is the patch queued last: another window gets it in the same frame
  fn same(&self, patch: &ViewPatch) -> Option<(u64, Arc<str>)> {
    self
      .last
      .as_ref()
      .filter(|last| last.base == patch.base && last.next == patch.next)
      .map(|last| (last.seq, Arc::clone(&last.action)))
  }
}

impl Channel {
  pub(crate) fn new(id: u64, path: String) -> Self {
    Self {
      id,
      uri: format!("{URI_PREFIX}{id}"),
      path,
      state: Mutex::default(),
    }
  }

  pub(crate) fn state(&self) -> MutexGuard<'_, ChannelState> {
    lock(&self.state)
  }

  /// Whether each subscribed window of `connection` holds `version` by the frames queued to it: a version of another
  /// epoch than the one the channel follows waits for nothing
  fn reached(&self, connection: ConnectionId, version: Version) -> bool {
    let state = self.state();
    if state.epoch != Some(version.epoch) {
      return true;
    }
    state
      .windows
      .iter()
      .filter(|((of, _), _)| *of == connection)
      .all(|(_, window)| window.joining.is_none() && window.delivered.is_some_and(|delivered| delivered.epoch == version.epoch && delivered.seq >= version.seq))
  }

  /// The windows of `connection` subscribed now
  fn windows_of(&self, connection: ConnectionId) -> Vec<String> {
    self.state().windows.keys().filter(|(of, _)| *of == connection).map(|(_, label)| label.clone()).collect()
  }
}

/// The patches of a session to one window of a channel: queued by the host for the window's connection
struct WindowPatches {
  host: Weak<Host>,
  channel: Arc<Channel>,
  window: WindowKey,
  ended: Arc<AtomicBool>,
}

impl EventSink<ViewPatch> for WindowPatches {
  fn send(&self, patch: ViewPatch) -> Result<(), SinkGone> {
    if self.ended.load(Ordering::SeqCst) {
      return Err(SinkGone);
    }
    let host = self.host.upgrade().ok_or(SinkGone)?;
    host.publish_patch(&self.channel, &self.window, patch)
  }
}

/// The session dropped the subscription without the host ending it: the session closed
impl Drop for WindowPatches {
  fn drop(&mut self) {
    if self.ended.swap(true, Ordering::SeqCst) {
      return;
    }
    if let Some(host) = self.host.upgrade() {
      host.subscription_ended(&self.channel, &self.window, &self.ended, "sessionClosed");
    }
  }
}

/// The `x-branch-deck/viewPatched` action of `patch`
fn action_of(patch: &ViewPatch) -> Arc<str> {
  let action = json!({ "type": "x-branch-deck/viewPatched", "patch": patch });
  Arc::from(action.to_string())
}

/// The frame of `action` to `window`, with its `serverSeq`
fn action_frame(channel: &Channel, window: &str, seq: u64, action: &str) -> Frame {
  let window = Value::String(window.to_owned());
  Frame::Text(format!(
    r#"{{"jsonrpc":"2.0","method":"action","params":{{"channel":"{}","action":{action},"window":{window},"serverSeq":{seq}}}}}"#,
    channel.uri
  ))
}

/// The snapshot of a channel: the view, or `held` when the window holds `have`
fn snapshot(channel: &Channel, view: Option<&RepositoryView>, held: Option<Version>, from_seq: u64) -> Value {
  match view {
    Some(view) => json!({ "resource": channel.uri, "state": view, "fromSeq": from_seq }),
    None => json!({ "resource": channel.uri, "fromSeq": from_seq, "_meta": { "x-branch-deck": { "held": held } } }),
  }
}

/// What a window subscribed to: the answer of the session, and what the window said it holds
struct Joined {
  window: WindowKey,
  connection: Arc<Connection>,
  answer: ViewSubscription,
  have: Option<Version>,
}

/// The params of a channel command: its channel and the window its `_meta` (or its params) names
fn window_of(params: &Value) -> Option<String> {
  let named = params["_meta"]["x-branch-deck"]["window"].as_str().or_else(|| params["window"].as_str())?;
  (!named.is_empty()).then(|| named.to_owned())
}

impl Host {
  /// `x-branch-deck/repositoryChannel {repositoryPath}`: the URI of the channel of the path
  pub(crate) fn repository_channel(&self, connection: &Connection, id: &Value, params: &Value) {
    let Some(path) = params["repositoryPath"].as_str().filter(|path| !path.is_empty()) else {
      return connection.fail(id, &Failure::bad_params("repositoryChannel names its repositoryPath"));
    };
    let channel = self.channel_of_path(path);
    connection.answer(id, json!({ "channel": channel.uri }));
  }

  fn channel_param(&self, params: &Value) -> Result<Arc<Channel>, Failure> {
    let uri = params["channel"].as_str().unwrap_or_default();
    self.channel_of_uri(uri).ok_or_else(|| Failure::channel("unknownChannel", format!("No channel is {uri}")))
  }

  /// `subscribe {channel, _meta["x-branch-deck"]: {window, have?}}`: the window of the connection subscribes to the
  /// session of the prefix the git config names now; a channel that follows another session moves to it, and its
  /// other windows get a resync. The window is reserved at once, in the order of the lines of the connection, so a
  /// second subscribe of it is `alreadySubscribed`
  pub(crate) fn subscribe(&self, connection: &Arc<Connection>, id: Value, params: &Value) {
    let Some(of) = connection.id() else {
      return connection.fail(&id, &Failure::invalid("notInitialized", "initialize first"));
    };
    let channel = match self.channel_param(params) {
      Ok(channel) => channel,
      Err(failure) => return connection.fail(&id, &failure),
    };
    let Some(label) = window_of(params) else {
      return connection.fail(&id, &Failure::bad_params("A subscribe names its window: _meta[\"x-branch-deck\"].window"));
    };
    let have = match params["_meta"]["x-branch-deck"].get("have") {
      None | Some(Value::Null) => None,
      Some(have) => match serde_json::from_value::<Version>(have.clone()) {
        Ok(have) => Some(have),
        Err(error) => return connection.fail(&id, &Failure::bad_params(format!("have is a Version: {error}"))),
      },
    };
    let window = (of, label);
    let ended = Arc::new(AtomicBool::new(false));
    {
      let mut state = channel.state();
      if state.windows.contains_key(&window) {
        return connection.fail(
          &id,
          &Failure::new(-32011, "alreadySubscribed", format!("{} subscribes to {} already", window.1, channel.uri)),
        );
      }
      state.windows.insert(
        window.clone(),
        WindowSubscription {
          connection: Arc::downgrade(connection),
          subscription: None,
          ended: Arc::clone(&ended),
          delivered: None,
          joining: Some(Vec::new()),
        },
      );
    }
    let (host, connection) = (self.me(), Arc::clone(connection));
    self.ctx.runtime.spawn(
      async move {
        host.subscribe_reserved(&connection, &id, &channel, window, ended, have).await;
      }
      .in_current_span(),
    );
  }

  /// The rest of `subscribe`, for the window reserved
  async fn subscribe_reserved(&self, connection: &Arc<Connection>, id: &Value, channel: &Arc<Channel>, window: WindowKey, ended: Arc<AtomicBool>, have: Option<Version>) {
    let channel = Arc::clone(channel);
    let key = match self.ctx.session_key_now(&channel.path).await {
      Ok(key) => key,
      Err(error) => {
        channel.state().windows.remove(&window);
        return connection.fail(id, &Failure::api_error(&error));
      }
    };
    let moved = self.follow(&channel, &key, &window);
    let mine = match self.join(&channel, connection, &window, &ended, &key, have).await {
      Ok(joined) => Ok(joined),
      Err(error) => {
        ended.store(true, Ordering::SeqCst);
        channel.state().windows.remove(&window);
        Err(Failure::api_error(&error))
      }
    };
    let state_of_mine = mine.as_ref().ok().and_then(|joined| joined.answer.view.clone());
    let mut others = Vec::new();
    for (other, other_ended) in moved {
      let Some(other_connection) = channel.state().windows.get(&other).and_then(|window| window.connection.upgrade()) else {
        continue;
      };
      let have = state_of_mine.as_ref().map(|view| view.version);
      match self.join(&channel, &other_connection, &other, &other_ended, &key, have).await {
        Ok(joined) => others.push(joined),
        Err(error) => {
          tracing::warn!(window = %other.1, channel = %channel.uri, %error, "a window of a channel that moved cannot subscribe again");
          other_ended.store(true, Ordering::SeqCst);
          channel.state().windows.remove(&other);
          other_connection.notify("x-branch-deck/subscriptionEnded", json!({ "channel": channel.uri, "window": other.1, "reason": "closed" }));
        }
      }
    }
    let moving = !others.is_empty();
    // the windows of a move start from one view, the session's once every one joined: the patches each heard before
    // it are in it
    let current = if moving { self.ctx.sessions.get_snapshot(&key).await } else { None };
    let state_of_all = current.as_ref().or(state_of_mine.as_ref());
    self.publish(|sequence| {
      let from_seq = if moving { sequence.next() } else { sequence.now() };
      for joined in &others {
        let view = current.as_ref().or(joined.answer.view.as_ref()).or(state_of_all);
        let resync = json!({ "channel": channel.uri, "window": joined.window.1, "snapshot": snapshot(&channel, view, None, from_seq) });
        joined.connection.notify("x-branch-deck/resync", resync);
        Self::joined(&channel, joined, view.map(|view| view.version), sequence);
      }
      match &mine {
        Ok(joined) => {
          let view = joined.answer.view.as_ref().map(|view| current.as_ref().unwrap_or(view));
          let held = view.is_none().then_some(joined.have).flatten();
          connection.answer(id, json!({ "snapshot": snapshot(&channel, view, held, from_seq) }));
          Self::joined(&channel, joined, view.map(|view| view.version).or(held), sequence);
        }
        Err(failure) => connection.fail(id, failure),
      }
    });
  }

  /// The channel follows the session of `key` from now on: when it followed another, the windows subscribed to it but
  /// `window` leave that session and join again, which this answers, each with its new flag
  fn follow(&self, channel: &Channel, key: &SessionKey, window: &WindowKey) -> Vec<(WindowKey, Arc<AtomicBool>)> {
    let mut left = Vec::new();
    let mut moved = Vec::new();
    {
      let mut state = channel.state();
      let other_session = state.key.as_ref().is_some_and(|followed| followed != key);
      state.key = Some(key.clone());
      if other_session {
        state.last = None;
        for (other, subscription) in state.windows.iter_mut().filter(|(other, _)| *other != window) {
          let Some(session) = subscription.subscription.take() else {
            continue;
          };
          subscription.ended.store(true, Ordering::SeqCst);
          let fresh = Arc::new(AtomicBool::new(false));
          subscription.ended = Arc::clone(&fresh);
          subscription.joining = Some(Vec::new());
          subscription.delivered = None;
          left.push(session);
          moved.push((other.clone(), fresh));
        }
      }
    }
    for (subscription, call) in left {
      self.ctx.windows.drop_subscription(&call, subscription);
      self.ctx.sessions.unsubscribe(subscription);
    }
    moved
  }

  /// The window subscribes to the session of `key` (`subscribe_view`): the subscription is the window's, joining until
  /// its answer is queued
  async fn join(
    &self,
    channel: &Arc<Channel>,
    connection: &Arc<Connection>,
    window: &WindowKey,
    ended: &Arc<AtomicBool>,
    key: &SessionKey,
    have: Option<Version>,
  ) -> Result<Joined, domain_types::error::ApiError> {
    let call = self.ctx.call_of(&WindowId::of(window.0, window.1.clone()));
    let sink = Arc::new(WindowPatches {
      host: Arc::downgrade(&self.me()),
      channel: Arc::clone(channel),
      window: window.clone(),
      ended: Arc::clone(ended),
    });
    let params = SubscribeViewParams {
      repository_path: channel.path.clone(),
      have,
    };
    let answer = subscribe_view(&self.ctx, &call, key.clone(), params, sink).await?;
    if let Some(subscription) = channel.state().windows.get_mut(window) {
      subscription.subscription = Some((SubscriptionId(answer.id), call));
    }
    Ok(Joined {
      window: window.clone(),
      connection: Arc::clone(connection),
      answer,
      have,
    })
  }

  /// The answer of a window that joined is queued: its catch-up follows it, then the patches that came meanwhile from
  /// the version it holds; it is live from now on. Within the publish lock
  fn joined(channel: &Channel, joined: &Joined, held: Option<Version>, sequence: &mut Sequence<'_>) {
    let mut state = channel.state();
    let Some(subscription) = state.windows.get_mut(&joined.window) else {
      return;
    };
    subscription.delivered = held;
    let mut waiting = subscription.joining.take().unwrap_or_default();
    let of: WindowOf = (channel.id, joined.window.1.clone());
    if let Some(catch_up) = &joined.answer.catch_up
      && Some(catch_up.base) == subscription.delivered
    {
      let frame = action_frame(channel, &joined.window.1, sequence.next(), &action_of(catch_up));
      if joined.connection.outbox().push_action(&of, frame) == Pushed::Queued {
        subscription.delivered = Some(catch_up.next);
      }
    }
    let mut holds = subscription.delivered;
    if let Some(version) = holds {
      state.epoch = Some(version.epoch);
    }
    drop(state);
    // the patches the view of the answer holds already are left out, those after it follow in their order
    waiting.retain(|patch| holds.is_none_or(|holds| patch.base.epoch != holds.epoch || patch.base.seq >= holds.seq));
    for patch in waiting {
      if holds.is_none_or(|holds| patch.base == holds) && Self::queue_patch(channel, &joined.window, &patch, None, sequence).is_ok() {
        holds = Some(patch.next);
      }
    }
  }

  /// A patch of the session to `window`: queued now, or kept until the answer of its subscribe
  fn publish_patch(&self, channel: &Arc<Channel>, window: &WindowKey, patch: ViewPatch) -> Result<(), SinkGone> {
    let cached = {
      let mut state = channel.state();
      let subscription = state.windows.get_mut(window).ok_or(SinkGone)?;
      if let Some(waiting) = &mut subscription.joining {
        waiting.push(patch);
        return Ok(());
      }
      state.same(&patch)
    };
    let encoded = cached.is_none().then(|| action_of(&patch));
    self.publish(|sequence| Self::queue_patch(channel, window, &patch, encoded, sequence))
  }

  /// Queues the frame of `patch` to `window`, with the `serverSeq` of the patch when another window got it, else a new
  /// one. Within the publish lock
  fn queue_patch(channel: &Channel, window: &WindowKey, patch: &ViewPatch, encoded: Option<Arc<str>>, sequence: &mut Sequence<'_>) -> Result<(), SinkGone> {
    let mut state = channel.state();
    // a patch the view of the window holds already (the session sent it to every window after the snapshot of a new
    // one): not sent
    let delivered = state.windows.get(window).ok_or(SinkGone)?.delivered;
    if delivered.is_some_and(|delivered| delivered.epoch == patch.next.epoch && delivered.seq >= patch.next.seq) {
      return Ok(());
    }
    let (seq, action) = if let Some(same) = state.same(patch) {
      same
    } else {
      let action = encoded.unwrap_or_else(|| action_of(patch));
      let seq = sequence.next();
      state.last = Some(Last {
        base: patch.base,
        next: patch.next,
        seq,
        action: Arc::clone(&action),
      });
      (seq, action)
    };
    let subscription = state.windows.get_mut(window).ok_or(SinkGone)?;
    let connection = subscription.connection.upgrade().ok_or(SinkGone)?;
    let of: WindowOf = (channel.id, window.1.clone());
    match connection.outbox().push_action(&of, action_frame(channel, &window.1, seq, &action)) {
      Pushed::Queued => {
        subscription.delivered = Some(patch.next);
        state.epoch = Some(patch.next.epoch);
        Ok(())
      }
      Pushed::Stale => Ok(()),
      Pushed::Closed => Err(SinkGone),
    }
  }

  /// The session of a subscription went without the host ending it: the window gets `subscriptionEnded`
  fn subscription_ended(&self, channel: &Channel, window: &WindowKey, ended: &Arc<AtomicBool>, reason: &str) {
    let removed = {
      let mut state = channel.state();
      match state.windows.get(window) {
        Some(subscription) if Arc::ptr_eq(&subscription.ended, ended) && subscription.joining.is_none() => state.windows.remove(window),
        _ => None,
      }
    };
    let Some(removed) = removed else {
      return;
    };
    if let Some((_, call)) = &removed.subscription
      && let Some(subscription) = removed.subscription.as_ref().map(|(id, _)| *id)
    {
      self.ctx.windows.drop_subscription(call, subscription);
    }
    if let Some(connection) = removed.connection.upgrade() {
      connection.outbox().forget(&(channel.id, window.1.clone()));
      connection.notify("x-branch-deck/subscriptionEnded", json!({ "channel": channel.uri, "window": window.1, "reason": reason }));
    }
  }

  /// The channel and the window a channel command of `connection` names; its failure answered else
  fn window_command(&self, connection: &Connection, id: &Value, params: &Value) -> Option<(Arc<Channel>, WindowKey)> {
    let Some(of) = connection.id() else {
      connection.fail(id, &Failure::invalid("notInitialized", "initialize first"));
      return None;
    };
    let channel = match self.channel_param(params) {
      Ok(channel) => channel,
      Err(failure) => {
        connection.fail(id, &failure);
        return None;
      }
    };
    let Some(label) = window_of(params) else {
      connection.fail(id, &Failure::bad_params("A command of a channel names its window"));
      return None;
    };
    Some((channel, (of, label)))
  }

  /// `unsubscribe {channel, _meta["x-branch-deck"].window}`
  pub(crate) fn unsubscribe(&self, connection: &Connection, id: &Value, params: &Value) {
    let Some((channel, window)) = self.window_command(connection, id, params) else {
      return;
    };
    if self.end_window(&channel, &window) {
      connection.answer(id, json!({}));
    } else {
      connection.fail(id, &Failure::channel("notSubscribed", format!("{} subscribes to {} not", window.1, channel.uri)));
    }
  }

  /// Ends the subscription of `window` to `channel`: false when it had none
  fn end_window(&self, channel: &Channel, window: &WindowKey) -> bool {
    let Some(removed) = channel.state().windows.remove(window) else {
      return false;
    };
    removed.ended.store(true, Ordering::SeqCst);
    if let Some(connection) = removed.connection.upgrade() {
      connection.outbox().forget(&(channel.id, window.1.clone()));
    }
    if let Some((subscription, call)) = removed.subscription {
      self.ctx.windows.drop_subscription(&call, subscription);
      self.ctx.sessions.unsubscribe(subscription);
    }
    true
  }

  /// The window `label` of `connection` closed or loads again: its subscriptions of every channel end
  pub(crate) fn end_windows(&self, connection: ConnectionId, label: Option<&str>) {
    for channel in self.channels() {
      for window in channel.windows_of(connection) {
        if label.is_none_or(|label| label == window) {
          self.end_window(&channel, &(connection, window));
        }
      }
    }
  }

  /// `x-branch-deck/setDemand {channel, window, demand}`: the demand of the subscription of the window
  pub(crate) fn set_demand(&self, connection: &Connection, id: &Value, params: &Value) {
    let Some((channel, window)) = self.window_command(connection, id, params) else {
      return;
    };
    let demand: Demand = match serde_json::from_value(params["demand"].clone()) {
      Ok(demand) => demand,
      Err(error) => return connection.fail(id, &Failure::bad_params(format!("demand is a Demand: {error}"))),
    };
    let subscription = channel
      .state()
      .windows
      .get(&window)
      .and_then(|subscription| subscription.subscription.as_ref().map(|(subscription, _)| *subscription));
    match subscription {
      Some(subscription) => {
        self.ctx.sessions.set_demand(subscription, &demand);
        connection.answer(id, json!({}));
      }
      None => connection.fail(id, &Failure::channel("notSubscribed", format!("{} subscribes to {} not", window.1, channel.uri))),
    }
  }

  /// `dispatchAction` on a repository channel: the echo rejects it (an intent is the root command `execute`)
  pub(crate) fn dispatch_action(&self, connection: &Connection, params: &Value) {
    let Ok(channel) = self.channel_param(params) else {
      return;
    };
    let origin = json!({ "clientId": connection.client_id(), "clientSeq": params["clientSeq"] });
    self.publish(|sequence| {
      let echo = json!({
        "channel": channel.uri,
        "serverSeq": sequence.next(),
        "action": params["action"],
        "origin": origin,
        "rejectionReason": "unknownAction",
      });
      connection.notify("action", echo);
    });
  }

  /// The stale windows of `connection` get one resync each: the snapshot of the session, after which their actions are
  /// queued again
  pub(crate) async fn resync_stale(&self, connection: &Arc<Connection>, windows: Vec<WindowOf>) {
    let Some(of) = connection.id() else {
      return;
    };
    for (channel_id, label) in windows {
      let Some(channel) = self.channel_of_uri(&format!("{URI_PREFIX}{channel_id}")) else {
        continue;
      };
      let Some(key) = channel.state().key.clone() else {
        continue;
      };
      for _ in 0..8 {
        let Some(view) = self.ctx.sessions.get_snapshot(&key).await else {
          break;
        };
        let queued = self.publish(|sequence| {
          let mut state = channel.state();
          if state
            .last
            .as_ref()
            .is_some_and(|last| last.next.epoch == view.version.epoch && last.next.seq > view.version.seq)
          {
            // a patch after the snapshot was queued meanwhile: the snapshot again
            return false;
          }
          let Some(subscription) = state.windows.get_mut(&(of, label.clone())) else {
            return true;
          };
          let resync = json!({ "channel": channel.uri, "window": label, "snapshot": snapshot(&channel, Some(&view), None, sequence.next()) });
          connection.notify("x-branch-deck/resync", resync);
          subscription.delivered = Some(view.version);
          connection.outbox().resynced(&(channel_id, label.clone()));
          true
        });
        if queued {
          break;
        }
      }
    }
  }

  /// The fence: waits until each window of `connection` subscribed to the channel of `path` got the actions up to
  /// `version`, behind a resync when one is due; at once when none subscribes
  pub(crate) async fn fence(&self, connection: ConnectionId, path: &str, version: Version) {
    let Some(channel) = self.existing_channel(path) else {
      return;
    };
    let deadline = tokio::time::Instant::now() + FENCE_LIMIT;
    loop {
      let notified = self.published.notified();
      tokio::pin!(notified);
      notified.as_mut().enable();
      if channel.reached(connection, version) {
        return;
      }
      if tokio::time::timeout_at(deadline, notified).await.is_err() {
        tracing::warn!(channel = %channel.uri, ?version, "an answer waited too long for the actions up to its version");
        return;
      }
    }
  }
}

#[cfg(test)]
#[path = "fence_test.rs"]
mod fence_test;

#[cfg(test)]
#[path = "channel_test.rs"]
mod channel_test;
