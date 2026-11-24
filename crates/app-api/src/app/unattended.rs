//! Unattended Mode of the app (openspec/specs/repository/unattended-mode/spec.md): its switch in the title bar and the
//! command palette (`set_unattended_mode`), and the requests its hook denied (`get_unattended_mode`), which the toast of
//! a switch off and the list of Denied Requests show. The setting goes through the one writer of the settings, as
//! `brd unattended` writes it (`app_settings::unattended::set_unattended`), and the windows hear the change; the journal of
//! denials lies beside the settings in the app data directory of the host, the hook state is the agent CLI's.

use crate::api_error::api_error;
use crate::context::AppContext;
use crate::events::settings_changed;
use crate::operation::operation;
use crate::transport::Call;
use app_settings::unattended::{denials_file, read_denials, set_unattended};
use domain_types::error::ApiError;
use domain_types::unattended::UnattendedMode;
use tracing::instrument;

/// Turns Unattended Mode on or off for the window of `call`, its label the origin of the change, which every window
/// hears; answers the mode with the requests denied since it was last turned on: none after a switch on, the night's
/// after a switch off
#[instrument(skip_all, fields(window = %call.window, on))]
pub async fn set_unattended_mode(ctx: &AppContext, call: &Call, on: bool) -> Result<UnattendedMode, ApiError> {
  operation(async move {
    let journal = denials_file(ctx.settings.directory());
    let switch = set_unattended(&ctx.settings, &journal, on, call.window.as_str()).map_err(|e| api_error(anyhow::Error::new(e).context("Cannot change Unattended Mode")))?;
    if let Some(change) = &switch.change {
      settings_changed(&*ctx.events, change);
    }
    let status = ctx.desktop(&call.window).agent_cli.status();
    Ok(UnattendedMode {
      on,
      changed: switch.changed,
      hook: status.unattended_hook,
      installable: status.installable,
      denials: read_denials(&journal),
    })
  })
  .await
}

/// Unattended Mode as it is: whether it is on, whether its hook is installed, and the requests denied since it was last
/// turned on
#[instrument(skip_all, fields(window = %call.window))]
pub async fn get_unattended_mode(ctx: &AppContext, call: &Call) -> Result<UnattendedMode, ApiError> {
  operation(async move {
    let status = ctx.desktop(&call.window).agent_cli.status();
    Ok(UnattendedMode {
      on: ctx.settings.app().unattended,
      changed: false,
      hook: status.unattended_hook,
      installable: status.installable,
      denials: read_denials(&denials_file(ctx.settings.directory())),
    })
  })
  .await
}
