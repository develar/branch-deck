//! `bd undo`: the undo of a token a rewrite, `bd update` or `bd pick` printed. A rewrite token goes to
//! `history_rewrite::regroup::undo_regroup_core`; `update:` and `pick:` tokens to the undo of their command, which moves
//! the files back too

use crate::pick_command::undo_pick;
use crate::update_command::undo_update;
use bd_core::context::RepoContext;
use bd_core::error::{CliError, ErrorCode, regroup_error};
use bd_core::output::SchemaVersion;
use git_ops::revs::read_head;
use history_rewrite::baseline::NoBaseline;
use history_rewrite::regroup::{UndoToken, undo_regroup_core};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, clap::Args)]
pub struct UndoArgs {
  /// The `undo_token` a rewrite printed: `<previous head>..<new head>`, `groups:<previous>..<new>`,
  /// `stacks:<previous>..<new>`, or several of them joined by `+` in this order; or the `update:…` of `bd update` or
  /// the `pick:…[+follow:…]` of `bd pick`, which move the files back too
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
  /// For an `update:` or `pick:` token: it was undone before, and nothing changed now; a repeated undo is no error
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub already_undone: Option<bool>,
  /// For a `pick:` token with `+follow:`: whether the worktree that followed the pick moved back (`false`: it was
  /// back already)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub follow_undone: Option<bool>,
}

/// HEAD after an undo: the previous head of the token's move, or HEAD as it is for a token without one
fn head_after_undo(context: &RepoContext, moved: Option<&(String, String)>) -> Result<String, CliError> {
  match moved {
    Some((previous_head, _)) => Ok(previous_head.clone()),
    None => Ok(read_head(&context.git, &context.repository)?.ok_or_else(|| anyhow::Error::new(NoBaseline::NoCommits))?.0),
  }
}

pub fn undo(context: &RepoContext, args: &UndoArgs) -> Result<UndoOutput, CliError> {
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
    });
  }
  let Some(token) = UndoToken::parse(&args.token) else {
    return Err(CliError::new(
      ErrorCode::InvalidArguments,
      format!(
        "{:?} is not an undo token (<previous head>..<new head>, groups:<previous>..<new>, stacks:<previous>..<new>, parked:<previous>..<new>, or several joined by + in this \
         order; update:… and pick:… as bd update and bd pick printed them)",
        args.token
      ),
    ));
  };
  undo_regroup_core(&context.git, &context.repository, &token, context.lock_timeout).map_err(|error| {
    let mut error = regroup_error(error);
    error.code = match error.code {
      // the token names no write of the issue groups or the stack parents: a wrong argument, not a wrong plan
      ErrorCode::InvalidPlan => ErrorCode::InvalidArguments,
      // a ref moved on since the rewrite: the same token can never succeed, as for `update:` and `pick:`
      ErrorCode::HeadMoved | ErrorCode::GroupsMoved | ErrorCode::StacksMoved | ErrorCode::ParkedMoved => ErrorCode::UndoStale,
      code => code,
    };
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
  })
}
