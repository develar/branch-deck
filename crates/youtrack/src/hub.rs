//! The permanent tokens of Hub, made and revoked with an access token that may ask Hub itself
//! (`SignInPurpose::PermanentToken`): the token the app hands to the `youtrack` CLI
//! (spec/repository/youtrack.spec.md#the-youtrack-cli). Only the token the app made is ever revoked.

use crate::client::{YouTrackClient, YouTrackError};
use crate::url::url_of;
use serde::Deserialize;
use tracing::instrument;

/// A permanent token Hub made: its id, which revokes it, and the token itself (`perm-…`)
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct PermanentToken {
  pub id: String,
  pub token: String,
}

impl std::fmt::Debug for PermanentToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PermanentToken").field("id", &self.id).finish_non_exhaustive()
  }
}

/// A permanent token named `name` of the user of `access`, for the services `scope` (the YouTrack's)
#[instrument(skip(client, access))]
pub async fn create_permanent_token(client: &YouTrackClient, hub_url: &str, access: &str, name: &str, scope: &[&str]) -> Result<PermanentToken, YouTrackError> {
  let mut url = url_of(hub_url, &["api", "rest", "users", "me", "permanenttokens"]).map_err(YouTrackError::InvalidResponse)?;
  url.query_pairs_mut().append_pair("fields", "id,token");
  let scope: Vec<serde_json::Value> = scope.iter().map(|id| serde_json::json!({ "id": id })).collect();
  client.post(&url, access, &serde_json::json!({ "name": name, "scope": scope })).await
}

/// Revokes the permanent token `id` of the user of `access`
#[instrument(skip(client, access))]
pub async fn delete_permanent_token(client: &YouTrackClient, hub_url: &str, access: &str, id: &str) -> Result<(), YouTrackError> {
  let url = url_of(hub_url, &["api", "rest", "users", "me", "permanenttokens", id]).map_err(YouTrackError::InvalidResponse)?;
  client.delete(&url, access).await
}
