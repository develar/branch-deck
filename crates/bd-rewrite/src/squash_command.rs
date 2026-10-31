//! `bd squash <into> <commit>…`: the commits the user asked to squash go into the oldest of them, as the app's Squash
//! does (`history_rewrite::squash`, spec/commit/squash.spec.md), recorded in the operations registry as the same
//! intent (`RewriteIntent::Squash`); its core takes the repository lock after it computed the squash, for the witness
//! of HEAD and the move only. One rewrite that keeps HEAD's tree, so the index and the working tree stay as they are;
//! `bd undo` takes it back. `--dry-run` builds the squash and drops it, so a conflict shows before the user approves

use crate::rewrite::{after_rewrite_exit_code, sync_after_rewrite};
use bd_core::context::RepoContext;
use bd_core::error::{CliError, ErrorBody};
use bd_core::operation_record::{asked_on, recorded_unless};
use bd_core::output::SchemaVersion;
use bd_core::sync_command::SyncOutput;
use bd_core::text_input::read_text;
use domain_types::error::ErrorCode;
use domain_types::operation::Intent;
use domain_types::rewrite::RewriteIntent;
use git_ops::revs::{DetachedHead, current_branch};
use git_ops::rewrite_guard::{HeadMoved, ensure_no_operation_in_progress};
use git_ops::rewrite_plan::refused_in;
use history_rewrite::regroup::UndoToken;
use history_rewrite::repository_lock::{LockWait, RepositoryLocked, RewriteOptions};
use history_rewrite::squash::{SquashCommitsParams, SquashSelection, SquashSelectionRefused, SquashText, check_squash_commits_core, select_squash, squash_commits_core};
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Default, clap::Args)]
pub struct SquashArgs {
  /// The commit the others go into, the oldest of them, by the id `bd status` printed (abbreviated or full)
  pub into: String,
  /// The commits squashed into it, newer ones of its branch
  #[arg(required = true)]
  pub commits: Vec<String>,
  /// The subject of the squashed commit, the `(prefix)` included; the body of `<into>` stays. Without --subject,
  /// --message and --message-file the message of `<into>` stays
  #[arg(long, conflicts_with_all = ["message", "message_file"])]
  pub subject: Option<String>,
  /// The whole message of the squashed commit, the `(prefix)` of its subject included
  #[arg(long, conflicts_with = "message_file")]
  pub message: Option<String>,
  /// The whole message from a file, `-` for stdin
  #[arg(long, value_name = "FILE")]
  pub message_file: Option<PathBuf>,
  /// Squash only if HEAD is still this commit (the `head` that `bd status` printed)
  #[arg(long, value_name = "ID")]
  pub expect_head: Option<String>,
  /// Build the squash and drop it: a conflict refuses it as the squash would; write nothing
  #[arg(long)]
  pub dry_run: bool,
  /// Then rebuild the virtual branches as `bd sync` does, after the repository lock is released
  #[arg(long, conflicts_with = "dry_run")]
  pub sync: bool,
}

/// `bd squash`
#[derive(Debug, Serialize, JsonSchema)]
pub struct SquashOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub dry_run: bool,
  pub previous_head: String,
  /// HEAD afterwards; `previous_head` for a dry run
  pub new_head: String,
  /// The commit the others went into, before the squash
  pub into: String,
  /// The squashed commit; omitted for a dry run
  #[serde(skip_serializing_if = "Option::is_none")]
  pub new_id: Option<String>,
  /// The commits folded into it, oldest first
  pub squashed: Vec<String>,
  /// The subject of the squashed commit
  pub subject: String,
  /// Its branch; omitted for an unassigned commit
  #[serde(skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  /// The branches the next sync builds anew, so a pushed one needs a push again: the branch of the squashed commit and
  /// those of the commits between it and the newest squashed one. The copies of the later commits stay
  pub rebuilt_branches: Vec<String>,
  /// `<previous_head>..<new_head>`: `bd undo` restores `previous_head`; omitted for a dry run
  #[serde(skip_serializing_if = "Option::is_none")]
  pub undo_token: Option<String>,
  /// With --sync: the `bd sync` output after the squash; exit code 1 when a branch is not built
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sync: Option<SyncOutput>,
  /// The sync after the squash failed: the squash stands, `undo_token` reverts it; retry `bd sync`, never the squash
  #[serde(skip_serializing_if = "Option::is_none")]
  pub after_error: Option<ErrorBody>,
}

impl SquashOutput {
  /// 0, 1 when the sync conflicts, or the exit code of `after_error`
  #[must_use]
  pub fn exit_code(&self) -> i32 {
    after_rewrite_exit_code(self.after_error.as_ref(), self.sync.as_ref())
  }
}

pub fn squash(context: &RepoContext, args: &SquashArgs) -> Result<SquashOutput, CliError> {
  ensure_guards(context)?;
  let text = match (&args.subject, &args.message, &args.message_file) {
    (Some(subject), _, _) => SquashText::Subject(subject.clone()),
    (_, Some(message), _) => SquashText::Message(message.clone()),
    (_, _, Some(path)) => SquashText::Message(read_text(path, "the message")?),
    (None, None, None) => SquashText::Target,
  };
  let selection = select_squash(&context.git, &context.repository, &args.into, &args.commits, &text).map_err(|e| match e.downcast_ref::<SquashSelectionRefused>() {
    Some(refused) => CliError {
      commits: refused.commits().to_vec(),
      ..CliError::new(refused.code(), refused.to_string())
    },
    None => CliError::from(e),
  })?;
  let params = SquashCommitsParams {
    repository_path: context.repository.clone(),
    commit_ids: std::iter::once(selection.target.clone()).chain(selection.folded.iter().cloned()).collect(),
    message: selection.message.clone(),
  };
  let options = RewriteOptions {
    expect_head: args.expect_head.clone(),
    lock_wait: LockWait::Bounded(context.lock_timeout),
    held_lock: None,
  };
  if args.dry_run {
    let head = check_squash_commits_core(&context.git, &params, &options).map_err(squash_error)?;
    return Ok(squash_output(context, &selection, (&head, &head), None));
  }

  let intent = RewriteIntent::Squash {
    commit_ids: params.commit_ids.clone(),
    message: params.message.clone(),
    witness: asked_on(args.expect_head.as_deref()),
  };
  let mut output = recorded_unless(context, false, &Intent::Rewrite(Box::new(intent)), || {
    let result = squash_commits_core(&context.git, &params, &options).map_err(squash_error)?;
    Ok(squash_output(context, &selection, (&result.previous_head, &result.new_head), Some(&result.new_commit_id)))
  })?;
  if args.sync {
    (output.sync, output.after_error) = sync_after_rewrite(context);
  }
  Ok(output)
}

/// The guards of a rewrite of the checked out branch, with their codes, before the commits are read
fn ensure_guards(context: &RepoContext) -> Result<(), CliError> {
  ensure_no_operation_in_progress(&context.git, &context.repository).map_err(|e| CliError::new(ErrorCode::OperationInProgress, format!("{e:#}")))?;
  current_branch(&context.git, &context.repository)?.ok_or_else(|| CliError::new(ErrorCode::DetachedHead, DetachedHead.to_string()))?;
  Ok(())
}

/// The code of an error of a squash (also of `bd fold`) or of its trial: the refusal of the history with the commits and
/// files it names, a HEAD that moved, the lock another process holds
pub(crate) fn squash_error(error: anyhow::Error) -> CliError {
  if let Some(refused) = refused_in(&error) {
    return CliError {
      commits: refused.commits.clone(),
      paths: refused.paths.clone(),
      ..CliError::new(refused.code(), refused.message.clone())
    };
  }
  if let Some(moved) = error.downcast_ref::<HeadMoved>() {
    return CliError::new(ErrorCode::HeadMoved, moved.to_string());
  }
  match error.downcast_ref::<RepositoryLocked>() {
    Some(locked) => CliError::new(ErrorCode::Locked, locked.to_string()),
    None => CliError::from(error),
  }
}

/// The undo token of a rewrite that moved only HEAD (a squash, a fold): `<previous_head>..<new_head>`; none when it
/// moved nothing
pub(crate) fn head_undo_token(previous_head: &str, new_head: &str) -> Option<String> {
  (new_head != previous_head).then(|| {
    UndoToken {
      head: Some((previous_head.to_string(), new_head.to_string())),
      issue_groups: None,
      stack_parents: None,
      parked: None,
    }
    .to_string()
  })
}

fn squash_output(context: &RepoContext, selection: &SquashSelection, (previous_head, new_head): (&str, &str), new_id: Option<&str>) -> SquashOutput {
  let ids = std::iter::once(selection.target.as_str())
    .chain(selection.folded.iter().map(String::as_str))
    .chain(new_id)
    .chain([previous_head, new_head]);
  let short = context.short_ids(ids);
  SquashOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    dry_run: new_id.is_none(),
    previous_head: short.get(previous_head).to_string(),
    new_head: short.get(new_head).to_string(),
    into: short.get(&selection.target).to_string(),
    new_id: new_id.map(|id| short.get(id).to_string()),
    squashed: selection.folded.iter().map(|id| short.get(id).to_string()).collect(),
    subject: selection.message.lines().next().unwrap_or_default().to_string(),
    branch: selection.branch.clone(),
    rebuilt_branches: selection.rebuilt_branches.clone(),
    undo_token: head_undo_token(previous_head, new_head),
    sync: None,
    after_error: None,
  }
}
