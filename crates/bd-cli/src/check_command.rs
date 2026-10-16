//! `bd check`: would the branches build with this plan? A trial sync (`sync_core::check`) that writes git objects and
//! the cache only

use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::group_plan::{GroupPlan, check_descriptions, parse_group_plan};
use crate::install::shell_word;
use crate::output::{SchemaVersion, ShortIds};
use crate::rewrite::regroup_error;
use crate::status::{StackBlockSummary, StackIssue, stack_issues};
use crate::text_input::{read_stdin_unless_terminal, read_text};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use sync_core::check::{BranchCheck, CheckOutcome, CheckReport, CheckSession};
use sync_core::diff_scan::read_commit_diffs_cached;
use sync_core::missing_commits::{changed_files, missing_commits};
use sync_core::regroup::RegroupPlan;
use sync_types::stack::BranchStack;

#[derive(Debug, Default, clap::Args)]
pub struct CheckArgs {
  /// The plan of `bd group`, `-` for stdin; default: stdin if it is not a terminal, and without a plan the current
  /// grouping
  #[arg(long, value_name = "FILE")]
  pub plan: Option<PathBuf>,
  /// Then run this shell command in the tree of each branch that is ok, extracted into a temporary directory outside
  /// the working tree; a failure is `build_failed`. Slow on a large repository: narrow it with --build-paths
  #[arg(long, value_name = "COMMAND")]
  pub build: Option<String>,
  /// With --build: extract only these paths (git pathspecs) of each tree
  #[arg(long, value_name = "PATHSPEC", num_args = 1.., requires = "build")]
  pub build_paths: Vec<String>,
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
  /// What may break a build though no commit proves it: `possible_dangling_reference`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub warnings: Vec<CheckWarning>,
}

/// A use of a name a branch removes that no commit outside it changes: in its virtual tree with no fix in HEAD, or on
/// the baseline tip, which got it after the base. The exit code doesn't count it
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CheckWarning {
  /// `possible_dangling_reference`
  pub kind: String,
  pub branch: String,
  pub identifier: String,
  pub file: String,
  pub line: u32,
  pub removed_by: String,
  /// The use is on the baseline tip, not in the virtual tree
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub on_baseline: bool,
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
  /// `ok`, `conflict`, `dangling_reference`, `blocked`, `problems` or `error`
  pub result: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub conflict: Option<CheckConflict>,
  /// The branch builds, but removes declarations its virtual tree still uses: a commit of another branch changes the
  /// uses (`result` is `dangling_reference`)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub dangling_references: Vec<DanglingReferenceSummary>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub blocked_by: Option<StackBlockSummary>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub problems: Vec<StackIssue>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
  /// With --build: how the command ended in the tree of the branch (`result` is `build_failed` when it failed)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub build: Option<BuildResult>,
  /// The branch conflicts (or has a dangling reference) because it needs a commit of a branch stacked on it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub inverted_stack: Option<InvertedStack>,
}

/// A parent that needs a commit of its own child: no order of the stack builds both
#[derive(Debug, Serialize, JsonSchema)]
pub struct InvertedStack {
  /// The parent
  pub branch: String,
  /// The branch stacked on it that has the commit it needs
  pub child: String,
  /// The fixes that a trial build passes, each as the `branches` and `unstack` to add to the plan of `bd group`
  pub fixes: Vec<InvertedStackFix>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InvertedStackFix {
  /// `move_to_child`: the commits of the parent that need the child move into it; `swap`: the parent stacks on the
  /// child
  pub kind: String,
  pub branches: BTreeMap<String, Vec<String>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub unstack: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BuildResult {
  /// `null` when the command could not start or a signal ended it
  pub exit_code: Option<i32>,
  /// The last lines of its stdout and stderr when it failed
  #[serde(skip_serializing_if = "Option::is_none")]
  pub output: Option<String>,
}

/// A use of a name the branch removes, in its virtual tree, that a commit outside the branch changes in HEAD
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DanglingReferenceSummary {
  pub identifier: String,
  /// Where the virtual tree uses it
  pub file: String,
  pub line: u32,
  /// The commit of the branch that removes the declaration
  pub removed_by: String,
  /// The commit outside the branch whose patch changes the use: the branch needs it
  pub fixed_by: String,
  /// The change in `fixed_by` uses what the branch declares: stacking either branch on the other can't help; move the
  /// change of `file` into `removed_by`
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub mutual: bool,
  /// With `mutual`: how the change of the use gets into the branch
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fix: Option<DanglingFix>,
}

/// The fix of a mutual dangling reference, to apply after the user approves
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DanglingFix {
  /// `move_commit`: `fixed_by` changes only `file`, so it moves into the branch (`branches`, to add to the plan of
  /// `bd group`); `move_hunk`: `command` moves the change of `file` from `fixed_by` into `removed_by` (the whole change
  /// of the file; narrow it with `--hunk`)
  pub kind: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub command: Option<String>,
  #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
  pub branches: BTreeMap<String, Vec<String>>,
}

/// Whether a branch of `report` has a proven mutual dangling reference
fn has_mutual_reference(report: &CheckReport) -> bool {
  report.dangling.values().flatten().any(|reference| reference.is_confirmed() && reference.mutual)
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
  let session = CheckSession::open(&context.git, &context.repository, &context.prefix, &cache)
    .map_err(regroup_error)?
    .verifying_mapping_notes(!context.use_cache);
  let report = session.check(plan.as_ref().map(|plan| &plan.regroup)).map_err(regroup_error)?;
  if let Some(plan) = &plan {
    check_descriptions(&plan.descriptions, &report.regroup.commits).map_err(regroup_error)?;
  }

  // the files of each commit, for the commits a conflict likely needs
  let ids: Vec<String> = report.regroup.commits.iter().map(|commit| commit.id.clone()).collect();
  let has_conflict = report.branches.iter().any(|branch| matches!(branch.outcome, CheckOutcome::Conflict { .. }));
  let files_of = if has_conflict || has_mutual_reference(&report) {
    changed_files(&read_commit_diffs_cached(&context.git, &context.repository, &cache, &ids)?)
  } else {
    HashMap::new()
  };

  let short = context.short_ids(ids.iter().map(String::as_str).chain([report.head.as_str()]).chain(report.base.as_deref()));
  let mut branches = checked_branches(&report, &files_of, &short, &context.bd);
  let warnings = possible_dangling_references(&report, &short);
  let user_plan = plan.as_ref().map_or_default(|plan| plan.regroup.clone());
  for (checked, branch) in branches.iter_mut().zip(&report.branches) {
    checked.inverted_stack = inverted_stack(&session, &user_plan, &report, branch, &files_of, &short);
  }
  if let Some(command) = &args.build {
    for (checked, branch) in branches.iter_mut().zip(&report.branches) {
      let CheckOutcome::Ok { head } = &branch.outcome else { continue };
      if checked.result != "ok" {
        continue;
      }
      let build = build_tree(context, &branch.name, head, command, &args.build_paths)?;
      if build.exit_code != Some(0) {
        checked.result = "build_failed".to_string();
      }
      checked.build = Some(build);
    }
  }

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
    warnings,
  })
}

/// The dangling references of `report` that no commit proves, as warnings
pub(crate) fn possible_dangling_references(report: &CheckReport, short: &ShortIds) -> Vec<CheckWarning> {
  report
    .dangling
    .iter()
    .flat_map(|(branch, references)| references.iter().map(move |reference| (branch, reference)))
    .filter(|(_, reference)| !reference.is_confirmed())
    .map(|(branch, reference)| CheckWarning {
      kind: "possible_dangling_reference".to_string(),
      branch: branch.clone(),
      identifier: reference.identifier.clone(),
      file: reference.file.clone(),
      line: reference.line,
      removed_by: short.get(&reference.removed_by).to_string(),
      on_baseline: reference.on_baseline,
    })
    .collect()
}

/// The dangling references of `branch` that a commit proves
pub(crate) fn confirmed_dangling_references(
  report: &CheckReport,
  branch: &str,
  short: &ShortIds,
  bd: &str,
  files_of: &HashMap<String, HashSet<String>>,
) -> Vec<DanglingReferenceSummary> {
  report
    .dangling
    .get(branch)
    .into_iter()
    .flatten()
    .filter_map(|reference| {
      let fixed_by = reference.fixed_by.as_deref().filter(|_| reference.is_confirmed())?;
      let (removed_by, fixed_by) = (short.get(&reference.removed_by).to_string(), short.get(fixed_by).to_string());
      Some(DanglingReferenceSummary {
        fix: reference.mutual.then(|| {
          let only_the_file = files_of
            .get(reference.fixed_by.as_deref().unwrap_or_default())
            .is_some_and(|files| files.iter().all(|path| *path == reference.file));
          match report.branches.iter().find(|check| check.name == branch).filter(|_| only_the_file) {
            Some(check) => DanglingFix {
              kind: "move_commit".to_string(),
              command: None,
              branches: BTreeMap::from([(plan_path(check), vec![fixed_by.clone()])]),
            },
            None => DanglingFix {
              kind: "move_hunk".to_string(),
              command: Some(format!("{bd} move-hunk {fixed_by} {removed_by} {}", shell_word(&reference.file))),
              branches: BTreeMap::new(),
            },
          }
        }),
        identifier: reference.identifier.clone(),
        file: reference.file.clone(),
        line: reference.line,
        removed_by,
        fixed_by,
        mutual: reference.mutual,
      })
    })
    .collect()
}

/// The check of each branch of `report`; `files_of` holds the changed files of each commit, for the commits a conflict
/// likely needs
pub(crate) fn checked_branches(report: &CheckReport, files_of: &HashMap<String, HashSet<String>>, short: &ShortIds, bd: &str) -> Vec<CheckedBranch> {
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
        dangling_references: Vec::new(),
        blocked_by: None,
        problems: Vec::new(),
        error: None,
        build: None,
        inverted_stack: None,
      };
      checked.result = match &branch.outcome {
        CheckOutcome::Ok { .. } => {
          checked.dangling_references = confirmed_dangling_references(report, &branch.name, short, bd, files_of);
          if checked.dangling_references.is_empty() { "ok" } else { "dangling_reference" }
        }
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

/// The proven dangling references of some branches, each with its branch, and the possible ones as warnings
pub(crate) type BranchDangling = (Vec<(String, DanglingReferenceSummary)>, Vec<CheckWarning>);

/// The dangling references of `branches` (a branch and the branches it is built on) in a check of the current grouping:
/// the ones a commit proves, by branch, and the possible ones as warnings
pub(crate) fn dangling_references_of(context: &RepoContext, branches: &[&str]) -> Result<BranchDangling, CliError> {
  let report = CheckSession::open(&context.git, &context.repository, &context.prefix, &context.cache())
    .map_err(regroup_error)?
    .verifying_mapping_notes(!context.use_cache)
    .check(None)
    .map_err(regroup_error)?;
  let short = context.short_ids(report.regroup.commits.iter().map(|commit| commit.id.as_str()));
  let files_of = if has_mutual_reference(&report) {
    let ids: Vec<String> = report.regroup.commits.iter().map(|commit| commit.id.clone()).collect();
    changed_files(&read_commit_diffs_cached(&context.git, &context.repository, &context.cache(), &ids)?)
  } else {
    HashMap::new()
  };
  let confirmed = branches
    .iter()
    .flat_map(|branch| {
      confirmed_dangling_references(&report, branch, &short, &context.bd, &files_of)
        .into_iter()
        .map(|reference| (branch.to_string(), reference))
    })
    .collect();
  let warnings = possible_dangling_references(&report, &short)
    .into_iter()
    .filter(|warning| branches.contains(&warning.branch.as_str()))
    .collect();
  Ok((confirmed, warnings))
}

/// What the build of `branch` fails on, in a sentence: the first dangling reference and how to fix it
pub(crate) fn dangling_reference_message(branch: &str, references: &[(String, DanglingReferenceSummary)]) -> String {
  let Some((owner, first)) = references.first() else { return String::new() };
  let fix = if let Some(fix) = &first.fix {
    match &fix.command {
      Some(command) => format!("move that change of {} into {}: {command}", first.file, first.removed_by),
      None => format!("move {} into {owner}: it changes only {}", first.fixed_by, first.file),
    }
  } else {
    format!("move {} into {owner}, or stack {owner} on its branch", first.fixed_by)
  };
  let more = match references.len() {
    1 => String::new(),
    count => format!(" ({} more: bd check)", count - 1),
  };
  let of = if owner == branch { String::new() } else { format!(", which {branch} is built on,") };
  format!(
    "{owner}{of} removes {} in {}, and {}:{} still uses it: {} changes that use in another branch, so the build fails. To fix it, {fix}{more}; --allow-dangling starts anyway",
    first.identifier, first.removed_by, first.file, first.line, first.fixed_by
  )
}

/// The lines of a failed build's output that `BuildResult::output` keeps
const BUILD_OUTPUT_LINES: usize = 50;

/// `command` run in the tree of `head`, extracted (`git archive`, only `paths` if given) into a temporary directory
/// outside every working tree, which is removed afterwards. Its output is captured: stdout holds only the JSON
fn build_tree(context: &RepoContext, branch: &str, head: &str, command: &str, paths: &[String]) -> Result<BuildResult, CliError> {
  let io_error = |what: &str, e: std::io::Error| CliError::new(ErrorCode::GitError, format!("Cannot {what} for the build of {branch}: {e}"));
  let directory = tempfile::Builder::new()
    .prefix("bd-check-build-")
    .tempdir()
    .map_err(|e| io_error("create a directory", e))?;
  let archive_path = directory.path().join("tree.tar");
  let tree = directory.path().join("tree");
  // `git archive` fails on a pathspec that matches nothing, and a branch need not have every path
  let mut matching = Vec::with_capacity(paths.len());
  for path in paths {
    if !context
      .git
      .execute_command(&["ls-tree", "-r", "--name-only", head, "--", path], &context.repository)?
      .is_empty()
    {
      matching.push(path.as_str());
    }
  }
  if !paths.is_empty() && matching.is_empty() {
    std::fs::create_dir(&tree).map_err(|e| io_error("create a directory", e))?;
  } else {
    let mut archive = std::fs::File::create(&archive_path).map_err(|e| io_error("write the archive", e))?;
    let mut args = vec!["archive", "--format=tar", head];
    if !matching.is_empty() {
      args.push("--");
      args.extend(matching);
    }
    context.git.execute_command_streaming(&args, &context.repository, |chunk| {
      std::io::Write::write_all(&mut archive, chunk).map_err(anyhow::Error::from)
    })?;
    drop(archive);
    let archive = std::fs::File::open(&archive_path).map_err(|e| io_error("read the archive", e))?;
    tar::Archive::new(archive).unpack(&tree).map_err(|e| io_error("extract the tree", e))?;
    std::fs::remove_file(&archive_path).map_err(|e| io_error("remove the archive", e))?;
  }

  let mut shell = if cfg!(windows) {
    let mut shell = std::process::Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
  } else {
    let mut shell = std::process::Command::new("sh");
    shell.arg("-c").arg(command);
    shell
  };
  shell
    .current_dir(&tree)
    .env("BD_BRANCH", branch)
    .env("BD_COMMIT", head)
    .env_remove("GIT_DIR")
    .env_remove("GIT_WORK_TREE")
    .env_remove("GIT_INDEX_FILE")
    .env_remove("GIT_NOTES_REF")
    .stdin(std::process::Stdio::null());
  let output = match shell.output() {
    Ok(output) => output,
    Err(e) => {
      return Ok(BuildResult {
        exit_code: None,
        output: Some(format!("Cannot run {command:?}: {e}")),
      });
    }
  };
  let exit_code = output.status.code();
  let output = (exit_code != Some(0)).then(|| {
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(BUILD_OUTPUT_LINES)..].join("\n")
  });
  Ok(BuildResult { exit_code, output })
}

/// The path of `branch` in a plan: `parent/leaf` when it is stacked
fn plan_path(branch: &BranchCheck) -> String {
  branch
    .declared_parent
    .as_ref()
    .map_or_else(|| branch.name.clone(), |parent| format!("{parent}/{}", branch.name))
}

/// Whether `ancestor` is a branch that `branch` is built on, at any depth
fn is_built_on(report: &CheckReport, branch: &BranchCheck, ancestor: &str) -> bool {
  let mut current = branch.built_on.as_deref();
  for _ in 0..report.branches.len() {
    let Some(name) = current else { return false };
    if name == ancestor {
      return true;
    }
    current = report.branches.iter().find(|check| check.name == name).and_then(|check| check.built_on.as_deref());
  }
  false
}

/// `branch` conflicts, or has a dangling reference, because it needs a commit of a branch stacked on it: the fixes a
/// trial build of `plan` with them passes, `None` for no such case
fn inverted_stack(
  session: &CheckSession,
  plan: &RegroupPlan,
  report: &CheckReport,
  branch: &BranchCheck,
  files_of: &HashMap<String, HashSet<String>>,
  short: &ShortIds,
) -> Option<InvertedStack> {
  // the commits of the branch that need something, and what they need
  let (own, needed): (Vec<String>, Vec<String>) = match &branch.outcome {
    CheckOutcome::Conflict { commit, files } => (vec![commit.clone()], missing_commits(report, branch, commit, files, files_of)),
    CheckOutcome::Ok { .. } => report
      .dangling
      .get(&branch.name)
      .into_iter()
      .flatten()
      .filter(|reference| reference.is_confirmed())
      .filter_map(|reference| Some((reference.removed_by.clone(), reference.fixed_by.clone()?)))
      .unzip(),
    _ => return None,
  };
  let group_of: HashMap<&str, Option<&str>> = report.regroup.commits.iter().map(|commit| (commit.id.as_str(), commit.group.as_deref())).collect();
  let child = needed
    .iter()
    .filter_map(|id| group_of.get(id.as_str()).copied().flatten())
    .filter_map(|group| report.branches.iter().find(|check| check.name == group))
    .find(|check| is_built_on(report, check, &branch.name))?;

  // the change in the child uses what the parent adds: the child can't be built without it, which the trial build
  // doesn't see, so a swap is no fix
  let mutual = report
    .dangling
    .get(&branch.name)
    .into_iter()
    .flatten()
    .any(|reference| reference.is_confirmed() && reference.mutual);
  let mut own: Vec<String> = own;
  own.sort();
  own.dedup();
  let candidates = [
    InvertedStackFix {
      kind: "move_to_child".to_string(),
      branches: BTreeMap::from([(plan_path(child), own)]),
      unstack: Vec::new(),
    },
    InvertedStackFix {
      kind: "swap".to_string(),
      branches: BTreeMap::from([(format!("{}/{}", child.name, branch.name), Vec::new())]),
      unstack: vec![child.name.clone()],
    },
  ];
  let mut fixes = Vec::new();
  for fix in candidates.into_iter().filter(|fix| !(mutual && fix.kind == "swap")) {
    let mut fixed = plan.clone();
    for (path, commits) in &fix.branches {
      fixed.branches.entry(path.clone()).or_default().extend(commits.iter().cloned());
    }
    fixed.unstack.extend(fix.unstack.iter().cloned());
    // a fix the regroup refuses, or one after which either branch still fails, is no fix
    let Ok(trial) = session.check_with_dangling_of(Some(&fixed), &[branch.name.clone(), child.name.clone()]) else {
      continue;
    };
    let builds = |name: &str| {
      trial
        .branches
        .iter()
        .find(|check| check.name == name)
        .is_some_and(|check| matches!(check.outcome, CheckOutcome::Ok { .. }))
        && trial
          .dangling
          .get(name)
          .is_none_or(|references| references.iter().all(|reference| !reference.is_confirmed()))
    };
    if builds(&branch.name) && builds(&child.name) {
      fixes.push(InvertedStackFix {
        branches: fix
          .branches
          .into_iter()
          .map(|(path, commits)| (path, commits.iter().map(|id| short.get(id).to_string()).collect()))
          .collect(),
        ..fix
      });
    }
  }
  Some(InvertedStack {
    branch: branch.name.clone(),
    child: child.name.clone(),
    fixes,
  })
}
