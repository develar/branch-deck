use crate::command_error::CommandError;
use bd_cli::install::{InstallArgs, install};
use bd_core::bd_command::bd_command;
use bd_core::context::Environment;
use sync_utils::child_process::login_shell_path;
use tracing::instrument;

/// What the "Install Agent CLI…" menu item installed: `bd install` (docs/guidelines/agent-cli.md)
#[derive(Debug, serde::Serialize, specta::Type)]
pub struct AgentCliInstall {
  /// The command the skills name: the `bd` symlink (macOS, Linux) or `bd.exe` next to the app (Windows)
  pub bd: String,
  /// Whether `bd` resolves to it in the user's shell
  pub on_path: bool,
  /// The skill files written
  pub skills: Vec<String>,
}

#[tauri::command]
#[specta::specta]
#[instrument]
pub async fn install_agent_cli() -> Result<AgentCliInstall, CommandError> {
  tokio::task::spawn_blocking(|| {
    let mut environment = Environment::from_process();
    // an app started from the Finder has a minimal PATH; the user's shell decides whether `bd` runs there
    if let Some(path) = login_shell_path() {
      environment.path = Some(path);
    }
    let output = install(&environment, &InstallArgs::default()).map_err(|e| CommandError::new(e.message))?;
    Ok(AgentCliInstall {
      bd: output.bd,
      on_path: output.on_path,
      skills: output.skills,
    })
  })
  .await
  .map_err(|e| CommandError::new(format!("The install task failed: {e}")))?
}

/// How the prompts the app copies run this app's bd in a repository
#[derive(Debug, serde::Serialize, specta::Type)]
pub struct AgentCliCommand {
  /// bd with `-C <repository>`, quoted for the shell, to run as written: the `bd` symlink in `~/.local/bin` when it
  /// points at this app, else the app binary with `cli` (on Windows `bd.exe` next to it)
  pub command: String,
  /// The shell is PowerShell (Windows), where a quoted path runs with `&`
  pub powershell: bool,
}

/// The bd command of the prompts the app copies. Writes nothing and needs no install, so the bd of a prompt is always
/// the version of the app that wrote it; an error when the app can't tell its binary (no `bd.exe` next to it)
#[tauri::command]
#[specta::specta]
#[instrument]
#[expect(clippy::needless_pass_by_value, reason = "Tauri deserializes a command argument into an owned value")]
pub fn agent_cli_command(repository_path: String) -> Result<AgentCliCommand, CommandError> {
  let command = bd_command(&Environment::from_process()).map_err(|e| CommandError::new(e.message))?;
  Ok(AgentCliCommand {
    command: command.for_shell_in(&repository_path),
    powershell: cfg!(windows),
  })
}
