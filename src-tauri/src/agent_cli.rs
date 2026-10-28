//! The agent CLI of the app (`AgentCliHost` of app-api): `bd` is this app's own binary, installed by `bd install`
//! (`bd_cli::install`, docs/guidelines/agent-cli.md) and named by the prompts the app copies (`bd_core::bd_command`)

use app_api::app::agent_cli::{AgentCliCommand, AgentCliInstalled, AgentCliStatus, AgentRules};
use app_api::transport::AgentCliHost;
use bd_cli::install::{InstallArgs, install, install_status};
use bd_core::bd_command::bd_command;
use bd_core::context::Environment;
use domain_types::error::ApiError;
use sync_utils::child_process::login_shell_path;

/// The `bd` of this app
pub struct AppAgentCli;

impl AgentCliHost for AppAgentCli {
  fn install(&self) -> Result<AgentCliInstalled, ApiError> {
    let mut environment = Environment::from_process();
    // an app started from the Finder has a minimal PATH; the user's shell decides whether `bd` runs there
    if let Some(path) = login_shell_path() {
      environment.path = Some(path);
    }
    let output = install(&environment, &InstallArgs::default()).map_err(|e| ApiError::new(e.code, e.message))?;
    Ok(AgentCliInstalled {
      bd: output.bd,
      on_path: output.on_path,
      skills: output.skills,
      agents: output.agents.map(|agents| AgentRules {
        lease: agents.lease,
        subject_branch: agents.subject_branch,
      }),
    })
  }

  fn command(&self, repository_path: &str) -> Result<AgentCliCommand, ApiError> {
    let command = bd_command(&Environment::from_process()).map_err(|e| ApiError::new(e.code, e.message))?;
    Ok(AgentCliCommand {
      command: command.for_shell_in(repository_path),
      powershell: cfg!(windows),
    })
  }

  fn status(&self) -> AgentCliStatus {
    let status = install_status(&Environment::from_process());
    AgentCliStatus {
      bd: status.bd,
      skills: status.skills,
    }
  }
}
