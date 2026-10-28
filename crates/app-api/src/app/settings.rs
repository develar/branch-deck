//! The settings of the windows (docs/design/distributed-settings.md): a window changes them through the one writer
//! (`set_setting`, `SettingsService`), every window that gets the key hears the change (`settings_changed`), and the
//! main window's page catches up after it starts to listen (`get_settings_changes`; a sub-window's gets its catch-up
//! from `sub_window_loaded`)

use crate::api_error::api_error;
use crate::context::AppContext;
use crate::events::settings_changed;
use crate::operation::operation;
use crate::transport::Call;
use app_settings::windows::window_changes_since;
use domain_types::error::ApiError;
use domain_types::settings::{SettingsChange, SettingsPatch};
use serde::Deserialize;
use specta::Type;
use tracing::instrument;

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
