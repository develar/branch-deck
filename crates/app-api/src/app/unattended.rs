//! Unattended Mode of the app (openspec/specs/repository/unattended-mode/spec.md): its switch in the title bar and the
//! command palette (`set_unattended_mode`), and the requests its hook denied (`get_unattended_mode`), which the toast of
//! a switch off and the list of Denied Requests show. The setting goes through the one writer of the settings, as
//! `brd unattended` writes it (`app_settings::unattended::set_unattended`), which saves it before the switch answers,
//! and the windows hear the change, with the hook in the settings of Claude Code only while the mode is on, where the
//! agent CLI of the host says (`AgentCliHost::unattended_hook`); the journal of denials lies beside the settings in the
//! app data directory of the host.

use crate::api_error::api_error;
use crate::context::AppContext;
use crate::events::settings_changed;
use crate::operation::{operation, run_blocking};
use crate::transport::Call;
use app_settings::unattended::{UnattendedError, denials_file, read_denials, set_unattended};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::unattended::UnattendedMode;
use tracing::instrument;

/// Turns Unattended Mode on or off for the window of `call`, its label the origin of the change, which every window
/// hears; answers the mode with the requests denied since it was last turned on: none after a switch on, the night's
/// after a switch off. On adds the hook to the settings of Claude Code first and stays off when it cannot
/// (`install_refused`); off removes it after, a removal that fails being the `warning` of a switch off
#[instrument(skip_all, fields(window = %call.window, on))]
pub async fn set_unattended_mode(ctx: &AppContext, call: &Call, on: bool) -> Result<UnattendedMode, ApiError> {
  operation(async move {
    let journal = denials_file(ctx.settings.directory());
    let hook = ctx.desktop(&call.window).agent_cli.unattended_hook();
    // the switch answers once the setting is saved, as the hook reads the file: a failed save is an error, not a switch
    let (settings, file, origin) = (std::sync::Arc::clone(&ctx.settings), journal.clone(), call.window.as_str().to_owned());
    let switch = run_blocking(move || set_unattended(&settings, &file, on, &origin, hook.as_ref()).map_err(|e| switch_error(on, e))).await?;
    if let Some(change) = &switch.change {
      settings_changed(&*ctx.events, change);
    }
    Ok(UnattendedMode {
      on,
      changed: switch.changed,
      warning: switch.warning,
      denials: read_denials(&journal),
    })
  })
  .await
}

/// The error of a switch that changed nothing: `install_refused` when the hook cannot go into the settings of Claude
/// Code, which the toast says as it is
fn switch_error(on: bool, error: UnattendedError) -> ApiError {
  match error {
    UnattendedError::NoHookFile | UnattendedError::Hook(_) => ApiError::new(ErrorCode::InstallRefused, upper_first(&error.to_string())),
    UnattendedError::Journal(_) | UnattendedError::Settings(_) => {
      api_error(anyhow::Error::new(error).context(if on { "Cannot turn on Unattended Mode" } else { "Cannot turn off Unattended Mode" }))
    }
  }
}

/// `text` with its first letter upper case, as the sentence of a toast
fn upper_first(text: &str) -> String {
  let mut chars = text.chars();
  chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// Unattended Mode as it is: whether it is on, and the requests denied since it was last turned on
#[instrument(skip_all, fields(window = %call.window))]
pub async fn get_unattended_mode(ctx: &AppContext, call: &Call) -> Result<UnattendedMode, ApiError> {
  operation(async move {
    Ok(UnattendedMode {
      on: ctx.settings.app().unattended,
      changed: false,
      warning: None,
      denials: read_denials(&denials_file(ctx.settings.directory())),
    })
  })
  .await
}
