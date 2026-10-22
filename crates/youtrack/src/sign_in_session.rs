//! The app's own sign-in to YouTrack (spec/repository/youtrack.spec.md): one browser sign-in at a time through the Hub
//! of the YouTrack (`sign_in.rs`), from the Hub page to the stored token; a pasted token; a cancel and a sign-out.
//! Shared by the app's commands, which only tell the windows (`youtrack-auth-changed`), and the E2E test server, which
//! hosts the same session and service behind a fake YouTrack (`fake_youtrack.rs`).

use crate::issue::fetch_me;
use crate::service::{YouTrackService, expires_at};
use crate::sign_in::{PendingSignIn, finish_sign_in, start_sign_in};
use crate::token_store::{RefreshGrant, StoredToken};
use crate::url::normalize_base_url;
use std::sync::Mutex;
use sync_types::youtrack::{YouTrackSignIn, YouTrackUser};
use sync_utils::secret_store::{remove_secret, write_secret_json};
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tracing::{info, instrument};

/// The browser sign-in of one app; the tokens go to the token file of its service
#[derive(Default)]
pub struct YouTrackSignInSession {
  /// Under one lock: a start, a wait and a cancel each see the state whole, so nothing stays pending that no wait
  /// can end
  state: Mutex<SignInState>,
}

#[derive(Default)]
struct SignInState {
  /// A newer sign-in, a cancel or a sign-out ends the wait of an older one
  generation: u64,
  /// The sign-in waiting for the browser, with its generation and its YouTrack; `wait` takes it
  pending: Option<(u64, String, PendingSignIn)>,
  /// The generation of the sign-in started and not ended yet; 0 for none
  in_progress: u64,
}

impl YouTrackSignInSession {
  fn state(&self) -> std::sync::MutexGuard<'_, SignInState> {
    self.state.lock().unwrap()
  }

  /// A sign-in was started and has not ended: it waits for `wait`, or `wait` waits for the browser
  pub fn is_pending(&self) -> bool {
    self.state().in_progress != 0
  }

  /// Starts the browser sign-in to the YouTrack at `url`, which replaces the one before: the Hub page to open. `wait`
  /// waits for the browser to come back.
  #[instrument(skip(self, service))]
  pub async fn start(&self, service: &YouTrackService, url: &str) -> Result<YouTrackSignIn, String> {
    let base = normalize_base_url(url)?;
    let pending = start_sign_in(&service.client, &base).await.map_err(|e| e.to_string())?;
    let answer = YouTrackSignIn {
      authorize_url: pending.authorize_url.clone(),
    };
    let mut state = self.state();
    state.generation += 1;
    let generation = state.generation;
    state.pending = Some((generation, base, pending));
    state.in_progress = generation;
    Ok(answer)
  }

  /// Waits until the browser came back from the Hub page of `start`, then keeps the tokens in the token file and
  /// forgets what the service knew; the user. A newer sign-in, `cancel` or `sign_out` ends it with "canceled".
  #[instrument(skip(self, service))]
  pub async fn wait(&self, service: &YouTrackService) -> Result<YouTrackUser, String> {
    let (generation, base, pending) = self.state().pending.take().ok_or("No YouTrack sign-in is in progress")?;
    let signed_in = self.finish(service, generation, base, pending).await;
    // only this sign-in ends here: a newer one stays in progress
    let mut state = self.state();
    if state.in_progress == generation {
      state.in_progress = 0;
    }
    signed_in
  }

  async fn finish(&self, service: &YouTrackService, generation: u64, base: String, pending: PendingSignIn) -> Result<YouTrackUser, String> {
    let hub = pending.hub.clone();
    let client = &service.client;
    let tokens = finish_sign_in(client, pending, || self.state().generation != generation).await.map_err(|e| e.to_string())?;
    let user = fetch_me(client, &base, &tokens.access_token).await.map_err(|e| e.message())?;
    let refresh = tokens.refresh_token.map(|refresh_token| RefreshGrant {
      hub_url: hub.url,
      service_id: hub.service_id,
      refresh_token,
    });
    keep(
      service,
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
  pub fn cancel(&self) {
    let mut state = self.state();
    state.generation += 1;
    state.pending = None;
    state.in_progress = 0;
  }

  /// Keeps a token the user created in YouTrack (Profile → Account Security), once YouTrack accepted it; the user
  #[instrument(skip(self, service, token))]
  pub async fn save_token(&self, service: &YouTrackService, url: &str, token: &str) -> Result<YouTrackUser, String> {
    let base = normalize_base_url(url)?;
    let token = token.trim().to_string();
    if token.is_empty() {
      return Err("Paste the token YouTrack created".to_string());
    }
    let user = fetch_me(&service.client, &base, &token).await.map_err(|e| e.message())?;
    keep(
      service,
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

  /// Removes the token of the app's own sign-in and ends the wait of a sign-in in progress. The Hub keeps the
  /// authorization until it is revoked there (Account Security of the YouTrack profile).
  #[instrument(skip(self, service))]
  pub async fn sign_out(&self, service: &YouTrackService) -> Result<(), String> {
    self.cancel();
    if let Some(file) = service.token_file().cloned() {
      tokio::task::spawn_blocking(in_current_span(move || remove_secret(&file)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Cannot remove the YouTrack token: {e}"))?;
    }
    service.forget().await;
    Ok(())
  }
}

/// Writes `token` to the token file of `service`, which reads it again
async fn keep(service: &YouTrackService, token: StoredToken) -> Result<(), String> {
  let file = service.token_file().cloned().ok_or("The app has no data directory to keep the YouTrack token in")?;
  tokio::task::spawn_blocking(in_current_span(move || write_secret_json(&file, &token)))
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("Cannot keep the YouTrack token: {e}"))?;
  service.forget().await;
  Ok(())
}
