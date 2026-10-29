//! `bd review-plan`: the plan of a review of a branch or a range (`review_plan::review_plan_core`,
//! spec/branch/review-plan.spec.md), as JSON for an agent or an offline evaluation: for a branch marked reviewed
//! (`bd mark-reviewed`), its commits paired with the reviewed ones and the plain range of the ones added on top; the
//! public declarations the reviewed commits add, remove or change, ranked by their usages at the head from accio
//! (`AccioCli::discover`, never canceled, never an index build); the risk of each changed file; the changed files in
//! dependency order. Reads only; the baseline is that of the checked out branch

use crate::mark_reviewed_command::refused;
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::output::{SchemaVersion, ShortIds};
use domain_types::error::ErrorCode;
use file_diffs::accio::{AccioCli, SymbolIndex};
use git_ops::landed_pairing::PathLines;
use review_plan::api::{ApiChange, ApiChangeKind, FormChange};
use review_plan::interdiff::{Interdiff, PairState};
use review_plan::risk::FileRisk;
use review_plan::{PlanSources, ReviewPlanParams, ReviewSubject, SinceReviewedOfRange, UnknownRevision, review_plan_core};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, clap::Args)]
pub struct ReviewPlanArgs {
  /// The branch, named as `bd mark-reviewed` names it (`auth`), or a range `<base>..<head>`
  pub subject: String,
  /// The commit the branch is reviewed on, for a branch stacked on another: its parent, as `bd mark-reviewed --base`;
  /// without it, the merge base with the baseline. Not for a range
  #[arg(long)]
  pub base: Option<String>,
  /// Only the commits added or rewritten since the branch was marked reviewed; not for a range
  #[arg(long)]
  pub since_reviewed: bool,
}

/// `bd review-plan`
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReviewPlanOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  /// The full name of the branch (`{prefix}/virtual/<name>` for a virtual branch); `null` for a range
  pub branch: Option<String>,
  /// The reviewed commits are `base..head`: for a branch the merge base of its tip and `--base` (else the baseline),
  /// and its tip
  pub base: String,
  pub head: String,
  /// What changed since the branch was marked reviewed; `null` for a range and for a branch never marked
  pub interdiff: Option<InterdiffOutput>,
  /// The public declarations the reviewed commits add, remove or change in form, the most usages first. The reviewed
  /// commits are those of `base..head`; with `--since-reviewed` only the added and rewritten ones, each against its own
  /// parent
  pub api: Vec<ApiChangeOutput>,
  /// Every file the reviewed commits change and keep, at its last path (a deleted one is not listed): the `risky` ones
  /// first, then by the most usages, `untested`, path
  pub risk: Vec<FileRiskOutput>,
  /// Every path the reviewed commits change, once, at its last path (a deleted one too), in dependency order: a file
  /// that declares what another uses before it, a test right after its subject; the tree order inside a cycle and
  /// between files with no edge
  pub order: Vec<String>,
  /// Why a removed or changed declaration has no `fan_in`: accio is not installed, has no index of this repository, or
  /// failed; `null` when every query answered or none was asked
  pub index_note: Option<String>,
}

/// A public declaration the reviewed commits add, remove or change
#[derive(Debug, Serialize, JsonSchema)]
pub struct ApiChangeOutput {
  /// The file that declares it after the commits; of a removal, the file before the first commit that touched it, the
  /// old path when that commit renamed it
  pub path: String,
  pub line: u32,
  pub name: String,
  pub kind: ApiChangeKind,
  /// Of a change: the parts of its form that changed
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub changes: Vec<FormChange>,
  /// The header before the commits (the tokens before the body, one space apart), of a change or a removal
  pub before: Option<String>,
  /// The header after the commits, of a change or an addition
  pub after: Option<String>,
  /// Of a removal or a change: its usages at the head, from `accio references`; `null` without an answer (`index_note`)
  /// and for the entries past the first 20
  pub fan_in: Option<FanInOutput>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FanInOutput {
  pub usages: u32,
  /// False when accio cut its answer at 100 usages
  pub complete: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FileRiskOutput {
  pub path: String,
  /// The most usages of a declaration it removes or changes
  pub max_fan_in: Option<u32>,
  /// A source file changed while none of its tests changed
  pub untested: bool,
  /// An API with usages removed or changed, or untested
  pub risky: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InterdiffOutput {
  /// The pair `bd mark-reviewed` kept
  pub reviewed_base: String,
  pub reviewed_tip: String,
  /// The commits of the branch, oldest first, then the dropped ones; with `--since-reviewed`, only the added and the
  /// rewritten ones
  pub commits: Vec<PairedCommitOutput>,
  /// The commits added on top as a plain range when nothing else changed (`head..head`, empty, when nothing changed);
  /// `null` when a commit was rewritten, dropped or added below a reviewed one
  pub scope: Option<RangeOutput>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PairedCommitOutput {
  /// The commit of the branch; `null` when dropped
  pub id: Option<String>,
  /// The reviewed commit it pairs with: the same commit, a copy with the same identity (author, author time, subject),
  /// or a reworded one with the same changed lines; `null` when added
  pub reviewed_id: Option<String>,
  pub subject: String,
  pub state: PairState,
  /// Of a rewritten commit: the lines it changes and its reviewed version did not (`+` added, `-` removed), at most 20
  /// paths of 20 lines
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub new_only: Vec<PathLines>,
  /// Of a rewritten commit: the lines its reviewed version changed and it does not
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub reviewed_only: Vec<PathLines>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RangeOutput {
  pub base: String,
  pub head: String,
}

/// The subject of `args`: a range when it holds `..`, else a branch
fn subject_of(args: &ReviewPlanArgs) -> Result<ReviewSubject, CliError> {
  match args.subject.split_once("..") {
    Some(_) if args.base.is_some() => Err(CliError::new(ErrorCode::InvalidArguments, "--base is for a branch, not for a range")),
    Some((base, head)) => Ok(ReviewSubject::Range {
      base: base.to_string(),
      head: head.to_string(),
    }),
    None => Ok(ReviewSubject::Branch {
      name: args.subject.clone(),
      base: args.base.clone(),
      baseline_branch: None,
    }),
  }
}

pub fn review_plan(context: &RepoContext, args: &ReviewPlanArgs) -> Result<ReviewPlanOutput, CliError> {
  let params = ReviewPlanParams {
    repository_path: context.repository.clone(),
    subject: subject_of(args)?,
    since_reviewed: args.since_reviewed,
  };
  let (cache, never_canceled) = (context.cache(), || false);
  let search = AccioCli::discover().map(|accio| accio.search(&never_canceled));
  let sources = PlanSources {
    cache: &cache,
    file_diffs: context.file_diffs(),
    index: search.as_ref().map(|search| search as &dyn SymbolIndex),
  };
  let plan = review_plan_core(&context.git, sources, &context.prefix, &params).map_err(|error| {
    if error.downcast_ref::<UnknownRevision>().is_some() || error.downcast_ref::<SinceReviewedOfRange>().is_some() {
      return CliError::new(ErrorCode::InvalidArguments, error.to_string());
    }
    refused(error, args.base.is_some())
  })?;
  let interdiff_ids = plan.interdiff.iter().flat_map(|interdiff| {
    let commits = interdiff.commits.iter().flat_map(|commit| commit.id.iter().chain(&commit.reviewed_id));
    [&interdiff.reviewed_base, &interdiff.reviewed_tip].into_iter().chain(commits)
  });
  let short = context.short_ids([&plan.base, &plan.head].into_iter().chain(interdiff_ids).map(String::as_str));
  Ok(ReviewPlanOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    base: short.get(&plan.base).to_string(),
    head: short.get(&plan.head).to_string(),
    interdiff: plan.interdiff.as_ref().map(|interdiff| interdiff_output(interdiff, &short)),
    branch: plan.branch,
    api: plan.api.into_iter().map(api_output).collect(),
    risk: plan.risk.into_iter().map(risk_output).collect(),
    order: plan.order,
    index_note: plan.index_note,
  })
}

fn api_output(change: ApiChange) -> ApiChangeOutput {
  ApiChangeOutput {
    path: change.path,
    line: change.line,
    name: change.name,
    kind: change.kind,
    changes: change.changes,
    before: change.before,
    after: change.after,
    fan_in: change.fan_in.map(|fan_in| FanInOutput {
      usages: fan_in.usages,
      complete: fan_in.complete,
    }),
  }
}

fn risk_output(risk: FileRisk) -> FileRiskOutput {
  FileRiskOutput {
    path: risk.path,
    max_fan_in: risk.max_fan_in,
    untested: risk.untested,
    risky: risk.risky,
  }
}

fn interdiff_output(interdiff: &Interdiff, short: &ShortIds) -> InterdiffOutput {
  let id = |id: &Option<String>| id.as_deref().map(|id| short.get(id).to_string());
  InterdiffOutput {
    reviewed_base: short.get(&interdiff.reviewed_base).to_string(),
    reviewed_tip: short.get(&interdiff.reviewed_tip).to_string(),
    commits: interdiff
      .commits
      .iter()
      .map(|commit| {
        let changes = commit.changes.clone().unwrap_or_default();
        PairedCommitOutput {
          id: id(&commit.id),
          reviewed_id: id(&commit.reviewed_id),
          subject: commit.subject.clone(),
          state: commit.state,
          new_only: changes.new_only,
          reviewed_only: changes.reviewed_only,
        }
      })
      .collect(),
    scope: interdiff.scope.as_ref().map(|scope| RangeOutput {
      base: short.get(&scope.base).to_string(),
      head: short.get(&scope.head).to_string(),
    }),
  }
}
