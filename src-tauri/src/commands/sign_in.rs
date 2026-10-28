//! The sign-ins of the app's accounts (spec/repository/github-sign-in.spec.md): commands of the app, not intents of a
//! session, since an account has no repository (docs/design/rust-domain-authority.md, "Решение 2026-10-28"). They run
//! on `remote_facts::sign_in`: one sign-in of each provider at a time, a second call joins it, its events on the channel
//! of each call. A sign-in or a sign-out that changed an account has every session read the fact `auth` again (and,
//! after GitHub, the pull requests and the review requests) and tells every window `auth-changed`, which a window
//! without a session reads `get_auth_status` again on.

use crate::commands::github::GitHubState;
use crate::commands::youtrack::YouTrackState;
use app_api::api_error::api_error;
use app_api::context::AppContext;
use app_api::operation::operation;
use cli_tools::environment::ToolEnvironment;
use domain_types::error::ApiError;
use domain_types::session::{FactFamily, FactRefreshReason};
use domain_types::sign_in::{Account, AuthStatus, SignIn, SignInDone, SignInEvent, SignInProvider};
use github::device_flow::GITHUB_OAUTH_CLIENT_ID;
use github::service::GitHubService;
use remote_facts::sign_in::{SignInHost, SignIns};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{instrument, warn};
use youtrack::service::YouTrackService;

/// Sent to every window when a sign-in or a sign-out changed an account: a window without a session reads the status
/// again (`get_auth_status`); the sessions read their fact `auth` themselves
pub const AUTH_CHANGED_EVENT: &str = "auth-changed";

/// The sign-ins of the app
pub struct SignInsState(pub SignIns);

/// What the sign-ins of the app run with: the services of the app, the OAuth app of the build, the `youtrack` CLI of the
/// user, and the sessions and windows a change of an account tells
pub struct AppSignIns {
  pub app: AppHandle,
}

impl SignInHost for AppSignIns {
  fn github(&self) -> Arc<GitHubService> {
    Arc::clone(&self.app.state::<GitHubState>().service)
  }

  fn github_client_id(&self) -> Option<String> {
    GITHUB_OAUTH_CLIENT_ID.map(str::to_owned)
  }

  fn youtrack(&self) -> Arc<YouTrackService> {
    Arc::clone(&self.app.state::<YouTrackState>().service)
  }

  fn youtrack_cli(&self) -> (ToolEnvironment, Option<PathBuf>) {
    (ToolEnvironment::from_process(), self.app.state::<YouTrackState>().handed_token.clone())
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  fn changed(&self, account: Account) {
    tell_auth_changed(&self.app, account);
  }
}

/// `account` changed: every session reads its sign-ins again, after GitHub its pull requests and review requests too,
/// and every window hears `auth-changed`
pub fn tell_auth_changed(app: &AppHandle, account: Account) {
  let ctx = app.state::<AppContext>();
  let sessions = &ctx.sessions;
  sessions.refresh_all(FactFamily::Auth, FactRefreshReason::AfterWrite);
  if account == Account::GitHub {
    sessions.refresh_all(FactFamily::PullRequests, FactRefreshReason::Manual);
    sessions.refresh_all(FactFamily::ReviewRequests, FactRefreshReason::Manual);
  }
  if let Err(error) = app.emit(AUTH_CHANGED_EVENT, ()) {
    warn!(%error, "cannot tell the windows that the sign-in changed");
  }
}

/// Signs in by `request`, or joins the sign-in of its provider that runs: its events on `events`, `done` last; a cancel
/// ends it with the code `canceled`
#[tauri::command]
#[instrument(skip_all, fields(provider = ?request.provider()))]
#[specta::specta]
pub async fn sign_in(sign_ins: State<'_, SignInsState>, request: SignIn, events: Channel<SignInEvent>) -> Result<SignInDone, ApiError> {
  operation(async move {
    let tell = |event: SignInEvent| {
      // a window that went away hears nothing; the sign-in goes on for the others
      let _ = events.send(event);
    };
    sign_ins.0.sign_in(request, &tell).await
  })
  .await
}

/// Ends the sign-in of `provider` that runs, if any: its calls answer `canceled`
#[tauri::command]
#[instrument(skip(sign_ins))]
#[specta::specta]
pub async fn cancel_sign_in(sign_ins: State<'_, SignInsState>, provider: SignInProvider) -> Result<(), ApiError> {
  operation(async move {
    sign_ins.0.cancel(provider);
    Ok(())
  })
  .await
}

/// Removes the token of the app's own sign-in to `account`, after a sign-in of it that runs ended as canceled. GitHub
/// and the Hub keep the authorization until it is revoked there
#[tauri::command]
#[instrument(skip(sign_ins))]
#[specta::specta]
pub async fn sign_out(sign_ins: State<'_, SignInsState>, account: Account) -> Result<(), ApiError> {
  operation(async move { sign_ins.0.sign_out(account).await }).await
}

/// The sign-ins of both accounts and where their tokens come from: for a window without a session (Settings); a window
/// of a repository has them as the fact `auth` of its session
#[tauri::command]
#[instrument(skip(sign_ins))]
#[specta::specta]
pub async fn get_auth_status(sign_ins: State<'_, SignInsState>) -> Result<AuthStatus, ApiError> {
  operation(async move { Ok(sign_ins.0.status().await) }).await
}
