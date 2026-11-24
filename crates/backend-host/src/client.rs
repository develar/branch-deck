//! What a client of the backend has of its own (docs/design/backend-protocol.md, "The Desktop of a connection",
//! "Events"): the events of app-api as `x-branch-deck/event` on its connection, and its desktop, made of its host
//! requests and its capabilities: an open of a window is `x-branch-deck/openWindow`, which the command answers once the
//! client answered, a show `x-branch-deck/showWindow`, `is_open` from `windowShown` and `windowClosed`; the dialog, the
//! menu, the restart, the updater and the agent CLI are the app's for a client that declares them, and a desktop without
//! them else (`app_api::connections::without`).

use crate::connection::Connection;
use crate::host::lock;
use crate::wire::ROOT;
use app_api::app::windows::{OpenSubWindowParams, Shown};
use app_api::connections::ConnectionHosts;
use app_api::connections::without::Without;
use app_api::transport::{AgentCliHost, AppEvents, Desktop, DialogHost, Later, MenuHost, RestartHost, Updater, WindowHost, WindowId, at_once};
use domain_types::error::ApiError;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};

/// What the desktop of the windows of a client has (`_meta["x-branch-deck"].capabilities` of its `initialize`): each
/// flag absent is false
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "the flags of the capabilities as initialize names them, each absent false")]
pub struct Capabilities {
  pub folder_dialog: bool,
  pub updater: bool,
  pub restart: bool,
  pub agent_cli: bool,
  pub native_menu: bool,
  /// The host requests the client answers: `openWindow`, `showWindow`
  pub host_requests: HashSet<String>,
}

impl Capabilities {
  pub fn of(value: &Value) -> Self {
    let flag = |name: &str| value[name].as_bool().unwrap_or(false);
    Self {
      folder_dialog: flag("folderDialog"),
      updater: flag("updater"),
      restart: flag("restart"),
      agent_cli: flag("agentCli"),
      native_menu: flag("nativeMenu"),
      host_requests: value["hostRequests"]
        .as_array()
        .map_or_else(HashSet::new, |requests| requests.iter().filter_map(Value::as_str).map(str::to_owned).collect()),
    }
  }
}

/// The windows a client shows (`windowShown` until `windowClosed`) and the label it gave each window the host opened,
/// by the window id of the open
#[derive(Default)]
pub struct ClientWindows {
  shown: Mutex<HashSet<String>>,
  opened: Mutex<HashMap<String, String>>,
}

impl ClientWindows {
  pub(crate) fn shown(&self, label: &str) {
    lock(&self.shown).insert(label.to_owned());
  }

  /// The window closed: true when the client showed it or the host opened it
  pub(crate) fn closed(&self, label: &str) -> bool {
    let shown = lock(&self.shown).remove(label);
    let mut opened = lock(&self.opened);
    let before = opened.len();
    opened.retain(|_, opened| opened != label);
    shown || opened.len() != before
  }

  fn opened(&self, window_id: &str, label: &str) {
    lock(&self.opened).insert(window_id.to_owned(), label.to_owned());
  }

  /// The label of the window of `label`, a window id of an open or a label of the client
  fn label_of(&self, label: &str) -> Option<String> {
    if let Some(opened) = lock(&self.opened).get(label) {
      return Some(opened.clone());
    }
    lock(&self.shown).contains(label).then(|| label.to_owned())
  }
}

/// The desktop and the events of the client of `connection`
pub(crate) fn client_hosts(connection: &Arc<Connection>, capabilities: &Capabilities, app: &Desktop) -> ConnectionHosts {
  let without = Arc::new(Without);
  let windows: Arc<dyn WindowHost> = if capabilities.host_requests.contains("openWindow") {
    Arc::new(HostWindows {
      connection: Arc::downgrade(connection),
      windows: Arc::clone(&connection.windows),
      shows: capabilities.host_requests.contains("showWindow"),
    })
  } else {
    Arc::clone(&without) as Arc<dyn WindowHost>
  };
  ConnectionHosts {
    desktop: Desktop {
      windows,
      dialog: if capabilities.folder_dialog {
        Arc::clone(&app.dialog)
      } else {
        Arc::clone(&without) as Arc<dyn DialogHost>
      },
      menu: if capabilities.native_menu {
        Arc::clone(&app.menu)
      } else {
        Arc::clone(&without) as Arc<dyn MenuHost>
      },
      restart: if capabilities.restart {
        Arc::clone(&app.restart)
      } else {
        Arc::clone(&without) as Arc<dyn RestartHost>
      },
      updater: if capabilities.updater {
        Arc::clone(&app.updater)
      } else {
        Arc::clone(&without) as Arc<dyn Updater>
      },
      agent_cli: if capabilities.agent_cli {
        Arc::clone(&app.agent_cli)
      } else {
        without as Arc<dyn AgentCliHost>
      },
    },
    events: Arc::new(ClientEvents {
      connection: Arc::downgrade(connection),
    }),
  }
}

/// The events of app-api on the connection of a client: `x-branch-deck/event`, to every window of the connection, or
/// with `window` to that one
struct ClientEvents {
  connection: Weak<Connection>,
}

impl AppEvents for ClientEvents {
  fn emit(&self, event: &str, payload: Value) {
    if let Some(connection) = self.connection.upgrade() {
      connection.notify("x-branch-deck/event", json!({ "channel": ROOT, "event": event, "payload": payload }));
    }
  }

  fn emit_to(&self, window: &WindowId, event: &str, payload: Value) {
    if let Some(connection) = self.connection.upgrade() {
      connection.notify(
        "x-branch-deck/event",
        json!({ "channel": ROOT, "event": event, "payload": payload, "window": window.as_str() }),
      );
    }
  }
}

/// The windows of a client by its host requests
struct HostWindows {
  connection: Weak<Connection>,
  windows: Arc<ClientWindows>,
  /// The client answers `showWindow`
  shows: bool,
}

impl HostWindows {
  /// `x-branch-deck/openWindow` of `params` for `opener`: answered once the client answered, with the label it gives
  /// the window
  fn open(&self, opener: &WindowId, params: &OpenSubWindowParams, update: Option<&str>, front: bool) -> Later<'static, Result<(), ApiError>> {
    let Some(connection) = self.connection.upgrade() else {
      return at_once(Err(ApiError::internal("The client of the window went away")));
    };
    let request = json!({ "channel": ROOT, "opener": opener.as_str(), "params": params, "update": update, "front": front });
    let answer = connection.request("x-branch-deck/openWindow", &request);
    let (windows, window_id) = (Arc::clone(&self.windows), params.window_id.clone());
    Box::pin(async move {
      match answer.await {
        Ok(Ok(result)) => {
          let label = result["window"].as_str().unwrap_or(&window_id).to_owned();
          windows.opened(&window_id, &label);
          Ok(())
        }
        Ok(Err(error)) => Err(ApiError::internal(format!(
          "The client opens no window: {}",
          error["message"].as_str().unwrap_or("no reason")
        ))),
        Err(_) => Err(ApiError::internal("The client of the window went away before it answered")),
      }
    })
  }
}

impl WindowHost for HostWindows {
  fn is_open(&self, label: &str) -> bool {
    self.windows.label_of(label).is_some()
  }

  fn reopen(&self, opener: &WindowId, params: &OpenSubWindowParams, update: Option<&str>, front: bool) -> Later<'_, Result<(), ApiError>> {
    self.open(opener, params, update, front)
  }

  fn build(&self, opener: &WindowId, params: &OpenSubWindowParams, _page_script: &str, _zoom: f64) -> Later<'_, Result<(), ApiError>> {
    self.open(opener, params, None, true)
  }

  /// `x-branch-deck/showWindow` of the window the client shows, if it shows one of the label
  fn show(&self, label: &str, _shown: Shown) -> Later<'_, ()> {
    let (Some(connection), Some(window), true) = (self.connection.upgrade(), self.windows.label_of(label), self.shows) else {
      return at_once(());
    };
    let answer = connection.request("x-branch-deck/showWindow", &json!({ "channel": ROOT, "window": window }));
    Box::pin(async move {
      let _ = answer.await;
    })
  }
}
