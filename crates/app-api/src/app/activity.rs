//! Activity (spec/repository/activity.spec.md): the page reads the log from the journals (`read_activity`: the journals
//! of the repositories of the recent projects, which `subscribe_view` remembered, and the journal of the page in the app
//! data directory), records what only it knows in the journal of the page (`record_activity`) and forgets the log of a
//! repository (`clear_activity`). A write tells every window `activity-changed`, as the watch of a repository does when
//! its journal changed (`RegistryFiles::on_journal`, which each host sets): the windows read the log again.

use crate::context::AppContext;
use crate::events::activity_changed;
use crate::operation::{operation, run_blocking};
use app_settings::journals::activity_projects;
use domain_types::activity::{ActivityLog, ClearActivityParams, PageActivity, PageEntry};
use domain_types::error::ApiError;
use operations::journal::{ACTIVITY_READ_TIMEOUT, LateListener, clear, read_activity as read_journals, record_page};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::instrument;

/// The log of Activity: the journals of the repositories of the recent projects, each once and each waited for within
/// `ACTIVITY_READ_TIMEOUT` (one that is not read by then shows what it answered last, and tells every window
/// `activity-changed` once it answered other entries), and the journal of the page; no git process
#[instrument(skip_all)]
pub async fn read_activity(ctx: &AppContext) -> Result<ActivityLog, ApiError> {
  operation(async move {
    let events = Arc::clone(&ctx.events);
    let late: LateListener = Arc::new(move || activity_changed(&*events));
    Ok(read_journals(&activity_projects(&ctx.settings), ctx.settings.directory(), ACTIVITY_READ_TIMEOUT, &late).await)
  })
  .await
}

/// Records what the page told of a repository in the journal of the page, and answers its entry
#[instrument(skip_all, fields(kind = ?params.kind))]
pub async fn record_activity(ctx: &AppContext, params: PageActivity) -> Result<PageEntry, ApiError> {
  operation(async move {
    let directory = ctx.settings.directory().to_path_buf();
    let entry = run_blocking(move || record_page(&directory, params)).await?;
    activity_changed(&*ctx.events);
    Ok(entry)
  })
  .await
}

/// Clear Log: the journal of the repository of the recent project and the entries of the page of it are forgotten
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
pub async fn clear_activity(ctx: &AppContext, params: ClearActivityParams) -> Result<(), ApiError> {
  operation(async move {
    let journal = journal_of(ctx, &params.repository_path);
    let directory = ctx.settings.directory().to_path_buf();
    run_blocking(move || clear(journal.as_deref(), &directory, &params.repository_path)).await?;
    activity_changed(&*ctx.events);
    Ok(())
  })
  .await
}

/// The journal remembered for the recent project at `repository_path`
fn journal_of(ctx: &AppContext, repository_path: &str) -> Option<PathBuf> {
  activity_projects(&ctx.settings)
    .into_iter()
    .find(|(path, _)| path == repository_path)
    .and_then(|(_, journal)| journal.map(PathBuf::from))
}
