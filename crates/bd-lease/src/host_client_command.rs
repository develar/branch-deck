//! `bd host`: the git of a client of a remote run (the client of yonder, IJAI-4792; spec/cli/lease.spec.md, "Host
//! runs"), so the client starts no git of its own: the commit the run ships with the commits the host may have
//! (`snapshot`), its thin pack into a private file (`pack`), and the changes of the command applied to the working tree
//! (`apply`). The cores are `git_ops::host_client` and `git_ops::host_objects`; the host side is `bd lease host`

use crate::host_command::ShownBlobOutput;
use crate::lease_command::{LeaseContext, lease_error};
use bd_core::context::Environment;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use git_ops::host_client::{FARTHEST, apply_patch, candidates, commit_count, pack_commit, repository_name, working_tree_commit};
use git_ops::host_objects::show_commit;
use git_ops::lease_slot::lease_config;
use git_ops::worktree_snapshot::{StatusEntry, local_changes, read_head_commit};
use schemars::JsonSchema;
use serde::Serialize;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The directory of the run records and the patches of the client, in the git dir of a checkout
const PATCH_DIR: &str = "yonder";
/// The directory of the pack files of `bd host pack`, in the git dir of a checkout
const PACK_DIR: &str = "branch-deck/host/packs";

#[derive(Debug, clap::Args)]
pub struct HostArgs {
  #[command(subcommand)]
  pub command: HostCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum HostCommand {
  /// The commit a run ships (HEAD, or with --working-tree a commit of the working tree, built in a copy of the index),
  /// the local changes, the commits the host may have, the base and a file of the commit. No ref, index or file changes
  Snapshot(HostSnapshotArgs),
  /// The thin pack of a commit against the commits the host has, into a private file whose path the answer names
  Pack(HostPackArgs),
  /// Apply the patch of the changes of a run to the files of the working tree, not the index: all or nothing
  Apply(HostApplyArgs),
}

#[derive(Debug, Default, clap::Args)]
pub struct HostSnapshotArgs {
  /// Ship the working tree: the tracked and untracked files that git does not ignore, as a commit on HEAD
  #[arg(long, conflicts_with = "no_status")]
  pub working_tree: bool,
  /// Skip the status: no `changed`, only HEAD and the candidates
  #[arg(long)]
  pub no_status: bool,
  /// A file of the commit whose text the answer carries (at most 1 MiB), such as `.yonder.toml`
  #[arg(long, value_name = "PATH")]
  pub blob: Option<String>,
  /// The farthest first-parent distance from HEAD of a candidate
  #[arg(long, value_name = "N", default_value_t = FARTHEST)]
  pub farthest: usize,
}

#[derive(Debug, Default, clap::Args)]
pub struct HostPackArgs {
  /// The commit of the pack, as a full id
  pub commit: String,
  /// The commits the host has, as full ids; at least one: a pack of the whole history is no thin pack
  #[arg(long, value_name = "ID", num_args = 1.., required = true)]
  pub have: Vec<String>,
}

#[derive(Debug, Default, clap::Args)]
pub struct HostApplyArgs {
  /// The patch of `bd lease host changes`
  pub patch: PathBuf,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HostSnapshotOutput {
  pub schema: SchemaVersion,
  /// The commit the run ships: HEAD, or the commit of the working tree
  pub commit: String,
  pub head: String,
  /// The lines of `git status --porcelain` (`XY path`) beyond HEAD; absent with --no-status
  #[serde(skip_serializing_if = "Option::is_none")]
  pub changed: Option<Vec<String>>,
  /// The directory name of the main worktree, without `.git`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
  /// The git dir of the checkout
  pub git_dir: String,
  /// The directory of the run records and the patches of the client, `<git_dir>/yonder`; bd does not make it
  pub patch_dir: String,
  /// The tip of `origin/master`, else of `master`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub base: Option<String>,
  /// HEAD, its first-parent ancestors at the distances 1, 2, 4 … --farthest, the merge bases with `origin/master` and
  /// `master`
  pub candidates: Vec<String>,
  /// With --blob: `null` when the commit has no file at the path
  #[serde(skip_serializing_if = "Option::is_none")]
  pub blob: Option<Option<ShownBlobOutput>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HostPackOutput {
  pub schema: SchemaVersion,
  /// The pack file, mode 0600; the caller sends it and removes it
  pub path: String,
  pub bytes: u64,
  /// The commits of the pack: those of the commit that the --have commits lack
  pub commits: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HostApplyOutput {
  pub schema: SchemaVersion,
  pub patch: String,
  /// The files the patch changed
  pub files: usize,
}

/// `bd host snapshot`
pub fn host_snapshot(context: &LeaseContext, args: &HostSnapshotArgs) -> Result<HostSnapshotOutput, CliError> {
  let git = &context.git;
  let repo = context.dirs.top_level.as_str();
  let git_dir = Path::new(&context.dirs.git_dir);
  let head = read_head_commit(git, repo).map_err(lease_error)?;
  let (commit, changed) = if args.no_status {
    (head.commit.clone(), None)
  } else {
    let config = lease_config(git, repo).map_err(lease_error)?;
    let (commit, entries) = if args.working_tree {
      let shipped = working_tree_commit(git, repo, git_dir, &head, &config).map_err(lease_error)?;
      (shipped.commit, shipped.entries)
    } else {
      (head.commit.clone(), local_changes(git, repo, &config).map_err(lease_error)?)
    };
    (commit, Some(entries.iter().map(StatusEntry::line).collect()))
  };
  let found = candidates(git, repo, &head.commit, args.farthest).map_err(lease_error)?;
  let blob = match &args.blob {
    Some(path) => {
      let shown = show_commit(git, repo, &commit, Some(path), &[]).map_err(lease_error)?;
      Some(shown.blob.map(|blob| ShownBlobOutput {
        path: path.clone(),
        id: blob.id,
        text: blob.text,
      }))
    }
    None => None,
  };
  Ok(HostSnapshotOutput {
    schema: SchemaVersion,
    commit,
    head: head.commit,
    changed,
    repository: repository_name(Path::new(&context.dirs.git_common_dir)),
    git_dir: context.dirs.git_dir.clone(),
    patch_dir: git_dir.join(PATCH_DIR).to_string_lossy().into_owned(),
    base: found.base,
    candidates: found.commits,
    blob,
  })
}

/// `bd host pack`: git's progress on stderr, a line each
pub fn host_pack(context: &LeaseContext, args: &HostPackArgs) -> Result<HostPackOutput, CliError> {
  if let Some(bad) = std::iter::once(&args.commit).chain(&args.have).find(|id| !git_ops::revs::is_object_id(id)) {
    return Err(CliError::new(ErrorCode::InvalidArguments, format!("{bad:?} is no full commit id")));
  }
  let repo = context.dirs.top_level.as_str();
  let dir = Path::new(&context.dirs.git_dir).join(PACK_DIR);
  let mut progress = |line: &str| {
    let _ = writeln!(std::io::stderr(), "bd host pack: {line}");
  };
  let pack = pack_commit(&context.git, repo, &args.commit, &args.have, &dir, &mut progress).map_err(lease_error)?;
  let commits = commit_count(&context.git, repo, &args.commit, &args.have).map_err(lease_error)?;
  Ok(HostPackOutput {
    schema: SchemaVersion,
    path: pack.path.to_string_lossy().into_owned(),
    bytes: pack.bytes,
    commits,
  })
}

/// `bd host apply`: the patch relative to the current directory
pub fn host_apply(context: &LeaseContext, environment: &Environment, args: &HostApplyArgs) -> Result<HostApplyOutput, CliError> {
  let patch = environment.cwd.join(&args.patch);
  if !patch.is_file() {
    return Err(CliError::new(ErrorCode::InvalidArguments, format!("No patch file {}", patch.display())));
  }
  let files = apply_patch(&context.git, &context.dirs.top_level, &patch).map_err(lease_error)?;
  Ok(HostApplyOutput {
    schema: SchemaVersion,
    patch: patch.to_string_lossy().into_owned(),
    files,
  })
}
