use bd_cli::context::Environment;
use bd_cli::install::{InstallArgs, bd_command, install};
use model_tauri::command_error::CommandError;
use std::ffi::OsString;
use tracing::{instrument, warn};

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

/// The command line that runs this app's bd, for the prompts the app copies: the `bd` symlink in `~/.local/bin` when it
/// points at this app, else the app binary with `cli` (on Windows `bd.exe` next to it), quoted for a shell. Writes
/// nothing and needs no install, so the bd of a prompt is always the version of the app that wrote it
#[tauri::command]
#[specta::specta]
#[instrument]
pub fn agent_cli_command() -> Result<String, CommandError> {
  bd_command(&Environment::from_process())
    .map(|command| command.for_shell())
    .map_err(|e| CommandError::new(e.message))
}

/// `PATH` of an interactive login shell of the user (`$SHELL`), where `~/.local/bin` is usually added; `None` on Windows
/// or when the shell fails
fn login_shell_path() -> Option<OsString> {
  if cfg!(windows) {
    return None;
  }
  let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
  let output = std::process::Command::new(&shell)
    .args(["-i", "-l", "-c", "printf '\\n%s' \"$PATH\""])
    .stdin(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .output()
    .inspect_err(|e| warn!(error = %e, "can't run the login shell for PATH"))
    .ok()?;
  // the rc files may print before it: PATH is the last line
  let stdout = String::from_utf8_lossy(&output.stdout);
  stdout.rsplit('\n').next().filter(|path| output.status.success() && !path.is_empty()).map(OsString::from)
}
