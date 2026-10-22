use crate::command_error::CommandError;
use crate::operation::operation;
use git_executor::git_command_executor::GitCommandExecutor;
use github::client::GitHubClient;
use github::device_flow::GITHUB_OAUTH_CLIENT_ID;
use github::service::GitHubService;
use github::sign_in_session::GitHubSignInSession;
use github::token::{TokenChain, TokenSources};
use github::token_store::token_file;
use std::path::Path;
use sync_types::pull_request::{GitHubAuthStatus, GitHubDeviceCode};
use tauri::{AppHandle, Emitter, State};
use tracing::instrument;

/// Sent to every window when the app's own sign-in changed: the pull requests and review requests are asked again
pub const GITHUB_AUTH_CHANGED_EVENT: &str = "github-auth-changed";

/// The GitHub client of the app: one HTTP client (kept-alive connection) and the token chain with what GitHub accepted
/// and rejected, shared by the windows; and the app's own sign-in (`github::sign_in_session`)
pub struct GitHubState {
  pub service: GitHubService,
  sign_in: GitHubSignInSession,
}

impl GitHubState {
  pub fn new(app_version: &str, git: GitCommandExecutor, app_data_dir: Option<&Path>) -> Self {
    Self {
      service: GitHubService::new(
        GitHubClient::new(&app_user_agent(app_version)),
        TokenChain::new(TokenSources::from_process(app_data_dir), git),
      ),
      sign_in: GitHubSignInSession::new(app_data_dir.map(token_file)),
    }
  }
}

/// `Branch Deck/<version> (<os>; <arch>)`, the user agent of the app's HTTP requests
pub fn app_user_agent(app_version: &str) -> String {
  format!("Branch Deck/{app_version} ({}; {})", std::env::consts::OS, std::env::consts::ARCH)
}

/// The app's own sign-in and where the token comes from, for Settings
#[tauri::command]
#[instrument(skip(github))]
#[specta::specta]
pub async fn get_github_auth_status(github: State<'_, GitHubState>) -> Result<GitHubAuthStatus, CommandError> {
  operation(async move { Ok(github.sign_in.status(&github.service, GITHUB_OAUTH_CLIENT_ID).await) }).await
}

/// Starts the app's own sign-in: the code the user enters on GitHub. `wait_github_sign_in` waits for it.
#[tauri::command]
#[instrument(skip(github))]
#[specta::specta]
pub async fn start_github_sign_in(github: State<'_, GitHubState>) -> Result<GitHubDeviceCode, CommandError> {
  operation(async move { github.sign_in.start(&github.service, GITHUB_OAUTH_CLIENT_ID).await.map_err(CommandError::new) }).await
}

/// Waits until the user entered the code of `start_github_sign_in` on GitHub, keeps the token and tells every window;
/// the login. A newer sign-in or `cancel_github_sign_in` ends it with "canceled".
#[tauri::command]
#[instrument(skip(app, github))]
#[specta::specta]
pub async fn wait_github_sign_in(app: AppHandle, github: State<'_, GitHubState>) -> Result<String, CommandError> {
  operation(async move {
    let login = github.sign_in.wait(&github.service, GITHUB_OAUTH_CLIENT_ID).await.map_err(CommandError::new)?;
    app.emit(GITHUB_AUTH_CHANGED_EVENT, ())?;
    Ok(login)
  })
  .await
}

/// Ends the wait of the sign-in in progress, if any
#[tauri::command]
#[instrument(skip(github))]
#[specta::specta]
pub async fn cancel_github_sign_in(github: State<'_, GitHubState>) -> Result<(), CommandError> {
  operation(async move {
    github.sign_in.cancel();
    Ok(())
  })
  .await
}

/// Removes the token of the app's own sign-in and tells every window; GitHub keeps the authorization until it is
/// revoked there
#[tauri::command]
#[instrument(skip(app, github))]
#[specta::specta]
pub async fn sign_out_of_github(app: AppHandle, github: State<'_, GitHubState>) -> Result<(), CommandError> {
  operation(async move {
    github.sign_in.sign_out(&github.service).await.map_err(CommandError::new)?;
    app.emit(GITHUB_AUTH_CHANGED_EVENT, ())?;
    Ok(())
  })
  .await
}
