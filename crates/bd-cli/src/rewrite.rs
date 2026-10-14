//! `bd group`, `bd rename`, `bd restack`, `bd unstack` and `bd undo`: message-only history rewrites through
//! `sync_core::regroup`, which validates before it writes and holds the repository lock

use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::output::SchemaVersion;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use sync_core::regroup::{
  RegroupError, RegroupOptions, RegroupOutcome, RegroupPlan, describe_problem, problem_commits, regroup_commits_core, rename_branch_core, restack_branch_core, undo_rewrite_core,
};
use sync_core::stack::StackForest;

/// The options every rewrite takes
#[derive(Debug, Default, clap::Args)]
pub struct RewriteArgs {
  /// Print the resulting subjects and the stack forest; write nothing
  #[arg(long)]
  pub dry_run: bool,
  /// Rewrite only if HEAD is still this commit (the `head` that `bd status` printed)
  #[arg(long, value_name = "ID")]
  pub expect_head: Option<String>,
  /// Also print the old -> new id of every commit
  #[arg(long)]
  pub map: bool,
}

#[derive(Debug, Default, clap::Args)]
pub struct GroupArgs {
  /// The plan `{"branches": {"path": [ids]}, "unassign": [ids]}`; default: stdin
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
  /// The `undo_token` a rewrite printed: `<previous head>..<new head>`
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
  /// Pass to `bd undo` to restore `previous_head`; omitted for a dry run or when nothing changed
  #[serde(skip_serializing_if = "Option::is_none")]
  pub undo_token: Option<String>,
  /// Every branch afterwards with its commits (new ids), oldest first
  pub groups: BTreeMap<String, Vec<String>>,
  /// The unassigned commits afterwards, oldest first
  pub unassigned: Vec<String>,
  /// Stack problems that already existed: reported, not refused
  #[serde(skip_serializing_if = "Vec::is_empty")]
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
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SubjectChange {
  pub id: String,
  pub from: String,
  pub to: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ForestBranch {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent: Option<String>,
  /// `false` for an ancestor only prefixes declare
  pub has_commits: bool,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub problems: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProblemReport {
  pub branch: String,
  pub message: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub commits: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct IdMapping {
  pub from: String,
  pub to: String,
}

/// `bd undo`
#[derive(Debug, Serialize, JsonSchema)]
pub struct UndoOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  /// HEAD now: the `previous_head` of the token
  pub head: String,
  /// The HEAD that was undone
  pub undone: String,
}

fn options(context: &RepoContext, args: &RewriteArgs) -> RegroupOptions {
  RegroupOptions {
    expect_head: args.expect_head.clone(),
    dry_run: args.dry_run,
    lock_timeout: context.lock_timeout,
  }
}

pub fn group(context: &RepoContext, args: &GroupArgs) -> Result<RewriteOutput, CliError> {
  let text = match &args.plan {
    Some(path) => std::fs::read_to_string(path).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("Cannot read {}: {e}", path.display())))?,
    None => std::io::read_to_string(std::io::stdin()).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("Cannot read the plan from stdin: {e}")))?,
  };
  let plan: RegroupPlan = serde_json::from_str(&text).map_err(|e| CliError::new(ErrorCode::InvalidPlan, format!("The plan is not valid JSON of the expected shape: {e}")))?;
  let outcome = regroup_commits_core(&context.git, &context.repository, &plan, &options(context, &args.rewrite)).map_err(regroup_error)?;
  Ok(rewrite_output(context, "group", &args.rewrite, &outcome))
}

pub fn rename(context: &RepoContext, args: &RenameArgs) -> Result<RewriteOutput, CliError> {
  let outcome = rename_branch_core(&context.git, &context.repository, &args.old, &args.new, &options(context, &args.rewrite)).map_err(regroup_error)?;
  Ok(rewrite_output(context, "rename", &args.rewrite, &outcome))
}

pub fn restack(context: &RepoContext, args: &RestackArgs) -> Result<RewriteOutput, CliError> {
  let outcome = restack_branch_core(&context.git, &context.repository, &args.branch, Some(&args.parent), &options(context, &args.rewrite)).map_err(regroup_error)?;
  Ok(rewrite_output(context, "restack", &args.rewrite, &outcome))
}

pub fn unstack(context: &RepoContext, args: &UnstackArgs) -> Result<RewriteOutput, CliError> {
  let outcome = restack_branch_core(&context.git, &context.repository, &args.branch, None, &options(context, &args.rewrite)).map_err(regroup_error)?;
  Ok(rewrite_output(context, "unstack", &args.rewrite, &outcome))
}

pub fn undo(context: &RepoContext, args: &UndoArgs) -> Result<UndoOutput, CliError> {
  let Some((previous_head, new_head)) = args.token.split_once("..").filter(|(previous, new)| !previous.is_empty() && !new.is_empty()) else {
    return Err(CliError::new(
      ErrorCode::InvalidArguments,
      format!("{:?} is not an undo token (<previous head>..<new head>)", args.token),
    ));
  };
  undo_rewrite_core(&context.git, &context.repository, previous_head, new_head, context.lock_timeout).map_err(regroup_error)?;
  let short = context.short_ids([previous_head, new_head]);
  Ok(UndoOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    head: short.get(previous_head).to_string(),
    undone: short.get(new_head).to_string(),
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
    RegroupError::Locked(_) => ErrorCode::Locked,
    RegroupError::OperationInProgress(_) => ErrorCode::OperationInProgress,
    RegroupError::DetachedHead(_) => ErrorCode::DetachedHead,
    RegroupError::Other(_) => ErrorCode::GitError,
  };
  CliError {
    code,
    message,
    commits: error.commits().to_vec(),
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
  let wrote = !args.dry_run && outcome.new_head != outcome.previous_head;
  RewriteOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    operation: operation.to_string(),
    dry_run: args.dry_run,
    previous_head: short.get(&outcome.previous_head).to_string(),
    new_head: short.get(&outcome.new_head).to_string(),
    rewritten: changed,
    undo_token: wrote.then(|| format!("{}..{}", outcome.previous_head, outcome.new_head)),
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
          id: short.get(&commit.id).to_string(),
          from: first_line(&commit.previous_message),
          to: first_line(&commit.message),
        })
        .collect()
    }),
    forest: args.dry_run.then_some(outcome.forest.as_ref()).flatten().map(forest_branches),
    map: args.map.then(|| {
      outcome
        .commits
        .iter()
        .map(|commit| IdMapping {
          from: short.get(&commit.id).to_string(),
          to: short.get(&commit.new_id).to_string(),
        })
        .collect()
    }),
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
      has_commits: node.has_commits,
      problems: node.problems.iter().map(|problem| describe_problem(name, problem)).collect(),
    })
    .collect()
}
