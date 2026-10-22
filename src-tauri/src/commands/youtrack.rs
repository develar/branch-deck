use crate::command_error::CommandError;
use crate::commands::github::app_user_agent;
use crate::operation::operation;
use std::path::Path;
use sync_types::youtrack::{YouTrackAuthStatus, YouTrackIssueAnswer, YouTrackSignIn, YouTrackStateChange, YouTrackUser};
use tauri::{AppHandle, Emitter, State};
use tracing::instrument;
use youtrack::client::YouTrackClient;
use youtrack::service::{EnvironmentToken, YouTrackService};
use youtrack::sign_in_session::YouTrackSignInSession;
use youtrack::token_store::token_file;

/// Sent to every window when the YouTrack sign-in changed: the popovers ask again
pub const YOUTRACK_AUTH_CHANGED_EVENT: &str = "youtrack-auth-changed";

/// The YouTrack of the app, shared by the windows, and its own sign-in (`youtrack::sign_in_session`)
pub struct YouTrackState {
  pub service: YouTrackService,
  sign_in: YouTrackSignInSession,
}

impl YouTrackState {
  pub fn new(app_version: &str, app_data_dir: Option<&Path>) -> Self {
    Self {
      service: YouTrackService::new(
        YouTrackClient::new(&app_user_agent(app_version)),
        EnvironmentToken::from_process(),
        app_data_dir.map(token_file),
      ),
      sign_in: YouTrackSignInSession::default(),
    }
  }
}

/// The YouTrack sign-in and where the token comes from, for Settings and the popovers
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn get_youtrack_auth_status(youtrack: State<'_, YouTrackState>) -> Result<YouTrackAuthStatus, CommandError> {
  operation(async move { Ok(youtrack.service.status().await) }).await
}

/// Starts the browser sign-in to the YouTrack at `url`: the Hub page to open. `wait_youtrack_sign_in` waits for the
/// browser to come back.
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn start_youtrack_sign_in(youtrack: State<'_, YouTrackState>, url: String) -> Result<YouTrackSignIn, CommandError> {
  operation(async move { youtrack.sign_in.start(&youtrack.service, &url).await.map_err(CommandError::new) }).await
}

/// Waits until the browser came back from the Hub page of `start_youtrack_sign_in`, then keeps the tokens and tells
/// every window; the user. A newer sign-in or `cancel_youtrack_sign_in` ends it with "canceled".
#[tauri::command]
#[instrument(skip(app, youtrack))]
#[specta::specta]
pub async fn wait_youtrack_sign_in(app: AppHandle, youtrack: State<'_, YouTrackState>) -> Result<YouTrackUser, CommandError> {
  operation(async move {
    let user = youtrack.sign_in.wait(&youtrack.service).await.map_err(CommandError::new)?;
    app.emit(YOUTRACK_AUTH_CHANGED_EVENT, ())?;
    Ok(user)
  })
  .await
}

/// Ends the wait of the sign-in in progress, if any
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn cancel_youtrack_sign_in(youtrack: State<'_, YouTrackState>) -> Result<(), CommandError> {
  operation(async move {
    youtrack.sign_in.cancel();
    Ok(())
  })
  .await
}

/// Keeps a token the user created in YouTrack (Profile → Account Security), once YouTrack accepted it
#[tauri::command]
#[instrument(skip(app, youtrack, token))]
#[specta::specta]
pub async fn save_youtrack_token(app: AppHandle, youtrack: State<'_, YouTrackState>, url: String, token: String) -> Result<YouTrackUser, CommandError> {
  operation(async move {
    let user = youtrack.sign_in.save_token(&youtrack.service, &url, &token).await.map_err(CommandError::new)?;
    app.emit(YOUTRACK_AUTH_CHANGED_EVENT, ())?;
    Ok(user)
  })
  .await
}

/// Removes the token of the app's own sign-in. The Hub keeps the authorization until it is revoked there (Account
/// Security of the YouTrack profile).
#[tauri::command]
#[instrument(skip(app, youtrack))]
#[specta::specta]
pub async fn sign_out_of_youtrack(app: AppHandle, youtrack: State<'_, YouTrackState>) -> Result<(), CommandError> {
  operation(async move {
    youtrack.sign_in.sign_out(&youtrack.service).await.map_err(CommandError::new)?;
    app.emit(YOUTRACK_AUTH_CHANGED_EVENT, ())?;
    Ok(())
  })
  .await
}

/// The issue of `key` for the popover, from the YouTrack the token is for
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn get_youtrack_issue(youtrack: State<'_, YouTrackState>, key: String) -> Result<YouTrackIssueAnswer, CommandError> {
  operation(async move { Ok(youtrack.service.issue(&key).await) }).await
}

/// Sets the state field of the issue (Mark Fixed, and its undo with `expected`, the value the change set); the
/// previous value
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn set_youtrack_issue_state(youtrack: State<'_, YouTrackState>, key: String, state: String, expected: Option<String>) -> Result<YouTrackStateChange, CommandError> {
  operation(async move { youtrack.service.set_state(&key, &state, expected.as_deref()).await.map_err(CommandError::new) }).await
}
