//! Activity (spec/repository/activity.spec.md): the page reads the log from the journals (`read_activity`: the journals
//! of the repositories of the recent projects, which `subscribe_view` remembered, and the journal of the page in the app
//! data directory), records what only it knows in the journal of the page (`record_activity`) and forgets the log of a
//! repository (`clear_activity`). A write tells every window `activity-changed`, as the watch of a repository does when
//! its journal changed (`RegistryFiles::on_journal`): the windows read the log again.

use crate::commands::branch_prefix::Settings;
use crate::operation::{operation, run_blocking};
use app_settings::journals::activity_projects;
use domain_types::activity::{ActivityLog, ClearActivityParams, PageActivity, PageEntry};
use domain_types::error::ApiError;
use operations::journal::{ACTIVITY_READ_TIMEOUT, LateListener, clear, read_activity as read_journals, record_page};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Runtime};
use tracing::{instrument, warn};

/// The event a window reads Activity again on: a journal it reads changed
pub const ACTIVITY_CHANGED_EVENT: &str = "activity-changed";

/// Tells every window that a journal Activity reads changed
pub fn emit_activity_changed<R: Runtime>(app: &AppHandle<R>) {
  if let Err(error) = app.emit(ACTIVITY_CHANGED_EVENT, ()) {
    warn!(%error, "cannot tell the windows that Activity changed");
  }
}

/// The log of Activity: the journals of the repositories of the recent projects, each once and each waited for within
/// `ACTIVITY_READ_TIMEOUT` (one that is not read by then shows what it answered last, and tells every window
/// `activity-changed` once it answered other entries), and the journal of the page; no git process
#[tauri::command]
#[instrument(skip_all)]
#[specta::specta]
pub async fn read_activity(app: AppHandle, settings: Settings<'_>) -> Result<ActivityLog, ApiError> {
  operation(async move {
    let late: LateListener = Arc::new(move || emit_activity_changed(&app));
    Ok(read_journals(&activity_projects(&settings), settings.directory(), ACTIVITY_READ_TIMEOUT, &late).await)
  })
  .await
}

/// Records what the page told of a repository in the journal of the page, and answers its entry
#[tauri::command]
#[instrument(skip_all, fields(kind = ?params.kind))]
#[specta::specta]
pub async fn record_activity(app: AppHandle, settings: Settings<'_>, params: PageActivity) -> Result<PageEntry, ApiError> {
  operation(async move {
    let directory = settings.directory().to_path_buf();
    let entry = run_blocking(move || record_page(&directory, params)).await?;
    emit_activity_changed(&app);
    Ok(entry)
  })
  .await
}

/// Clear Log: the journal of the repository of the recent project and the entries of the page of it are forgotten
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
#[specta::specta]
pub async fn clear_activity(app: AppHandle, settings: Settings<'_>, params: ClearActivityParams) -> Result<(), ApiError> {
  operation(async move {
    let journal = journal_of(&settings, &params.repository_path);
    let directory = settings.directory().to_path_buf();
    run_blocking(move || clear(journal.as_deref(), &directory, &params.repository_path)).await?;
    emit_activity_changed(&app);
    Ok(())
  })
  .await
}

/// The journal remembered for the recent project at `repository_path`
fn journal_of(settings: &Settings<'_>, repository_path: &str) -> Option<PathBuf> {
  activity_projects(settings)
    .into_iter()
    .find(|(path, _)| path == repository_path)
    .and_then(|(_, journal)| journal.map(PathBuf::from))
}
