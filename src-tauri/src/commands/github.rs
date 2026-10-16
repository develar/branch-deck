use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use github::client::{GITHUB_URL, GitHubClient};
use github::device_flow::{DeviceCode, GITHUB_OAUTH_CLIENT_ID, request_device_code, wait_for_token};
use github::service::GitHubService;
use github::token::{TokenChain, TokenSources};
use github::token_store::{StoredToken, read_token, remove_token, token_file, write_token};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use sync_types::pull_request::{GitHubAuthStatus, GitHubDeviceCode};
use sync_utils::timestamp::unix_now;
use tauri::{AppHandle, Emitter, State};
use tracing::{info, instrument};

/// Sent to every window when the app's own sign-in changed: the pull requests and review requests are asked again
pub const GITHUB_AUTH_CHANGED_EVENT: &str = "github-auth-changed";

/// The GitHub client of the app: one HTTP client (kept-alive connection) and the token chain with what GitHub accepted
/// and rejected, shared by the windows; and the sign-in in progress
pub struct GitHubState {
  pub service: GitHubService,
  /// The token file of the app's own sign-in; None without an app data directory
  token_file: Option<PathBuf>,
  /// The device code of the sign-in in progress, with its generation
  sign_in: Mutex<Option<(u64, DeviceCode)>>,
  /// A newer sign-in, a cancel or a sign-out ends the wait of an older one
  generation: AtomicU64,
}

impl GitHubState {
  pub fn new(app_version: &str, git: GitCommandExecutor, app_data_dir: Option<&Path>) -> Self {
    Self {
      service: GitHubService::new(
        GitHubClient::new(&app_user_agent(app_version)),
        TokenChain::new(TokenSources::from_process(app_data_dir), git),
      ),
      token_file: app_data_dir.map(token_file),
      sign_in: Mutex::new(None),
      generation: AtomicU64::new(0),
    }
  }

  fn token_file(&self) -> Result<PathBuf, CommandError> {
    self
      .token_file
      .clone()
      .ok_or_else(|| CommandError::new("The app has no data directory to keep the GitHub token in"))
  }
}

/// `Branch Deck/<version> (<os>; <arch>)`, the user agent of the app's HTTP requests
pub fn app_user_agent(app_version: &str) -> String {
  format!("Branch Deck/{app_version} ({}; {})", std::env::consts::OS, std::env::consts::ARCH)
}

fn client_id() -> Result<&'static str, CommandError> {
  GITHUB_OAUTH_CLIENT_ID.ok_or_else(|| CommandError::new("This build of Branch Deck can't sign in to GitHub itself: run gh auth login, or set GH_TOKEN"))
}

/// The app's own sign-in and where the token comes from, for Settings
#[tauri::command]
#[specta::specta]
pub async fn get_github_auth_status(github: State<'_, GitHubState>) -> Result<GitHubAuthStatus, CommandError> {
  let file = github.token_file.clone();
  let stored = run_blocking(move || Ok::<_, CommandError>(file.as_deref().and_then(read_token))).await?;
  Ok(GitHubAuthStatus {
    sign_in_available: GITHUB_OAUTH_CLIENT_ID.is_some(),
    signed_in_as: stored.map(|stored| stored.login),
    token_source: github.service.tokens.first_source().await,
    manage_url: GITHUB_OAUTH_CLIENT_ID.map(|id| format!("{GITHUB_URL}/settings/connections/applications/{id}")),
  })
}

/// Starts the app's own sign-in: the code the user enters on GitHub. `wait_github_sign_in` waits for it.
#[tauri::command]
#[instrument(skip(github))]
#[specta::specta]
pub async fn start_github_sign_in(github: State<'_, GitHubState>) -> Result<GitHubDeviceCode, CommandError> {
  let code = request_device_code(&github.service.client, client_id()?)
    .await
    .map_err(|e| CommandError::new(e.to_string()))?;
  let generation = github.generation.fetch_add(1, Ordering::SeqCst) + 1;
  let answer = GitHubDeviceCode {
    user_code: code.user_code.clone(),
    verification_uri: code.verification_uri.clone(),
    expires_in: u32::try_from(code.expires_in).unwrap_or(u32::MAX),
  };
  *github.sign_in.lock().unwrap() = Some((generation, code));
  Ok(answer)
}

/// Waits until the user entered the code of `start_github_sign_in` on GitHub, then keeps the token (`token_store`) and
/// tells every window; the login. A newer sign-in or `cancel_github_sign_in` ends it with "canceled".
#[tauri::command]
#[instrument(skip(app, github))]
#[specta::specta]
pub async fn wait_github_sign_in(app: AppHandle, github: State<'_, GitHubState>) -> Result<String, CommandError> {
  let (generation, code) = github
    .sign_in
    .lock()
    .unwrap()
    .clone()
    .ok_or_else(|| CommandError::new("No GitHub sign-in is in progress"))?;
  let client = &github.service.client;
  let access = wait_for_token(client, client_id()?, &code, || github.generation.load(Ordering::SeqCst) != generation)
    .await
    .map_err(|e| CommandError::new(e.to_string()))?;
  let user: GhUser = client.get(&access.token, &["user"], &[]).await.map_err(|e| CommandError::new(e.message()))?;
  let stored = StoredToken {
    token: access.token,
    login: user.login.clone(),
    scopes: access.scopes,
    created_at: unix_now(),
  };
  let file = github.token_file()?;
  run_blocking(move || write_token(&file, &stored).map_err(|e| CommandError::new(format!("Cannot keep the GitHub token: {e}")))).await?;
  github.sign_in.lock().unwrap().take_if(|(pending, _)| *pending == generation);
  github.service.tokens.forget().await;
  info!(login = %user.login, "signed in to GitHub");
  app.emit(GITHUB_AUTH_CHANGED_EVENT, ())?;
  Ok(user.login)
}

/// Ends the wait of the sign-in in progress, if any
#[tauri::command]
#[specta::specta]
pub async fn cancel_github_sign_in(github: State<'_, GitHubState>) -> Result<(), CommandError> {
  github.generation.fetch_add(1, Ordering::SeqCst);
  github.sign_in.lock().unwrap().take();
  Ok(())
}

/// Removes the token of the app's own sign-in. GitHub keeps the authorization until it is revoked there
/// (`manage_url`): revoking needs the client secret, which the app has not.
#[tauri::command]
#[specta::specta]
pub async fn sign_out_of_github(app: AppHandle, github: State<'_, GitHubState>) -> Result<(), CommandError> {
  github.generation.fetch_add(1, Ordering::SeqCst);
  let file = github.token_file()?;
  run_blocking(move || remove_token(&file).map_err(|e| CommandError::new(format!("Cannot remove the GitHub token: {e}")))).await?;
  github.service.tokens.forget().await;
  app.emit(GITHUB_AUTH_CHANGED_EVENT, ())?;
  Ok(())
}

/// `GET /user`
#[derive(Deserialize)]
struct GhUser {
  login: String,
}
