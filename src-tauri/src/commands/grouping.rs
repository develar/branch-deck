//! The group a typed message or branch name puts a commit into (`preview_group`, the grouping of the session's table),
//! answered by Rust so the page keeps no grammar of its own; the rest of that grammar is `app_api::reads::grammar`. A
//! read: no git, no lock, no effect.

use crate::api::{Prefixes, Settings};
use crate::commands::session::{SessionsState, session_key};
use app_api::context::PrefixInputs;
use app_api::operation::operation;
use domain_types::commit::{CommitGroup, PreviewGroupParams};
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use tauri::State;
use tracing::instrument;

/// The group `message` would put its commit into, with the stacked branches, the issue groups and the stack parents
/// the table of the repository's session was grouped with (`Sessions::preview_group`): the "Move to…" form previews a
/// typed name as `(<name>)`, the message editor a typed subject. Null for an unassigned commit and before the session
/// is open
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
#[specta::specta]
pub async fn preview_group(
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: PreviewGroupParams,
) -> Result<Option<CommitGroup>, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    Ok(state.sessions().preview_group(&key, &params.message).await)
  })
  .await
}
