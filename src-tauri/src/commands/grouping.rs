//! The grammar of what the page shows and the user types, answered by Rust so the page keeps none of its own: the group a
//! typed message or branch name puts a commit into (`preview_group`, the grouping of the session's table), the check of a
//! typed branch name (`validate_branch_name`) and the blocks of a commit body (`commit_body_blocks`, the grammar the
//! body of a pull request is written from). Reads: no git, no lock, no effect.

use crate::commands::branch_prefix::{PrefixInputs, Prefixes, Settings};
use crate::commands::session::{SessionsState, session_key};
use crate::operation::operation;
use domain_types::branch_names::{BranchNameCheck, ValidateBranchNameParams, check_branch_name};
use domain_types::commit::{CommitBodyParams, CommitGroup, PreviewGroupParams};
use domain_types::commit_body::CommitBodyBlock;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_utils::commit_body::{body_blocks, message_body};
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

/// What a typed branch name is and the name the branch gets (`domain_types::branch_names::check_branch_name`); the page
/// words the state
#[tauri::command]
#[instrument(skip_all)]
#[specta::specta]
#[expect(clippy::needless_pass_by_value, reason = "Tauri hands a command its arguments by value")]
pub fn validate_branch_name(params: ValidateBranchNameParams) -> BranchNameCheck {
  check_branch_name(&params.name, params.rules)
}

/// The blocks of the body of `message`, the lines after its subject line (`sync_utils::commit_body::body_blocks`)
#[tauri::command]
#[instrument(skip_all)]
#[specta::specta]
#[expect(clippy::needless_pass_by_value, reason = "Tauri hands a command its arguments by value")]
pub fn commit_body_blocks(params: CommitBodyParams) -> Vec<CommitBodyBlock> {
  body_blocks(message_body(&params.message))
}
