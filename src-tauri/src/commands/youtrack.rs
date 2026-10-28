//! The YouTrack of the app: its service, shared by the windows, the sessions (`SetIssueState`) and the sign-ins
//! (`commands::sign_in`), and the issue of a popover (spec/repository/youtrack.spec.md)

use crate::commands::github::app_user_agent;
use crate::operation::operation;
use domain_types::error::ApiError;
use domain_types::youtrack::YouTrackIssueAnswer;
use remote_facts::youtrack_cli::handed_token_file;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;
use tracing::instrument;
use youtrack::client::YouTrackClient;
use youtrack::service::{EnvironmentToken, YouTrackService};
use youtrack::token_store::token_file;

/// The YouTrack of the app, shared by the windows
pub struct YouTrackState {
  /// Shared with the operations of the sessions (`SetIssueState` through `ServiceHost::youtrack`) and the sign-ins
  pub service: Arc<YouTrackService>,
  /// The id of the permanent token the app made for the `youtrack` CLI (`remote_facts::youtrack_cli`)
  pub(crate) handed_token: Option<PathBuf>,
}

impl YouTrackState {
  pub fn new(app_version: &str, app_data_dir: Option<&Path>) -> Self {
    Self {
      service: Arc::new(YouTrackService::new(
        YouTrackClient::new(&app_user_agent(app_version)),
        EnvironmentToken::from_process(),
        app_data_dir.map(token_file),
      )),
      handed_token: app_data_dir.map(handed_token_file),
    }
  }
}

/// The issue of `key` for the popover, from the YouTrack the token is for
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn get_youtrack_issue(youtrack: State<'_, YouTrackState>, key: String) -> Result<YouTrackIssueAnswer, ApiError> {
  operation(async move { Ok(youtrack.service.issue(&key).await) }).await
}
