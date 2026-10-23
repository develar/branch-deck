//! The `youtrack` CLI signed in with the app's YouTrack (spec/repository/youtrack.spec.md#the-youtrack-cli): the one-shot
//! sign-in for it (`YouTrackSignInSession::start_for_cli`) gives an access token that may ask Hub, which makes a
//! permanent token named `TOKEN_NAME` for the YouTrack alone; the CLI takes it on its stdin
//! (`cli_tools::youtrack_login`). The app keeps the id of the token it made, never the token, and revokes that one when
//! it hands over the next; a token the user made stays.

use cli_tools::environment::ToolEnvironment;
use cli_tools::youtrack_login::sign_in_youtrack_with_token;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use sync_utils::span::in_current_span;
use tracing::{instrument, warn};
use youtrack::hub::{create_permanent_token, delete_permanent_token};
use youtrack::service::YouTrackService;
use youtrack::sign_in_session::CliGrant;

/// The name of the permanent token in the Hub profile, where the user sees and revokes it
pub const TOKEN_NAME: &str = "Branch Deck: youtrack CLI";

/// The token the app made for the CLI: its YouTrack and its id in Hub (no secret)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandedToken {
  pub url: String,
  pub id: String,
}

pub fn handed_token_file(app_data_dir: &Path) -> PathBuf {
  app_data_dir.join("youtrack-cli-token.json")
}

/// Makes the permanent token of `grant` and signs the CLI in with it; the toast: "Signed youtrack in to yt.example as
/// jane". A CLI that refuses it gets its token revoked again; the token the app made before is revoked once the CLI
/// took the new one
#[instrument(skip_all, fields(url = %grant.base))]
pub async fn hand_token_to_cli(service: &YouTrackService, grant: CliGrant, environment: ToolEnvironment, kept: Option<&Path>) -> Result<String, String> {
  let CliGrant { base, hub, access_token } = grant;
  if environment.find_tool(cli_tools_tool()).is_none() {
    return Err("youtrack is not installed".to_string());
  }
  let token = create_permanent_token(&service.client, &hub.url, &access_token, TOKEN_NAME, &[&hub.service_id])
    .await
    .map_err(|e| format!("Hub made no token: {}", e.message()))?;
  let (url, secret) = (base.clone(), token.token.clone());
  let signed_in = tokio::task::spawn_blocking(in_current_span(move || sign_in_youtrack_with_token(&environment, &url, &secret)))
    .await
    .unwrap_or_else(|e| Err(format!("the sign-in of youtrack panicked: {e}")));
  let login = match signed_in {
    Ok(login) => login,
    Err(error) => {
      revoke(service, &hub.url, &access_token, &token.id).await;
      return Err(error);
    }
  };
  if let Some(file) = kept {
    let before = std::fs::read(file).ok().and_then(|bytes| serde_json::from_slice::<HandedToken>(&bytes).ok());
    if let Some(before) = before.filter(|before| before.url == base && before.id != token.id) {
      revoke(service, &hub.url, &access_token, &before.id).await;
    }
    let handed = HandedToken { url: base.clone(), id: token.id };
    if let Err(e) = serde_json::to_vec(&handed).map_err(std::io::Error::other).and_then(|bytes| std::fs::write(file, bytes)) {
      warn!(error = %e, "can't keep the id of the token of the youtrack CLI");
    }
  }
  let host = base.split("://").nth(1).unwrap_or(&base).to_string();
  Ok(format!("Signed youtrack in to {host} as {login}"))
}

const fn cli_tools_tool() -> domain_types::cli_tools::CliTool {
  domain_types::cli_tools::CliTool::Youtrack
}

async fn revoke(service: &YouTrackService, hub_url: &str, access_token: &str, id: &str) {
  if let Err(e) = delete_permanent_token(&service.client, hub_url, access_token, id).await {
    warn!(error = %e.message(), id, "can't revoke the token of the youtrack CLI");
  }
}
