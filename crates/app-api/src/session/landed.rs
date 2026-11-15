//! The hide of the Landed island (openspec/specs/repository/landed-commits/spec.md): a sync hid the branches that landed
//! up to a time after the user saw them, kept for the repository (`landedHiddenUntil`, a field of its lease,
//! `app_settings::service::LEASE_FIELDS`) under the lease of the view it was asked on

use crate::context::AppContext;
use crate::events::settings_changed;
use crate::operation::run_blocking;
use crate::transport::Call;
use app_settings::projects::hide_landed as hide_landed_in;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::settings::SettingsChange;
use domain_types::worktree_epoch::WorktreeEpoch;
use operations::witness::lease_read_again_of;
use repository_model::session::{AnotherLease, SessionKey};
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
/// view it was asked on (`Sessions::lease_of_view`, `hide_under`); the windows hear the change
///
/// # Errors
/// `lease_read_again` of `hide_under`
pub async fn hide_landed(ctx: &AppContext, _call: &Call, key: SessionKey, params: HideLandedParams) -> Result<(), ApiError> {
  let of_view = ctx.sessions.lease_of_view(&key, params.epoch).await;
  let (settings, path) = (Arc::clone(&ctx.settings), key.worktree);
  let change = run_blocking(move || hide_under(&settings, &path, of_view, u64::from(params.landed_at))).await?;
  if let Some(change) = change {
    settings_changed(&*ctx.events, &change);
  }
  Ok(())
}

/// The hide of what landed in the worktree at `path` up to `landed_at`, asked on a view of the lease `of_view` names
/// (`Sessions::lease_of_view`): written under that lease (`app_settings::projects::hide_landed`); nothing for a view of
/// another lease, as the report of the next lease is its own
///
/// # Errors
/// `lease_read_again`, nothing written, when the session knew no lease of its slot, which reads one now: the session
/// opens anew under that lease, and the island asks again on its view (`hideAfterSync` of `src/stores/landed.ts`)
pub(crate) fn hide_under(settings: &SettingsService, path: &str, of_view: Result<WorktreeEpoch, AnotherLease>, landed_at: u64) -> Result<Option<SettingsChange>, ApiError> {
  let lease = match of_view {
    Ok(lease) => lease,
    Err(AnotherLease::Held) => {
      debug!("the view of the hide is of another lease: nothing hidden");
      return Ok(None);
    }
    Err(AnotherLease::ReadAgain) => {
      return Err(lease_read_again_of(
        "the hide of the landed branches",
        "the island hides them again on the view of that lease",
      ));
    }
  };
  hide_landed_in(settings, path, &lease, landed_at).map_err(|error| ApiError::internal(format!("Cannot hide the landed branches: {error}")))
}

#[cfg(test)]
#[path = "landed_test.rs"]
mod landed_test;
