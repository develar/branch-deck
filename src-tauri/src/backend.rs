//! The one backend of the user in the process of the app (docs/design/backend-protocol.md, "The socket";
//! openspec/specs/repository/backend-protocol/spec.md, "Socket"): the host of app-api for the bridges of the IDEs
//! (`branch-deck ide`) on the local socket of the key of this app (its data directory and its binary), and the single
//! instance of every platform.
//!
//! - **The claim** (`claim`), at the start of the setup, before the services and the main window: the process takes
//!   `backend.lock` of its key and listens; a lock another holds is a second start, which asks the backend to show its
//!   main window (`x-branch-deck/showMain`) and exits (a start `--background` of a bridge just exits: a backend serves);
//!   a holder that answers not (a backend that exits, or starts) is waited for a few seconds, then the process serves.
//! - **The serve** (`serve`): the host on the context of the app, its hooks the life of the process (`AppLife`): the main
//!   window it shows, whether a window is open, the exit of a stale backend without a window; the count of its clients is
//!   the count of the bridges the life reads (`BridgeConnections`).

use crate::life::AppLife;
use app_api::app::life::MainWindowHost;
use app_api::context::AppContext;
use backend_host::host::{Host, HostHooks};
use backend_host::identity::{Executable, build_id};
use backend_host::socket::{Listener, SocketError, app_key, dir_of, show_main};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::watch;
use tracing::{Instrument, info, warn};

/// What the start of the process is to the socket of its key
pub enum Claim {
  /// The backend of the key: it listens once its host exists
  Backend(Listener),
  /// Another backend serves the key: this process exits (a second start, which it asked to show its main window)
  Served,
  /// No socket (the directory refused, the lock never free): the app runs alone, and a bridge serves its IDE itself
  Alone,
}

/// The directory of the key of this app on its data directory `data_dir`
pub fn key_dir(data_dir: &Path) -> Option<PathBuf> {
  let executable = std::env::current_exe().ok()?;
  Some(dir_of(&app_key(data_dir, &executable)))
}

/// Claims the key of `dir` for this process, started `--background` or not
pub fn claim(dir: &Path, background: bool) -> Claim {
  tauri::async_runtime::block_on(async {
    match Listener::bind_waiting(dir, Duration::ZERO).await {
      Ok(listener) => return Claim::Backend(listener),
      Err(SocketError::Held { .. }) => {}
      Err(error) => {
        warn!(%error, "the app serves no socket: an IDE serves itself");
        return Claim::Alone;
      }
    }
    if background {
      info!("a backend serves the key already: the start for an IDE ends");
      return Claim::Served;
    }
    match show_main(dir, &build_id()).await {
      Ok(()) => {
        info!("the app runs already: it shows its main window");
        return Claim::Served;
      }
      Err(error) => info!(%error, "the backend of the key answers not: it exits or starts"),
    }
    match Listener::bind(dir).await {
      Ok(listener) => Claim::Backend(listener),
      Err(error) => {
        warn!(%error, "the app serves no socket: an IDE serves itself");
        Claim::Alone
      }
    }
  })
}

/// The host of the backend on `ctx`, its hooks the life of the process, and the count of its clients for that life; it
/// listens on `listener` when the process claimed its key
pub fn serve(app: &AppHandle, ctx: Arc<AppContext>, life: &Arc<AppLife>, listener: Option<Listener>, bridges: watch::Sender<usize>) -> Arc<Host> {
  let executable = Executable::current();
  let hooks = HostHooks {
    show_main: {
      let (app, life) = (app.clone(), Arc::clone(life));
      Box::new(move || {
        let life = Arc::clone(&life);
        if let Err(error) = app.run_on_main_thread(move || life.show_main()) {
          warn!(%error, "cannot show the main window");
        }
      })
    },
    stale: Box::new(move || executable.as_ref().is_some_and(Executable::is_stale)),
    has_window: {
      let life = Arc::clone(life);
      Box::new(move || life.has_window())
    },
    exit: {
      let app = app.clone();
      Box::new(move || {
        info!("the binary of the app was replaced and no window is open: the stale backend exits");
        app.exit(0);
      })
    },
    restart_to_update: Box::new(|| warn!("the binary of the app was replaced: restart the app to run the new build")),
  };
  let host = Host::new(ctx, hooks);
  let mut clients = host.clients();
  tauri::async_runtime::spawn(
    async move {
      loop {
        let count = *clients.borrow_and_update();
        if bridges.send(count).is_err() || clients.changed().await.is_err() {
          return;
        }
      }
    }
    .in_current_span(),
  );
  if let Some(listener) = listener {
    info!(dir = %listener.dir().display(), "the backend listens");
    tauri::async_runtime::spawn(listener.serve(Arc::clone(&host)).in_current_span());
  }
  app.manage(Arc::clone(&host));
  host
}
