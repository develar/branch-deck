//! Where a coding agent works and how it writes a commit subject, asked before its first edit (`bd workspace`,
//! spec/cli/workspace.spec.md): `branchdeck.agentLease` (on by default) says to work in a lease of the pool and land each
//! commit with `bd lease pick`, `branchdeck.agentSubjectBranch` (on by default) to start each subject with its `(branch)`.
//! Read only: one `rev-parse`, one `git config`, and in the shared checkout or a lease one `status` of the shared
//! checkout; in a lease one more `rev-parse` of the branch the shared checkout has checked out now; in the shared
//! checkout with the lease on, one more `git config` for the pool directory.
//! `agent_defaults` reads the same two rules of the global git config, what `bd install` and the app's Install Agent CLI
//! report.

use crate::lease::{LeaseHome, SlotState, canonical_path, lease_status_core, pool_dir};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::lease_config;
use git_ops::revs::{read_head, repository_dirs};
use std::path::{Path, PathBuf};
use sync_core::repository_config::{ConfigSetting, OriginFacts, read_config_entries, repository_config_from_entries};
use sync_core::uncommitted_changes::{GetUncommittedChangesParams, get_uncommitted_changes};
use tracing::instrument;

/// Where the agent runs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceKind {
  /// The main worktree: the user's checkout, which other sessions share
  Main,
  /// A linked worktree outside the pool: a checkout of its own already
  Linked,
  /// A slot of the pool, leased
  Lease,
  /// No working tree: a bare repository, a `.git` directory, no repository
  None,
}

/// The lease of a slot of the pool
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveLease {
  pub lease_id: String,
  pub holder: String,
  /// The slot, its top level
  pub path: String,
  /// Where its commits go
  pub home: LeaseHome,
}

#[derive(Debug, Clone)]
pub struct AgentWorkspace {
  pub kind: WorkspaceKind,
  /// `branchdeck.agentLease`
  pub lease: ConfigSetting<bool>,
  /// `branchdeck.agentSubjectBranch`
  pub subject_branch: ConfigSetting<bool>,
  /// `branchdeck.stacks`: a new stack is declared with `(parent/leaf)`
  pub stacks: bool,
  /// The top level of the working tree; None without one
  pub top_level: Option<String>,
  /// Where the agent runs below the top level, `/`-separated, empty at the top level
  pub subdir: String,
  /// The checkout the commits land on: the main worktree itself, or the source of the lease; None in a linked worktree
  pub shared_checkout: Option<String>,
  /// The lease of this slot (`WorkspaceKind::Lease`)
  pub this_lease: Option<ActiveLease>,
  /// In a lease: the branch its home checkout has checked out now (`None`: a detached HEAD), when it is not the branch
  /// of the get: `bd lease pick` refuses until it is back
  pub home_switched: Option<Option<String>>,
  /// Every lease of the pool that is ready
  pub active_leases: Vec<ActiveLease>,
  /// The number of local changes of the shared checkout: files a lease does not have
  pub local_changes: Option<usize>,
  /// The lease is on, but its pool can't be used: why (a sandbox that can't write the pool directory, no home directory)
  pub pool_unusable: Option<String>,
}

/// What `bd workspace` answers for `directory`; `home` is the home directory of the default pool
#[instrument(skip(git))]
pub fn agent_workspace(git: &GitCommandExecutor, directory: &str, home: Option<&Path>) -> Result<AgentWorkspace> {
  let Some(dirs) = repository_dirs(git, directory)? else {
    // no working tree to edit: the global settings, for what they say
    let config = repository_config_from_entries(&read_config_entries(git, "")?, OriginFacts::default());
    return Ok(AgentWorkspace {
      kind: WorkspaceKind::None,
      lease: config.agent_lease,
      subject_branch: config.agent_subject_branch,
      stacks: config.stacks.value,
      top_level: None,
      subdir: String::new(),
      shared_checkout: None,
      this_lease: None,
      home_switched: None,
      active_leases: Vec::new(),
      local_changes: None,
      pool_unusable: None,
    });
  };
  let config = repository_config_from_entries(&read_config_entries(git, &dirs.top_level)?, OriginFacts::default());
  let top_level = canonical_path(Path::new(&dirs.top_level));

  let (slots, _) = lease_status_core(Path::new(&dirs.git_common_dir), false)?;
  let active_leases: Vec<ActiveLease> = slots
    .iter()
    .filter(|status| status.state == SlotState::Ready)
    .filter_map(|status| {
      let lease = status.slot.lease.as_ref()?;
      Some(ActiveLease {
        lease_id: lease.id.clone(),
        holder: lease.holder.clone(),
        path: status.slot.path.clone(),
        home: lease.home(),
      })
    })
    .collect();
  let this_lease = active_leases.iter().find(|lease| lease.path == top_level).cloned();

  let kind = if this_lease.is_some() {
    WorkspaceKind::Lease
  } else if dirs.git_dir == dirs.git_common_dir {
    WorkspaceKind::Main
  } else {
    WorkspaceKind::Linked
  };
  let shared_checkout = match (&kind, &this_lease) {
    (WorkspaceKind::Lease, Some(lease)) if !lease.home.checkout.is_empty() => Some(lease.home.checkout.clone()),
    (WorkspaceKind::Main, _) => Some(top_level.clone()),
    _ => None,
  };
  let local_changes = match &shared_checkout {
    Some(checkout) if Path::new(checkout).is_dir() => Some(
      get_uncommitted_changes(
        git,
        GetUncommittedChangesParams {
          repository_path: checkout.clone(),
        },
      )
      .map_err(|error| anyhow!(error))?
      .files
      .len(),
    ),
    _ => None,
  };
  let home_switched = match (&kind, &this_lease, &shared_checkout) {
    (WorkspaceKind::Lease, Some(lease), Some(checkout)) if Path::new(checkout).is_dir() => {
      let now = read_head(git, checkout)?.and_then(|(_, branch)| branch);
      (now != lease.home.branch).then_some(now)
    }
    _ => None,
  };
  let pool_unusable = if kind == WorkspaceKind::Main && config.agent_lease.value {
    match pool_dir(&lease_config(git, &dirs.top_level)?, &dirs.top_level, Path::new(&dirs.git_common_dir), home) {
      Ok(pool) => unwritable_dir(&pool).map(|dir| format!("{dir} can't be written (a sandbox?)")),
      Err(error) => Some(format!("{error:#}")),
    }
  } else {
    None
  };

  Ok(AgentWorkspace {
    kind,
    lease: config.agent_lease,
    subject_branch: config.agent_subject_branch,
    stacks: config.stacks.value,
    subdir: subdir(directory, &top_level),
    top_level: Some(top_level),
    shared_checkout,
    this_lease,
    home_switched,
    active_leases,
    local_changes,
    pool_unusable,
  })
}

/// `directory` below `top_level`, `/`-separated; empty at the top level or outside it
pub(crate) fn subdir(directory: &str, top_level: &str) -> String {
  let directory = PathBuf::from(canonical_path(Path::new(directory)));
  directory
    .strip_prefix(top_level)
    .map_or_default(|relative| relative.components().map(|component| component.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"))
}

/// The first existing directory of `dir` and its parents, when a file can't be created in it: a get would fail there.
/// The probe file is removed at once
pub(crate) fn unwritable_dir(dir: &Path) -> Option<String> {
  let existing = dir.ancestors().find(|ancestor| ancestor.is_dir())?;
  let probe = existing.join(format!(".bd-workspace-probe-{}", std::process::id()));
  match std::fs::File::create_new(&probe) {
    Ok(_) => {
      let _ = sync_utils::fs_retry::remove_file(&probe);
      None
    }
    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
    Err(_) => Some(canonical_path(existing)),
  }
}

/// The rules of `bd workspace` that the global git config gives every repository without its own value (`bd install`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct AgentDefaults {
  /// `branchdeck.agentLease`: work in a lease of their own and land each commit with `bd pick` (on by default)
  pub lease: bool,
  /// `branchdeck.agentSubjectBranch`: start each subject with `(branch)` (on by default)
  pub subject_branch: bool,
}

/// The agent rules of the global git config, one `git config --global`; the defaults without a git, or when it can't
/// be run, as `bd workspace` would find none either
pub fn agent_defaults(git: Option<&GitCommandExecutor>) -> AgentDefaults {
  let entries = git.and_then(|git| read_config_entries(git, "").ok()).unwrap_or_default();
  let config = repository_config_from_entries(&entries, OriginFacts::default());
  AgentDefaults {
    lease: config.agent_lease.value,
    subject_branch: config.agent_subject_branch.value,
  }
}
