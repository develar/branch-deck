//! The connections of the backend (docs/design/backend-protocol.md, "The windows of a connection", "Events", "The
//! Desktop of a connection"): the app's own (`ConnectionId::APP`), whose desktop and events the host built the context
//! with, and each client `backend-host` opened, with the desktop of its host requests and capabilities and the events
//! it sends on its connection. An event of app-api goes to every connection ([`Connections`] is the `AppEvents` of the
//! context), an event to a window to the connection of that window, an event to the main window to the main window of
//! each; a desktop is the one of the connection of the window that asks (`AppContext::desktop`). A news of a
//! repository is told once by the backend: every client claims it before it tells it (`claim_news`), the first claim
//! wins.
//!
//! [`without`] is the desktop of a connection that lacks a part (a capability its client did not declare, a connection
//! that ended): each part answers as a desktop without it.

use crate::transport::{AppEvents, ConnectionId, Desktop, WindowId};
use domain_types::session::NewsClaim;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// What a client of the backend has of its own: the desktop of its windows and the events it hears
#[derive(Clone)]
pub struct ConnectionHosts {
  pub desktop: Desktop,
  pub events: Arc<dyn AppEvents>,
}

/// The connections of a context: the events of the app's own, and the clients open now, by their ids
pub struct Connections {
  app: Arc<dyn AppEvents>,
  last: AtomicU64,
  open: Mutex<BTreeMap<ConnectionId, ConnectionHosts>>,
  /// The news of each repository claimed so far, by the path of the repository without a trailing separator
  claimed: Mutex<HashSet<(String, NewsClaim)>>,
}

impl Connections {
  /// The connections of a context whose own connection hears `app`
  pub fn new(app: Arc<dyn AppEvents>) -> Self {
    Self {
      app,
      last: AtomicU64::new(ConnectionId::APP.0),
      open: Mutex::default(),
      claimed: Mutex::default(),
    }
  }

  /// The first claim of `news` of the repository `repository_path` (as a window names it): true once, false for every
  /// later claim of any connection, so the app and the IDE tell one news once
  pub fn claim_news(&self, repository_path: &str, news: NewsClaim) -> bool {
    let trimmed = repository_path.trim_end_matches(['/', '\\']);
    let path = if trimmed.is_empty() { repository_path } else { trimmed };
    self.claimed.lock().unwrap_or_else(PoisonError::into_inner).insert((path.to_owned(), news))
  }

  fn open_now(&self) -> MutexGuard<'_, BTreeMap<ConnectionId, ConnectionHosts>> {
    self.open.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// A new connection with `hosts`: its id, never one of another connection of the context
  pub fn open(&self, hosts: ConnectionHosts) -> ConnectionId {
    let id = ConnectionId(self.last.fetch_add(1, Ordering::SeqCst) + 1);
    self.open_now().insert(id, hosts);
    id
  }

  /// The hosts of the open connection `id`; none for the app's own, whose hosts are of the context, and for one that
  /// ended
  pub fn hosts(&self, id: ConnectionId) -> Option<ConnectionHosts> {
    self.open_now().get(&id).cloned()
  }

  /// The connection `id` ended: nothing reaches it any more (`AppContext::close_connection` forgets its windows)
  pub(crate) fn close(&self, id: ConnectionId) {
    self.open_now().remove(&id);
  }

  fn each(&self) -> Vec<(ConnectionId, Arc<dyn AppEvents>)> {
    self.open_now().iter().map(|(id, hosts)| (*id, Arc::clone(&hosts.events))).collect()
  }
}

/// An event to every connection, an event to a window to its connection alone, an event to the main window to the main
/// window of each connection
impl AppEvents for Connections {
  fn emit(&self, event: &str, payload: serde_json::Value) {
    for (_, events) in self.each() {
      events.emit(event, payload.clone());
    }
    self.app.emit(event, payload);
  }

  fn emit_to(&self, window: &WindowId, event: &str, payload: serde_json::Value) {
    if window.connection() == ConnectionId::APP {
      self.app.emit_to(window, event, payload);
    } else if let Some(hosts) = self.hosts(window.connection()) {
      hosts.events.emit_to(window, event, payload);
    }
  }

  fn emit_to_main(&self, event: &str, payload: serde_json::Value) {
    for (id, events) in self.each() {
      events.emit_to(&WindowId::main_of(id), event, payload.clone());
    }
    self.app.emit_to_main(event, payload);
  }
}

pub mod without {
  //! The parts of a desktop that a connection lacks: no window opens, no dialog, no menu, no restart, no updater, no
  //! agent CLI. Each answers as a desktop without it (docs/design/backend-protocol.md, "The Desktop of a connection"):
  //! nothing, or the `ApiError` of the part it lacks

  use crate::app::agent_cli::{AgentCliCommand, AgentCliInstalled, AgentCliStatus, IdeApp};
  use crate::app::windows::{OpenSubWindowParams, Shown};
  use crate::transport::{AgentCliHost, Desktop, DialogHost, Later, MenuHost, RestartHost, Updater, WindowHost, WindowId, at_once};
  use domain_types::error::ApiError;
  use domain_types::update::UpdateInfo;
  use std::sync::Arc;

  /// A desktop without any part
  pub fn desktop() -> Desktop {
    let without = Arc::new(Without);
    Desktop {
      windows: Arc::clone(&without) as Arc<dyn WindowHost>,
      dialog: Arc::clone(&without) as Arc<dyn DialogHost>,
      menu: Arc::clone(&without) as Arc<dyn MenuHost>,
      restart: Arc::clone(&without) as Arc<dyn RestartHost>,
      updater: Arc::clone(&without) as Arc<dyn Updater>,
      agent_cli: without,
    }
  }

  /// The part a desktop lacks
  pub struct Without;

  fn lacks(part: &str) -> ApiError {
    ApiError::internal(format!("The desktop of this window has no {part}: the desktop app has it"))
  }

  impl WindowHost for Without {
    fn is_open(&self, _label: &str) -> bool {
      false
    }

    fn reopen(&self, _opener: &WindowId, _params: &OpenSubWindowParams, _update: Option<&str>, _front: bool) -> Later<'_, Result<(), ApiError>> {
      at_once(Err(lacks("windows")))
    }

    fn build(&self, _opener: &WindowId, _params: &OpenSubWindowParams, _page_script: &str, _zoom: f64) -> Later<'_, Result<(), ApiError>> {
      at_once(Err(lacks("windows")))
    }

    fn show(&self, _label: &str, _shown: Shown) -> Later<'_, ()> {
      at_once(())
    }
  }

  impl DialogHost for Without {
    fn pick_folder(&self, _title: &str) -> Option<String> {
      None
    }

    fn has_folder_dialog(&self) -> bool {
      false
    }
  }

  impl MenuHost for Without {
    fn set_auto_sync_checked(&self, _checked: bool) -> Result<(), ApiError> {
      Ok(())
    }

    fn set_unattended_checked(&self, _checked: bool) -> Result<(), ApiError> {
      Ok(())
    }

    fn show_window_menu(&self, _window: &WindowId) -> Result<(), ApiError> {
      Ok(())
    }
  }

  impl RestartHost for Without {
    fn restart(&self) -> Result<(), ApiError> {
      Err(lacks("restart"))
    }
  }

  impl Updater for Without {
    fn check(&self) -> Later<'_, Result<UpdateInfo, ApiError>> {
      at_once(Err(lacks("updater")))
    }

    fn check_failure(&self) -> Option<String> {
      None
    }

    fn status(&self) -> Result<UpdateInfo, ApiError> {
      Err(lacks("updater"))
    }

    fn install(&self) -> Later<'_, Result<(), ApiError>> {
      at_once(Err(lacks("updater")))
    }

    fn downloaded(&self) -> Option<String> {
      None
    }
  }

  impl AgentCliHost for Without {
    fn install(&self) -> Result<AgentCliInstalled, ApiError> {
      Err(lacks("agent CLI"))
    }

    fn command(&self, _repository_path: &str) -> Result<AgentCliCommand, ApiError> {
      Err(lacks("agent CLI"))
    }

    fn status(&self) -> AgentCliStatus {
      AgentCliStatus::new(None, Vec::new(), None, false)
    }

    fn install_ide_app(&self) -> Result<IdeApp, ApiError> {
      Err(lacks("agent CLI"))
    }

    fn remove_ide_app(&self) -> Result<(), ApiError> {
      Err(lacks("agent CLI"))
    }
  }
}
