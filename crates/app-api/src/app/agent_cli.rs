//! The agent CLI of the app, `bd` (docs/guidelines/agent-cli.md, spec/cli/agent-cli.spec.md): "Install Agent CLI…"
//! (`install_agent_cli`, `bd install`), the `bd` command the copied prompts name (`agent_cli_command`) and the skills
//! Settings → Agents shows (`agent_cli_status`). The install and the binary are the host's (`AgentCliHost`: the app's
//! `bd_cli::install`, a fixed path in the e2e server); the rules of `bd workspace` where the install wrote none are the
//! global git config's, read here.

use crate::context::AppContext;
use crate::operation::{operation, run_blocking};
use crate::transport::Call;
use domain_types::error::ApiError;
use lease_pool::agent_workspace::agent_defaults;
use std::sync::Arc;
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

/// What the host's `bd install` did: the rules of `bd workspace` only where it wrote them
#[derive(Debug)]
pub struct AgentCliInstalled {
  pub bd: String,
  pub on_path: bool,
  pub skills: Vec<String>,
  pub agents: Option<AgentRules>,
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

/// Whether the agent skill is installed (Settings → Agents)
#[derive(Debug, serde::Serialize, specta::Type)]
pub struct AgentCliStatus {
  /// The command the skills name; `null` while no skill is installed
  pub bd: Option<String>,
  /// The skill files that exist
  pub skills: Vec<String>,
}

/// `bd install`: the `bd` command, the skills, and the rules of `bd workspace` as the global git config has them where
/// the install wrote none
#[instrument(skip_all)]
pub async fn install_agent_cli(ctx: &AppContext) -> Result<AgentCliInstall, ApiError> {
  let (host, git) = (Arc::clone(&ctx.desktop.agent_cli), ctx.git.clone());
  run_blocking(move || {
    let installed = host.install()?;
    let agents = installed.agents.unwrap_or_else(|| {
      let defaults = agent_defaults(Some(&git));
      AgentRules {
        lease: defaults.lease,
        subject_branch: defaults.subject_branch,
      }
    });
    Ok::<_, ApiError>(AgentCliInstall {
      bd: installed.bd,
      on_path: installed.on_path,
      skills: installed.skills,
      agents,
    })
  })
  .await
}

/// The bd command of the prompts the app copies. Writes nothing and needs no install, so the bd of a prompt is always
/// the version of the app that wrote it; an error when the app can't tell its binary (no `bd.exe` next to it)
#[instrument(skip_all, fields(repository_path = %repository_path))]
pub async fn agent_cli_command(ctx: &AppContext, repository_path: String) -> Result<AgentCliCommand, ApiError> {
  operation(async move { ctx.desktop.agent_cli.command(&repository_path) }).await
}

/// The skill files of `bd install` that exist, without writing anything
#[instrument(skip_all)]
pub fn agent_cli_status(ctx: &AppContext, _call: &Call) -> AgentCliStatus {
  ctx.desktop.agent_cli.status()
}
