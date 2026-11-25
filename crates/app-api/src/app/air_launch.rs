//! Start in Air (openspec/specs/branch/start-in-air/spec.md): the agents of the IDE that has the repository open
//! (`air_launch_targets`), the launch of a session with the prompt of a robot (`air_launch`), and the watch of the
//! session it started (`air_watch_session`), over the local AHP socket of Air as a local app (`air_launch`). The
//! directory of the instance files is the host's (`AppHost::air_instances`): the user's in the app, none in the e2e
//! server, so no test reaches an IDE of the user. A refusal of the IDE and no IDE at all are answers the page words; a
//! connection that fails is an error.
//!
//! The watches of the sessions of one IDE share one connection (`IdeWatch`): the root `subscribe`, `listSessions`, then
//! the root notices. A watch follows the row whose `resource` is its local session id until the row is removed, the
//! connection ends, the page stops it (`stop_air_watch`), or the page of its window goes away
//! (`AirWatches::forget_window`): a Tauri channel of a page that went away never fails its send. The last watch of an
//! IDE that ends closes its connection.

use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::transport::{Call, Sink, WindowId};
use air_launch::client::{Client, ClientError, Stopper};
use air_launch::discovery::{Instance, find_instance};
use air_launch::protocol::{Activity, LaunchRequest, Refusal, RootChange, SessionSummary, Target};
use air_launch::watch::RootWatch;
use domain_types::error::ApiError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};

/// What `air_launch_targets` asks
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AirTargetsParams {
  /// The absolute path of the directory the session works in
  pub project: String,
}

/// The IDE that answered
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AirIde {
  /// The product code (`IU`)
  pub product: String,
  /// The build (`IU-263.1234`)
  pub build: String,
  /// The launch directory of the IDE that holds the directory (its host): the directory or one of its parents
  pub project: String,
}

/// The agents of the IDE of a directory
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AirTargetsAnswer {
  /// No running IDE with Air has the directory or a parent of it open
  NoIde,
  Listed {
    ide: AirIde,
    targets: Vec<Target>,
  },
  /// The code of the refusal (`DENIED`, `error.data.code`), and the English text of the IDE for a log
  Refused {
    ide: AirIde,
    code: String,
    message: Option<String>,
  },
}

/// How a launch went
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AirLaunchAnswer {
  NoIde,
  /// The session started; `local_session_id` is the key of its row, none for a draft
  #[serde(rename_all = "camelCase")]
  Accepted {
    ide: AirIde,
    local_session_id: Option<String>,
  },
  Refused {
    ide: AirIde,
    code: String,
    message: Option<String>,
  },
}

/// The IDE of `project` among the instance files of the host, asked for their hosts, with the client of its socket
fn connect(dir: Option<&Path>, project: &str) -> Option<(AirIde, Client)> {
  let (instance, base) = find_instance(dir?, Path::new(project))?;
  let Instance { product, build, socket, .. } = instance;
  let ide = AirIde {
    product,
    build,
    project: base.to_string_lossy().into_owned(),
  };
  Some((ide, Client::new(socket)))
}

fn exchange_error(error: &ClientError) -> ApiError {
  ApiError::internal(error.to_string())
}

/// The agents Air can start in the IDE that has `params.project` open, with their models and efforts. The IDE may ask
/// the user first, so the answer can take up to `air_launch::client::RESPONSE_TIMEOUT`
#[instrument(skip_all, fields(project = %params.project))]
pub async fn air_launch_targets(ctx: &AppContext, params: AirTargetsParams) -> Result<AirTargetsAnswer, ApiError> {
  let dir: Option<PathBuf> = ctx.host.air_instances();
  run_blocking(move || -> Result<AirTargetsAnswer, ApiError> {
    let Some((ide, client)) = connect(dir.as_deref(), &params.project) else {
      return Ok(AirTargetsAnswer::NoIde);
    };
    Ok(match client.targets(&params.project) {
      Ok(targets) => AirTargetsAnswer::Listed { ide, targets },
      Err(ClientError::Refused(Refusal { code, message, .. })) => AirTargetsAnswer::Refused { ide, code, message },
      Err(error) => return Err(exchange_error(&error)),
    })
  })
  .await
}

/// Starts a session in the IDE that has `params.project` open (the default of its New Session button without a target),
/// or opens a draft of it; the IDE may ask the user first
#[instrument(skip_all, fields(project = %params.project, target = ?params.launch_target_id, start = ?params.start))]
pub async fn air_launch(ctx: &AppContext, params: LaunchRequest) -> Result<AirLaunchAnswer, ApiError> {
  let dir: Option<PathBuf> = ctx.host.air_instances();
  run_blocking(move || -> Result<AirLaunchAnswer, ApiError> {
    let Some((ide, client)) = connect(dir.as_deref(), &params.project) else {
      return Ok(AirLaunchAnswer::NoIde);
    };
    Ok(match client.launch(&params) {
      Ok(started) => AirLaunchAnswer::Accepted {
        ide,
        local_session_id: started.map(|started| started.session.local_session_id),
      },
      Err(ClientError::Refused(Refusal { code, message, .. })) => AirLaunchAnswer::Refused { ide, code, message },
      Err(error) => return Err(exchange_error(&error)),
    })
  })
  .await
}

/// What `air_watch_session` asks
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AirWatchParams {
  /// The absolute path of the directory the session works in, as the launch named it
  pub project: String,
  /// The local session id, as the accepted launch answered it: the `resource` of the row of the session
  pub local_session_id: String,
}

/// The status of the watched session, now and after each change
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AirSessionStatus {
  /// The title of the session list, if the IDE names one
  pub title: Option<String>,
  pub activity: Activity,
}

/// How a watch ended
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AirWatchEnd {
  NoIde,
  /// The IDE refused the connection or the list of the rows (`DENIED`)
  Refused {
    code: String,
    message: Option<String>,
  },
  /// The row of the session is removed: the session is gone
  Gone,
  /// The IDE closed the connection
  Closed,
  /// The page stopped it, a newer watch of the session replaced it, or the page of its window went away
  Stopped,
}

/// How a watch ended, or why its connection failed
type WatchOutcome = Result<AirWatchEnd, String>;

/// A watch of one session
struct Watcher {
  ticket: u64,
  window: WindowId,
  session: String,
  statuses: Sink<AirSessionStatus>,
  /// The status the page heard last: a change of another field of the row tells it nothing
  last: Option<AirSessionStatus>,
  end: mpsc::Sender<WatchOutcome>,
}

impl Watcher {
  /// Tells the page `status` unless it heard it last; false when the page went away
  fn tell(&mut self, status: &AirSessionStatus) -> bool {
    if self.last.as_ref() == Some(status) {
      return true;
    }
    self.last = Some(status.clone());
    self.statuses.send(status.clone()).is_ok()
  }

  fn finish(self, end: WatchOutcome) {
    // the command that waits for it may be gone
    let _ = self.end.send(end);
  }
}

/// The rows of one IDE and the watches that follow them, over one connection
struct IdeWatch {
  socket: PathBuf,
  stopper: Stopper,
  state: Mutex<IdeState>,
}

#[derive(Default)]
struct IdeState {
  /// The status of each row by its local session id
  rows: HashMap<String, AirSessionStatus>,
  watchers: Vec<Watcher>,
  /// The connection ended or stops: a new watch opens another one
  ended: bool,
}

fn status_of(row: &SessionSummary) -> AirSessionStatus {
  AirSessionStatus {
    title: row.title.clone().filter(|title| !title.is_empty()),
    activity: row.activity(),
  }
}

impl IdeWatch {
  fn state(&self) -> MutexGuard<'_, IdeState> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// Takes `watcher`: tells it the status of its row if the IDE listed it
  fn join(&self, mut watcher: Watcher) {
    let mut state = self.state();
    if state.ended {
      drop(state);
      watcher.finish(Ok(AirWatchEnd::Closed));
      return;
    }
    if let Some(status) = state.rows.get(&watcher.session).cloned()
      && !watcher.tell(&status)
    {
      drop(state);
      watcher.finish(Ok(AirWatchEnd::Stopped));
      return;
    }
    state.watchers.push(watcher);
  }

  /// Ends each watch that `ends` names with `end`; once none is left, the connection closes
  fn end_where(&self, ends: impl Fn(&Watcher) -> bool, end: &AirWatchEnd) {
    let mut state = self.state();
    let (ended, kept) = std::mem::take(&mut state.watchers).into_iter().partition::<Vec<_>, _>(|it| ends(it));
    state.watchers = kept;
    let idle = state.watchers.is_empty() && !ended.is_empty();
    if idle {
      state.ended = true;
      self.stopper.stop();
    }
    drop(state);
    for watcher in ended {
      watcher.finish(Ok(end.clone()));
    }
  }

  /// The rows of `listSessions`, and the status of each watched row
  fn listed(&self, rows: &[SessionSummary]) {
    let mut state = self.state();
    for row in rows {
      state.rows.insert(row.resource.clone(), status_of(row));
    }
    let sessions: Vec<String> = state.watchers.iter().map(|it| it.session.clone()).collect();
    drop(state);
    for session in sessions {
      self.tell(&session);
    }
  }

  /// Tells each watch of `session` the status of its row; a watch whose page went away ends
  fn tell(&self, session: &str) {
    let mut state = self.state();
    let Some(status) = state.rows.get(session).cloned() else {
      return;
    };
    let mut gone = Vec::new();
    for watcher in &mut state.watchers {
      if watcher.session == session && !watcher.tell(&status) {
        debug!(session, "the page of the watch went away");
        gone.push(watcher.ticket);
      }
    }
    drop(state);
    if !gone.is_empty() {
      self.end_where(|it| gone.contains(&it.ticket), &AirWatchEnd::Stopped);
    }
  }

  fn apply(&self, change: RootChange) {
    match change {
      RootChange::Added(row) => {
        self.state().rows.insert(row.resource.clone(), status_of(&row));
        self.tell(&row.resource);
      }
      RootChange::Changed { session, changes } => {
        let mut state = self.state();
        let Some(status) = state.rows.get_mut(&session) else {
          return;
        };
        if let Some(title) = changes.title.clone() {
          status.title = Some(title).filter(|title| !title.is_empty());
        }
        if let Some(activity) = changes.activity() {
          status.activity = activity;
        }
        drop(state);
        self.tell(&session);
      }
      RootChange::Removed { session } => {
        self.state().rows.remove(&session);
        self.end_where(|it| it.session == session, &AirWatchEnd::Gone);
      }
    }
  }

  /// The connection ended with `end`: each watch ends with it
  fn close(&self, end: &WatchOutcome) {
    let mut state = self.state();
    state.ended = true;
    let watchers = std::mem::take(&mut state.watchers);
    drop(state);
    for watcher in watchers {
      watcher.finish(end.clone());
    }
  }
}

/// The watches of the sessions, one connection per IDE (by the socket of its instance file): a newer watch of the same
/// session of a window stops the one before
#[derive(Default)]
pub struct AirWatches {
  last_ticket: AtomicU64,
  ides: Mutex<HashMap<PathBuf, Arc<IdeWatch>>>,
}

impl AirWatches {
  fn ides(&self) -> MutexGuard<'_, HashMap<PathBuf, Arc<IdeWatch>>> {
    self.ides.lock().unwrap_or_else(PoisonError::into_inner)
  }

  fn every_ide(&self) -> Vec<Arc<IdeWatch>> {
    self.ides().values().cloned().collect()
  }

  /// Watches the session `session` of the IDE of `client` for the page of `window`: the connection of the IDE, opened
  /// now if none runs. The receiver gets how the watch ended
  fn watch(self: &Arc<Self>, client: &Client, window: &WindowId, session: &str, statuses: Sink<AirSessionStatus>) -> Result<mpsc::Receiver<WatchOutcome>, ClientError> {
    let ide = self.ide_of(client)?;
    let ticket = self.last_ticket.fetch_add(1, Ordering::SeqCst) + 1;
    let (end, ended) = mpsc::channel();
    ide.join(Watcher {
      ticket,
      window: window.clone(),
      session: session.to_owned(),
      statuses,
      last: None,
      end,
    });
    // after the join: the connection of the watch before stays for this one
    for ide in self.every_ide() {
      ide.end_where(|it| it.window == *window && it.session == session && it.ticket != ticket, &AirWatchEnd::Stopped);
    }
    Ok(ended)
  }

  /// The running connection of the IDE of `client`, else a new one, whose rows a thread of its own follows
  fn ide_of(self: &Arc<Self>, client: &Client) -> Result<Arc<IdeWatch>, ClientError> {
    let socket = client.socket().to_path_buf();
    let running = |ides: &HashMap<PathBuf, Arc<IdeWatch>>| ides.get(&socket).filter(|ide| !ide.state().ended).cloned();
    let found = running(&self.ides());
    if let Some(ide) = found {
      return Ok(ide);
    }
    // the connection opens outside the lock: the greeting of an IDE that hangs stops no other watch
    let watch = client.watch()?;
    let ide = Arc::new(IdeWatch {
      socket: socket.clone(),
      stopper: watch.stopper()?,
      state: Mutex::default(),
    });
    let mut ides = self.ides();
    if let Some(other) = running(&ides) {
      // another watch opened one meanwhile
      ide.stopper.stop();
      return Ok(other);
    }
    ides.insert(socket, Arc::clone(&ide));
    drop(ides);
    let watches = Arc::clone(self);
    let follower = Arc::clone(&ide);
    std::thread::spawn(in_current_span(move || watches.follow(watch, &follower)));
    Ok(ide)
  }

  /// Lists the rows of `ide`, then applies each change until the connection ends, and ends each watch of it
  fn follow(&self, mut watch: RootWatch, ide: &Arc<IdeWatch>) {
    let end = match watch.list_sessions() {
      Ok(rows) => {
        ide.listed(&rows);
        follow_changes(watch, ide)
      }
      Err(ClientError::Refused(Refusal { code, message, .. })) => Ok(AirWatchEnd::Refused { code, message }),
      Err(error) => Err(error.to_string()),
    };
    debug!(socket = %ide.socket.display(), ?end, "the connection of the rows ended");
    ide.close(&end);
    let mut ides = self.ides();
    if ides.get(&ide.socket).is_some_and(|it| Arc::ptr_eq(it, ide)) {
      ides.remove(&ide.socket);
    }
  }

  /// Stops the watch of `session` that `window` runs, if any
  fn stop(&self, window: &WindowId, session: &str) {
    for ide in self.every_ide() {
      ide.end_where(|it| it.window == *window && it.session == session, &AirWatchEnd::Stopped);
    }
  }

  /// The page of `window` went away: its watches stop
  pub fn forget_window(&self, window: &WindowId) {
    for ide in self.every_ide() {
      ide.end_where(|it| it.window == *window, &AirWatchEnd::Stopped);
    }
  }
}

/// Applies each change of the rows to `ide` until the connection ends
fn follow_changes(watch: RootWatch, ide: &IdeWatch) -> WatchOutcome {
  for change in watch {
    match change {
      Ok(change) => ide.apply(change),
      // the read of a connection that its last watch stopped may fail rather than end
      Err(_) if ide.state().ended => return Ok(AirWatchEnd::Stopped),
      Err(error) => return Err(error.to_string()),
    }
  }
  Ok(if ide.state().ended { AirWatchEnd::Stopped } else { AirWatchEnd::Closed })
}

/// Watches the session `params.local_session_id` in the IDE that has `params.project` open: each status of its row to
/// `statuses`, now and after each change, until the row is removed, the IDE closes the connection, the page stops it
/// (`stop_air_watch`) or its page goes away. No timeout
#[instrument(skip_all, fields(window = %call.window, session = %params.local_session_id))]
pub async fn air_watch_session(ctx: &AppContext, call: &Call, params: AirWatchParams, statuses: Sink<AirSessionStatus>) -> Result<AirWatchEnd, ApiError> {
  let dir: Option<PathBuf> = ctx.host.air_instances();
  let watches = Arc::clone(&ctx.air_watches);
  let window = call.window.clone();
  run_blocking(move || -> Result<AirWatchEnd, ApiError> {
    let Some((_, client)) = connect(dir.as_deref(), &params.project) else {
      return Ok(AirWatchEnd::NoIde);
    };
    let ended = match watches.watch(&client, &window, &params.local_session_id, statuses) {
      Ok(ended) => ended,
      Err(ClientError::Refused(Refusal { code, message, .. })) => return Ok(AirWatchEnd::Refused { code, message }),
      Err(error) => return Err(exchange_error(&error)),
    };
    // a watch whose connection went away without a word ended with it
    ended.recv().unwrap_or(Ok(AirWatchEnd::Closed)).map_err(ApiError::internal)
  })
  .await
}

/// Stops the watch of the session `local_session_id` that the calling window runs, if any: it answers `stopped`
#[instrument(skip_all, fields(window = %call.window, session = %local_session_id))]
#[expect(clippy::needless_pass_by_value, reason = "the registry hands a body its arguments by value")]
pub fn stop_air_watch(ctx: &AppContext, call: &Call, local_session_id: String) {
  ctx.air_watches.stop(&call.window, &local_session_id);
}

#[cfg(test)]
#[path = "air_launch_test.rs"]
mod air_launch_test;
