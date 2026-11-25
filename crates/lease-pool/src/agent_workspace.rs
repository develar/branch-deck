//! Where a coding agent works and how it writes a commit subject, asked before its first edit (`brd workspace`,
//! openspec/specs/cli/workspace/spec.md): `branchdeck.agentLease` (on by default) says to work in a lease of the pool and land each
//! commit with `brd lease pick`, `branchdeck.agentSubjectBranch` (on by default) to start each subject with its `(branch)`,
//! `branchdeck.agentPullRequest` (off by default) to push the branch after the pick and open its pull request, which the
//! Push route of the repository turns off (`LandingSetting::agent_pull_request_off`).
//! Read only: one `rev-parse`, one `git config`, and in the shared checkout or a lease one `status` of the shared
//! checkout; in a lease one more `rev-parse` of the branch the shared checkout has checked out now, and with a home
//! branch one `diff-tree` since the fork point beside the `status`, with one `cat-file` only when it names a build config
//! file (`build_config`); in the shared checkout with the lease on, one more `git config` for the pool directory.
//! `agent_defaults` reads the same three rules of the global git config, what `brd install` and the app's Install Agent CLI
//! report.

use crate::build_config::build_config_changed;
use crate::lease::{IdleStatus, LeaseHome, SlotState, StatusScan, canonical_path, lease_status_core, pool_dir, reclaimed_refusal};
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::lease_config;
use git_ops::revs::{read_head, repository_dirs};
use std::path::{Path, PathBuf};
use sync_core::repository_config::{ConfigSetting, OriginFacts, read_config_entries, repository_config_from_entries};
use sync_core::uncommitted_changes::{GetUncommittedChangesParams, get_uncommitted_changes};
use sync_utils::span::in_current_span;
use tracing::{instrument, warn};

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
  /// Whether the process it was got for still runs
  pub holder_alive: Option<bool>,
  /// Its holder is gone (`Lease::is_idle`): idle, with work, or busy with the processes of another session in its slot
  pub idle: Option<IdleStatus>,
}

#[derive(Debug, Clone)]
pub struct AgentWorkspace {
  pub kind: WorkspaceKind,
  /// `branchdeck.agentLease`
  pub lease: ConfigSetting<bool>,
  /// `branchdeck.agentSubjectBranch`
  pub subject_branch: ConfigSetting<bool>,
  /// `branchdeck.agentPullRequest`
  pub pull_request: ConfigSetting<bool>,
  /// Why `pull_request` does not apply here whatever its value: the route of the repository (`branchdeck.landing`)
  pub pull_request_off: Option<String>,
  /// The top level of the working tree; None without one
  pub top_level: Option<String>,
  /// Where the agent runs below the top level, `/`-separated, empty at the top level
  pub subdir: String,
  /// The checkout the commits land on: the main worktree itself, or the source of the lease; None in a linked worktree
  pub shared_checkout: Option<String>,
  /// The lease of this slot (`WorkspaceKind::Lease`)
  pub this_lease: Option<ActiveLease>,
  /// In a lease: the branch its home checkout has checked out now (`None`: a detached HEAD), when it is not the branch
  /// of the get: `brd lease pick` refuses until it is back
  pub home_switched: Option<Option<String>>,
  /// In a lease with a home branch: the build config files that the branch changed since the lease forked from it, and
  /// that the lease has in another form (`build_config`); empty when the read failed
  pub build_config_changed: Vec<String>,
  /// Every lease of the pool that is ready
  pub active_leases: Vec<ActiveLease>,
  /// The number of local changes of the shared checkout: files a lease does not have
  pub local_changes: Option<usize>,
  /// The lease is on, but its pool can't be used: why (a sandbox that can't write the pool directory, no home directory)
  pub pool_unusable: Option<String>,
}

/// What `brd workspace` answers for `directory`; `home` is the home directory of the default pool
#[instrument(skip(git))]
pub fn agent_workspace(git: &GitCommandExecutor, directory: &str, home: Option<&Path>) -> Result<AgentWorkspace> {
  let Some(dirs) = repository_dirs(git, directory)? else {
    // no working tree to edit: the global settings, for what they say
    let config = repository_config_from_entries(&read_config_entries(git, "")?, OriginFacts::default());
    return Ok(AgentWorkspace {
      kind: WorkspaceKind::None,
      lease: config.agent_lease,
      subject_branch: config.agent_subject_branch,
      pull_request: config.agent_pull_request,
      pull_request_off: config.agent_pull_request_off,
      top_level: None,
      subdir: String::new(),
      shared_checkout: None,
      this_lease: None,
      home_switched: None,
      build_config_changed: Vec::new(),
      active_leases: Vec::new(),
      local_changes: None,
      pool_unusable: None,
    });
  };
  // a resumed session in a slot taken over from its dead holder learns it here, before its first edit
  if let Some(refusal) = reclaimed_refusal(Path::new(&dirs.git_common_dir), &dirs.top_level, true, None)? {
    bail!(refusal);
  }
  let config = repository_config_from_entries(&read_config_entries(git, &dirs.top_level)?, OriginFacts::default());
  let top_level = canonical_path(Path::new(&dirs.top_level));

  let (slots, _) = lease_status_core(Path::new(&dirs.git_common_dir), StatusScan::Idle)?;
  let active_leases: Vec<ActiveLease> = slots
    .into_iter()
    .filter(|status| status.state == SlotState::Ready)
    .filter_map(|status| {
      let lease = status.slot.lease.as_ref()?;
      Some(ActiveLease {
        lease_id: lease.id.clone(),
        holder: lease.holder.clone(),
        path: status.slot.path.clone(),
        home: lease.home(),
        holder_alive: lease.holder_alive(),
        idle: status.idle,
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
  let home_branch = match (&kind, &this_lease) {
    (WorkspaceKind::Lease, Some(lease)) => lease.home.branch.as_deref(),
    _ => None,
  };
  // the two reads are independent, and the `status` sets the wall time
  let (local_changes, build_config_changed) = std::thread::scope(|scope| {
    let slot = top_level.as_str();
    let build_config = home_branch.map(|branch| scope.spawn(in_current_span(move || build_config_changed(git, slot, branch))));
    let local_changes = match &shared_checkout {
      Some(checkout) if Path::new(checkout).is_dir() => get_uncommitted_changes(
        git,
        GetUncommittedChangesParams {
          repository_path: checkout.clone(),
        },
      )
      .map(|changes| Some(changes.files.len()))
      .map_err(|error| anyhow!(error)),
      _ => Ok(None),
    };
    let build_config_changed = build_config
      .map(|read| read.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
      .transpose()
      // a hint: a failed read must not fail the command
      .unwrap_or_else(|error| {
        warn!("Cannot read the build config that the home branch changed: {error:#}");
        None
      })
      .unwrap_or_default();
    (local_changes, build_config_changed)
  });
  let local_changes = local_changes?;
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
    pull_request: config.agent_pull_request,
    pull_request_off: config.agent_pull_request_off,
    subdir: subdir(directory, &top_level),
    top_level: Some(top_level),
    shared_checkout,
    this_lease,
    home_switched,
    build_config_changed,
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

/// The rules of `brd workspace` that the global git config gives every repository without its own value (`brd install`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct AgentDefaults {
  /// `branchdeck.agentLease`: work in a lease of their own and land each commit with `brd pick` (on by default)
  pub lease: bool,
  /// `branchdeck.agentSubjectBranch`: start each subject with `(branch)` (on by default)
  pub subject_branch: bool,
  /// `branchdeck.agentPullRequest`: push the branch after the pick and open its pull request, without asking (off by
  /// default)
  pub pull_request: bool,
}

/// The agent rules of the global git config, one `git config --global`; the defaults without a git, or when it can't
/// be run, as `brd workspace` would find none either
pub fn agent_defaults(git: Option<&GitCommandExecutor>) -> AgentDefaults {
  let entries = git.and_then(|git| read_config_entries(git, "").ok()).unwrap_or_default();
  let config = repository_config_from_entries(&entries, OriginFacts::default());
  AgentDefaults {
    lease: config.agent_lease.value,
    subject_branch: config.agent_subject_branch.value,
    pull_request: config.agent_pull_request.value,
  }
}
