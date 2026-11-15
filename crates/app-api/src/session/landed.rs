//! The hide of the Landed island (openspec/specs/repository/landed-commits/spec.md): a sync hid the branches that landed
//! up to a time after the user saw them, kept for the repository (`landedHiddenUntil`, a field of its lease,
//! `app_settings::service::LEASE_FIELDS`) under the lease of the view it was asked on

use crate::context::AppContext;
use crate::events::settings_changed;
use crate::operation::run_blocking;
use crate::transport::Call;
use app_settings::projects::hide_landed as hide_landed_in;
use domain_types::error::ApiError;
use repository_model::session::SessionKey;
use serde::Deserialize;
use std::sync::Arc;
use tracing::debug;

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HideLandedParams {
  pub repository_path: String,
  /// The island hides what landed up to here (`landedAt`, Unix seconds)
  pub landed_at: u32,
  /// The epoch of the view the island showed (`Version.epoch`): a view of another lease hides nothing
  #[serde(default)]
  #[specta(optional)]
  pub epoch: Option<u32>,
}

/// Records that the Landed island of the session of `key` hides what landed up to `landed_at`, under the lease of the
/// view it was asked on (`Sessions::lease_of_view`, `app_settings::projects::hide_landed`): nothing for a view of
/// another lease, as the report of the next lease is its own; the windows hear the change
pub async fn hide_landed(ctx: &AppContext, _call: &Call, key: SessionKey, params: HideLandedParams) -> Result<(), ApiError> {
  let Ok(lease) = ctx.sessions.lease_of_view(&key, params.epoch).await else {
    debug!("the view of the hide is of another lease: nothing hidden");
    return Ok(());
  };
  let (settings, path) = (Arc::clone(&ctx.settings), key.worktree);
  let change = run_blocking(move || {
    hide_landed_in(&settings, &path, &lease, u64::from(params.landed_at)).map_err(|error| ApiError::internal(format!("Cannot hide the landed branches: {error}")))
  })
  .await?;
  if let Some(change) = change {
    settings_changed(&*ctx.events, &change);
  }
  Ok(())
}
