//! `bd group`, `bd rename`, `bd restack`, `bd unstack` and `bd undo`: message-only history rewrites through
//! `sync_core::regroup`, which validates before it writes. bd holds the repository lock over the rewrite, the
//! descriptions of a plan and, with --sync, the sync

use crate::context::RepoContext;
use crate::error::{CliError, ErrorBody, ErrorCode};
use crate::group_plan::{DescriptionChange, changed_descriptions, check_descriptions, parse_group_plan, write_descriptions};
use crate::output::SchemaVersion;
use crate::pick_command::undo_pick;
use crate::status::ParentSourceName;
use crate::sync_command::{SyncOutput, lock, sync_locked};
use crate::text_input::read_text;
use crate::update_command::undo_update;
use git_ops::branch_description::Description;
use git_ops::revs::read_head;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use sync_core::baseline::NoBaseline;
use sync_core::regroup::{
  RegroupError, RegroupOptions, RegroupOutcome, UndoToken, describe_problem, problem_commits, regroup_commits_accepted, rename_branch_core, restack_branch_core, undo_regroup_core,
};
use sync_core::stack::StackForest;

/// The options every rewrite takes
#[derive(Debug, Default, clap::Args)]
#[expect(clippy::struct_excessive_bools, reason = "command-line flags")]
pub struct RewriteArgs {
  /// Print the resulting subjects and the stack forest; write nothing
  #[arg(long)]
  pub dry_run: bool,
  /// Then rebuild the virtual branches as `bd sync` does, under the same repository lock
  #[arg(long, conflicts_with = "dry_run")]
  pub sync: bool,
  /// Rewrite only if HEAD is still this commit (the `head` that `bd status` printed)
  #[arg(long, value_name = "ID")]
  pub expect_head: Option<String>,
  /// With --expect-head: also accept commits added on top of it since then; they are printed as `appended`
  #[arg(long, requires = "expect_head")]
  pub allow_append: bool,
  /// Also print the old -> new id of every commit
  #[arg(long)]
  pub map: bool,
}

#[derive(Debug, Default, clap::Args)]
pub struct GroupArgs {
  /// The plan `{"branches": {"path": [ids]}, "unassign": [ids], "issue_groups": {"name": [keys]}, "ungroup_issues":
  /// [keys], "unstack": [branches], "descriptions": {"branch": {"title", "body"}}}`; default: stdin
  #[arg(long, value_name = "FILE")]
  pub plan: Option<PathBuf>,
  #[command(flatten)]
  pub rewrite: RewriteArgs,
}

#[derive(Debug, clap::Args)]
pub struct RenameArgs {
  /// The branch to rename
  pub old: String,
  /// The new name, a branch name without `/`
  pub new: String,
  #[command(flatten)]
  pub rewrite: RewriteArgs,
}

#[derive(Debug, clap::Args)]
pub struct RestackArgs {
  pub branch: String,
  /// The new parent branch
  pub parent: String,
  #[command(flatten)]
  pub rewrite: RewriteArgs,
}

#[derive(Debug, clap::Args)]
pub struct UnstackArgs {
  pub branch: String,
  #[command(flatten)]
  pub rewrite: RewriteArgs,
}

#[derive(Debug, clap::Args)]
pub struct UndoArgs {
  /// The `undo_token` a rewrite printed: `<previous head>..<new head>`, `groups:<previous>..<new>`,
  /// `stacks:<previous>..<new>`, or several of them joined by `+` in this order; or the `update:…` of `bd update` or
  /// the `pick:…[+follow:…]` of `bd pick`, which move the files back too
  pub token: String,
}

/// The output of `bd group`, `rename`, `restack` and `unstack`
#[derive(Debug, Serialize, JsonSchema)]
pub struct RewriteOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  /// `group`, `rename`, `restack` or `unstack`
  pub operation: String,
  pub dry_run: bool,
  pub previous_head: String,
  /// HEAD afterwards; `previous_head` for a dry run or when nothing changed
  pub new_head: String,
  /// The commits whose message changed (or would change)
  pub rewritten: usize,
  /// With --allow-append: the commits added on top of --expect-head since then, oldest first, by their new ids
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub appended: Vec<AppendedCommit>,
  /// Pass to `bd undo` to restore `previous_head`, the issue groups and the stack parents: the parts `H1..H2`,
  /// `groups:G1..G2` and `stacks:S1..S2` of the refs that moved, joined by `+`; omitted for a dry run or when nothing
  /// changed
  #[serde(skip_serializing_if = "Option::is_none")]
  pub undo_token: Option<String>,
  /// When the issue groups change (or would change): every issue group afterwards with its keys
  #[serde(skip_serializing_if = "Option::is_none")]
  pub issue_groups: Option<BTreeMap<String, Vec<String>>>,
  /// When the stack parents change (or would change): every stack parent afterwards, child -> parent, those of other
  /// worktrees included
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stack_parents: Option<BTreeMap<String, String>>,
  /// Every branch afterwards with its commits (new ids), oldest first
  pub groups: BTreeMap<String, Vec<String>>,
  /// The unassigned commits afterwards, oldest first
  pub unassigned: Vec<String>,
  /// Stack problems that already existed: reported, not refused
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub problems: Vec<ProblemReport>,
  /// With --dry-run: the subjects that change
  #[serde(skip_serializing_if = "Option::is_none")]
  pub changes: Option<Vec<SubjectChange>>,
  /// With --dry-run and stacked branches on: every branch and its parent afterwards
  #[serde(skip_serializing_if = "Option::is_none")]
  pub forest: Option<Vec<ForestBranch>>,
  /// With --map: old -> new id of every commit ahead of the baseline
  #[serde(skip_serializing_if = "Option::is_none")]
  pub map: Option<Vec<IdMapping>>,
  /// The branches whose description the plan's `descriptions` wrote (with --dry-run: would write), the ones that differ
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub described: Vec<String>,
  /// The texts those descriptions replaced, by branch; `bd undo` doesn't restore them, `bd describe` can
  #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
  pub replaced_descriptions: BTreeMap<String, Description>,
  /// With --sync: the `bd sync` output after the rewrite; exit code 1 when a branch is not built
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sync: Option<SyncOutput>,
  /// A step after the rewrite failed: a description write (the sync is skipped then) or the sync. The rewrite stands,
  /// `undo_token` reverts it, and the exit code is this error's; retry that step (`bd sync`), never the rewrite
  #[serde(skip_serializing_if = "Option::is_none")]
  pub after_error: Option<ErrorBody>,
}

impl RewriteOutput {
  /// 0, 1 when the sync conflicts, or the exit code of `error`
  pub fn exit_code(&self) -> i32 {
    after_rewrite_exit_code(self.after_error.as_ref(), self.sync.as_ref())
  }
}

/// The exit code of a rewrite whose output carries its `--sync` and the `error` of a step after it: that error's, 1
/// when the sync conflicts, else 0
pub fn after_rewrite_exit_code(error: Option<&ErrorBody>, sync: Option<&SyncOutput>) -> i32 {
  match (error, sync) {
    (Some(error), _) => error.code.exit_code(),
    (None, Some(sync)) if sync.has_unbuilt() => ErrorCode::Conflict.exit_code(),
    _ => 0,
  }
}

/// `--sync` after a rewrite, under the lock the caller holds: the sync output, or its error, which the rewrite output
/// carries so that its undo token stays
pub fn sync_after_rewrite(context: &RepoContext) -> (Option<SyncOutput>, Option<ErrorBody>) {
  match sync_locked(context) {
    Ok(sync) => (Some(sync), None),
    Err(error) => (None, Some(error.to_output().error)),
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AppendedCommit {
  pub commit: String,
  pub subject: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SubjectChange {
  pub commit: String,
  pub old_subject: String,
  pub new_subject: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ForestBranch {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent: Option<String>,
  /// Where `parent` comes from: `prefix` or `store` (the stack parents)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent_source: Option<ParentSourceName>,
  /// `false` for an ancestor only prefixes or the stack parents declare
  pub has_commits: bool,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub problems: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProblemReport {
  pub branch: String,
  pub message: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub commits: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
/// A commit and its copy: the id before a rewrite and after it
pub struct IdMapping {
  pub commit: String,
  pub new_id: String,
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

pub fn group(context: &RepoContext, args: &GroupArgs) -> Result<RewriteOutput, CliError> {
  let text = read_text(args.plan.as_deref().unwrap_or_else(|| Path::new("-")), "the plan")?;
  let plan = parse_group_plan(&text)?;
  rewrite(context, "group", &args.rewrite, &plan.descriptions, |options| {
    regroup_commits_accepted(&context.git, &context.repository, &plan.regroup, options, |outcome| {
      check_descriptions(&plan.descriptions, &outcome.commits)
    })
  })
}

pub fn rename(context: &RepoContext, args: &RenameArgs) -> Result<RewriteOutput, CliError> {
  rewrite(context, "rename", &args.rewrite, &BTreeMap::new(), |options| {
    rename_branch_core(&context.git, &context.repository, &args.old, &args.new, options)
  })
}

pub fn restack(context: &RepoContext, args: &RestackArgs) -> Result<RewriteOutput, CliError> {
  rewrite(context, "restack", &args.rewrite, &BTreeMap::new(), |options| {
    restack_branch_core(&context.git, &context.repository, &args.branch, Some(&args.parent), options)
  })
}

pub fn unstack(context: &RepoContext, args: &UnstackArgs) -> Result<RewriteOutput, CliError> {
  rewrite(context, "unstack", &args.rewrite, &BTreeMap::new(), |options| {
    restack_branch_core(&context.git, &context.repository, &args.branch, None, options)
  })
}

/// Run `regroup` under the repository lock (none for a dry run), then under the same lock write the changed
/// `descriptions` and, with --sync, sync. A failure after the rewrite is the `error` of the rewrite output, so the
/// output keeps the undo token
fn rewrite(
  context: &RepoContext,
  operation: &str,
  args: &RewriteArgs,
  descriptions: &BTreeMap<String, Description>,
  regroup: impl FnOnce(&RegroupOptions) -> Result<RegroupOutcome, RegroupError>,
) -> Result<RewriteOutput, CliError> {
  let held_lock = if args.dry_run { None } else { Some(lock(context)?) };
  let options = RegroupOptions {
    expect_head: args.expect_head.clone(),
    allow_append: args.allow_append,
    dry_run: args.dry_run,
    lock_timeout: context.lock_timeout,
    held_lock: held_lock.as_ref(),
  };
  // read before the rewrite: only the writes can fail after it
  let changed = changed_descriptions(context, descriptions)?;
  let outcome = regroup(&options).map_err(regroup_error)?;
  let mut output = rewrite_output(context, operation, args, &outcome);
  if args.dry_run {
    set_described(&mut output, &changed);
    return Ok(output);
  }
  let (written, error) = write_descriptions(context, &changed);
  set_described(&mut output, &changed[..written]);
  if let Some(error) = error {
    output.after_error = Some(error.to_output().error);
    return Ok(output);
  }
  if args.sync {
    (output.sync, output.after_error) = sync_after_rewrite(context);
  }
  Ok(output)
}

fn set_described(output: &mut RewriteOutput, changes: &[DescriptionChange]) {
  output.described = changes.iter().map(|change| change.branch.to_string()).collect();
  output.replaced_descriptions = changes.iter().filter_map(|change| Some((change.branch.to_string(), change.previous.clone()?))).collect();
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
        "{:?} is not an undo token (<previous head>..<new head>, groups:<previous>..<new>, stacks:<previous>..<new>, or several joined by + in this \
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
      ErrorCode::HeadMoved | ErrorCode::GroupsMoved | ErrorCode::StacksMoved => ErrorCode::UndoStale,
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

/// The error code of a refused or failed regroup; by value, for `map_err`
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(regroup_error)`")]
pub fn regroup_error(error: RegroupError) -> CliError {
  let message = error.to_string();
  let code = match &error {
    RegroupError::InvalidPlan { .. } => ErrorCode::InvalidPlan,
    RegroupError::NotAheadOfBaseline { .. } => ErrorCode::NotAheadOfBaseline,
    RegroupError::HeadMoved(_) => ErrorCode::HeadMoved,
    RegroupError::GroupsMoved(_) => ErrorCode::GroupsMoved,
    RegroupError::StacksMoved(_) => ErrorCode::StacksMoved,
    RegroupError::Locked(_) => ErrorCode::Locked,
    RegroupError::OperationInProgress(_) => ErrorCode::OperationInProgress,
    RegroupError::DetachedHead(_) => ErrorCode::DetachedHead,
    RegroupError::Other(error) if error.downcast_ref::<NoBaseline>().is_some() => ErrorCode::NoBaseline,
    RegroupError::Other(_) => ErrorCode::GitError,
  };
  CliError {
    code,
    message,
    commits: error.commits().to_vec(),
    branches: Vec::new(),
    paths: Vec::new(),
    review: None,
  }
}

fn rewrite_output(context: &RepoContext, operation: &str, args: &RewriteArgs, outcome: &RegroupOutcome) -> RewriteOutput {
  let short = context.short_ids(
    outcome
      .commits
      .iter()
      .flat_map(|commit| [commit.id.as_str(), commit.new_id.as_str()])
      .chain([outcome.previous_head.as_str(), outcome.new_head.as_str()]),
  );
  let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
  let mut unassigned = Vec::new();
  for commit in &outcome.commits {
    let id = short.get(&commit.new_id).to_string();
    match &commit.group {
      Some(group) => groups.entry(group.clone()).or_default().push(id),
      None => unassigned.push(id),
    }
  }
  let changed = outcome.changed_count();
  let issue_groups = outcome.issue_groups.as_ref().map(|change| {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (key, name) in &change.entries {
      groups.entry(name.clone()).or_default().push(key.clone());
    }
    groups
  });
  RewriteOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    operation: operation.to_string(),
    dry_run: args.dry_run,
    previous_head: short.get(&outcome.previous_head).to_string(),
    new_head: short.get(&outcome.new_head).to_string(),
    rewritten: changed,
    appended: outcome
      .appended
      .iter()
      .filter_map(|id| outcome.commits.iter().find(|commit| commit.id == *id))
      .map(|commit| AppendedCommit {
        commit: short.get(&commit.new_id).to_string(),
        subject: commit.message.lines().next().unwrap_or_default().to_string(),
      })
      .collect(),
    undo_token: UndoToken::of(outcome).map(|token| token.to_string()),
    issue_groups,
    stack_parents: outcome.stack_parents.as_ref().map(|change| change.entries.clone()),
    groups,
    unassigned,
    problems: outcome
      .kept_problems
      .iter()
      .map(|(branch, problem)| ProblemReport {
        branch: branch.clone(),
        message: describe_problem(branch, problem),
        commits: problem_commits(problem).iter().map(|id| short.get(id).to_string()).collect(),
      })
      .collect(),
    changes: args.dry_run.then(|| {
      outcome
        .commits
        .iter()
        .filter(|commit| commit.changed)
        .map(|commit| SubjectChange {
          commit: short.get(&commit.id).to_string(),
          old_subject: first_line(&commit.previous_message),
          new_subject: first_line(&commit.message),
        })
        .collect()
    }),
    forest: args.dry_run.then_some(outcome.forest.as_ref()).flatten().map(forest_branches),
    map: args.map.then(|| {
      outcome
        .commits
        .iter()
        .map(|commit| IdMapping {
          commit: short.get(&commit.id).to_string(),
          new_id: short.get(&commit.new_id).to_string(),
        })
        .collect()
    }),
    described: Vec::new(),
    replaced_descriptions: BTreeMap::new(),
    sync: None,
    after_error: None,
  }
}

fn first_line(message: &str) -> String {
  message.lines().next().unwrap_or_default().to_string()
}

fn forest_branches(forest: &StackForest) -> Vec<ForestBranch> {
  forest
    .nodes
    .iter()
    .map(|(name, node)| ForestBranch {
      name: name.clone(),
      parent: node.declared_parent.clone(),
      parent_source: node.parent_source.map(ParentSourceName::from),
      has_commits: node.has_commits,
      problems: node.problems.iter().map(|problem| describe_problem(name, problem)).collect(),
    })
    .collect()
}
