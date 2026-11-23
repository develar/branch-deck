//! The Landed island seen (openspec/specs/repository/landed-commits/spec.md): a window with the focus shows the island
//! up to a time (`landed_seen`), which its session keeps; a successful sync after it hides what was seen
//! (`domain_policy::landed_island`), and the session records the hide for the repository (`record_hide`:
//! `landedHiddenUntil`, a field of its lease, `app_settings::service::LEASE_FIELDS`) under the lease it opened under

use crate::context::AppContext;
use crate::events::settings_changed;
use crate::transport::Call;
use app_settings::projects::hide_landed;
use domain_types::error::ApiError;
use domain_types::worktree_epoch::WorktreeEpoch;
use repository_model::session::SessionKey;
use serde::Deserialize;
use tracing::{debug, warn};

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LandedSeenParams {
  pub repository_path: String,
  /// The island shows what landed up to here (the newest `landedAt` of its branches, Unix seconds)
  pub landed_at: u32,
  /// The epoch of the view the island shows (`Version.epoch`): a sighting on a view of another epoch is of no report the
  /// session shows
  #[serde(default)]
  #[specta(optional)]
  pub epoch: Option<u32>,
}

/// The window of `call` shows the Landed island of the session of `key` up to `landed_at` with the focus
/// (`Sessions::landed_seen`, by the subscriptions of the page of the call)
///
/// # Errors
/// None: a session that is not open keeps nothing
pub async fn landed_seen(ctx: &AppContext, call: &Call, key: SessionKey, params: LandedSeenParams) -> Result<(), ApiError> {
  ctx
    .sessions
    .landed_seen(&key, ctx.windows.subscriptions_of(call), u64::from(params.landed_at), params.epoch);
  Ok(())
}

/// Records that a successful sync of the session of `key` hid the Landed island up to `landed_at`, the session opened
/// under `epoch` (`SessionEnvironment::hide_landed`): nothing while the worktree holds another lease, as each lease has
/// its own report; the windows hear the change. A failure is only logged: the island shows until the next sync hides
/// what is seen again
pub fn record_hide(ctx: &AppContext, key: &SessionKey, epoch: &WorktreeEpoch, landed_at: u64) {
  match hide_landed(&ctx.settings, &key.worktree, epoch, landed_at) {
    Ok(Some(change)) => settings_changed(&*ctx.events, &change),
    Ok(None) => debug!(worktree = %key.worktree, "the worktree holds another lease than the session: the island hides nothing"),
    Err(error) => warn!(worktree = %key.worktree, %error, "cannot hide the landed branches"),
  }
}
