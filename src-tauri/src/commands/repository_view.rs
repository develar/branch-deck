//! What Rust derives of the branch table of the last sync for the page (docs/design/git-operations.md, "Repository
//! view"): the page asks it when its robots, its remote facts or its table change, and reads it by branch

use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use repository_view::views::RepositoryViews;
use sync_types::repository_view::{GetRepositoryViewParams, RepositoryView};
use tauri::State;
use tracing::instrument;

/// The view of the table the last sync of the repository finished: what the robots land, landed and hold
/// (`patronus_view`), with the page's robots, their matches and what the page knows of the remote branches. Git only
/// for robots the hashes and the memo don't answer
#[tauri::command]
#[instrument(skip(git_executor, views))]
#[specta::specta]
pub async fn get_repository_view(
  git_executor: State<'_, GitCommandExecutor>,
  views: State<'_, RepositoryViews>,
  params: GetRepositoryViewParams,
) -> Result<RepositoryView, CommandError> {
  let git = git_executor.inner().clone();
  let views = views.inner().clone();
  run_blocking(move || Ok::<_, anyhow::Error>(views.view(&git, &params))).await
}
