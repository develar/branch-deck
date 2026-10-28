use app_api::operation::{operation_now, run_blocking};
use bd_cli::install::{InstallArgs, install, install_status};
use bd_core::bd_command::bd_command;
use bd_core::context::Environment;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use lease_pool::agent_workspace::agent_defaults;
use sync_utils::child_process::login_shell_path;
use tauri::State;
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
  /// How agents work where a repository has no value of its own (the global git config)
  pub agents: AgentRules,
}

/// The two rules of `bd workspace` (Settings → Agents)
#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
pub struct AgentRules {
  /// `branchdeck.agentLease`: in a worktree of their own, each commit landed on the user's branch
  pub lease: bool,
  /// `branchdeck.agentSubjectBranch`: `(branch)` at the start of each subject
  pub subject_branch: bool,
}

#[tauri::command]
#[specta::specta]
#[instrument(skip_all)]
pub async fn install_agent_cli(git_executor: State<'_, GitCommandExecutor>) -> Result<AgentCliInstall, ApiError> {
  let git = (*git_executor).clone();
  run_blocking(move || {
    let mut environment = Environment::from_process();
    // an app started from the Finder has a minimal PATH; the user's shell decides whether `bd` runs there
    if let Some(path) = login_shell_path() {
      environment.path = Some(path);
    }
    let output = install(&environment, &InstallArgs::default()).map_err(|e| ApiError::new(e.code, e.message))?;
    let agents = output.agents.unwrap_or_else(|| agent_defaults(Some(&git)));
    Ok::<_, ApiError>(AgentCliInstall {
      bd: output.bd,
      on_path: output.on_path,
      skills: output.skills,
      agents: AgentRules {
        lease: agents.lease,
        subject_branch: agents.subject_branch,
      },
    })
  })
  .await
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
pub fn agent_cli_command(repository_path: String) -> Result<AgentCliCommand, ApiError> {
  operation_now(|| {
    let command = bd_command(&Environment::from_process()).map_err(|e| ApiError::new(e.code, e.message))?;
    Ok(AgentCliCommand {
      command: command.for_shell_in(&repository_path),
      powershell: cfg!(windows),
    })
  })
}

/// Whether the agent skill is installed (Settings → Agents)
#[derive(Debug, serde::Serialize, specta::Type)]
pub struct AgentCliStatus {
  /// The command the skills name; `null` while no skill is installed
  pub bd: Option<String>,
  /// The skill files that exist
  pub skills: Vec<String>,
}

/// The skill files of `bd install` that exist, without writing anything
#[tauri::command]
#[specta::specta]
#[instrument]
pub fn agent_cli_status() -> AgentCliStatus {
  let status = install_status(&Environment::from_process());
  AgentCliStatus {
    bd: status.bd,
    skills: status.skills,
  }
}
