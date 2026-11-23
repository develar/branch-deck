//! The host (docs/design/backend-protocol.md, "Architecture"): one per backend, over the `AppContext` of the process.
//! It keeps the connections, one global `serverSeq` with its publish lock, a channel for each repository path, and the
//! build of the process with the hooks the app gives it ([`HostHooks`]: the main window, the rule of another build);
//! the news claimed so far are of app-api (`Connections::claim_news`), which a page claims too.
//!
//! Every frame that carries a `serverSeq` (an action of a channel, an echo, a resync) is queued under the publish lock
//! (`publish`), so the frames of a connection are in the order of their `serverSeq`; a frame is encoded before it.

use crate::channel::Channel;
use crate::connection::{Connection, Kind};
use crate::identity::build_id;
use app_api::context::AppContext;
use domain_types::session::NewsClaim;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use tokio::sync::{Notify, watch};

/// What the app gives its host: what only the app can do, and what tells whether its executable is stale
pub struct HostHooks {
  /// `x-branch-deck/showMain`: builds the main window, or shows it (a second start of the app, a backend started
  /// `--background`)
  pub show_main: Box<dyn Fn() + Send + Sync>,
  /// The executable of the process is no longer its build on disk (`Executable::is_stale`)
  pub stale: Box<dyn Fn() -> bool + Send + Sync>,
  /// The app shows a window now
  pub has_window: Box<dyn Fn() -> bool + Send + Sync>,
  /// A stale backend without a window exits, after its answer of `otherBuild` is written
  pub exit: Box<dyn Fn() + Send + Sync>,
  /// A stale backend with a window stays, and tells "Restart to Update"
  pub restart_to_update: Box<dyn Fn() + Send + Sync>,
}

impl Default for HostHooks {
  /// The hooks of a host of the tests: never stale, a window shown, no main window to build
  fn default() -> Self {
    Self {
      show_main: Box::new(|| {}),
      stale: Box::new(|| false),
      has_window: Box::new(|| true),
      exit: Box::new(|| {}),
      restart_to_update: Box::new(|| {}),
    }
  }
}

/// The backend of the process for every client
pub struct Host {
  pub(crate) ctx: Arc<AppContext>,
  pub(crate) build: String,
  pub(crate) hooks: HostHooks,
  /// The last `serverSeq`; held to queue a frame that carries one
  publish: Mutex<u64>,
  /// Wakes the answers that wait for a version (the fence) after each queued action
  pub(crate) published: Notify,
  channels: Mutex<Channels>,
  connections: Mutex<BTreeMap<u64, Weak<Connection>>>,
  last_connection: AtomicU64,
  /// The clients connected now that initialized, the app's own connection left out: what keeps a backend without a
  /// window alive (`app_api::app::life`)
  clients: watch::Sender<usize>,
  me: Weak<Self>,
}

#[derive(Default)]
struct Channels {
  by_path: HashMap<String, Arc<Channel>>,
  by_id: BTreeMap<u64, Arc<Channel>>,
}

/// The path of a repository as a window names it, its trailing separators left out
pub fn repository_path(path: &str) -> &str {
  let trimmed = path.trim_end_matches(['/', '\\']);
  if trimmed.is_empty() { path } else { trimmed }
}

impl Host {
  /// The host of `ctx`, of the build of this binary
  pub fn new(ctx: Arc<AppContext>, hooks: HostHooks) -> Arc<Self> {
    Self::of_build(ctx, build_id(), hooks)
  }

  /// The host of `ctx` with the build id `build`
  pub fn of_build(ctx: Arc<AppContext>, build: String, hooks: HostHooks) -> Arc<Self> {
    Arc::new_cyclic(|me| Self {
      ctx,
      build,
      hooks,
      publish: Mutex::new(0),
      published: Notify::new(),
      channels: Mutex::default(),
      connections: Mutex::default(),
      last_connection: AtomicU64::new(0),
      clients: watch::Sender::new(0),
      me: Weak::clone(me),
    })
  }

  pub const fn context(&self) -> &Arc<AppContext> {
    &self.ctx
  }

  /// The build id of the host
  pub fn build(&self) -> &str {
    &self.build
  }

  pub(crate) fn me(&self) -> Arc<Self> {
    self.me.upgrade().expect("a host lives while it serves")
  }

  /// A new connection of a client: its windows, its events and its desktop its own once it initialized
  pub fn connect(&self) -> Arc<Connection> {
    self.open(Kind::Client)
  }

  /// The connection of the app's own windows (the Tauri shell in the process): its desktop and its events are the
  /// app's (`ConnectionId::APP`)
  pub fn connect_app(&self) -> Arc<Connection> {
    self.open(Kind::App)
  }

  fn open(&self, kind: Kind) -> Arc<Connection> {
    let key = self.last_connection.fetch_add(1, Ordering::SeqCst) + 1;
    let connection = Connection::new(self.me(), key, kind);
    lock(&self.connections).insert(key, Arc::downgrade(&connection));
    connection
  }

  /// The connection `key` ended
  pub(crate) fn forget_connection(&self, key: u64) {
    lock(&self.connections).remove(&key);
  }

  /// A client initialized (`up`), or one that initialized ended
  pub(crate) fn count_client(&self, up: bool) {
    self.clients.send_modify(|count| *count = if up { *count + 1 } else { count.saturating_sub(1) });
  }

  /// The count of the clients connected now that initialized, the app's own connection left out, and its changes: the
  /// bridges of the IDEs, a second start of the app while it asks `showMain`
  pub fn clients(&self) -> watch::Receiver<usize> {
    self.clients.subscribe()
  }

  /// The connections open now that initialized
  pub fn connection_count(&self) -> usize {
    lock(&self.connections)
      .values()
      .filter_map(Weak::upgrade)
      .filter(|connection| connection.id().is_some())
      .count()
  }

  /// The channel of the repository `path` (as its window names it): the same for the life of the host
  pub(crate) fn channel_of_path(&self, path: &str) -> Arc<Channel> {
    let path = repository_path(path);
    let mut channels = lock(&self.channels);
    if let Some(channel) = channels.by_path.get(path) {
      return Arc::clone(channel);
    }
    let id = channels.by_id.len() as u64 + 1;
    let channel = Arc::new(Channel::new(id, path.to_owned()));
    channels.by_path.insert(path.to_owned(), Arc::clone(&channel));
    channels.by_id.insert(id, Arc::clone(&channel));
    channel
  }

  /// The channel of `uri`, none for a URI no channel has
  pub(crate) fn channel_of_uri(&self, uri: &str) -> Option<Arc<Channel>> {
    let id: u64 = uri.strip_prefix(crate::channel::URI_PREFIX)?.parse().ok()?;
    lock(&self.channels).by_id.get(&id).cloned()
  }

  /// The channel of the repository `path` if a window asked for it
  pub(crate) fn existing_channel(&self, path: &str) -> Option<Arc<Channel>> {
    lock(&self.channels).by_path.get(repository_path(path)).cloned()
  }

  /// Every channel
  pub(crate) fn channels(&self) -> Vec<Arc<Channel>> {
    lock(&self.channels).by_id.values().cloned().collect()
  }

  /// Runs `queue` under the publish lock with the `serverSeq` now: what it queues is in the order of the sequence
  pub(crate) fn publish<T>(&self, queue: impl FnOnce(&mut Sequence) -> T) -> T {
    let queued = {
      let mut last = lock(&self.publish);
      let mut sequence = Sequence { last: &mut last };
      queue(&mut sequence)
    };
    self.published.notify_waiters();
    queued
  }

  /// The `serverSeq` now
  pub fn server_seq(&self) -> u64 {
    *lock(&self.publish)
  }

  /// The first claim of `news` of the repository `path`: true once, false for every later claim, of this connection or
  /// another, also of a page's `claim_news` (`Connections::claim_news`)
  pub fn claim_news(&self, path: &str, news: NewsClaim) -> bool {
    self.ctx.connections.claim_news(path, news)
  }
}

/// The `serverSeq` within the publish lock
pub struct Sequence<'a> {
  last: &'a mut u64,
}

impl Sequence<'_> {
  /// The `serverSeq` now, of the last frame queued
  pub const fn now(&self) -> u64 {
    *self.last
  }

  /// The `serverSeq` of a new frame
  pub const fn next(&mut self) -> u64 {
    *self.last += 1;
    *self.last
  }
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
  mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
