//! The app's own sign-in to GitHub as Settings shows it: who the stored token (`token_store.rs`) belongs to and which
//! source the token chain would take first. Shared by the app's command and the E2E test server, which hosts the same
//! service behind a fake GitHub.

use crate::client::GITHUB_URL;
use crate::service::GitHubService;
use crate::token_store::read_token;
use std::path::PathBuf;
use sync_types::pull_request::GitHubAuthStatus;
use sync_utils::span::in_current_span;

/// The sign-in of `token_file` (None without an app data directory) and where the token comes from; `client_id` is the
/// OAuth app of the build (`GITHUB_OAUTH_CLIENT_ID`), without which the app offers no sign-in of its own
pub async fn auth_status(service: &GitHubService, token_file: Option<PathBuf>, client_id: Option<&str>) -> GitHubAuthStatus {
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
