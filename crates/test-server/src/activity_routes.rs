//! Activity of the page mock (spec/repository/activity.spec.md), as the app answers it (`src-tauri/src/commands/
//! activity.rs`): over the journals of the requesting page's repository, the ones its sessions remembered in its
//! settings (`subscribe_view`) and the journal of the page in its app data directory. The page reads the log again
//! after its own writes; a test reads it through the page.

use crate::state::AppState;
use crate::tauri_command_bridge::{CommandResult, requesting_repository, run_command};
use app_settings::journals::activity_projects;
use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use domain_types::activity::{ActivityLog, ClearActivityParams, PageActivity, PageEntry};
use operations::journal::{ACTIVITY_READ_TIMEOUT, clear, read_activity as read_journals, record_page};
use std::path::PathBuf;
use std::sync::Arc;

/// `read_activity` of the app
pub async fn read_activity(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<ActivityLog> {
  let (projects, app_data) = {
    let repository = requesting_repository(&state, &headers)?;
    (activity_projects(&repository.store), repository.store.directory().to_path_buf())
  };
  Ok(Json(read_journals(&projects, &app_data, ACTIVITY_READ_TIMEOUT).await))
}

/// `record_activity` of the app
pub async fn record_activity(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(params): Json<PageActivity>) -> CommandResult<PageEntry> {
  let app_data = requesting_repository(&state, &headers)?.store.directory().to_path_buf();
  run_command("record the activity", move || record_page(&app_data, params)).await
}

/// `clear_activity` of the app
pub async fn clear_activity(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(params): Json<ClearActivityParams>) -> CommandResult<()> {
  let (journal, app_data) = {
    let repository = requesting_repository(&state, &headers)?;
    let journal = activity_projects(&repository.store)
      .into_iter()
      .find(|(path, _)| path == &params.repository_path)
      .and_then(|(_, journal)| journal.map(PathBuf::from));
    (journal, repository.store.directory().to_path_buf())
  };
  run_command("clear the activity", move || clear(journal.as_deref(), &app_data, &params.repository_path)).await
}
