use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::commands::github::app_user_agent;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use sync_types::youtrack::{YouTrackAuthStatus, YouTrackIssueAnswer, YouTrackSignIn, YouTrackStateChange, YouTrackUser};
use sync_utils::private_file::{remove_private, write_private_json};
use sync_utils::timestamp::unix_now;
use tauri::{AppHandle, Emitter, State};
use tracing::{info, instrument};
use youtrack::client::YouTrackClient;
use youtrack::issue::fetch_me;
use youtrack::service::{EnvironmentToken, YouTrackService, expires_at};
use youtrack::sign_in::{PendingSignIn, finish_sign_in, start_sign_in};
use youtrack::token_store::{RefreshGrant, StoredToken, token_file};
use youtrack::url::normalize_base_url;

/// Sent to every window when the YouTrack sign-in changed: the popovers ask again
pub const YOUTRACK_AUTH_CHANGED_EVENT: &str = "youtrack-auth-changed";

/// The YouTrack of the app, shared by the windows, and the browser sign-in in progress
pub struct YouTrackState {
  pub service: YouTrackService,
  /// The sign-in waiting for the browser, with its generation and its YouTrack
  sign_in: Mutex<Option<(u64, String, PendingSignIn)>>,
  /// A newer sign-in, a cancel or a sign-out ends the wait of an older one
  generation: AtomicU64,
}

impl YouTrackState {
  pub fn new(app_version: &str, app_data_dir: Option<&Path>) -> Self {
    Self {
      service: YouTrackService::new(
        YouTrackClient::new(&app_user_agent(app_version)),
        EnvironmentToken::from_process(),
        app_data_dir.map(token_file),
      ),
      sign_in: Mutex::new(None),
      generation: AtomicU64::new(0),
    }
  }

  async fn keep(&self, app: &AppHandle, token: StoredToken) -> Result<(), CommandError> {
    let file = self
      .service
      .token_file()
      .cloned()
      .ok_or_else(|| CommandError::new("The app has no data directory to keep the YouTrack token in"))?;
    run_blocking(move || write_private_json(&file, &token).map_err(|e| CommandError::new(format!("Cannot keep the YouTrack token: {e}")))).await?;
    self.service.forget().await;
    app.emit(YOUTRACK_AUTH_CHANGED_EVENT, ())?;
    Ok(())
  }
}

/// The YouTrack sign-in and where the token comes from, for Settings and the popovers
#[tauri::command]
#[specta::specta]
pub async fn get_youtrack_auth_status(youtrack: State<'_, YouTrackState>) -> Result<YouTrackAuthStatus, CommandError> {
  Ok(youtrack.service.status().await)
}

/// Starts the browser sign-in to the YouTrack at `url`: the Hub page to open. `wait_youtrack_sign_in` waits for the
/// browser to come back.
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn start_youtrack_sign_in(youtrack: State<'_, YouTrackState>, url: String) -> Result<YouTrackSignIn, CommandError> {
  let base = normalize_base_url(&url).map_err(CommandError::new)?;
  let pending = start_sign_in(&youtrack.service.client, &base).await.map_err(|e| CommandError::new(e.to_string()))?;
  let generation = youtrack.generation.fetch_add(1, Ordering::SeqCst) + 1;
  let answer = YouTrackSignIn {
    authorize_url: pending.authorize_url.clone(),
  };
  *youtrack.sign_in.lock().unwrap() = Some((generation, base, pending));
  Ok(answer)
}

/// Waits until the browser came back from the Hub page of `start_youtrack_sign_in`, then keeps the tokens and tells
/// every window; the user. A newer sign-in or `cancel_youtrack_sign_in` ends it with "canceled".
#[tauri::command]
#[instrument(skip(app, youtrack))]
#[specta::specta]
pub async fn wait_youtrack_sign_in(app: AppHandle, youtrack: State<'_, YouTrackState>) -> Result<YouTrackUser, CommandError> {
  let (generation, base, pending) = youtrack
    .sign_in
    .lock()
    .unwrap()
    .take()
    .ok_or_else(|| CommandError::new("No YouTrack sign-in is in progress"))?;
  let hub = pending.hub.clone();
  let client = &youtrack.service.client;
  let tokens = finish_sign_in(client, pending, || youtrack.generation.load(Ordering::SeqCst) != generation)
    .await
    .map_err(|e| CommandError::new(e.to_string()))?;
  let user = fetch_me(client, &base, &tokens.access_token).await.map_err(|e| CommandError::new(e.message()))?;
  let refresh = tokens.refresh_token.map(|refresh_token| RefreshGrant {
    hub_url: hub.url,
    service_id: hub.service_id,
    refresh_token,
  });
  youtrack
    .keep(
      &app,
      StoredToken {
        url: base,
        login: user.login.clone(),
        full_name: user.full_name.clone(),
        access_token: tokens.access_token,
        access_expires_at: tokens.expires_in.map(expires_at),
        refresh,
        created_at: unix_now(),
      },
    )
    .await?;
  info!(login = %user.login, "signed in to YouTrack");
  Ok(user)
}

/// Ends the wait of the sign-in in progress, if any
#[tauri::command]
#[specta::specta]
pub async fn cancel_youtrack_sign_in(youtrack: State<'_, YouTrackState>) -> Result<(), CommandError> {
  youtrack.generation.fetch_add(1, Ordering::SeqCst);
  youtrack.sign_in.lock().unwrap().take();
  Ok(())
}

/// Keeps a token the user created in YouTrack (Profile → Account Security), once YouTrack accepted it
#[tauri::command]
#[instrument(skip(app, youtrack, token))]
#[specta::specta]
pub async fn save_youtrack_token(app: AppHandle, youtrack: State<'_, YouTrackState>, url: String, token: String) -> Result<YouTrackUser, CommandError> {
  let base = normalize_base_url(&url).map_err(CommandError::new)?;
  let token = token.trim().to_string();
  if token.is_empty() {
    return Err(CommandError::new("Paste the token YouTrack created"));
  }
  let user = fetch_me(&youtrack.service.client, &base, &token).await.map_err(|e| CommandError::new(e.message()))?;
  youtrack
    .keep(
      &app,
      StoredToken {
        url: base,
        login: user.login.clone(),
        full_name: user.full_name.clone(),
        access_token: token,
        access_expires_at: None,
        refresh: None,
        created_at: unix_now(),
      },
    )
    .await?;
  Ok(user)
}

/// Removes the token of the app's own sign-in. The Hub keeps the authorization until it is revoked there (Account
/// Security of the YouTrack profile).
#[tauri::command]
#[specta::specta]
pub async fn sign_out_of_youtrack(app: AppHandle, youtrack: State<'_, YouTrackState>) -> Result<(), CommandError> {
  youtrack.generation.fetch_add(1, Ordering::SeqCst);
  if let Some(file) = youtrack.service.token_file().cloned() {
    run_blocking(move || remove_private(&file).map_err(|e| CommandError::new(format!("Cannot remove the YouTrack token: {e}")))).await?;
  }
  youtrack.service.forget().await;
  app.emit(YOUTRACK_AUTH_CHANGED_EVENT, ())?;
  Ok(())
}

/// The issue of `key` for the popover, from the YouTrack the token is for
#[tauri::command]
#[specta::specta]
pub async fn get_youtrack_issue(youtrack: State<'_, YouTrackState>, key: String) -> Result<YouTrackIssueAnswer, CommandError> {
  Ok(youtrack.service.issue(&key).await)
}

/// Sets the state field of the issue (Mark Fixed, and its undo with `expected`, the value the change set); the
/// previous value
#[tauri::command]
#[instrument(skip(youtrack))]
#[specta::specta]
pub async fn set_youtrack_issue_state(youtrack: State<'_, YouTrackState>, key: String, state: String, expected: Option<String>) -> Result<YouTrackStateChange, CommandError> {
  youtrack.service.set_state(&key, &state, expected.as_deref()).await.map_err(CommandError::new)
}
