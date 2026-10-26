//! The operations of the remote as the page asks them (docs/design/rust-domain-authority.md, "Планирование и
//! исполнение операций"): `prepare` answers the plan of an intent in the session of the repository, which the popover
//! of a start renders and a click starts by (`operations::engine::prepare`).

use crate::commands::branch_prefix::{PrefixInputs, Prefixes, Settings};
use crate::commands::session::{SessionsState, session_key};
use crate::operation::operation;
use domain_types::error::ApiError;
use domain_types::operation::{Plan, PrepareParams};
use git_executor::git_command_executor::GitCommandExecutor;
use tauri::State;
use tracing::instrument;

/// The plan of an intent over the facts the session of the repository holds: what a start of a Safe Push or a Dry Run
/// does now (`operations::land::prepare`, the rule `bd safe-push` decides by), what the user accepts first, what refuses
/// it, the facts it waits for and the profiles it offers. A read: no lock, no effect. Null while the session is not open,
/// before its Patronus view, or for a subject its table lacks
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
