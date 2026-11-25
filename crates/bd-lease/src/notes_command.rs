//! `brd lease notes`: where the notes of a lease of the pool are, held or returned, and their files
//! (openspec/specs/cli/lease/spec.md, "Notes"; the core is `lease_pool::lease_notes`). Read only

use crate::lease_command::{LeaseContext, PreservedOutput, lease_error};
use bd_core::context::Environment;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use git_ops::lease_slot::{LEASE_NOTES_DIR, lease_config};
use lease_pool::lease::{Pool, canonical_path, ready_slot_at};
use lease_pool::lease_notes::{DEFAULT_NOTES_KEEP, LeaseNotes, config_pool_dir, expires_at, held_leases, note_files, notes_keep};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Default, clap::Args)]
pub struct LeaseNotesArgs {
  /// The notes of this lease, held or returned
  #[arg(long, value_name = "ID", conflicts_with = "holder")]
  pub lease_id: Option<String>,
  /// The notes of every lease of this holder (`--holder` of its get), the one got last first
  #[arg(long, value_name = "LABEL")]
  pub holder: Option<String>,
  /// Also list the files of each, by path with their bytes
  #[arg(long)]
  pub list: bool,
}

/// `brd lease notes`
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseNotesOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  /// The directory of the notes of the leases of the pool, `notes` in its pool directory; the notes a brd before kept in
  /// the common dir stay there, and `path` of each names where it is
  pub root: String,
  /// How long the notes of a returned lease stay, in seconds (`branchdeck.leaseNotesKeep`, 14 days by default); 0: for
  /// ever
  pub keep_seconds: u64,
  /// The notes asked for, the lease got last first; empty when there are none
  pub notes: Vec<NotesOutput>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct NotesOutput {
  pub lease_id: String,
  pub holder: String,
  /// The worktree the lease held
  pub slot: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  /// The directory of the notes: `notes.local` of the worktree links to it while the lease is held
  pub path: String,
  /// Unix seconds
  pub leased_at: u64,
  /// Unix seconds of the return; omitted while the lease is held
  #[serde(skip_serializing_if = "Option::is_none")]
  pub returned_at: Option<u64>,
  /// The pool holds the lease: its notes never expire
  pub held: bool,
  /// Unix seconds after which a trim or the keeper of a host deletes them; omitted while held and when kept for ever
  #[serde(skip_serializing_if = "Option::is_none")]
  pub expires_at: Option<u64>,
  /// The bytes on disk of the directory
  pub bytes: u64,
  /// The local changes of the idle lease that a get or a reclaim preserved here as patches before it took the slot
  #[serde(skip_serializing_if = "Option::is_none")]
  pub preserved: Option<PreservedOutput>,
  /// Unix seconds of that preserve: the notes expire `keep_seconds` after it at the earliest
  #[serde(skip_serializing_if = "Option::is_none")]
  pub preserved_at: Option<u64>,
  /// With --list: the files, by path from `path`, the first 1000
  #[serde(skip_serializing_if = "Option::is_none")]
  pub files: Option<Vec<NoteFileOutput>>,
  /// With --list: there are more files than `files` names
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub files_omitted: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct NoteFileOutput {
  pub path: String,
  pub bytes: u64,
}

/// The notes of `--lease-id`, of `--holder`, without either of the lease of the worktree of -C (when it is a leased slot),
/// else of every lease of the pool
pub fn lease_notes(context: &LeaseContext, environment: &Environment, args: &LeaseNotesArgs) -> Result<LeaseNotesOutput, CliError> {
  let git_common_dir = canonical_path(Path::new(&context.dirs.git_common_dir));
  let pool = Pool::new(Path::new(&git_common_dir));
  let config = lease_config(&context.git, &context.dirs.top_level).ok();
  let keep = config.as_ref().map_or(DEFAULT_NOTES_KEEP, |config| notes_keep(config).0);
  let held = held_leases(&pool.read().map_err(lease_error)?);
  let pool_dir = config
    .as_ref()
    .and_then(|config| config_pool_dir(config, Path::new(&git_common_dir), environment.home.as_deref()).ok());
  let roots = pool.notes_roots(pool_dir.as_deref(), environment.home.as_deref());
  let own = if args.lease_id.is_none() && args.holder.is_none() {
    ready_slot_at(Path::new(&git_common_dir), &context.dirs.top_level)
      .map_err(lease_error)?
      .and_then(|slot| slot.lease.map(|lease| lease.id))
  } else {
    None
  };
  let notes: Vec<LeaseNotes> = match (args.lease_id.as_deref().or(own.as_deref()), &args.holder) {
    (Some(lease_id), _) => roots.find(lease_id).into_iter().collect(),
    (None, Some(holder)) => roots.all().into_iter().filter(|notes| notes.record.holder == *holder).collect(),
    (None, None) => roots.all(),
  };
  Ok(LeaseNotesOutput {
    schema: SchemaVersion,
    repository: context.dirs.top_level.clone(),
    root: pool_dir
      .map(|dir| dir.join(LEASE_NOTES_DIR))
      .or_else(|| roots.places().next().map(Path::to_path_buf))
      .unwrap_or_default()
      .to_string_lossy()
      .into_owned(),
    keep_seconds: keep.as_secs(),
    notes: notes.iter().map(|notes| notes_output(notes, &held, keep, args.list)).collect(),
  })
}

fn notes_output(notes: &LeaseNotes, held: &HashSet<String>, keep: Duration, list: bool) -> NotesOutput {
  let record = &notes.record;
  let (files, files_omitted) = if list {
    let (files, more) = note_files(&notes.dir);
    (
      Some(
        files
          .into_iter()
          .map(|file| NoteFileOutput {
            path: file.path,
            bytes: file.bytes,
          })
          .collect(),
      ),
      more,
    )
  } else {
    (None, false)
  };
  NotesOutput {
    lease_id: record.lease_id.clone(),
    holder: record.holder.clone(),
    slot: record.slot.clone(),
    branch: record.branch.clone(),
    path: notes.dir.to_string_lossy().into_owned(),
    leased_at: record.leased_at,
    returned_at: record.returned_at,
    held: held.contains(&record.lease_id),
    expires_at: expires_at(record, held, keep),
    bytes: notes.bytes(),
    preserved: record.preserved.as_ref().map(|preserved| PreservedOutput {
      // the expiry of the notes as it is now
      expires_at: expires_at(record, held, keep),
      ..PreservedOutput::of(preserved)
    }),
    preserved_at: record.preserved_at,
    files,
    files_omitted,
  }
}
