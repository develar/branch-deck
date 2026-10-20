//! `bd check`: would the branches build with this plan? A trial sync (`sync_core::check`) that writes git objects and
//! the cache only

use crate::bd_command::shell_word;
use crate::branch_view::{StackBlockSummary, StackIssue, stack_issues};
use crate::context::RepoContext;
use crate::error::regroup_error;
use crate::error::{CliError, ErrorCode};
use crate::group_plan::{GroupPlan, check_descriptions, parse_group_plan};
use crate::output::{SchemaVersion, ShortIds};
use crate::suggest_plan::SuggestPlan;
use crate::text_input::read_text;
use git_ops::cat_file_batch::object_ids_and_types;
use git_ops::tree_archive::write_tree_archive;
use history_rewrite::regroup::{RegroupError, RegroupPlan};
use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use sync_core::check::{BranchCheck, CheckOutcome, CheckReport, CheckSession};
use sync_core::conflict_fix::{ConflictFix, FixBudget, FixRun, FixStepKind, SearchState};
use sync_core::dangling_fix::{DanglingFixProver, ProvenFix};
use sync_core::dangling_references::{DanglingKind, DanglingReference, DeclaredIn};
use sync_core::inverted_stack::{InvertedStackFixKind, inverted_stack};
use sync_core::missing_commits::{changed_files, missing_commits};
use sync_core::missing_declarations::missing_declarations;
use sync_core::sync::SyncReport;
use sync_types::stack::BranchStack;
use sync_utils::list_cut::kept;

#[derive(Debug, Default, clap::Args)]
pub struct CheckArgs {
  /// The plan of `bd group`, `-` for stdin; without it the current grouping, and stdin is never read
  #[arg(long, value_name = "FILE")]
  pub plan: Option<PathBuf>,
  /// Then run this shell command in the tree of each branch that is ok, extracted into a temporary directory outside
  /// the working tree; a failure is `build_failed`. Slow on a large repository: narrow it with --build-paths. A build
  /// that passed on the same tree of the same branch with the same command is not run again (`cached`)
  #[arg(long, value_name = "COMMAND")]
  pub build: Option<String>,
  /// With --build: extract only these paths (files or directories, names, not patterns) of each tree
  #[arg(long, value_name = "PATH", num_args = 1.., requires = "build")]
  pub build_paths: Vec<String>,
  /// With --build: run the command even on a tree it passed on, after a change outside the tree (a JDK, a toolchain)
  #[arg(long, requires = "build")]
  pub rebuild: bool,
  /// Also find the regroup after which the conflicting branches apply cleanly, by trial builds (`fix`); nothing is
  /// applied. Each trial is a line on stderr
  #[arg(long)]
  pub fix: bool,
  /// With --fix: the trial builds in all [default: 12]
  #[arg(long, value_name = "N", requires = "fix")]
  pub max_trials: Option<usize>,
  /// With --fix: the seconds of the whole command, its first check included [default: 180]; then the fix is the best
  /// plan so far, with `"search": "budget"`
  #[arg(long, value_name = "SECONDS", requires = "fix")]
  pub time_budget: Option<u64>,
  /// Count a branch whose only failure is a dangling reference as building: its `result` stays `dangling_reference`,
  /// `ok` and the exit code take it, as `bd safe-push --allow-dangling` starts anyway. For a reference the user says is
  /// a false alarm
  #[arg(long)]
  pub allow_dangling: bool,
}

/// The trial builds of `bd check --fix` without --max-trials
const DEFAULT_MAX_TRIALS: usize = 12;
/// The seconds of `bd check --fix` without --time-budget: a trial of a large stack takes a minute
const DEFAULT_TIME_BUDGET: u64 = 180;

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
  /// Every branch builds; with --allow-dangling a `dangling_reference` counts as building
  pub ok: bool,
  /// Parents before their children
  pub branches: Vec<CheckedBranch>,
  pub unassigned_count: usize,
  /// What may break a build though no commit proves it: `possible_dangling_reference`, `possible_changed_declaration`,
  /// `possible_duplicate_declaration`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub warnings: Vec<CheckWarning>,
  /// With `branchdeck.symbols on`: a commit of a branch uses a name that a commit outside the branch and the branches
  /// it is built on adds, by the syntax trees (`symbol_dependencies`). git merges it, the build would fail: stack the
  /// branch on the one that adds the name, or move one of the two. Not counted in `ok`: names, not a compiler
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub missing_declarations: Vec<MissingDeclaration>,
  /// With --fix: the regroup after which the branches that conflict (or have a proven dangling reference) apply cleanly
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fix: Option<CheckFix>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MissingDeclaration {
  pub branch: String,
  pub identifier: String,
  /// Where the branch uses it
  pub file: String,
  pub line: u32,
  /// The commit of the branch that uses it, and the commit outside that adds it
  pub used_by: String,
  pub declared_by: String,
  /// The branch of `declared_by`; `null` for an unassigned commit
  pub declared_in: Option<String>,
  /// `import`, `qualified`, `same_file`, `same_module`, `star_import`, `same_package_type`, `override` (an override of
  /// it in a subtype), `inherited` (a call of it in a subclass), `receiver` (a use on a receiver of its type), `facade`
  /// (a Java use through the class of its Kotlin file) or `removes_after`
  pub evidence: &'static str,
}

/// The fix of `bd check --fix`, found by trial builds on top of the plan; nothing is applied
#[derive(Debug, Serialize, JsonSchema)]
pub struct CheckFix {
  /// `complete`; `budget` when the trial or time limit stopped it (run it again, the cache makes it faster);
  /// `canceled`
  pub search: String,
  /// The trial builds it ran
  pub trials: u32,
  /// Independent parts, each with the full plan (the `--plan` included) for `bd group --plan … --expect-head <head>`
  pub components: Vec<CheckFixComponent>,
  /// The branches that still conflict or have a proven dangling reference afterwards
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub unresolved: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CheckFixComponent {
  pub steps: Vec<CheckFixStep>,
  pub plan: SuggestPlan,
  /// The branches that conflicted (or had a proven dangling reference) and apply cleanly afterwards
  pub fixed: Vec<String>,
  /// The branches blocked by a parent that apply cleanly afterwards
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub unblocked: Vec<String>,
  /// The branches left without commits
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub emptied: Vec<String>,
  /// The branches stacked on an emptied one, built on its nearest ancestor with commits
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub orphaned_children: Vec<String>,
  /// The subjects the plan rewrites
  pub rewritten: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CheckFixStep {
  /// `move`: `commits` leave `from` for `to`, whose stack has what they need; `pull_in`: what `to` needs comes into it;
  /// `restack`: `fixes` is stacked on `to`
  pub kind: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub commits: Vec<String>,
  /// The branches the commits leave (an unassigned commit leaves none), the old parent of a restack
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub from: Vec<String>,
  pub to: String,
  /// The branch whose conflict or dangling reference the step is for
  pub fixes: String,
}

fn fix_output(fix: &ConflictFix, short: &ShortIds) -> CheckFix {
  let shorts = |ids: &[String]| -> Vec<String> { ids.iter().map(|id| short.get(id).to_string()).collect() };
  CheckFix {
    search: match fix.search {
      SearchState::Complete => "complete",
      SearchState::Budget => "budget",
      SearchState::Canceled => "canceled",
    }
    .to_string(),
    trials: fix.trials,
    components: fix
      .components
      .iter()
      .map(|component| CheckFixComponent {
        steps: component
          .steps
          .iter()
          .map(|step| CheckFixStep {
            kind: match step.kind {
              FixStepKind::Move => "move",
              FixStepKind::PullIn => "pull_in",
              FixStepKind::Restack => "restack",
            }
            .to_string(),
            commits: shorts(&step.commits),
            from: step.from.clone(),
            to: step.to.clone(),
            fixes: step.fixes.clone(),
          })
          .collect(),
        plan: SuggestPlan {
          branches: component.plan.branches.iter().map(|(path, ids)| (path.clone(), shorts(ids))).collect(),
          unassign: shorts(&component.plan.unassign),
          issue_groups: component.plan.issue_groups.clone(),
          ungroup_issues: component.plan.ungroup_issues.clone(),
          unstack: component.plan.unstack.clone(),
        },
        fixed: component.fixed.clone(),
        unblocked: component.unblocked.clone(),
        emptied: component.emptied.clone(),
        orphaned_children: component.orphaned_children.clone(),
        rewritten: component.rewritten,
      })
      .collect(),
    unresolved: fix.unresolved.clone(),
  }
}

/// A use of a name a branch removes or changes, or a second declaration of a name it adds, that no commit outside it
/// changes: in its virtual tree with no fix in HEAD, or on the baseline tip, which got it after the base. The exit code
/// doesn't count it
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CheckWarning {
  /// `possible_dangling_reference` (a removed declaration), `possible_changed_declaration`,
  /// `possible_duplicate_declaration`, as `kind` of a dangling reference says
  pub kind: String,
  pub branch: String,
  pub identifier: String,
  pub file: String,
  pub line: u32,
  /// The commit of the branch that removes, changes or adds the declaration
  pub removed_by: String,
  /// A `possible_changed_declaration` whose use a commit outside the branch changes, though the summary can't tell
  /// that the use breaks (a changed return type or nullability, new type parameters, a suspend call in the lambda of
  /// an unknown function): what to stack the branch on if the build fails
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fixed_by: Option<String>,
  /// The use is on the baseline tip, not in the virtual tree
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub on_baseline: bool,
  /// As `via` of a dangling reference
  pub via: &'static str,
  /// As `declared_in` of a dangling reference
  pub declared_in: DeclarationPlace,
}

/// Where the declaration is that `removed_by` removes, changes or adds
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DeclarationPlace {
  pub file: String,
  /// 1-based, in the file before `removed_by` (after it for `duplicate_declaration`); none when that file can't be read
  #[serde(skip_serializing_if = "Option::is_none")]
  pub line: Option<u32>,
}

impl DeclarationPlace {
  fn of(declared_in: &DeclaredIn) -> Self {
    Self {
      file: declared_in.file.clone(),
      line: declared_in.line,
    }
  }
}

/// How the uses of a dangling reference were found, for `via`
const fn found_via(reference: &DanglingReference) -> &'static str {
  if reference.by_syntax { "syntax_tree" } else { "lines" }
}

/// The `kind` of a dangling reference
pub(crate) const fn kind_name(kind: DanglingKind) -> &'static str {
  match kind {
    DanglingKind::Removed => "removed_declaration",
    DanglingKind::Changed => "changed_declaration",
    DanglingKind::Duplicate => "duplicate_declaration",
  }
}

/// The `kind` of a warning of a dangling reference that no commit proves
const fn warning_kind(kind: DanglingKind) -> &'static str {
  match kind {
    DanglingKind::Removed => "possible_dangling_reference",
    DanglingKind::Changed => "possible_changed_declaration",
    DanglingKind::Duplicate => "possible_duplicate_declaration",
  }
}

/// What the branch does to the declaration, in a sentence: `removes`, `changes the form of`, `declares again`
pub(crate) const fn kind_verb(kind: &str) -> &'static str {
  match kind.as_bytes() {
    b"changed_declaration" | b"possible_changed_declaration" => "changes the form of",
    b"duplicate_declaration" | b"possible_duplicate_declaration" => "declares again",
    _ => "removes",
  }
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
  /// Without `fixes`: why neither fix builds both, and where to look further
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
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
  /// It passed on this tree before and did not run again: the same command, branch and extracted objects (the object
  /// cache; `--rebuild` runs it)
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub cached: bool,
}

/// A use of a name the branch removes or changes, or a second declaration of a name it adds, in its virtual tree, that a
/// commit outside the branch changes in HEAD
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DanglingReferenceSummary {
  /// `removed_declaration`: the branch removes the declaration; `changed_declaration`: it changes its form (its header,
  /// such as `suspend` or the return type, its parameters, its reach, the constructors of its type), and the use, which
  /// the old form takes, may not fit; `duplicate_declaration`: the branch declares the name again, and `file` is the
  /// other declaration, of the same key, package and module
  pub kind: &'static str,
  pub identifier: String,
  /// Where the virtual tree uses it (declares it, for `duplicate_declaration`)
  pub file: String,
  pub line: u32,
  /// The commit of the branch that removes, changes or adds the declaration
  pub removed_by: String,
  /// The commit outside the branch whose patch changes the use (removes the other declaration): the branch needs it
  pub fixed_by: String,
  /// The change in `fixed_by` uses what the branch declares: stacking either branch on the other can't help; move the
  /// change of `file` into `removed_by`
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub mutual: bool,
  /// `syntax_tree`: the syntax trees of both sides confirmed the uses; `lines`: the lines alone found them (a language
  /// without a grammar, or a file the parser can't read well)
  pub via: &'static str,
  /// Where the declaration is that `removed_by` removes, changes or adds: a reader judges by it whether the use means it
  pub declared_in: DeclarationPlace,
  /// With `mutual`: how the change of the use gets into the branch, as a trial build of the plan with it proves it (the
  /// branch builds, no branch that built breaks); none when neither move builds, or `bd check` didn't try
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fix: Option<DanglingFix>,
}

/// The fix of a mutual dangling reference, proven by a trial build, to apply after the user approves
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DanglingFix {
  /// `move_commit`: `fixed_by` changes only `file`, so it moves into the branch (`branches`, to add to the plan of
  /// `bd group`); `move_hunk`: `command` moves the change of `file` from `fixed_by` into `removed_by` (the whole change
  /// of the file; narrow it with `--block`)
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
  let started = Instant::now();
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
    changed_files(session.commit_diffs().map_err(regroup_error)?)
  } else {
    HashMap::new()
  };

  let short = context.short_ids(ids.iter().map(String::as_str).chain([report.head.as_str()]).chain(report.base.as_deref()));
  let user_plan = plan.as_ref().map_or_default(|plan| plan.regroup.clone());
  let mut prover = FixProver::new(context, &session, &user_plan, &report, &files_of, &short);
  let mut branches = checked_branches(&report, &files_of, &short, &mut |branch, reference| prover.prove(branch, reference));
  let warnings = possible_dangling_references(&report, &short);
  for (checked, branch) in branches.iter_mut().zip(&report.branches) {
    checked.inverted_stack = inverted_stack(&session, &user_plan, &report, branch, &files_of).map(|found| inverted_stack_output(found, &short));
  }
  if let Some(command) = &args.build {
    let heads: Vec<(usize, &str)> = branches
      .iter()
      .zip(&report.branches)
      .enumerate()
      .filter_map(|(index, (checked, branch))| match &branch.outcome {
        CheckOutcome::Ok { head } if checked.result == "ok" => Some((index, head.as_str())),
        _ => None,
      })
      .collect();
    let extracted = extracted_objects(context, &heads, &args.build_paths)?;
    let keys: Vec<[u8; 32]> = heads
      .iter()
      .zip(&extracted)
      .map(|((index, _), objects)| build_key(command, &report.branches[*index].name, objects))
      .collect();
    let passed = if args.rebuild {
      vec![false; keys.len()]
    } else {
      cache.passed_builds(BUILD_KEY_VERSION, &keys)
    };
    let mut passes = Vec::new();
    for (((index, head), objects), (key, passed)) in heads.into_iter().zip(&extracted).zip(keys.into_iter().zip(passed)) {
      let build = if passed {
        BuildResult {
          exit_code: Some(0),
          output: None,
          cached: true,
        }
      } else {
        // only the paths the tree has: `git archive` fails on a pathspec that matches nothing
        let paths: Option<Vec<&str>> = (!args.build_paths.is_empty()).then(|| objects.iter().map(|(path, _)| *path).collect());
        let build = build_tree(context, &report.branches[index].name, head, command, paths.as_deref())?;
        if build.exit_code == Some(0) {
          passes.push(key);
        }
        build
      };
      let checked = &mut branches[index];
      if build.exit_code != Some(0) {
        checked.result = "build_failed".to_string();
      }
      checked.build = Some(build);
    }
    // only a pass: a failure may be by chance (a flaky test, a full disk), so it runs again
    cache.put_passed_builds(BUILD_KEY_VERSION, &passes);
  }

  let fix = if args.fix {
    let run = FixRun {
      budget: FixBudget {
        trials: args.max_trials.unwrap_or(DEFAULT_MAX_TRIALS),
        time: Duration::from_secs(args.time_budget.unwrap_or(DEFAULT_TIME_BUDGET)),
        ..FixBudget::default()
      },
      started,
      cancel: &AtomicBool::new(false),
      // the JSON goes to stdout at the end; an agent that waits sees each trial at once
      progress: &|line| {
        let _ = writeln!(std::io::stderr().lock(), "{line}");
      },
    };
    let found = session.find_conflict_fix(Some(&user_plan), Some(report.clone()), &run).map_err(regroup_error)?;
    Some(fix_output(&found, &short))
  } else {
    None
  };

  Ok(CheckOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    head: short.get(&report.head).to_string(),
    baseline: report.baseline_branch.clone(),
    base_commit: report.base.as_deref().map(|base| short.get(base).to_string()),
    rewritten: report.regroup.changed_count(),
    ok: branches
      .iter()
      .all(|branch| branch.result == "ok" || (args.allow_dangling && branch.result == "dangling_reference")),
    branches,
    unassigned_count: report.unassigned,
    warnings,
    missing_declarations: missing_declaration_outputs(&session, &report, &short).map_err(regroup_error)?,
    fix,
  })
}

/// The names the built branches of `report` use and a commit outside their trees adds (`missing_declarations`), by
/// short ids
fn missing_declaration_outputs(session: &CheckSession, report: &CheckReport, short: &ShortIds) -> Result<Vec<MissingDeclaration>, RegroupError> {
  Ok(
    missing_declarations(session, report)?
      .into_iter()
      .map(|missing| MissingDeclaration {
        used_by: short.get(&missing.used_by).to_string(),
        declared_by: short.get(&missing.declared_by).to_string(),
        evidence: missing.evidence.name(),
        branch: missing.branch,
        identifier: missing.identifier,
        file: missing.file,
        line: missing.line,
        declared_in: missing.declared_in,
      })
      .collect(),
  )
}

/// The dangling references of `report` that no commit proves, as warnings
pub(crate) fn possible_dangling_references(report: &CheckReport, short: &ShortIds) -> Vec<CheckWarning> {
  report
    .dangling
    .iter()
    .flat_map(|(branch, references)| references.iter().map(move |reference| (branch, reference)))
    .filter(|(_, reference)| !reference.is_confirmed())
    .map(|(branch, reference)| CheckWarning {
      kind: warning_kind(reference.kind).to_string(),
      branch: branch.clone(),
      identifier: reference.identifier.clone(),
      file: reference.file.clone(),
      line: reference.line,
      removed_by: short.get(&reference.removed_by).to_string(),
      fixed_by: reference.fixed_by.as_deref().filter(|_| !reference.on_baseline).map(|id| short.get(id).to_string()),
      on_baseline: reference.on_baseline,
      via: found_via(reference),
      declared_in: DeclarationPlace::of(&reference.declared_in),
    })
    .collect()
}

/// Proves the fix of a mutual dangling reference of a branch: `None` for no proven fix
pub(crate) type ProveFix<'a> = dyn FnMut(&BranchCheck, &DanglingReference) -> Option<DanglingFix> + 'a;

/// The dangling references of `branch` that a commit proves, the mutual ones with the fix `prove` proves
pub(crate) fn confirmed_dangling_references(report: &CheckReport, branch: &BranchCheck, short: &ShortIds, prove: &mut ProveFix<'_>) -> Vec<DanglingReferenceSummary> {
  report
    .dangling
    .get(&branch.name)
    .into_iter()
    .flatten()
    .filter_map(|reference| {
      let fixed_by = reference.fixed_by.as_deref().filter(|_| reference.is_confirmed())?;
      let (removed_by, fixed_by) = (short.get(&reference.removed_by).to_string(), short.get(fixed_by).to_string());
      Some(DanglingReferenceSummary {
        kind: kind_name(reference.kind),
        fix: if reference.mutual { prove(branch, reference) } else { None },
        identifier: reference.identifier.clone(),
        file: reference.file.clone(),
        line: reference.line,
        removed_by,
        fixed_by,
        mutual: reference.mutual,
        via: found_via(reference),
        declared_in: DeclarationPlace::of(&reference.declared_in),
      })
    })
    .collect()
}

/// Proves the fixes of mutual dangling references by trial builds (`DanglingFixProver`), each as `bd check` names it
pub(crate) struct FixProver<'a> {
  context: &'a RepoContext,
  prover: DanglingFixProver<'a>,
  short: &'a ShortIds,
}

impl<'a> FixProver<'a> {
  pub(crate) const fn new(
    context: &'a RepoContext,
    session: &'a CheckSession,
    plan: &'a RegroupPlan,
    report: &'a CheckReport,
    files_of: &'a HashMap<String, HashSet<String>>,
    short: &'a ShortIds,
  ) -> Self {
    Self {
      context,
      prover: DanglingFixProver::new(session, plan, report, files_of),
      short,
    }
  }

  /// The proven fix of `reference`, a proven mutual dangling reference of `branch`
  pub(crate) fn prove(&mut self, branch: &BranchCheck, reference: &DanglingReference) -> Option<DanglingFix> {
    Some(match self.prover.prove(branch, reference)? {
      ProvenFix::MoveCommit { path, commit } => DanglingFix {
        kind: "move_commit".to_string(),
        command: None,
        branches: BTreeMap::from([(path, vec![self.short.get(&commit).to_string()])]),
      },
      ProvenFix::MoveHunk { from, to, file } => DanglingFix {
        kind: "move_hunk".to_string(),
        command: Some(format!(
          "{} move-hunk {} {} {}",
          self.context.bd,
          self.short.get(&from),
          self.short.get(&to),
          shell_word(&file)
        )),
        branches: BTreeMap::new(),
      },
    })
  }
}

/// The check of each branch of `report`; `files_of` holds the changed files of each commit, for the commits a conflict
/// likely needs
pub(crate) fn checked_branches(report: &CheckReport, files_of: &HashMap<String, HashSet<String>>, short: &ShortIds, prove: &mut ProveFix<'_>) -> Vec<CheckedBranch> {
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
          checked.dangling_references = confirmed_dangling_references(report, branch, short, prove);
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

/// The plan from `--plan` (`-` for stdin); `None` without one or for an empty one. Never stdin without `--plan`: an
/// agent's or a CI's stdin is an open pipe that never ends
fn read_plan(path: Option<&PathBuf>) -> Result<Option<GroupPlan>, CliError> {
  let Some(path) = path else { return Ok(None) };
  let text = read_text(path, "the plan")?;
  if text.trim().is_empty() {
    return Ok(None);
  }
  parse_group_plan(&text).map(Some)
}

/// The proven dangling references of some branches, each with its branch, and the possible ones as warnings
pub(crate) type BranchDangling = (Vec<(String, DanglingReferenceSummary)>, Vec<CheckWarning>);

/// A trial session on the grouping the sync of `report` read: no guards, HEAD, config, stores or baseline read again,
/// only the commits ahead of the baseline with the mapping notes the sync wrote and the branch tips
pub(crate) fn session_of_sync(context: &RepoContext, report: &SyncReport) -> Result<CheckSession, CliError> {
  Ok(
    CheckSession::of_sync(&context.git, &context.repository, &context.prefix, &context.cache(), report)
      .map_err(regroup_error)?
      .verifying_mapping_notes(!context.use_cache),
  )
}

/// The dangling references of `branches` (a branch and the branches it is built on) in a check of the grouping of
/// `session` (`session_of_sync`): the ones a commit proves, by branch, and the possible ones as warnings. With `chain`,
/// only that branch and the branches it is built on are built; else every branch (a whole stack that forks)
pub(crate) fn dangling_references_in(context: &RepoContext, session: &CheckSession, branches: &[&str], chain: Option<&str>) -> Result<BranchDangling, CliError> {
  let report = match chain {
    Some(chain) => {
      let of: Vec<String> = branches.iter().map(|name| (*name).to_string()).collect();
      session.check_landing(chain, &of)
    }
    None => session.check(None),
  }
  .map_err(regroup_error)?;
  let short = context.short_ids(report.regroup.commits.iter().map(|commit| commit.id.as_str()));
  let files_of = if has_mutual_reference(&report) {
    changed_files(session.commit_diffs().map_err(regroup_error)?)
  } else {
    HashMap::new()
  };
  let no_plan = RegroupPlan::default();
  let mut prover = FixProver::new(context, session, &no_plan, &report, &files_of, &short);
  let confirmed = branches
    .iter()
    .filter_map(|name| report.branches.iter().find(|branch| branch.name == *name))
    .flat_map(|branch| {
      confirmed_dangling_references(&report, branch, &short, &mut |branch, reference| prover.prove(branch, reference))
        .into_iter()
        .map(|reference| (branch.name.clone(), reference))
    })
    .collect();
  let warnings = possible_dangling_references(&report, &short)
    .into_iter()
    .filter(|warning| branches.contains(&warning.branch.as_str()))
    .collect();
  Ok((confirmed, warnings))
}

/// The dangling references a refusal describes; one more is described too, so the count of the rest is never 1
/// (`list_cut::kept`)
const DESCRIBED_REFERENCES: usize = 1;

/// What the build of `branch` fails on: each dangling reference it describes and how to fix it (two of two, else the
/// first and the count of the rest, which `bd check` lists)
pub(crate) fn dangling_reference_message(branch: &str, references: &[(String, DanglingReferenceSummary)]) -> String {
  if references.is_empty() {
    return String::new();
  }
  let described = kept(references.len(), DESCRIBED_REFERENCES);
  let sentences: Vec<String> = references[..described]
    .iter()
    .map(|(owner, reference)| describe_dangling_reference(branch, owner, reference))
    .collect();
  let more = match references.len() - described {
    0 => String::new(),
    rest => format!(" ({rest} more: bd check)"),
  };
  format!("{}{more}; --allow-dangling starts anyway", sentences.join(". "))
}

/// One dangling reference of the build of `branch` (`owner` the branch that removes the declaration), in a sentence:
/// what breaks and how to fix it
fn describe_dangling_reference(branch: &str, owner: &str, reference: &DanglingReferenceSummary) -> String {
  let fix = if let Some(fix) = &reference.fix {
    match &fix.command {
      Some(command) => format!("move that change of {} into {}: {command}", reference.file, reference.removed_by),
      None => format!("move {} into {owner}: it changes only {}", reference.fixed_by, reference.file),
    }
  } else {
    format!("move {} into {owner}, or stack {owner} on its branch", reference.fixed_by)
  };
  let of = if owner == branch { String::new() } else { format!(", which {branch} is built on,") };
  let (uses, change) = if reference.kind == "duplicate_declaration" {
    ("still declares it too", "removes that declaration")
  } else {
    ("still uses it", "changes that use")
  };
  format!(
    "{owner}{of} {} {} in {}, and {}:{} {uses}: {} {change} in another branch, so the build fails. To fix it, {fix}",
    kind_verb(reference.kind),
    reference.identifier,
    reference.removed_by,
    reference.file,
    reference.line,
    reference.fixed_by
  )
}

/// The lines of a failed build's output that `BuildResult::output` keeps
const BUILD_OUTPUT_LINES: usize = 50;

/// The version of the key of a passed build (`build_key`): bump it when what a build sees changes, and the store drops
/// the passes of the old one
const BUILD_KEY_VERSION: u32 = 1;

/// What the build of each of `heads` extracts, in their order: `("", tree id)` without `paths`, else `(path, object id)`
/// of each of `paths` the tree has (a file, a directory or a submodule), in the order of `paths`. One `cat-file
/// --batch-check` of every `<head>^{tree}` or `<head>:<path>`; `-Z`, because a path may hold a newline
fn extracted_objects<'a>(context: &RepoContext, heads: &[(usize, &str)], paths: &'a [String]) -> Result<Vec<Vec<(&'a str, String)>>, CliError> {
  let wanted: Vec<&'a str> = if paths.is_empty() { vec![""] } else { paths.iter().map(String::as_str).collect() };
  let names: Vec<String> = heads
    .iter()
    .flat_map(|(_, head)| {
      wanted
        .iter()
        .map(move |path| if paths.is_empty() { format!("{head}^{{tree}}") } else { format!("{head}:{path}") })
    })
    .collect();
  let mut entries = object_ids_and_types(&context.git, &context.repository, &names)?.into_iter();
  let mut extracted = Vec::with_capacity(heads.len());
  for (_, head) in heads {
    let mut objects = Vec::with_capacity(wanted.len());
    for path in &wanted {
      match entries.next().flatten() {
        Some((id, _)) => objects.push((*path, id)),
        None if paths.is_empty() => return Err(CliError::new(ErrorCode::GitError, format!("Cannot read the tree of {head} for the build"))),
        None => {}
      }
    }
    extracted.push(objects);
  }
  Ok(extracted)
}

/// The key of a passed build of `command` on `branch` (`BD_BRANCH`): a hash of the two and of the sorted `(path, id)`
/// pairs of `objects`, what it extracts. Not the commit (`BD_COMMIT`): a regroup writes new commits of the same trees,
/// which would always miss
fn build_key(command: &str, branch: &str, objects: &[(&str, String)]) -> [u8; 32] {
  let mut sorted: Vec<&(&str, String)> = objects.iter().collect();
  sorted.sort_unstable();
  sorted.dedup();
  let mut hasher = Sha256::new();
  let mut field = |value: &str| {
    hasher.update(value.as_bytes());
    hasher.update([0]);
  };
  field(command);
  field(branch);
  field(&sorted.len().to_string());
  for (path, id) in sorted {
    field(path);
    field(id);
  }
  hasher.finalize().into()
}

/// `command` run in the tree of `head`, extracted (`git archive`; only `paths`, the given ones the tree has, if given)
/// into a temporary directory outside every working tree, which is removed afterwards. Its output is captured: stdout
/// holds only the JSON
fn build_tree(context: &RepoContext, branch: &str, head: &str, command: &str, paths: Option<&[&str]>) -> Result<BuildResult, CliError> {
  let io_error = |what: &str, e: std::io::Error| CliError::new(ErrorCode::GitError, format!("Cannot {what} for the build of {branch}: {e}"));
  let directory = tempfile::Builder::new()
    .prefix("bd-check-build-")
    .tempdir()
    .map_err(|e| io_error("create a directory", e))?;
  let archive_path = directory.path().join("tree.tar");
  let tree = directory.path().join("tree");
  if paths.is_some_and(<[&str]>::is_empty) {
    std::fs::create_dir_all(&tree).map_err(|e| io_error("create a directory", e))?;
  } else {
    let mut archive = std::fs::File::create(&archive_path).map_err(|e| io_error("write the archive", e))?;
    write_tree_archive(&context.git, &context.repository, head, paths, &mut archive)?;
    drop(archive);
    let archive = std::fs::File::open(&archive_path).map_err(|e| io_error("read the archive", e))?;
    tar::Archive::new(archive).unpack(&tree).map_err(|e| io_error("extract the tree", e))?;
    std::fs::remove_file(&archive_path).map_err(|e| io_error("remove the archive", e))?;
  }

  let mut shell = shell(command);
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
        cached: false,
      });
    }
  };
  let exit_code = output.status.code();
  let output = (exit_code != Some(0)).then(|| {
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(BUILD_OUTPUT_LINES)..].join("\n")
  });
  Ok(BuildResult { exit_code, output, cached: false })
}

/// `sh -c <command>`
#[cfg(not(windows))]
fn shell(command: &str) -> std::process::Command {
  let mut shell = std::process::Command::new("sh");
  shell.arg("-c").arg(command);
  shell
}

/// `cmd /D /S /C "<command>"`, the command line passed as written: `Command::arg` quotes an argument for the MSVC
/// runtime (inner `"` as `\"`), which cmd does not read, so a quoted path broke the command. `/S` strips only the outer
/// quotes, also from a command that starts with a quoted program; `/D` skips the `AutoRun` commands of the registry
#[cfg(windows)]
fn shell(command: &str) -> std::process::Command {
  let mut shell = std::process::Command::new("cmd");
  std::os::windows::process::CommandExt::raw_arg(&mut shell, format!("/D /S /C \"{command}\""));
  shell
}

/// The output of an inverted stack (`sync_core::inverted_stack`): short ids, the kinds of the fixes, and without one why
fn inverted_stack_output(found: sync_core::inverted_stack::InvertedStack, short: &ShortIds) -> InvertedStack {
  let reason = found.fixes.is_empty().then(|| {
    let tried = if found.mutual {
      "moving its commits into the child doesn't build both, and the child's change uses what it adds, so no swap"
    } else {
      "neither moving its commits into the child nor stacking it on the child builds both"
    };
    format!("{tried}: `bd check --fix` searches the regroups")
  });
  InvertedStack {
    branch: found.branch,
    child: found.child,
    fixes: found
      .fixes
      .into_iter()
      .map(|fix| InvertedStackFix {
        kind: match fix.kind {
          InvertedStackFixKind::MoveToChild => "move_to_child",
          InvertedStackFixKind::Swap => "swap",
        }
        .to_string(),
        branches: fix
          .branches
          .into_iter()
          .map(|(path, commits)| (path, commits.iter().map(|id| short.get(id).to_string()).collect()))
          .collect(),
        unstack: fix.unstack,
      })
      .collect(),
    reason,
  }
}
