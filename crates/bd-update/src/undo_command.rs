//! `bd undo`: the undo of a token a rewrite, `bd update`, `bd pick` or `bd unapply` printed. A rewrite token goes to
//! `history_rewrite::regroup::undo_regroup_core`; `update:`, `pick:` and `unapply:` tokens to the undo of their command,
//! which moves the files back too. Each is recorded in the operations registry of the repository before the lock, as the
//! undo of its kind (`RewriteIntent::Undo`)

use crate::pick_command::undo_pick;
use crate::unapply_command::{undo_reapply, undo_unapply};
use crate::update_command::undo_update;
use bd_core::context::RepoContext;
use bd_core::error::{CliError, regroup_error};
use bd_core::operation_record::recorded;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use domain_types::operation::Intent;
use domain_types::rewrite::{RewriteIntent, UndoKind, UndoToken as DomainUndoToken};
use git_ops::revs::read_head;
use history_rewrite::baseline::NoBaseline;
use history_rewrite::regroup::{UndoToken, undo_regroup_core};
use history_rewrite::repository_lock::LockWait;
use schemars::JsonSchema;
use serde::Serialize;
use sync_core::reapply_branch::ReapplyUndoToken;
use sync_core::unapply_branch::UnapplyUndoToken;
use update_core::pick::PickUndoToken;
use update_core::update_project::UpdateUndoToken;

#[derive(Debug, clap::Args)]
pub struct UndoArgs {
  /// The `undo_token` a rewrite printed: `<previous head>..<new head>`, `groups:<previous>..<new>`,
  /// `stacks:<previous>..<new>`, or several of them joined by `+` in this order; or the `update:…` of `bd update`, the
  /// `pick:…[+follow:…]` of `bd pick`, the `unapply:…` of `bd unapply` or the `reapply:…` of `bd reapply`, which move the
  /// files back too
  pub token: String,
}

/// `bd undo`
#[derive(Debug, Serialize, JsonSchema)]
pub struct UndoOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  /// HEAD before the undo: the token's new head; omitted for a token without a head
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub previous_head: Option<String>,
  /// HEAD now: the token's previous head, or HEAD as it is for a token without a head
  pub new_head: String,
  /// The commit of the issue groups that was undone; omitted for a token without them
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub undone_issue_groups: Option<String>,
  /// The commit of the stack parents that was undone; omitted for a token without them
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub undone_stack_parents: Option<String>,
  /// For an `update:` token: whether the report of the landed commits is the one from before the update again
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub landed_restored: Option<bool>,
  /// For an `update:`, `pick:`, `unapply:` or `reapply:` token: it was undone before, and nothing changed now; a repeated undo is
  /// no error
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub already_undone: Option<bool>,
  /// For a `pick:` token with `+follow:`: whether the worktree that followed the pick moved back (`false`: it was
  /// back already)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub follow_undone: Option<bool>,
  /// For an `unapply:` token: the virtual branch that is back; for a `reapply:` token: the unapplied branch that is back
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub restored_branch: Option<String>,
}

/// HEAD after an undo: the previous head of the token's move, or HEAD as it is for a token without one
fn head_after_undo(context: &RepoContext, moved: Option<&(String, String)>) -> Result<String, CliError> {
  match moved {
    Some((previous_head, _)) => Ok(previous_head.clone()),
    None => Ok(read_head(&context.git, &context.repository)?.ok_or_else(|| anyhow::Error::new(NoBaseline::NoCommits))?.0),
  }
}

/// The output of the undo of an unapply or a reapply: HEAD back from `new` to `previous`, `restored_branch` back
fn moved_back(context: &RepoContext, (previous, new): (&str, &str), already_undone: bool, restored_branch: String) -> UndoOutput {
  let short = context.short_ids([previous, new]);
  UndoOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    previous_head: Some(short.get(new).to_string()),
    new_head: short.get(previous).to_string(),
    undone_issue_groups: None,
    undone_stack_parents: None,
    landed_restored: None,
    already_undone: Some(already_undone),
    follow_undone: None,
    restored_branch: Some(restored_branch),
  }
}

pub fn undo(context: &RepoContext, args: &UndoArgs) -> Result<UndoOutput, CliError> {
  let intent = RewriteIntent::Undo {
    token: DomainUndoToken {
      kind: undo_kind(&args.token),
      token: args.token.clone(),
    },
  };
  recorded(context, &Intent::Rewrite(Box::new(intent)), || undone(context, args))
}

/// The core that undoes `token`, by its kind
fn undo_kind(token: &str) -> UndoKind {
  if ReapplyUndoToken::is_reapply_token(token) {
    UndoKind::Reapply
  } else if UnapplyUndoToken::is_unapply_token(token) {
    UndoKind::Unapply
  } else if PickUndoToken::is_pick_token(token) {
    UndoKind::Pick
  } else if UpdateUndoToken::is_update_token(token) {
    UndoKind::Update
  } else {
    UndoKind::Regroup
  }
}

/// `undo` once recorded
fn undone(context: &RepoContext, args: &UndoArgs) -> Result<UndoOutput, CliError> {
  if let Some(result) = undo_reapply(context, &args.token) {
    let (token, undone) = result?;
    return Ok(moved_back(context, (&token.previous_head, &token.new_head), undone.already_undone, token.unapplied_branch));
  }
  if let Some(result) = undo_unapply(context, &args.token) {
    let (token, undone) = result?;
    return Ok(moved_back(context, (&token.previous_head, &token.new_head), undone.already_undone, token.virtual_branch));
  }
  if let Some(result) = undo_pick(context, &args.token) {
    let (token, undone) = result?;
    let head = head_after_undo(context, token.head.as_ref())?;
    let undone_head = token.head.as_ref().map(|(_, new_head)| new_head.as_str());
    let short = context.short_ids(std::iter::once(head.as_str()).chain(undone_head));
    return Ok(UndoOutput {
      schema: SchemaVersion,
      repository: context.repository.clone(),
      prefix: context.prefix.clone(),
      previous_head: undone_head.map(|id| short.get(id).to_string()),
      new_head: short.get(&head).to_string(),
      undone_issue_groups: None,
      undone_stack_parents: None,
      landed_restored: None,
      already_undone: Some(undone.already_undone),
      follow_undone: token.follow.is_some().then_some(undone.follow_undone),
      restored_branch: None,
    });
  }
  if let Some(result) = undo_update(context, &args.token) {
    let undone = result?;
    let short = context.short_ids([undone.head.as_str(), undone.undone.as_str()]);
    return Ok(UndoOutput {
      schema: SchemaVersion,
      repository: context.repository.clone(),
      prefix: context.prefix.clone(),
      previous_head: Some(short.get(&undone.undone).to_string()),
      new_head: short.get(&undone.head).to_string(),
      undone_issue_groups: None,
      undone_stack_parents: None,
      landed_restored: Some(undone.landed_restored),
      already_undone: Some(undone.already_undone),
      follow_undone: None,
      restored_branch: None,
    });
  }
  let Some(token) = UndoToken::parse(&args.token) else {
    return Err(CliError::new(
      ErrorCode::InvalidArguments,
      format!(
        "{:?} is not an undo token (<previous head>..<new head>, groups:<previous>..<new>, stacks:<previous>..<new>, parked:<previous>..<new>, or several joined by + in this \
         order; update:…, pick:…, unapply:… and reapply:… as bd update, bd pick, bd unapply and bd reapply printed them)",
        args.token
      ),
    ));
  };
  undo_regroup_core(&context.git, &context.repository, &token, LockWait::Bounded(context.lock_timeout)).map_err(|error| {
    // a ref moved on since the rewrite: the same token can never succeed, as for `update:` and `pick:`, and its words
    // say so instead of the advice to retry; the token names no write of the issue groups or the stack parents: a wrong
    // argument, not a wrong plan
    let stale_words = error.undo_message();
    let mut error = regroup_error(error);
    error.code = error.code.in_undo();
    if let Some(words) = stale_words {
      error.message = words;
    }
    error
  })?;
  let head = head_after_undo(context, token.head.as_ref())?;
  let undone = token.head.as_ref().map(|(_, new_head)| new_head.as_str());
  let undone_issue_groups = token.issue_groups.as_ref().map(|(_, new)| new.as_str());
  let undone_stack_parents = token.stack_parents.as_ref().map(|(_, new)| new.as_str());
  let short = context.short_ids(std::iter::once(head.as_str()).chain(undone).chain(undone_issue_groups).chain(undone_stack_parents));
  Ok(UndoOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    previous_head: undone.map(|id| short.get(id).to_string()),
    new_head: short.get(&head).to_string(),
    undone_issue_groups: undone_issue_groups.map(|id| short.get(id).to_string()),
    undone_stack_parents: undone_stack_parents.map(|id| short.get(id).to_string()),
    landed_restored: None,
    already_undone: None,
    follow_undone: None,
    restored_branch: None,
  })
}
