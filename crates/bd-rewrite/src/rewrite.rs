//! `bd group`, `bd rename`, `bd restack` and `bd unstack`: message-only history rewrites through
//! `history_rewrite::regroup`, which validates before it writes. The rewrite is recorded in the operations registry of
//! the repository before the lock (`bd_core::operation_record::recorded`: the app's windows show it, the journal keeps
//! it), then bd holds the repository lock over the rewrite and the descriptions of a plan; with --sync, the record ended
//! and the lock released, the sync runs, as `bd sync` does

use crate::group_plan::{DescriptionChange, changed_descriptions, check_descriptions, parse_group_plan, write_descriptions};
use bd_core::branch_view::ParentSourceName;
use bd_core::context::RepoContext;
use bd_core::error::regroup_error;
use bd_core::error::{CONFLICT_EXIT, CliError, ErrorBody};
use bd_core::operation_record::{asked_on, recorded_unless};
use bd_core::output::SchemaVersion;
use bd_core::sync_command::{SyncOutput, lock, sync};
use bd_core::text_input::read_text;
use domain_types::operation::Intent;
use domain_types::rewrite::RewriteIntent;
use git_ops::branch_description::Description;
use history_rewrite::regroup::{RegroupError, RegroupOptions, RegroupOutcome, UndoToken, problem_commits, regroup_commits_accepted, rename_branch_core, restack_branch_core};
use history_rewrite::repository_lock::{LockWait, RepositoryLock};
use history_rewrite::stack::{StackForest, describe_problem, problem_view};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The options every rewrite takes
#[derive(Debug, Default, clap::Args)]
#[expect(clippy::struct_excessive_bools, reason = "command-line flags")]
pub struct RewriteArgs {
  /// Print the resulting subjects and the stack forest; write nothing
  #[arg(long)]
  pub dry_run: bool,
  /// Then rebuild the virtual branches as `bd sync` does, after the repository lock is released
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
  /// [keys], "unstack": [branches], "descriptions": {"branch": {"title", "body"}}}`, `-` for stdin
  #[arg(long, value_name = "FILE")]
  pub plan: PathBuf,
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
  /// The branch to stack, by its leaf
  pub branch: String,
  /// The new parent branch
  pub parent: String,
  #[command(flatten)]
  pub rewrite: RewriteArgs,
}

#[derive(Debug, clap::Args)]
pub struct UnstackArgs {
  /// The stacked branch, by its leaf
  pub branch: String,
  #[command(flatten)]
  pub rewrite: RewriteArgs,
}

/// The output of `bd group`, `rename`, `restack`, `unstack` and `reword`
#[derive(Debug, Serialize, JsonSchema)]
pub struct RewriteOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  /// `group`, `rename`, `restack`, `unstack` or `reword`
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
  /// With --dry-run, and always for `reword`: the commits whose message changes (or would change)
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
    (None, Some(sync)) if sync.has_unbuilt() => CONFLICT_EXIT,
    _ => 0,
  }
}

/// `--sync` after a rewrite, which released its lock (`held_lock`, moved in): the sync output, or its error, which the
/// rewrite output carries so that its undo token stays. The sync takes no lock, so it never makes another writer wait
pub fn sync_after_rewrite(context: &RepoContext, held_lock: Option<RepositoryLock>) -> (Option<SyncOutput>, Option<ErrorBody>) {
  drop(held_lock);
  match sync(context) {
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
  /// The id before the rewrite
  pub commit: String,
  /// The id after it; omitted for a dry run
  #[serde(skip_serializing_if = "Option::is_none")]
  pub new_id: Option<String>,
  pub old_subject: String,
  pub new_subject: String,
  /// The message after the subject line changes too
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub body_changed: bool,
  /// The commit goes to another branch (its `(prefix)` changed): from where to where; omitted when it stays
  #[serde(skip_serializing_if = "Option::is_none")]
  pub moved: Option<BranchMove>,
}

/// The branch of a commit before and after a rewrite; `None` for unassigned
#[derive(Debug, Serialize, JsonSchema)]
pub struct BranchMove {
  pub from: Option<String>,
  pub to: Option<String>,
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

pub fn group(context: &RepoContext, args: &GroupArgs) -> Result<RewriteOutput, CliError> {
  let text = read_text(&args.plan, "the plan")?;
  let plan = parse_group_plan(&text)?;
  let intent = RewriteIntent::Group {
    plan: plan.regroup.clone(),
    witness: asked_on(args.rewrite.expect_head.as_deref()),
  };
  rewrite(context, ("group", intent), &args.rewrite, &plan.descriptions, |options| {
    regroup_commits_accepted(&context.git, &context.repository, &plan.regroup, options, |outcome| {
      check_descriptions(&plan.descriptions, &outcome.commits)
    })
  })
}

pub fn rename(context: &RepoContext, args: &RenameArgs) -> Result<RewriteOutput, CliError> {
  let intent = RewriteIntent::Rename {
    branch_name: args.old.clone(),
    new_name: args.new.clone(),
    witness: asked_on(args.rewrite.expect_head.as_deref()),
  };
  rewrite(context, ("rename", intent), &args.rewrite, &BTreeMap::new(), |options| {
    rename_branch_core(&context.git, &context.repository, &args.old, &args.new, options)
  })
}

pub fn restack(context: &RepoContext, args: &RestackArgs) -> Result<RewriteOutput, CliError> {
  let intent = RewriteIntent::Restack {
    branch_name: args.branch.clone(),
    parent: args.parent.clone(),
    witness: asked_on(args.rewrite.expect_head.as_deref()),
  };
  rewrite(context, ("restack", intent), &args.rewrite, &BTreeMap::new(), |options| {
    restack_branch_core(&context.git, &context.repository, &args.branch, Some(&args.parent), options)
  })
}

pub fn unstack(context: &RepoContext, args: &UnstackArgs) -> Result<RewriteOutput, CliError> {
  let intent = RewriteIntent::Unstack {
    branch_name: args.branch.clone(),
    witness: asked_on(args.rewrite.expect_head.as_deref()),
  };
  rewrite(context, ("unstack", intent), &args.rewrite, &BTreeMap::new(), |options| {
    restack_branch_core(&context.git, &context.repository, &args.branch, None, options)
  })
}

/// Run `regroup`, the rewrite `operation` (its name in the output, the intent of its record), recorded in the operations
/// registry of the repository (none for a dry run); its core takes the repository lock after it computed, for the
/// witness and the write. Changed `descriptions` are written under the lock of the rewrite: then bd takes it before the
/// regroup and writes them after it; with --sync, end the record, release the lock and sync. A failure after the
/// rewrite is the `error` of the rewrite output, so the output keeps the undo token
pub(crate) fn rewrite(
  context: &RepoContext,
  operation: (&str, RewriteIntent),
  args: &RewriteArgs,
  descriptions: &BTreeMap<String, Description>,
  regroup: impl FnOnce(&RegroupOptions) -> Result<RegroupOutcome, RegroupError>,
) -> Result<RewriteOutput, CliError> {
  let (name, intent) = operation;
  let locked = || rewrite_locked(context, name, args, descriptions, regroup);
  let (mut output, held_lock) = recorded_unless(context, args.dry_run, &Intent::Rewrite(Box::new(intent)), locked)?;
  if args.sync && output.after_error.is_none() {
    (output.sync, output.after_error) = sync_after_rewrite(context, held_lock);
  }
  Ok(output)
}

/// `rewrite` up to its sync: its output, and the lock it holds (only to write descriptions)
fn rewrite_locked(
  context: &RepoContext,
  operation: &str,
  args: &RewriteArgs,
  descriptions: &BTreeMap<String, Description>,
  regroup: impl FnOnce(&RegroupOptions) -> Result<RegroupOutcome, RegroupError>,
) -> Result<(RewriteOutput, Option<RepositoryLock>), CliError> {
  // read before the rewrite: only the writes can fail after it
  let changed = changed_descriptions(context, descriptions)?;
  // the descriptions go under the lock of the rewrite; else its core takes the lock after it computed
  let held_lock = if args.dry_run || changed.is_empty() { None } else { Some(lock(context)?) };
  let options = RegroupOptions {
    expect_head: args.expect_head.clone(),
    allow_append: args.allow_append,
    dry_run: args.dry_run,
    lock_wait: LockWait::Bounded(context.lock_timeout),
    held_lock: held_lock.as_ref(),
  };
  let outcome = regroup(&options).map_err(regroup_error)?;
  let mut output = rewrite_output(context, operation, args, &outcome);
  if args.dry_run {
    set_described(&mut output, &changed);
    return Ok((output, held_lock));
  }
  let (written, error) = write_descriptions(context, &changed);
  set_described(&mut output, &changed[..written]);
  // the sync is skipped after a failed write of a description
  output.after_error = error.map(|error| error.to_output().error);
  Ok((output, held_lock))
}

fn set_described(output: &mut RewriteOutput, changes: &[DescriptionChange]) {
  output.described = changes.iter().map(|change| change.branch.to_string()).collect();
  output.replaced_descriptions = changes.iter().filter_map(|change| Some((change.branch.to_string(), change.previous.clone()?))).collect();
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
        commits: problem_commits(&problem.problem).iter().map(|id| short.get(id).to_string()).collect(),
      })
      .collect(),
    changes: (args.dry_run || operation == "reword").then(|| {
      outcome
        .commits
        .iter()
        .filter(|commit| commit.changed)
        .map(|commit| SubjectChange {
          commit: short.get(&commit.id).to_string(),
          new_id: (!args.dry_run).then(|| short.get(&commit.new_id).to_string()),
          old_subject: first_line(&commit.previous_message),
          new_subject: first_line(&commit.message),
          body_changed: body(&commit.previous_message) != body(&commit.message),
          moved: (commit.previous_group != commit.group).then(|| BranchMove {
            from: commit.previous_group.clone(),
            to: commit.group.clone(),
          }),
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

/// The message after its subject line, trimmed
fn body(message: &str) -> &str {
  message.split_once('\n').map_or("", |(_, body)| body.trim())
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
      problems: node.problems.iter().map(|problem| describe_problem(name, &problem_view(forest, name, problem))).collect(),
    })
    .collect()
}
