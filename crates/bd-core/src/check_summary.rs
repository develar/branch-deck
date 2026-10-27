//! The summaries of a check that `bd check` and `bd safe-push` print: the dangling references a commit proves and the
//! possible ones as warnings, the proven fix of a mutual one, the conflict of a branch, and the refusal of a start that
//! a dangling reference fails

use crate::bd_command::shell_word;
use crate::context::RepoContext;
use crate::error::{CliError, regroup_error};
use crate::output::ShortIds;
use domain_types::rewrite::RegroupPlan;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use sync_core::sync::SyncReport;
use sync_utils::list_cut::kept;
use trial_check::check::{BranchCheck, CheckReport, CheckSession};
use trial_check::dangling_fix::{DanglingFixProver, ProvenFix};
use trial_check::dangling_references::{DanglingKind, DanglingReference, DeclaredIn};
use trial_check::missing_commits::changed_files;

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
const fn kind_name(kind: DanglingKind) -> &'static str {
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
pub const fn kind_verb(kind: &str) -> &'static str {
  match kind.as_bytes() {
    b"changed_declaration" | b"possible_changed_declaration" => "changes the form of",
    b"duplicate_declaration" | b"possible_duplicate_declaration" => "declares again",
    _ => "removes",
  }
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
pub fn has_mutual_reference(report: &CheckReport) -> bool {
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

/// The dangling references of `report` that no commit proves, as warnings
pub fn possible_dangling_references(report: &CheckReport, short: &ShortIds) -> Vec<CheckWarning> {
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
pub type ProveFix<'a> = dyn FnMut(&BranchCheck, &DanglingReference) -> Option<DanglingFix> + 'a;

/// The dangling references of `branch` that a commit proves, the mutual ones with the fix `prove` proves
pub fn confirmed_dangling_references(report: &CheckReport, branch: &BranchCheck, short: &ShortIds, prove: &mut ProveFix<'_>) -> Vec<DanglingReferenceSummary> {
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
pub struct FixProver<'a> {
  context: &'a RepoContext,
  prover: DanglingFixProver<'a>,
  short: &'a ShortIds,
}

impl<'a> FixProver<'a> {
  pub const fn new(
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
  pub fn prove(&mut self, branch: &BranchCheck, reference: &DanglingReference) -> Option<DanglingFix> {
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

/// The proven dangling references of some branches, each with its branch, and the possible ones as warnings
pub type BranchDangling = (Vec<(String, DanglingReferenceSummary)>, Vec<CheckWarning>);

/// A trial session on the grouping the sync of `report` read: no guards, HEAD, config, stores or baseline read again,
/// only the commits ahead of the baseline with the mapping notes the sync wrote and the branch tips
pub fn session_of_sync(context: &RepoContext, report: &SyncReport) -> Result<CheckSession, CliError> {
  Ok(
    CheckSession::of_sync(&context.git, &context.repository, &context.prefix, &context.cache(), report)
      .map_err(regroup_error)?
      .verifying_mapping_notes(!context.use_cache),
  )
}

/// The dangling references of `branches` in `report`, a check of the grouping of `session` (`session_of_sync`) of what a
/// start lands (`trial_check::landing_gate`): the ones a commit proves, by branch, and the possible ones as warnings
pub fn dangling_references_in(context: &RepoContext, session: &CheckSession, report: &CheckReport, branches: &[&str]) -> Result<BranchDangling, CliError> {
  let short = context.short_ids(report.regroup.commits.iter().map(|commit| commit.id.as_str()));
  let files_of = if has_mutual_reference(report) {
    changed_files(session.commit_diffs().map_err(regroup_error)?)
  } else {
    HashMap::new()
  };
  let no_plan = RegroupPlan::default();
  let mut prover = FixProver::new(context, session, &no_plan, report, &files_of, &short);
  let confirmed = branches
    .iter()
    .filter_map(|name| report.branches.iter().find(|branch| branch.name == *name))
    .flat_map(|branch| {
      confirmed_dangling_references(report, branch, &short, &mut |branch, reference| prover.prove(branch, reference))
        .into_iter()
        .map(|reference| (branch.name.clone(), reference))
    })
    .collect();
  let warnings = possible_dangling_references(report, &short)
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
pub fn dangling_reference_message(branch: &str, references: &[(String, DanglingReferenceSummary)]) -> String {
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
  format!(
    "{}{more}; --allow-dangling starts anyway, only when the user says it is a false alarm",
    sentences.join(". ")
  )
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
