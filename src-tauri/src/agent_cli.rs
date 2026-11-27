//! The agent CLI of the app (`AgentCliHost` of app-api): `brd` is this app's own binary, installed by `brd install`
//! (`bd_cli::install`, docs/contributing/agent-cli.md) and named by the prompts the app copies (`bd_core::bd_command`);
//! the same binary is `brd mcp` of an IDE with Air, whose app file `bd_cli::install::install_ide_app` writes

use app_api::app::agent_cli::{AgentCliCommand, AgentCliInstalled, AgentCliStatus, AgentRules, IdeApp};
use app_api::transport::AgentCliHost;
use app_settings::claude_hook::HookTarget;
use bd_cli::install::{InstallArgs, ide_app_status, install, install_ide_app, install_status, uninstall_ide_app};
use bd_cli::unattended::hook_target;
use bd_core::bd_command::{GlobalFlags, bd_command};
use bd_core::context::Environment;
use domain_types::error::ApiError;
use sync_utils::child_process::{find_tool, login_shell_path};
use sync_utils::known_tools::CLAUDE;

/// The `brd` of this app
pub struct AppAgentCli;

impl AgentCliHost for AppAgentCli {
  fn install(&self) -> Result<AgentCliInstalled, ApiError> {
    let mut environment = Environment::from_process();
    // an app started from the Finder has a minimal PATH; the user's shell decides whether `brd` runs there
    if let Some(path) = login_shell_path() {
      environment.path = Some(path);
    }
    let output = install(&environment, &InstallArgs::default()).map_err(|e| ApiError::new(e.code, e.message))?;
    Ok(AgentCliInstalled {
      bd: output.brd,
      on_path: output.on_path,
      skills: output.skills,
      agents: output.agents.map(|agents| AgentRules {
        lease: agents.lease,
        subject_branch: agents.subject_branch,
        pull_request: agents.pull_request,
      }),
    })
  }

  fn command(&self, repository_path: &str) -> Result<AgentCliCommand, ApiError> {
    let command = bd_command(&Environment::from_process()).map_err(|e| ApiError::new(e.code, e.message))?;
    Ok(AgentCliCommand {
      // the app's command line has no options of brd
      command: command.for_shell_in(repository_path, &GlobalFlags::default()),
      powershell: cfg!(windows),
    })
  }

  fn status(&self) -> AgentCliStatus {
    let environment = Environment::from_process();
    let status = install_status(&environment);
    let ide_app = ide_app_status(&environment).map(|app| IdeApp {
      file: app.file,
      current: app.current,
    });
    AgentCliStatus::new(status.bd, status.skills, ide_app, true)
  }

  fn install_ide_app(&self) -> Result<IdeApp, ApiError> {
    let file = install_ide_app(&Environment::from_process()).map_err(|e| ApiError::new(e.code, e.message))?;
    Ok(IdeApp { file, current: true })
  }

  /// The settings of Claude Code of the user and the `brd` of the prompts (`bd_command`): the symlink once it points
  /// here, on Windows `brd.exe` beside the app; the `claude` of the user's login shell too, as an app started from the
  /// Finder has a minimal `PATH`
  fn unattended_hook(&self) -> Option<HookTarget> {
    hook_target(&Environment::from_process()).map(|target| HookTarget {
      claude: find_tool(&CLAUDE),
      ..target
    })
  }

  fn remove_ide_app(&self) -> Result<(), ApiError> {
    uninstall_ide_app(&Environment::from_process()).map(drop).map_err(|e| ApiError::new(e.code, e.message))
  }
}
