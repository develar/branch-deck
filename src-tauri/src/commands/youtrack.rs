use crate::commands::cli_tools::CliToolActionDone;
use crate::commands::github::app_user_agent;
use crate::operation::{operation, tauri_error};
use app_api::api_error::IntoApiError;
use cli_tools::environment::ToolEnvironment;
use domain_types::error::ApiError;
use domain_types::youtrack::{YouTrackAuthStatus, YouTrackIssueAnswer, YouTrackSignIn, YouTrackStateChange, YouTrackUser};
use remote_facts::youtrack_cli::{hand_token_to_cli, handed_token_file};
use std::path::{Path, PathBuf};
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
  /// The id of the permanent token the app made for the `youtrack` CLI (`remote_facts::youtrack_cli`)
  handed_token: Option<PathBuf>,
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
      handed_token: app_data_dir.map(handed_token_file),
    }
  }
}

/// The YouTrack sign-in and where the token comes from, for Settings and the popovers
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn get_youtrack_auth_status(youtrack: State<'_, YouTrackState>) -> Result<YouTrackAuthStatus, ApiError> {
  operation(async move { Ok(youtrack.service.status()) }).await
}

/// Starts the browser sign-in to the YouTrack at `url`: the Hub page to open. `wait_youtrack_sign_in` waits for the
/// browser to come back.
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn start_youtrack_sign_in(youtrack: State<'_, YouTrackState>, url: String) -> Result<YouTrackSignIn, ApiError> {
  operation(async move { youtrack.sign_in.start(&youtrack.service, &url).await.map_err(ApiError::internal) }).await
}

/// Waits until the browser came back from the Hub page of `start_youtrack_sign_in`, then keeps the tokens and tells
/// every window; the user. A newer sign-in or `cancel_youtrack_sign_in` ends it with the code `canceled`.
#[tauri::command]
#[instrument(skip(app, youtrack))]
#[specta::specta]
pub async fn wait_youtrack_sign_in(app: AppHandle, youtrack: State<'_, YouTrackState>) -> Result<YouTrackUser, ApiError> {
  operation(async move {
    let user = youtrack.sign_in.wait(&youtrack.service).await.map_err(IntoApiError::into_api_error)?;
    app.emit(YOUTRACK_AUTH_CHANGED_EVENT, ()).map_err(tauri_error)?;
    Ok(user)
  })
  .await
}

/// Starts the one-shot browser sign-in that gets the `youtrack` CLI a permanent token of the YouTrack at `url`: the Hub
/// page to open; `wait_youtrack_cli_sign_in` waits for it. It replaces a sign-in in progress
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn start_youtrack_cli_sign_in(youtrack: State<'_, YouTrackState>, url: String) -> Result<YouTrackSignIn, ApiError> {
  operation(async move { youtrack.sign_in.start_for_cli(&youtrack.service, &url).await.map_err(ApiError::internal) }).await
}

/// Waits for the browser to come back from the Hub page of `start_youtrack_cli_sign_in`, then makes the permanent token
/// and signs the `youtrack` CLI in with it; the token the app made before is revoked. `cancel_youtrack_sign_in` ends it
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn wait_youtrack_cli_sign_in(youtrack: State<'_, YouTrackState>) -> Result<CliToolActionDone, ApiError> {
  operation(async move {
    let grant = youtrack.sign_in.wait_for_cli(&youtrack.service).await.map_err(IntoApiError::into_api_error)?;
    let title = hand_token_to_cli(&youtrack.service, grant, ToolEnvironment::from_process(), youtrack.handed_token.as_deref())
      .await
      .map_err(ApiError::internal)?;
    Ok(CliToolActionDone { title, skill_problem: None })
  })
  .await
}

/// Ends the wait of the sign-in in progress, if any
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn cancel_youtrack_sign_in(youtrack: State<'_, YouTrackState>) -> Result<(), ApiError> {
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
pub async fn save_youtrack_token(app: AppHandle, youtrack: State<'_, YouTrackState>, url: String, token: String) -> Result<YouTrackUser, ApiError> {
  operation(async move {
    let user = youtrack.sign_in.save_token(&youtrack.service, &url, &token).await.map_err(ApiError::internal)?;
    app.emit(YOUTRACK_AUTH_CHANGED_EVENT, ()).map_err(tauri_error)?;
    Ok(user)
  })
  .await
}

/// Removes the token of the app's own sign-in. The Hub keeps the authorization until it is revoked there (Account
/// Security of the YouTrack profile).
#[tauri::command]
#[instrument(skip(app, youtrack))]
#[specta::specta]
pub async fn sign_out_of_youtrack(app: AppHandle, youtrack: State<'_, YouTrackState>) -> Result<(), ApiError> {
  operation(async move {
    youtrack.sign_in.sign_out(&youtrack.service).await.map_err(ApiError::internal)?;
    app.emit(YOUTRACK_AUTH_CHANGED_EVENT, ()).map_err(tauri_error)?;
    Ok(())
  })
  .await
}

/// The issue of `key` for the popover, from the YouTrack the token is for
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn get_youtrack_issue(youtrack: State<'_, YouTrackState>, key: String) -> Result<YouTrackIssueAnswer, ApiError> {
  operation(async move { Ok(youtrack.service.issue(&key).await) }).await
}

/// Sets the state field of the issue (Mark Fixed, and its undo with `expected`, the value the change set); the
/// previous value
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn set_youtrack_issue_state(youtrack: State<'_, YouTrackState>, key: String, state: String, expected: Option<String>) -> Result<YouTrackStateChange, ApiError> {
  operation(async move { youtrack.service.set_state(&key, &state, expected.as_deref()).await.map_err(ApiError::internal) }).await
}
