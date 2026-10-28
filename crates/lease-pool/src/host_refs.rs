//! The refs that keep the commits of the runs of a host and the bases of its clients (spec/cli/lease.spec.md, "Host
//! objects"): `ingest` writes the pack of a client and points `refs/branch-deck/host/run/<id>` or
//! `refs/branch-deck/host/base/<name>` at its commit (`git_ops::host_objects`). When a pack last moved each ref is
//! `$GIT_COMMON_DIR/branch-deck/host-refs.json`, a state file of its own (`state_file`), whose lock is a leaf as the one
//! of the host slots: held while the file changes, never across git. The give-back of a run deletes its ref, and
//! `reclaim` deletes a run ref whose run has no lend and no ticket an hour after its pack, and a base ref that no pack
//! moved for 14 days: a ref outlives its run only when the give-back never came (a supervisor that died, a pack after
//! the end of its run). Both read the file first, so a run that sent no pack costs no git.
//!
//! The host-object calls (`have`, `ingest`, `show`) are git users of the host (`host_slots::HostUser`): each waits
//! behind a recorded upgrade of git. The deletes of the refs skip while a live process upgrades git

use crate::host_slots::{HostSlotRefusal, HostUser, now_ms};
use crate::state_file::StateFile;
use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::host_objects::{BASE_REFS, Ingested, RUN_REFS, ShownCommit, delete_host_refs, ingest_config, ingest_pack, point_host_ref, present_commits, show_commit};
use git_ops::lease_slot::start_deferred_maintenance;
use git_ops::revs::{RepositoryDirs, is_object_id};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::time::Duration;
use sync_utils::host_ref_name::is_host_ref_name;
use tracing::{debug, warn};

const STATE_FILE: &str = "host-refs.json";
const LOCK_FILE: &str = "host-refs.lock";
/// How long a run ref whose run has no lend and no ticket stays after its pack
pub const RUN_REF_AGE: Duration = Duration::from_hours(1);
/// How long a base ref that no pack moved stays
pub const BASE_REF_AGE: Duration = Duration::from_hours(14 * 24);

/// The refs of the host and when a pack last moved each
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct HostRefs {
  /// By the full ref
  #[serde(default)]
  refs: BTreeMap<String, RefRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RefRecord {
  commit: String,
  /// Unix milliseconds
  moved_ms: u64,
}

/// The ref a pack goes to
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRef {
  /// `run:<id>`: the commit of a run, until its give-back
  Run(String),
  /// `base:<name>`: the base of the later packs of a client
  Base(String),
}

/// Why a ref or a commit of `ingest` is refused, before any git
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostRefRefusal {
  #[error("{0:?} is no ref of the host: run:<id> or base:<name>, a name of 1 to 128 characters of A-Z a-z 0-9 . _ -, the first a letter or a digit")]
  BadRef(String),
  #[error("{0:?} is no full commit id")]
  BadCommit(String),
}

impl HostRef {
  /// `run:<id>` or `base:<name>`
  pub fn parse(text: &str) -> Result<Self> {
    let parsed = match text.split_once(':') {
      Some(("run", id)) if is_host_ref_name(id) => Self::Run(id.to_string()),
      Some(("base", name)) if is_host_ref_name(name) => Self::Base(name.to_string()),
      _ => bail!(HostRefRefusal::BadRef(text.to_string())),
    };
    Ok(parsed)
  }

  /// `refs/branch-deck/host/run/<id>` or `refs/branch-deck/host/base/<name>`
  pub fn full_ref(&self) -> String {
    match self {
      Self::Run(id) => format!("{RUN_REFS}{id}"),
      Self::Base(name) => format!("{BASE_REFS}{name}"),
    }
  }
}

fn refs_file(git_common_dir: &Path) -> StateFile {
  StateFile::new(git_common_dir, STATE_FILE, LOCK_FILE, "the host refs")
}

/// What `ingest` did
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestedPack {
  pub full_ref: String,
  pub ingested: Ingested,
  /// `branchdeck.leaseMaintenance` is true: git's maintenance was started after the ref
  pub maintenance_started: bool,
}

/// Write the pack that `input` streams into the repository of `dirs` (`ingest_pack`, by its unpack limit) and point
/// `target` at `commit` (a full id); then the time of the move goes into the file of the refs, and with
/// `branchdeck.leaseMaintenance` git's maintenance starts in the common dir: in a repository whose `maintenance.auto`
/// is false, the packs of the runs would else pile up. A bad commit id is refused before the pack is read, and then the
/// pack is read to its end. A git user (`HostUser::wait`) while it runs: the pack waits behind an upgrade of git
pub fn ingest(git: &GitCommandExecutor, dirs: &RepositoryDirs, target: &HostRef, commit: &str, input: &mut dyn Read, lock_timeout: Duration) -> Result<IngestedPack> {
  if !is_object_id(commit) {
    let _ = std::io::copy(input, &mut std::io::sink());
    bail!(HostRefRefusal::BadCommit(commit.to_string()));
  }
  let _user = HostUser::wait(Path::new(&dirs.git_common_dir), lock_timeout)?;
  let config = ingest_config(git, &dirs.top_level)?;
  let ingested = ingest_pack(git, &dirs.top_level, input, config.unpack_limit)?;
  let full_ref = target.full_ref();
  point_host_ref(git, &dirs.top_level, &full_ref, commit)?;
  refs_file(Path::new(&dirs.git_common_dir)).update(lock_timeout, |refs: &mut HostRefs| {
    refs.refs.insert(
      full_ref.clone(),
      RefRecord {
        commit: commit.to_string(),
        moved_ms: now_ms(),
      },
    );
    Ok(())
  })?;
  let maintenance_started = config.maintenance
    && start_deferred_maintenance(git, &dirs.git_common_dir)
      .inspect_err(|error| warn!(error = %format!("{error:#}"), "cannot start git's maintenance after a pack"))
      .is_ok();
  Ok(IngestedPack {
    full_ref,
    ingested,
    maintenance_started,
  })
}

/// The commits of `commits` (full ids) that the repository of `dirs` has as commit objects, in their order
/// (`present_commits`). A git user (`HostUser::wait`) while it runs
pub fn have(git: &GitCommandExecutor, dirs: &RepositoryDirs, commits: &[String], lock_timeout: Duration) -> Result<Vec<String>> {
  let _user = HostUser::wait(Path::new(&dirs.git_common_dir), lock_timeout)?;
  present_commits(git, &dirs.top_level, commits)
}

/// The text of the file `blob` of `commit` and the object ids of the paths `ids` (`show_commit`). A git user
/// (`HostUser::wait`) while it runs
pub fn show(git: &GitCommandExecutor, dirs: &RepositoryDirs, commit: &str, blob: Option<&str>, ids: &[String], lock_timeout: Duration) -> Result<ShownCommit> {
  let _user = HostUser::wait(Path::new(&dirs.git_common_dir), lock_timeout)?;
  show_commit(git, &dirs.top_level, commit, blob, ids)
}

/// Delete the run ref of `run` once its run ended (the give-back): the ref, when the file of the refs names it, and its
/// record. `None` when the run sent no pack, and then no git runs. `GitUpgrading` while a live process upgrades git:
/// the ref stays for `reclaim_refs`
pub fn delete_run_ref(git: &GitCommandExecutor, dirs: &RepositoryDirs, run: &str, lock_timeout: Duration) -> Result<Option<String>> {
  let full_ref = HostRef::Run(run.to_string()).full_ref();
  let file = refs_file(Path::new(&dirs.git_common_dir));
  if !file.read::<HostRefs>()?.refs.contains_key(&full_ref) {
    return Ok(None);
  }
  let _user = HostUser::unless_upgrading(Path::new(&dirs.git_common_dir), lock_timeout)?;
  delete_host_refs(git, &dirs.top_level, std::slice::from_ref(&full_ref))?;
  file.update(lock_timeout, |refs: &mut HostRefs| {
    refs.refs.remove(&full_ref);
    Ok(())
  })?;
  Ok(Some(full_ref))
}

/// Delete what no run needs any more: a run ref whose run is not in `live_runs` (a lend or a ticket of a live process)
/// an hour after its pack, and a base ref that no pack moved for 14 days; one ref transaction. The refs deleted, by
/// their full names; none while a live process upgrades git, and a later reclaim deletes them
pub fn reclaim_refs(git: &GitCommandExecutor, dirs: &RepositoryDirs, live_runs: &HashSet<String>, lock_timeout: Duration) -> Result<Vec<String>> {
  let file = refs_file(Path::new(&dirs.git_common_dir));
  let refs: HostRefs = file.read()?;
  let now = now_ms();
  let older_than = |record: &RefRecord, age: Duration| now.saturating_sub(record.moved_ms) >= u64::try_from(age.as_millis()).unwrap_or(u64::MAX);
  let stale: Vec<(String, u64)> = refs
    .refs
    .iter()
    .filter(|(full_ref, record)| match full_ref.strip_prefix(RUN_REFS) {
      Some(run) => !live_runs.contains(run) && older_than(record, RUN_REF_AGE),
      None => older_than(record, BASE_REF_AGE),
    })
    .map(|(full_ref, record)| (full_ref.clone(), record.moved_ms))
    .collect();
  let names: Vec<String> = stale.iter().map(|(full_ref, _)| full_ref.clone()).collect();
  if names.is_empty() {
    return Ok(names);
  }
  let _user = match HostUser::unless_upgrading(Path::new(&dirs.git_common_dir), lock_timeout) {
    Ok(user) => user,
    Err(error)
      if error
        .downcast_ref::<HostSlotRefusal>()
        .is_some_and(|refusal| matches!(refusal, HostSlotRefusal::GitUpgrading(_))) =>
    {
      debug!(refs = names.len(), error = %error, "the stale host refs stay while git is upgrading");
      return Ok(Vec::new());
    }
    Err(error) => return Err(error),
  };
  delete_host_refs(git, &dirs.top_level, &names)?;
  // a record that a pack moved meanwhile stays: the next reclaim deletes its ref once more, or later
  file.update(lock_timeout, |refs: &mut HostRefs| {
    for (full_ref, moved_ms) in &stale {
      if refs.refs.get(full_ref).is_some_and(|record| record.moved_ms == *moved_ms) {
        refs.refs.remove(full_ref);
      }
    }
    Ok(())
  })?;
  Ok(names)
}
