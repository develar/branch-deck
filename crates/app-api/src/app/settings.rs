//! The settings of the windows (docs/design/distributed-settings.md): a window changes them through the one writer
//! (`set_setting`, `SettingsService`), every window that gets the key hears the change (`settings_changed`), and the
//! main window's page catches up after it starts to listen (`get_settings_changes`; a sub-window's gets its catch-up
//! from `sub_window_loaded`)

use crate::api_error::api_error;
use crate::context::AppContext;
use crate::events::settings_changed;
use crate::operation::operation;
use crate::transport::Call;
use app_settings::service::SettingsService;
use app_settings::windows::window_changes_since;
use domain_types::error::ApiError;
use domain_types::settings::{SettingsChange, SettingsPatch};
use lease_pool::epoch::worktree_epoch;
use serde::Deserialize;
use specta::Type;
use tracing::instrument;

/// Gives the writer of the settings of a host the epochs of the worktrees (`lease_pool::epoch::worktree_epoch`): an entry
/// of `repositoryState` of a slot of a lease pool is of the lease it was written under, and the writer forgets one of an
/// earlier lease (`SettingsService::set_worktree_epochs`). The app, `brd mcp` and the E2E server call it as they open
/// the writer
pub fn give_worktree_epochs(settings: &SettingsService) {
  settings.set_worktree_epochs(|path| worktree_epoch(std::path::Path::new(path)));
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetSettingParams {
  pub patch: SettingsPatch,
}

/// Applies the change of the window of `call` to the settings, its label the origin of the change, and sends it to every
/// window that gets its key; answers the change
#[instrument(skip_all, fields(window = %call.window, key = params.patch.key()))]
pub async fn set_setting(ctx: &AppContext, call: &Call, params: SetSettingParams) -> Result<SettingsChange, ApiError> {
  operation(async move {
    let change = ctx
      .settings
      .set(params.patch, call.window.as_str())
      .map_err(|e| api_error(anyhow::Error::new(e).context("Cannot change the settings")))?;
    settings_changed(&*ctx.events, &change);
    Ok(change)
  })
  .await
}

/// The changes the window of `call` gets that came after `since`, the version of the snapshot its page started from
#[instrument(skip(ctx, call), fields(window = %call.window))]
pub async fn get_settings_changes(ctx: &AppContext, call: &Call, since: u32) -> Result<Vec<SettingsChange>, ApiError> {
  operation(async move { Ok(window_changes_since(&ctx.settings, call.window.as_str(), since)) }).await
}
