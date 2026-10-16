//! `bd check`: would the branches build with this plan? A trial sync (`sync_core::check`) that writes git objects and
//! the cache only

use crate::context::RepoContext;
use crate::error::CliError;
use crate::group_plan::{GroupPlan, check_descriptions, parse_group_plan};
use crate::output::{SchemaVersion, ShortIds};
use crate::rewrite::regroup_error;
use crate::status::{StackBlockSummary, StackIssue, stack_issues};
use crate::text_input::{read_stdin_unless_terminal, read_text};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use sync_core::check::{BranchCheck, CheckOutcome, CheckReport, CheckSession};
use sync_core::diff_scan::{CommitDiff, read_commit_diffs_cached};
use sync_types::stack::BranchStack;

#[derive(Debug, Default, clap::Args)]
pub struct CheckArgs {
  /// The plan of `bd group`, `-` for stdin; default: stdin if it is not a terminal, and without a plan the current
  /// grouping
  #[arg(long, value_name = "FILE")]
  pub plan: Option<PathBuf>,
}

/// `bd check`
#[derive(Debug, Serialize, JsonSchema)]
pub struct CheckOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub head: String,
  pub baseline: String,
  /// The commit the branches without a parent are built on
  pub base_commit: Option<String>,
  /// The subjects the plan changes
  pub rewritten: usize,
  /// Every branch builds
  pub ok: bool,
  /// Parents before their children
  pub branches: Vec<CheckedBranch>,
  pub unassigned_count: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CheckedBranch {
  pub name: String,
  /// The parent its commits declare
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent: Option<String>,
  /// The branch it would be built on; `null` for `base_commit`
  pub built_on: Option<String>,
  pub commit_count: usize,
  /// `ok`, `conflict`, `blocked`, `problems` or `error`
  pub result: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub conflict: Option<CheckConflict>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub blocked_by: Option<StackBlockSummary>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub problems: Vec<StackIssue>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CheckConflict {
  /// The commit whose cherry-pick conflicts
  pub commit: String,
  pub subject: String,
  pub files: Vec<String>,
  /// Older commits outside the branch and its ancestors that change these files: what the change likely needs
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub missing_commits: Vec<String>,
}

pub fn check(context: &RepoContext, args: &CheckArgs) -> Result<CheckOutput, CliError> {
  let plan = read_plan(args.plan.as_ref())?;
  let cache = context.cache();
  let report = CheckSession::open(&context.git, &context.repository, &context.prefix, &cache)
    .map_err(regroup_error)?
    .verifying_mapping_notes(!context.use_cache)
    .check(plan.as_ref().map(|plan| &plan.regroup))
    .map_err(regroup_error)?;
  if let Some(plan) = &plan {
    check_descriptions(&plan.descriptions, &report.regroup.commits).map_err(regroup_error)?;
  }

  // the files of each commit, for the commits a conflict likely needs
  let ids: Vec<String> = report.regroup.commits.iter().map(|commit| commit.id.clone()).collect();
  let has_conflict = report.branches.iter().any(|branch| matches!(branch.outcome, CheckOutcome::Conflict { .. }));
  let files_of = if has_conflict {
    changed_files(&read_commit_diffs_cached(&context.git, &context.repository, &cache, &ids)?)
  } else {
    HashMap::new()
  };

  let short = context.short_ids(ids.iter().map(String::as_str).chain([report.head.as_str()]).chain(report.base.as_deref()));
  let branches = checked_branches(&report, &files_of, &short);

  Ok(CheckOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    head: short.get(&report.head).to_string(),
    baseline: report.baseline_branch.clone(),
    base_commit: report.base.as_deref().map(|base| short.get(base).to_string()),
    rewritten: report.regroup.changed_count(),
    ok: branches.iter().all(|branch| branch.result == "ok"),
    branches,
    unassigned_count: report.unassigned,
  })
}

/// Commit id -> every path its changes name, old paths of renames included
pub(crate) fn changed_files(diffs: &[CommitDiff]) -> HashMap<String, HashSet<String>> {
  diffs
    .iter()
    .map(|diff| {
      let files = diff
        .files
        .iter()
        .flat_map(|change| std::iter::once(change.path.clone()).chain(change.old_path.clone()))
        .collect();
      (diff.id.clone(), files)
    })
    .collect()
}

/// The check of each branch of `report`; `files_of` holds the changed files of each commit, for the commits a conflict
/// likely needs
pub(crate) fn checked_branches(report: &CheckReport, files_of: &HashMap<String, HashSet<String>>, short: &ShortIds) -> Vec<CheckedBranch> {
  let subjects: HashMap<&str, &str> = report
    .regroup
    .commits
    .iter()
    .map(|commit| (commit.id.as_str(), commit.message.lines().next().unwrap_or_default()))
    .collect();
  report
    .branches
    .iter()
    .map(|branch| {
      let mut checked = CheckedBranch {
        name: branch.name.clone(),
        parent: branch.declared_parent.clone(),
        built_on: branch.built_on.clone(),
        commit_count: branch.commits.len(),
        result: String::new(),
        conflict: None,
        blocked_by: None,
        problems: Vec::new(),
        error: None,
      };
      checked.result = match &branch.outcome {
        CheckOutcome::Ok { .. } => "ok",
        CheckOutcome::Conflict { commit, files } => {
          checked.conflict = Some(CheckConflict {
            commit: short.get(commit).to_string(),
            subject: subjects.get(commit.as_str()).copied().unwrap_or_default().to_string(),
            files: files.clone(),
            missing_commits: missing_commits(report, branch, commit, files, files_of)
              .iter()
              .map(|id| short.get(id).to_string())
              .collect(),
          });
          "conflict"
        }
        CheckOutcome::Blocked { parent, reason } => {
          checked.blocked_by = Some(StackBlockSummary::of(parent, *reason));
          "blocked"
        }
        CheckOutcome::Problems(problems) => {
          let stack = BranchStack {
            problems: problems.clone(),
            ..BranchStack::default()
          };
          checked.problems = stack_issues(&branch.name, &stack, short);
          "problems"
        }
        CheckOutcome::Error { message } => {
          checked.error = Some(message.clone());
          "error"
        }
      }
      .to_string();
      checked
    })
    .collect()
}

/// The plan from `--plan` (`-` for stdin), else from stdin unless it is a terminal; `None` for no plan
fn read_plan(path: Option<&PathBuf>) -> Result<Option<GroupPlan>, CliError> {
  let text = match path {
    Some(path) => read_text(path, "the plan")?,
    None => read_stdin_unless_terminal("the plan")?,
  };
  if text.trim().is_empty() {
    return Ok(None);
  }
  parse_group_plan(&text).map(Some)
}

/// The commits older than `commit` that change one of `files` and are neither in the branch nor in the branches it is
/// built on
pub(crate) fn missing_commits(report: &CheckReport, branch: &BranchCheck, commit: &str, files: &[String], files_of: &HashMap<String, HashSet<String>>) -> Vec<String> {
  let mut present: HashSet<&str> = branch.commits.iter().map(String::as_str).collect();
  let mut ancestor = branch.built_on.as_deref();
  while let Some(name) = ancestor {
    let Some(parent) = report.branches.iter().find(|check| check.name == name) else { break };
    present.extend(parent.commits.iter().map(String::as_str));
    ancestor = parent.built_on.as_deref();
  }
  report
    .regroup
    .commits
    .iter()
    .map(|regrouped| regrouped.id.as_str())
    .take_while(|id| *id != commit)
    .filter(|id| !present.contains(id))
    .filter(|id| files_of.get(*id).is_some_and(|changed| files.iter().any(|file| changed.contains(file))))
    .map(str::to_string)
    .collect()
}
