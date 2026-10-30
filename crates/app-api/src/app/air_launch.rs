//! Start in Air (spec/branch/start-in-air.spec.md): the agents of the IDE that has the repository open
//! (`air_launch_targets`), the launch of a session with the prompt of a robot (`air_launch`), and the watch of the
//! session it started (`air_watch_session`), over the local launch socket of Air (`air_launch`, protocol v1). The
//! directory of the instance files is the host's (`AppHost::air_instances`): the user's in the app, none in the e2e
//! server, so no test reaches an IDE of the user. A refusal of the IDE and no IDE at all are answers the page words; a
//! connection that fails is an error.
//!
//! A watch holds its connection until the IDE ends it, the page stops it (`stop_air_watch`), or the page of its window
//! goes away (`AirWatches::forget_window`): a Tauri channel of a page that went away never fails its send.

use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::transport::{Call, EventSink, Sink, WindowId};
use air_launch::client::{Client, ClientError, WatchStopper};
use air_launch::discovery::{Instance, find_instance};
use air_launch::protocol::{Activity, Launch, LaunchRequest, Refusal, Session, Target, Targets, WatchLine};
use domain_types::error::ApiError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
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
  /// The base directory of its project that holds the directory
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
  /// The code of the protocol (`DENIED`), and its English text for a log
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
  /// The session started; `session_id` is the id of its provider, none for a draft
  #[serde(rename_all = "camelCase")]
  Accepted {
    ide: AirIde,
    session_id: Option<String>,
  },
  Refused {
    ide: AirIde,
    code: String,
    message: Option<String>,
  },
}

/// The IDE of `project` among the instance files of the host, with the client of its socket
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
    Ok(match client.targets(&params.project).map_err(|error| exchange_error(&error))? {
      Targets::Listed { targets } => AirTargetsAnswer::Listed { ide, targets },
      Targets::Refused(Refusal { code, message }) => AirTargetsAnswer::Refused {
        ide,
        code: code.as_str().to_owned(),
        message,
      },
    })
  })
  .await
}

/// Starts a session in the IDE that has `params.project` open, or opens a draft of it; the IDE may ask the user first
#[instrument(skip_all, fields(project = %params.project, target = %params.launch_target_id, start = ?params.start))]
pub async fn air_launch(ctx: &AppContext, params: LaunchRequest) -> Result<AirLaunchAnswer, ApiError> {
  let dir: Option<PathBuf> = ctx.host.air_instances();
  run_blocking(move || -> Result<AirLaunchAnswer, ApiError> {
    let Some((ide, client)) = connect(dir.as_deref(), &params.project) else {
      return Ok(AirLaunchAnswer::NoIde);
    };
    Ok(match client.launch(&params).map_err(|error| exchange_error(&error))? {
      Launch::Accepted { session_id } => AirLaunchAnswer::Accepted { ide, session_id },
      Launch::Refused(Refusal { code, message }) => AirLaunchAnswer::Refused {
        ide,
        code: code.as_str().to_owned(),
        message,
      },
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
  /// The id of the session, as the accepted launch answered it
  pub session_id: String,
}

/// The status of the watched session, now and after each change
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AirSessionStatus {
  /// The title of the session list, if the IDE names one
  pub title: Option<String>,
  pub activity: Activity,
}

/// How a watch ended
#[derive(Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AirWatchEnd {
  NoIde,
  /// Before the first status any code of the protocol (`BUSY`); after it `NO_SESSION`: the session is gone
  Refused {
    code: String,
    message: Option<String>,
  },
  /// The IDE ended the watch: `timeout` after 4 h
  Ended {
    reason: String,
  },
  /// The IDE closed the connection without an end
  Closed,
  /// The page stopped it, or the page of its window went away
  Stopped,
}

/// A watch that runs: its connection and whether the page stopped it
struct RunningWatch {
  ticket: u64,
  stopper: WatchStopper,
  stopped: AtomicBool,
}

impl RunningWatch {
  fn stop(&self) {
    self.stopped.store(true, Ordering::SeqCst);
    self.stopper.stop();
  }
}

/// The watches that run, by the window that asked and the session: a newer watch of the same session of a window stops
/// the one before
#[derive(Default)]
pub struct AirWatches {
  last_ticket: AtomicU64,
  running: Mutex<HashMap<(WindowId, String), Arc<RunningWatch>>>,
}

impl AirWatches {
  fn running(&self) -> MutexGuard<'_, HashMap<(WindowId, String), Arc<RunningWatch>>> {
    self.running.lock().unwrap_or_else(PoisonError::into_inner)
  }

  fn start(&self, window: &WindowId, session_id: &str, stopper: WatchStopper) -> Arc<RunningWatch> {
    let watch = Arc::new(RunningWatch {
      ticket: self.last_ticket.fetch_add(1, Ordering::SeqCst) + 1,
      stopper,
      stopped: AtomicBool::new(false),
    });
    let before = self.running().insert((window.clone(), session_id.to_owned()), Arc::clone(&watch));
    if let Some(before) = before {
      before.stop();
    }
    watch
  }

  fn end(&self, window: &WindowId, session_id: &str, ticket: u64) {
    let mut running = self.running();
    let key = (window.clone(), session_id.to_owned());
    if running.get(&key).is_some_and(|watch| watch.ticket == ticket) {
      running.remove(&key);
    }
  }

  fn stop(&self, window: &WindowId, session_id: String) {
    let watch = self.running().remove(&(window.clone(), session_id));
    if let Some(watch) = watch {
      watch.stop();
    }
  }

  /// The page of `window` went away: its watches stop
  pub fn forget_window(&self, window: &WindowId) {
    let mut running = self.running();
    running.retain(|(of, _), watch| {
      let keep = of != window;
      if !keep {
        watch.stop();
      }
      keep
    });
  }
}

/// Watches the session `params.session_id` in the IDE that has `params.project` open: each status to `statuses`, now
/// and after each change, until the IDE ends the watch, the page stops it (`stop_air_watch`) or its page goes away. No
/// timeout: the IDE ends a watch after 4 h
#[instrument(skip_all, fields(window = %call.window, session_id = %params.session_id))]
pub async fn air_watch_session(ctx: &AppContext, call: &Call, params: AirWatchParams, statuses: Sink<AirSessionStatus>) -> Result<AirWatchEnd, ApiError> {
  let dir: Option<PathBuf> = ctx.host.air_instances();
  let watches = Arc::clone(&ctx.air_watches);
  let window = call.window.clone();
  run_blocking(move || -> Result<AirWatchEnd, ApiError> {
    let Some((_, client)) = connect(dir.as_deref(), &params.project) else {
      return Ok(AirWatchEnd::NoIde);
    };
    let watch = client.watch(&params.project, &params.session_id).map_err(|error| exchange_error(&error))?;
    let running = watches.start(&window, &params.session_id, watch.stopper().map_err(|error| exchange_error(&error))?);
    let end = follow(watch, &|| running.stopped.load(Ordering::SeqCst), &*statuses);
    // the page that went away ends the watch here; a closed connection takes it as it is
    running.stopper.stop();
    watches.end(&window, &params.session_id, running.ticket);
    end
  })
  .await
}

/// Sends each status of `lines` to `statuses` until the end of the watch; `is_stopped`: the page stopped it
pub(crate) fn follow(
  lines: impl Iterator<Item = Result<WatchLine, ClientError>>,
  is_stopped: &dyn Fn() -> bool,
  statuses: &dyn EventSink<AirSessionStatus>,
) -> Result<AirWatchEnd, ApiError> {
  for line in lines {
    match line {
      Ok(WatchLine::Session {
        session: Session { title, activity, .. },
      }) => {
        if statuses.send(AirSessionStatus { title, activity }).is_err() {
          debug!("the page of the watch went away");
          return Ok(AirWatchEnd::Stopped);
        }
      }
      Ok(WatchLine::End { reason }) => return Ok(AirWatchEnd::Ended { reason }),
      Ok(WatchLine::Refused(Refusal { code, message })) => {
        return Ok(AirWatchEnd::Refused {
          code: code.as_str().to_owned(),
          message,
        });
      }
      // the read of a connection the page stopped may fail rather than end
      Err(_) if is_stopped() => return Ok(AirWatchEnd::Stopped),
      Err(error) => return Err(exchange_error(&error)),
    }
  }
  Ok(if is_stopped() { AirWatchEnd::Stopped } else { AirWatchEnd::Closed })
}

/// Stops the watch of the session `session_id` that the calling window runs, if any: it answers `stopped`
#[instrument(skip_all, fields(window = %call.window, session_id = %session_id))]
pub fn stop_air_watch(ctx: &AppContext, call: &Call, session_id: String) {
  ctx.air_watches.stop(&call.window, session_id);
}

#[cfg(test)]
#[path = "air_launch_test.rs"]
mod air_launch_test;
