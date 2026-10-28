//! The app on the desktop, beside its windows (`windows.rs`): the folder dialog that picks a repository
//! (`browse_repository`), the native menu's Auto-sync on Focus (`set_auto_sync_menu_checked`), the restart that applies
//! the system title bar (`restart_app`, spec/repository/title-bar.spec.md) and the updater (`check_for_updates`,
//! `get_update_status`, `install_update`). What the system does is the host's (`Desktop` of `transport.rs`).

use crate::api_error::api_error;
use crate::context::AppContext;
use crate::events::settings_changed;
use crate::operation::{operation, operation_now, run_blocking};
use crate::transport::Call;
use domain_types::error::ApiError;
use domain_types::settings::{APP_SETTINGS_KEY, SettingsPatch};
use domain_types::update::UpdateInfo;
use std::sync::Arc;
use sync_core::repository_validation::{BrowseResult, validate_and_create_result};
use tracing::instrument;

/// Opens a native file dialog to browse for a git repository: the folder picked, checked as a repository, or no path
/// when the user canceled
#[instrument(skip_all)]
pub async fn browse_repository(ctx: &AppContext) -> Result<BrowseResult, ApiError> {
  let dialog = Arc::clone(&ctx.desktop.dialog);
  // both the modal dialog and the file system checks block, off the async runtime
  run_blocking(move || {
    let result = match dialog.pick_folder("Select Project Repository") {
      Some(path) => validate_and_create_result(path),
      // no error: the user canceled
      None => BrowseResult {
        path: None,
        valid: false,
        error: None,
      },
    };
    Ok::<_, ApiError>(result)
  })
  .await
}

/// Checks Auto-sync on Focus of the native menu as the setting is, whichever window changed it: the main window calls it
/// for every value of `appSettings.autoSyncOnFocus` (`useMenuSync`). Without a native menu (a custom title bar on
/// Windows and Linux, whose main menu reads the setting itself) there is nothing to check.
#[instrument(skip_all, fields(checked = checked))]
pub fn set_auto_sync_menu_checked(ctx: &AppContext, _call: &Call, checked: bool) -> Result<(), ApiError> {
  operation_now(|| ctx.desktop.menu.set_auto_sync_checked(checked))
}

/// Restarts the app with "Use the system title bar" at `system_title_bar`: the title bar is decided once per process
/// (`init_title_bar` of the app). The value goes through the writer of the settings, which saves it now, before the
/// restart: the new process reads the file.
#[instrument(skip_all, fields(system_title_bar = system_title_bar))]
pub fn restart_app(ctx: &AppContext, call: &Call, system_title_bar: bool) -> Result<(), ApiError> {
  operation_now(|| {
    let fields = serde_json::Map::from_iter([("systemTitleBar".to_owned(), serde_json::Value::Bool(system_title_bar))]);
    let patch = SettingsPatch::Fields {
      key: APP_SETTINGS_KEY.to_owned(),
      fields,
    };
    let change = ctx
      .settings
      .set(patch, call.window.as_str())
      .map_err(|e| api_error(anyhow::Error::new(e).context("Cannot change the settings")))?;
    settings_changed(&*ctx.events, &change);
    ctx.settings.flush().map_err(|e| api_error(anyhow::Error::new(e).context("Cannot save the settings")))?;
    Err(ctx.desktop.restart.restart())
  })
}

/// Checks for an update of the app and downloads the one it finds
#[instrument(skip_all)]
pub async fn check_for_updates(ctx: &AppContext) -> Result<UpdateInfo, ApiError> {
  operation(async move { ctx.desktop.updater.check().await }).await
}

/// The state of the update since the last check
#[instrument(skip_all)]
pub async fn get_update_status(ctx: &AppContext) -> Result<UpdateInfo, ApiError> {
  operation(async move { ctx.desktop.updater.status() }).await
}

/// Installs the downloaded update and restarts the app
#[instrument(skip_all)]
pub async fn install_update(ctx: &AppContext) -> Result<(), ApiError> {
  operation(async move { ctx.desktop.updater.install().await }).await
}
