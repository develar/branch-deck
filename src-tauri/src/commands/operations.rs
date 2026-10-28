//! The operations of the remote as the page asks them (docs/design/rust-domain-authority.md, "Планирование и
//! исполнение операций"): `prepare` answers the plan of an intent in the session of the repository, which the popover
//! of a start renders and a click starts by (`operations::engine::prepare`); `execute` runs it on the plan the page
//! showed (`operations::engine::execute`: admitted over the operations of the session, the witness, the lock only for
//! the freeze, then the effects), its steps, the progress of its push and its outcome sent to the channel of the window
//! that asked; `cancel_operation` stops one that waits for the lock or whose next effect has not started.

use crate::api::{Prefixes, Settings};
use crate::commands::session::{SessionsState, session_key};
use app_api::context::PrefixInputs;
use app_api::operation::operation;
use domain_types::error::ApiError;
use domain_types::operation::{CancelOperationParams, ExecuteParams, Intent, OperationEvent, OperationId, Outcome, Plan, PrepareParams};
use git_executor::git_command_executor::GitCommandExecutor;
use operations::events::EventSink;
use operations::land_execute::Accepted;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State, Webview};
use tracing::instrument;

/// The plan of an intent over the facts the session of the repository holds: what a start of a Safe Push or a Dry Run
/// does now (`operations::land::prepare`, the rule `bd safe-push` decides by), what the user accepts first, what refuses
/// it, the facts it waits for and the profiles it offers; the restart or the cancel of a robot, the retarget of a pull
/// request (`operations::engine::prepare`). A read: no lock, no effect. Null while the session is not open, before its
/// Patronus view, for a subject its table lacks, a robot it offers no restart of, a pull request it doesn't list
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
#[specta::specta]
pub async fn prepare(
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: PrepareParams,
) -> Result<Option<Plan>, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    operations::engine::prepare(state.sessions(), &key, params.intent).await
  })
  .await
}

/// The events of an operation to the channel of the window that asked
struct ChannelEvents(Channel<OperationEvent>);

impl EventSink for ChannelEvents {
  fn send(&self, event: OperationEvent) {
    // a window that went away hears nothing: the operation goes on
    let _ = self.0.send(event);
  }
}

/// The review fetches each window runs (`Intent::FetchReview`): the close of a window stops them, as the fetch of a
/// pull request no window shows is of no use; every other operation of a closed window goes on
#[derive(Default)]
pub struct WindowFetches(Mutex<HashMap<String, Vec<OperationId>>>);

impl WindowFetches {
  fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<OperationId>>> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// The fetches of the window `label` until now: it closed
  pub fn forget(&self, label: &str) -> Vec<OperationId> {
    self.lock().remove(label).unwrap_or_default()
  }
}

/// The events of a review fetch of the window `label`: its first step names its operation, which the close of the
/// window stops, until its end
struct WindowFetchEvents {
  events: ChannelEvents,
  app: AppHandle,
  label: String,
}

impl EventSink for WindowFetchEvents {
  fn send(&self, event: OperationEvent) {
    let fetches = self.app.state::<WindowFetches>();
    match &event {
      OperationEvent::Step { op, .. } => {
        let mut fetches = fetches.lock();
        let ops = fetches.entry(self.label.clone()).or_default();
        if !ops.contains(op) {
          ops.push(op.clone());
        }
      }
      OperationEvent::Done { outcome } => {
        if let Some(ops) = fetches.lock().get_mut(&self.label) {
          ops.retain(|op| *op != outcome.op);
        }
      }
      OperationEvent::Progress { .. } => {}
    }
    self.events.send(event);
  }
}

/// Runs an intent on the plan the page showed (`fingerprint`, with the confirmations the user `accepted`): the plan is
/// prepared again and runs only on that fingerprint; its steps, the progress of its push and its outcome go to
/// `events` (`operations::engine::execute`). The review fetch of a window is stopped when the window closes
/// (`WindowFetches`)
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
#[specta::specta]
pub async fn execute(
  webview: Webview,
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: ExecuteParams,
  events: Channel<OperationEvent>,
) -> Result<Outcome, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    let app = webview.app_handle().clone();
    let events: Arc<dyn EventSink> = if matches!(params.intent, Intent::FetchReview { .. }) {
      Arc::new(WindowFetchEvents {
        events: ChannelEvents(events),
        app: app.clone(),
        label: webview.label().to_owned(),
      })
    } else {
      Arc::new(ChannelEvents(events))
    };
    let accepted = Accepted {
      fingerprint: params.fingerprint,
      confirmations: params.accepted,
    };
    // the run of a start is a large future: on the heap
    Box::pin(state.execute(&app, &key, params.intent, accepted, events)).await
  })
  .await
}

/// Stops the operation `op` of this app: a wait for the repository lock ends as `canceled`, and no effect starts after
/// it; a push that sends goes to its end. False for an operation that ended
#[tauri::command]
#[instrument(skip_all, fields(op = %params.op))]
#[specta::specta]
#[expect(clippy::needless_pass_by_value, reason = "Tauri hands a command its state and its arguments by value")]
pub fn cancel_operation(state: State<'_, SessionsState>, params: CancelOperationParams) -> bool {
  state.registry().cancel(&params.op)
}
