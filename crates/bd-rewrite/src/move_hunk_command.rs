//! `bd move-hunk`: move the changes of one file, or some of its blocks, from one commit ahead of the baseline into
//! another (`trial_check::move_hunk`), under the repository lock. One rewrite that keeps HEAD's tree, so the index and the
//! working tree stay as they are; `bd undo` takes it back, --sync rebuilds the virtual branches after the lock is released

use crate::block_patch::PatchBlock;
use crate::rewrite::{IdMapping, after_rewrite_exit_code, sync_after_rewrite};
use bd_core::context::RepoContext;
use bd_core::error::regroup_error;
use bd_core::error::{CliError, ErrorBody};
use bd_core::output::SchemaVersion;
use bd_core::sync_command::{SyncOutput, lock};
use domain_types::error::ErrorCode;
use git_ops::move_hunk::MoveHunkMode;
use history_rewrite::regroup::UndoToken;
use schemars::JsonSchema;
use serde::Serialize;
use trial_check::move_hunk::{MoveHunkError, MoveHunkOptions, MoveHunkParams, MoveHunkResult, move_hunk_core};

#[derive(Debug, Default, clap::Args)]
pub struct MoveHunkArgs {
  /// The commit the change is in
  pub from: String,
  /// The commit it moves into: older or newer, ahead of the baseline too
  pub to: String,
  /// The file, relative to the repository root, as `bd commits --patch` prints it
  pub path: String,
  /// Move only this block of the file in <FROM>, by the id that `bd commits <FROM> --patch` prints
  /// (`12,3:12,5@1a2b3c4d`); repeatable. Default: the whole change of the file
  #[arg(long = "block", value_name = "ID")]
  pub blocks: Vec<String>,
  /// Move only if HEAD is still this commit (the `head` that `bd status` printed)
  #[arg(long, value_name = "ID")]
  pub expect_head: Option<String>,
  /// Print the blocks that would move and check the rewrite; write nothing but git objects
  #[arg(long)]
  pub dry_run: bool,
  /// Then rebuild the virtual branches as `bd sync` does, after the repository lock is released: another rewrite may come
  /// in between, and the sync then shows the newer HEAD
  #[arg(long, conflicts_with = "dry_run")]
  pub sync: bool,
}

/// `bd move-hunk`
#[derive(Debug, Serialize, JsonSchema)]
pub struct MoveHunkOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub dry_run: bool,
  pub previous_head: String,
  /// HEAD afterwards; `previous_head` for a dry run. Its tree is the tree of `previous_head`
  pub new_head: String,
  pub moved: MovedSummary,
  /// Old -> new id of every recreated commit, oldest first: the older of the two commits and every commit above it;
  /// empty for a dry run
  pub commits: Vec<IdMapping>,
  /// The commits whose changes differ (or would differ): the two, and the ones between whose diff of the file changed.
  /// The others keep author and committer
  pub changed: usize,
  /// `<previous_head>..<new_head>`: `bd undo` restores `previous_head`; omitted for a dry run
  #[serde(skip_serializing_if = "Option::is_none")]
  pub undo_token: Option<String>,
  /// With --sync: the `bd sync` output after the move; exit code 1 when a branch is not built
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sync: Option<SyncOutput>,
  /// The sync after the move failed. The move stands, `undo_token` reverts it, and the exit code is this error's; retry
  /// `bd sync`, never the move
  #[serde(skip_serializing_if = "Option::is_none")]
  pub after_error: Option<ErrorBody>,
}

impl MoveHunkOutput {
  /// 0, 1 when the sync conflicts, or the exit code of `error`
  pub fn exit_code(&self) -> i32 {
    after_rewrite_exit_code(self.after_error.as_ref(), self.sync.as_ref())
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MovedSummary {
  pub path: String,
  /// The commit the change was in, and the one it moved into (ids before the move)
  pub from: String,
  pub to: String,
  /// The blocks that moved, as `bd commits <from> --patch` printed them; every block of the file when the whole change
  /// moved (none for a binary file)
  pub blocks: Vec<PatchBlock>,
}

pub fn move_hunk(context: &RepoContext, args: &MoveHunkArgs) -> Result<MoveHunkOutput, CliError> {
  let held_lock = if args.dry_run { None } else { Some(lock(context)?) };
  let params = MoveHunkParams {
    from: args.from.clone(),
    to: args.to.clone(),
    path: args.path.clone(),
    blocks: args.blocks.clone(),
  };
  let options = MoveHunkOptions {
    expect_head: args.expect_head.clone(),
    mode: if args.dry_run { MoveHunkMode::DryRun } else { MoveHunkMode::Apply },
    lock_timeout: context.lock_timeout,
    held_lock: held_lock.as_ref(),
  };
  let result = move_hunk_core(&context.git, &context.repository, &context.cache(), &params, &options).map_err(|error| move_hunk_error(context, error))?;
  let mut output = move_hunk_output(context, args.dry_run, &result);
  if args.sync {
    (output.sync, output.after_error) = sync_after_rewrite(context, held_lock);
  }
  Ok(output)
}

fn move_hunk_error(context: &RepoContext, error: MoveHunkError) -> CliError {
  let message = error.to_string();
  match error {
    MoveHunkError::InvalidArguments(_) => CliError::new(ErrorCode::InvalidArguments, message),
    MoveHunkError::BlockNotFound { blocks, .. } => CliError {
      blocks: Some(Box::new(blocks)),
      ..CliError::new(ErrorCode::BlockNotFound, message)
    },
    MoveHunkError::Conflict { commit, path, .. } => CliError {
      commits: vec![context.short_ids([commit.as_str()]).get(&commit).to_string()],
      paths: vec![path],
      ..CliError::new(ErrorCode::HunkConflict, message)
    },
    MoveHunkError::Rewrite(error) => regroup_error(error),
  }
}

fn move_hunk_output(context: &RepoContext, dry_run: bool, result: &MoveHunkResult) -> MoveHunkOutput {
  let short = context.short_ids(result.commits.iter().flat_map(|(old, new)| [old.as_str(), new.as_str()]).chain([
    result.previous_head.as_str(),
    result.new_head.as_str(),
    result.from.as_str(),
    result.to.as_str(),
  ]));
  let undo_token = (!dry_run && result.new_head != result.previous_head).then(|| {
    UndoToken {
      head: Some((result.previous_head.clone(), result.new_head.clone())),
      issue_groups: None,
      stack_parents: None,
      parked: None,
    }
    .to_string()
  });
  MoveHunkOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    dry_run,
    previous_head: short.get(&result.previous_head).to_string(),
    new_head: short.get(&result.new_head).to_string(),
    moved: MovedSummary {
      path: result.path.clone(),
      from: short.get(&result.from).to_string(),
      to: short.get(&result.to).to_string(),
      blocks: result.blocks.iter().map(PatchBlock::from).collect(),
    },
    commits: result
      .commits
      .iter()
      .map(|(old, new)| IdMapping {
        commit: short.get(old).to_string(),
        new_id: short.get(new).to_string(),
      })
      .collect(),
    changed: result.changed.len(),
    undo_token,
    sync: None,
    after_error: None,
  }
}
