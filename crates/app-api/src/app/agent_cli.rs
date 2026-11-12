//! The agent CLI of the app, `brd` (docs/contributing/agent-cli.md, openspec/specs/cli/agent-cli/spec.md): "Install Agent CLI…"
//! (`install_agent_cli`, `brd install`), the `brd` command the copied prompts name (`agent_cli_command`) and the skills
//! Settings → Agents shows (`agent_cli_status`). The install and the binary are the host's (`AgentCliHost`: the app's
//! `bd_cli::install`, a fixed path in the e2e server); the rules of `brd workspace` where the install wrote none are the
//! global git config's, read here. Beside it, Settings → Tools → IntelliJ IDEA writes the app file of an IDE with Air
//! (`install_ide_app`) and Remove deletes it (`remove_ide_app`), again through the host.

use crate::context::AppContext;
use crate::operation::{operation, run_blocking};
use crate::transport::Call;
use domain_types::error::ApiError;
use lease_pool::agent_workspace::agent_defaults;
use std::sync::Arc;
use tracing::instrument;

/// What the "Install Agent CLI…" menu item installed: `brd install` (docs/contributing/agent-cli.md)
#[derive(Debug, serde::Serialize, specta::Type)]
pub struct AgentCliInstall {
  /// The command the skills name: the `brd` symlink (macOS, Linux) or `brd.exe` next to the app (Windows)
  pub bd: String,
  /// Whether `brd` resolves to it in the user's shell
  pub on_path: bool,
  /// The skill files written
  pub skills: Vec<String>,
  /// How agents work where a repository has no value of its own (the global git config)
  pub agents: AgentRules,
}

/// The three rules of `brd workspace` (Settings → Agents)
#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
pub struct AgentRules {
  /// `branchdeck.agentLease`: in a worktree of their own, each commit landed on the user's branch
  pub lease: bool,
  /// `branchdeck.agentSubjectBranch`: `(branch)` at the start of each subject
  pub subject_branch: bool,
  /// `branchdeck.agentPullRequest`: after the pick, the branch pushed and its pull request opened without asking
  pub pull_request: bool,
}

/// What the host's `brd install` did: the rules of `brd workspace` only where it wrote them
#[derive(Debug)]
pub struct AgentCliInstalled {
  pub bd: String,
  pub on_path: bool,
  pub skills: Vec<String>,
  pub agents: Option<AgentRules>,
}

/// How the prompts the app copies run this app's brd in a repository
#[derive(Debug, serde::Serialize, specta::Type)]
pub struct AgentCliCommand {
  /// brd with `-C <repository>`, quoted for the shell, to run as written: the `brd` symlink in `~/.local/bin` when it
  /// points at this app, else the app binary with `cli` (on Windows `brd.exe` next to it)
  pub command: String,
  /// The shell is PowerShell (Windows), where a quoted path runs with `&`
  pub powershell: bool,
}

/// Whether the agent skill is installed (Settings → Agents), and Branch Deck in IntelliJ IDEA (Settings → Tools)
#[derive(Debug, serde::Serialize, specta::Type)]
pub struct AgentCliStatus {
  /// The command the skills name; `null` while no skill is installed
  pub bd: Option<String>,
  /// The skill files that exist
  pub skills: Vec<String>,
  /// The app file of an IDE with Air (`install_ide_app`); `null` while there is none
  pub ide_app: Option<IdeApp>,
}

/// The app file of an IDE with Air, `~/.air/apps/branch-deck.yaml` (docs/design/ide-embedding.md): the IDE starts
/// Branch Deck as `brd mcp` for each project and shows it in a tool window
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct IdeApp {
  /// Its path
  pub file: String,
  /// It starts this app, as an install writes it now; else another Branch Deck wrote it, and an install writes it anew
  pub current: bool,
}

/// `brd install`: the `brd` command, the skills, and the rules of `brd workspace` as the global git config has them where
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
        pull_request: defaults.pull_request,
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

/// The brd command of the prompts the app copies. Writes nothing and needs no install, so the brd of a prompt is always
/// the version of the app that wrote it; an error when the app can't tell its binary (no `brd.exe` next to it)
#[instrument(skip_all, fields(repository_path = %repository_path))]
pub async fn agent_cli_command(ctx: &AppContext, repository_path: String) -> Result<AgentCliCommand, ApiError> {
  operation(async move { ctx.desktop.agent_cli.command(&repository_path) }).await
}

/// The skill files of `brd install` that exist, without writing anything
#[instrument(skip_all)]
pub fn agent_cli_status(ctx: &AppContext, _call: &Call) -> AgentCliStatus {
  ctx.desktop.agent_cli.status()
}

/// Settings → Tools → IntelliJ IDEA: writes `~/.air/apps/branch-deck.yaml` for this app and its icons, which an IDE with
/// Air shows at once; a file there is written anew
#[instrument(skip_all)]
pub async fn install_ide_app(ctx: &AppContext) -> Result<IdeApp, ApiError> {
  let host = Arc::clone(&ctx.desktop.agent_cli);
  run_blocking(move || host.install_ide_app()).await
}

/// Settings → Tools → IntelliJ IDEA, Remove: deletes the app file of the IDE and its icons; the IDE closes the views of
/// Branch Deck
#[instrument(skip_all)]
pub async fn remove_ide_app(ctx: &AppContext) -> Result<(), ApiError> {
  let host = Arc::clone(&ctx.desktop.agent_cli);
  run_blocking(move || host.remove_ide_app()).await
}
