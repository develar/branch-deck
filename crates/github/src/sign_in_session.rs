//! The app's own sign-in to GitHub (spec/repository/github-sign-in.spec.md): who the stored token (`token_store.rs`)
//! belongs to and which source the token chain would take first; and the device flow (`device_flow.rs`) of one
//! sign-in at a time, from the code to the stored token, a cancel and a sign-out. Shared by the app's commands, which
//! only tell the windows (`github-auth-changed`), and the E2E test server, which hosts the same session and service
//! behind a fake GitHub.

use crate::client::GITHUB_URL;
use crate::device_flow::{DeviceCode, request_device_code, wait_for_token};
use crate::service::GitHubService;
use crate::token_store::{StoredToken, read_token, remove_token, write_token};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use sync_types::pull_request::{GitHubAuthStatus, GitHubDeviceCode};
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tracing::{info, instrument};

/// The error of a sign-in in a build without the OAuth app (`GITHUB_OAUTH_CLIENT_ID`)
pub const NO_CLIENT_ID_MESSAGE: &str = "This build of Branch Deck can't sign in to GitHub itself: run gh auth login, or set GH_TOKEN";

/// The sign-in of one app: its token file and the sign-in in progress
pub struct GitHubSignInSession {
  /// The token file of the app's own sign-in; None without an app data directory
  token_file: Option<PathBuf>,
  /// The device code of the sign-in in progress, with its generation
  pending: Mutex<Option<(u64, DeviceCode)>>,
  /// A newer sign-in, a cancel or a sign-out ends the wait of an older one
  generation: AtomicU64,
}

impl GitHubSignInSession {
  pub const fn new(token_file: Option<PathBuf>) -> Self {
    Self {
      token_file,
      pending: Mutex::new(None),
      generation: AtomicU64::new(0),
    }
  }

  /// A sign-in waits for the user to enter its code on GitHub: started, and not ended by `wait` (with its token or
  /// its error), `cancel` or `sign_out`
  pub fn is_pending(&self) -> bool {
    self.pending.lock().unwrap().is_some()
  }

  /// The sign-in of the token file and where the token comes from; `client_id` is the OAuth app of the build
  /// (`GITHUB_OAUTH_CLIENT_ID`), without which the app offers no sign-in of its own
  pub async fn status(&self, service: &GitHubService, client_id: Option<&str>) -> GitHubAuthStatus {
    let token_file = self.token_file.clone();
    let stored = tokio::task::spawn_blocking(in_current_span(move || token_file.as_deref().and_then(read_token)))
      .await
      .ok()
      .flatten();
    GitHubAuthStatus {
      sign_in_available: client_id.is_some(),
      signed_in_as: stored.map(|stored| stored.login),
      token_source: service.tokens.first_source().await,
      manage_url: client_id.map(|id| format!("{GITHUB_URL}/settings/connections/applications/{id}")),
    }
  }

  /// Starts a sign-in, which replaces the one before: the code the user enters on GitHub; `wait` waits for it
  #[instrument(skip(self, service))]
  pub async fn start(&self, service: &GitHubService, client_id: Option<&str>) -> Result<GitHubDeviceCode, String> {
    let code = request_device_code(&service.client, client_id.ok_or(NO_CLIENT_ID_MESSAGE)?)
      .await
      .map_err(|e| e.to_string())?;
    let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let answer = GitHubDeviceCode {
      user_code: code.user_code.clone(),
      verification_uri: code.verification_uri.clone(),
      expires_in: u32::try_from(code.expires_in).unwrap_or(u32::MAX),
    };
    *self.pending.lock().unwrap() = Some((generation, code));
    Ok(answer)
  }

  /// Waits until the user entered the code of `start` on GitHub, then keeps the token in the token file and forgets
  /// what the token chain knew; the login. A newer sign-in, `cancel` or `sign_out` ends it with "canceled". The sign-in
  /// ends with its token or with its error (a denied or expired code, a GitHub that does not answer): nothing is
  /// pending after either, unless a newer sign-in started meanwhile.
  #[instrument(skip(self, service))]
  pub async fn wait(&self, service: &GitHubService, client_id: Option<&str>) -> Result<String, String> {
    let (generation, code) = self.pending.lock().unwrap().clone().ok_or("No GitHub sign-in is in progress")?;
    let signed_in = self.finish(service, client_id, &code, generation).await;
    self.pending.lock().unwrap().take_if(|(pending, _)| *pending == generation);
    if signed_in.is_ok() {
      service.tokens.forget().await;
    }
    signed_in
  }

  async fn finish(&self, service: &GitHubService, client_id: Option<&str>, code: &DeviceCode, generation: u64) -> Result<String, String> {
    let client = &service.client;
    let is_canceled = || self.generation.load(Ordering::SeqCst) != generation;
    let access = wait_for_token(client, client_id.ok_or(NO_CLIENT_ID_MESSAGE)?, code, is_canceled)
      .await
      .map_err(|e| e.to_string())?;
    let login = client.viewer_login(&access.token).await.map_err(|e| e.message())?;
    let stored = StoredToken {
      token: access.token,
      login: login.clone(),
      scopes: access.scopes,
      created_at: unix_now(),
    };
    let file = self.require_token_file()?;
    tokio::task::spawn_blocking(in_current_span(move || write_token(&file, &stored)))
      .await
      .map_err(|e| e.to_string())?
      .map_err(|e| format!("Cannot keep the GitHub token: {e}"))?;
    info!(%login, "signed in to GitHub");
    Ok(login)
  }

  /// Ends the wait of the sign-in in progress, if any
  pub fn cancel(&self) {
    self.generation.fetch_add(1, Ordering::SeqCst);
    self.pending.lock().unwrap().take();
  }

  /// Removes the token of the app's own sign-in and ends the wait of a sign-in in progress. GitHub keeps the
  /// authorization until it is revoked there (`manage_url`): revoking needs the client secret, which the app has not.
  #[instrument(skip(self, service))]
  pub async fn sign_out(&self, service: &GitHubService) -> Result<(), String> {
    self.cancel();
    let file = self.require_token_file()?;
    tokio::task::spawn_blocking(in_current_span(move || remove_token(&file)))
      .await
      .map_err(|e| e.to_string())?
      .map_err(|e| format!("Cannot remove the GitHub token: {e}"))?;
    service.tokens.forget().await;
    Ok(())
  }

  fn require_token_file(&self) -> Result<PathBuf, String> {
    self
      .token_file
      .clone()
      .ok_or_else(|| "The app has no data directory to keep the GitHub token in".to_string())
  }
}
