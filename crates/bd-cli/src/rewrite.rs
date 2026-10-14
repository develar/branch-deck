//! `bd group`, `bd rename`, `bd restack`, `bd unstack` and `bd undo`: message-only history rewrites through
//! `sync_core::regroup`, which validates before it writes. bd holds the repository lock over the rewrite, the
//! descriptions of a plan and, with --sync, the sync

use crate::context::RepoContext;
use crate::describe::Description;
use crate::error::{CliError, ErrorBody, ErrorCode};
use crate::group_plan::{changed_descriptions, check_descriptions, parse_group_plan, write_descriptions};
use crate::output::SchemaVersion;
use crate::status::read_head;
use crate::sync_command::{SyncOutput, lock, sync_locked};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use sync_core::regroup::{
  RegroupError, RegroupOptions, RegroupOutcome, UndoToken, describe_problem, problem_commits, regroup_commits_accepted, rename_branch_core, restack_branch_core, undo_regroup_core,
};
use sync_core::stack::StackForest;

/// The options every rewrite takes
#[derive(Debug, Default, clap::Args)]
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
  /// Also print the old -> new id of every commit
  #[arg(long)]
  pub map: bool,
}

#[derive(Debug, Default, clap::Args)]
pub struct GroupArgs {
  /// The plan `{"branches": {"path": [ids]}, "unassign": [ids], "issue_groups": {"name": [keys]}, "ungroup_issues":
  /// [keys], "descriptions": {"branch": {"title", "body"}}}`; default: stdin
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
  /// The `undo_token` a rewrite printed: `<previous head>..<new head>`, `groups:<previous>..<new>` or both joined by `+`
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
  /// Pass to `bd undo` to restore `previous_head` and the issue groups: `H1..H2`, `groups:G1..G2` or
  /// `H1..H2+groups:G1..G2`; omitted for a dry run or when nothing changed
  #[serde(skip_serializing_if = "Option::is_none")]
  pub undo_token: Option<String>,
  /// When the issue groups change (or would change): every issue group afterwards with its keys
  #[serde(skip_serializing_if = "Option::is_none")]
  pub issue_groups: Option<BTreeMap<String, Vec<String>>>,
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
  /// The branches whose description the plan's `descriptions` set (or, with --dry-run, would set): the ones that differ
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub described: Vec<String>,
  /// With --sync: the `bd sync` output after the rewrite; exit code 1 when a branch conflicts
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sync: Option<SyncOutput>,
  /// A step after the rewrite failed (a description or the sync): the rewrite stays (`undo_token` undoes it), and the
  /// exit code is this error's
  #[serde(skip_serializing_if = "Option::is_none")]
  pub after_write_error: Option<ErrorBody>,
}

impl RewriteOutput {
  /// 0, 1 when the sync conflicts, or the exit code of `after_write_error`
  pub fn exit_code(&self) -> i32 {
    match (&self.after_write_error, &self.sync) {
      (Some(error), _) => error.code.exit_code(),
      (None, Some(sync)) if sync.has_conflicts() => ErrorCode::Conflict.exit_code(),
      _ => 0,
    }
  }
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
  /// HEAD now: the `previous_head` of the token, or HEAD as it is for a token of the issue groups only
  pub head: String,
  /// The HEAD that was undone; omitted for a token of the issue groups only
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub undone: Option<String>,
  /// The commit of the issue groups that was undone; omitted for a token without them
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub undone_issue_groups: Option<String>,
}

pub fn group(context: &RepoContext, args: &GroupArgs) -> Result<RewriteOutput, CliError> {
  let text = match &args.plan {
    Some(path) => std::fs::read_to_string(path).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("Cannot read {}: {e}", path.display())))?,
    None => std::io::read_to_string(std::io::stdin()).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("Cannot read the plan from stdin: {e}")))?,
  };
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
/// `descriptions` and, with --sync, sync. A failure after the rewrite is `after_write_error`, so the output keeps the
/// undo token
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
    dry_run: args.dry_run,
    lock_timeout: context.lock_timeout,
    held_lock: held_lock.as_ref(),
  };
  // read before the rewrite: only the writes can fail after it
  let described = changed_descriptions(context, descriptions)?;
  let outcome = regroup(&options).map_err(regroup_error)?;
  let mut output = rewrite_output(context, operation, args, &outcome);
  output.described = described.iter().map(|(branch, _, _)| (*branch).to_string()).collect();
  if args.dry_run {
    return Ok(output);
  }
  if let Err(error) = write_descriptions(context, &described) {
    output.after_write_error = Some(error.to_output().error);
    return Ok(output);
  }
  if args.sync {
    match sync_locked(context) {
      Ok(sync) => output.sync = Some(sync),
      Err(error) => output.after_write_error = Some(error.to_output().error),
    }
  }
  Ok(output)
}

pub fn undo(context: &RepoContext, args: &UndoArgs) -> Result<UndoOutput, CliError> {
  let Some(token) = UndoToken::parse(&args.token) else {
    return Err(CliError::new(
      ErrorCode::InvalidArguments,
      format!(
        "{:?} is not an undo token (<previous head>..<new head>, groups:<previous>..<new>, or both joined by +)",
        args.token
      ),
    ));
  };
  undo_regroup_core(&context.git, &context.repository, &token, context.lock_timeout).map_err(regroup_error)?;
  let head = match &token.head {
    Some((previous_head, _)) => previous_head.clone(),
    None => read_head(context)?.0,
  };
  let undone = token.head.as_ref().map(|(_, new_head)| new_head.as_str());
  let undone_issue_groups = token.issue_groups.as_ref().map(|(_, new)| new.as_str());
  let short = context.short_ids(std::iter::once(head.as_str()).chain(undone).chain(undone_issue_groups));
  Ok(UndoOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    head: short.get(&head).to_string(),
    undone: undone.map(|id| short.get(id).to_string()),
    undone_issue_groups: undone_issue_groups.map(|id| short.get(id).to_string()),
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
    RegroupError::Locked(_) => ErrorCode::Locked,
    RegroupError::OperationInProgress(_) => ErrorCode::OperationInProgress,
    RegroupError::DetachedHead(_) => ErrorCode::DetachedHead,
    RegroupError::Other(_) => ErrorCode::GitError,
  };
  CliError {
    code,
    message,
    commits: error.commits().to_vec(),
    branches: Vec::new(),
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
    undo_token: UndoToken::of(outcome).map(|token| token.to_string()),
    issue_groups,
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
    described: Vec::new(),
    sync: None,
    after_write_error: None,
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
