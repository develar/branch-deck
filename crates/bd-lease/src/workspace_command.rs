//! `bd workspace`: where a coding agent works and how it writes a commit subject, asked before its first edit
//! (spec/cli/workspace.spec.md). The core is `lease_pool::agent_workspace`; this module turns it into the rules and the
//! next commands of the agent

use crate::lease_command::{IdleOutput, LeaseHomeOutput, lease_next, refusal_error};
use bd_core::bd_command::shell_word;
use bd_core::context::Environment;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use git_executor::git_command_executor::GitCommandExecutor;
use lease_pool::agent_workspace::{ActiveLease, AgentWorkspace, WorkspaceKind, agent_workspace};
use lease_pool::lease::LeaseRefusal;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::Path;
use sync_core::repository_config::{ConfigScope, ConfigSetting};

/// `bd workspace`
#[derive(Debug, Serialize, JsonSchema)]
pub struct WorkspaceOutput {
  pub schema: SchemaVersion,
  /// Where bd runs: `main` (the shared checkout), `linked` (a worktree of its own outside the pool), `lease` (a leased
  /// slot of the pool) or `none` (no working tree: nothing to edit)
  pub kind: WorkspaceKindOutput,
  /// Work in a lease of your own and land each commit with `bd lease pick` (`branchdeck.agentLease`, on by default)
  pub lease: AgentRule,
  /// Start each commit subject with `(branch)` (`branchdeck.agentSubjectBranch`, off by default); off: write the
  /// subject as the repository does
  pub subject_branch: AgentRule,
  /// Stacked branches (`branchdeck.stacks`): a new stack is declared with `(parent/leaf)`
  pub stacks: bool,
  /// The top level of the working tree
  #[serde(skip_serializing_if = "Option::is_none")]
  pub top_level: Option<String>,
  /// Where bd runs below the top level, `/`-separated: continue there in the lease; empty at the top level
  pub subdir: String,
  /// The checkout your commits land on: the main worktree, or the source of this lease (`home.checkout`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub shared_checkout: Option<String>,
  /// In a lease: where its commits land (`bd lease pick`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub home: Option<LeaseHomeOutput>,
  /// The lease of this slot, for `bd lease return --lease-id`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub lease_id: Option<String>,
  /// The holder of this slot's lease
  #[serde(skip_serializing_if = "Option::is_none")]
  pub holder: Option<String>,
  /// The local changes of the shared checkout, which a lease does not have: ask the user when the task needs them
  #[serde(skip_serializing_if = "Option::is_none")]
  pub local_changes: Option<usize>,
  /// The ready leases of the pool: a session that lost its path finds its own by `holder`, never takes another's
  pub active_leases: Vec<ActiveLeaseOutput>,
  /// What to run next, in order; empty: work where you are
  pub next: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKindOutput {
  Main,
  Linked,
  Lease,
  None,
}

/// One rule of the agent: whether it applies here, and why
#[derive(Debug, Serialize, JsonSchema)]
pub struct AgentRule {
  /// Whether to follow it here
  pub on: bool,
  /// The git config that sets it: `local` (this repository), `worktree`, `command` (`git -c`), `global`, `system`;
  /// `default` when none does
  pub source: RuleSource,
  /// The value set is not a boolean; the default applies
  #[serde(skip_serializing_if = "Option::is_none")]
  pub invalid: Option<String>,
  /// Why the rule is set but does not apply here
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleSource {
  Default,
  System,
  Global,
  Local,
  Worktree,
  Command,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ActiveLeaseOutput {
  pub lease_id: String,
  pub holder: String,
  /// The slot, its top level
  pub path: String,
  /// Whether the process the lease was got for still runs; omitted for a lease of an older bd
  #[serde(skip_serializing_if = "Option::is_none")]
  pub holder_alive: Option<bool>,
  /// Its holder is gone: `idle` (a get takes it over once its work landed), or `idle_with_work` with what a check found,
  /// which no get touches: tell the user
  #[serde(skip_serializing_if = "Option::is_none")]
  pub idle: Option<IdleOutput>,
}

/// `bd workspace` in `directory`, else in the current directory: no working tree is an answer, not an error
pub fn workspace(environment: &Environment, directory: Option<&Path>) -> Result<WorkspaceOutput, CliError> {
  workspace_with(&bd_core::context::counted_git(environment)?, environment, directory)
}

/// `workspace` with the git of the caller
pub fn workspace_with(git: &GitCommandExecutor, environment: &Environment, directory: Option<&Path>) -> Result<WorkspaceOutput, CliError> {
  let directory = directory.map_or_else(|| environment.cwd.clone(), |directory| environment.cwd.join(directory));
  if !directory.is_dir() {
    return Err(CliError::new(ErrorCode::NotARepository, format!("No such directory: {}", directory.display())));
  }
  let answer = agent_workspace(git, &directory.to_string_lossy(), environment.home.as_deref()).map_err(|error| match error.downcast_ref::<LeaseRefusal>() {
    Some(refusal) => refusal_error(refusal),
    None => CliError::from(error),
  })?;
  Ok(workspace_output(answer))
}

fn workspace_output(answer: AgentWorkspace) -> WorkspaceOutput {
  let kind = match answer.kind {
    WorkspaceKind::Main => WorkspaceKindOutput::Main,
    WorkspaceKind::Linked => WorkspaceKindOutput::Linked,
    WorkspaceKind::Lease => WorkspaceKindOutput::Lease,
    WorkspaceKind::None => WorkspaceKindOutput::None,
  };
  let lease_reason = match kind {
    _ if !answer.lease.value => None,
    WorkspaceKindOutput::None => Some("No working tree here: nothing to edit".to_string()),
    WorkspaceKindOutput::Linked => Some("A worktree of its own already: work here, no lease and no pick".to_string()),
    WorkspaceKindOutput::Main => answer.pool_unusable.as_ref().map(|why| format!("The lease pool can't be used: {why}; work here")),
    WorkspaceKindOutput::Lease => None,
  };
  let lease = rule(&answer.lease, lease_reason);
  let subject_branch = rule(&answer.subject_branch, None);

  let top_level = answer.top_level.clone().unwrap_or_default();
  let this_lease = answer.this_lease.as_ref();
  let next = match kind {
    WorkspaceKindOutput::Main if lease.on => vec![format!("bd lease get --holder <your name> -C {}", shell_word(&top_level))],
    WorkspaceKindOutput::Lease => this_lease.map_or_else(Vec::new, |lease| lease_next(&top_level, &lease.home, &lease.lease_id)),
    _ => Vec::new(),
  };
  WorkspaceOutput {
    schema: SchemaVersion,
    kind,
    lease,
    subject_branch,
    stacks: answer.stacks,
    top_level: answer.top_level,
    subdir: answer.subdir,
    shared_checkout: answer.shared_checkout,
    home: this_lease.map(|lease| LeaseHomeOutput::new(&lease.home, answer.home_switched.clone())),
    lease_id: this_lease.map(|lease| lease.lease_id.clone()),
    holder: this_lease.map(|lease| lease.holder.clone()),
    local_changes: answer.local_changes,
    active_leases: answer.active_leases.into_iter().map(active_lease).collect(),
    next,
  }
}

fn rule(setting: &ConfigSetting<bool>, reason: Option<String>) -> AgentRule {
  AgentRule {
    on: setting.value && reason.is_none(),
    source: match setting.scope {
      None => RuleSource::Default,
      Some(ConfigScope::System) => RuleSource::System,
      Some(ConfigScope::Global) => RuleSource::Global,
      Some(ConfigScope::Local) => RuleSource::Local,
      Some(ConfigScope::Worktree) => RuleSource::Worktree,
      Some(ConfigScope::Command) => RuleSource::Command,
    },
    invalid: setting.invalid.clone(),
    reason,
  }
}

fn active_lease(lease: ActiveLease) -> ActiveLeaseOutput {
  ActiveLeaseOutput {
    lease_id: lease.lease_id,
    holder: lease.holder,
    path: lease.path,
    holder_alive: lease.holder_alive,
    idle: lease.idle.map(|check| IdleOutput::of(check.as_ref())),
  }
}
