//! The plan of a review (openspec/specs/branch/review-plan/spec.md, Status: Experimental): what the reviewer of a branch or a
//! range needs first, computed in one place for `bd` and the app (docs/design/git-operations.md, "Review plan").
//! - `mark_reviewed`: `mark_reviewed_core`, the base and the tip a local branch has now kept as its reviewed pair
//!   (`git_ops::reviewed_refs`), which a later review compares the branch with
//! - `interdiff`: the commits of a branch paired with those of its reviewed pair, and the range of the ones added on top
//! - `api`: the public declarations the reviewed commits add, remove or change in form
//! - `shown`: their forms as the card shows them, and what a change did to each parameter
//! - `fan_in`: the usages of the removed and changed ones at the head, from accio, updated in this change or left
//!   outside
//! - `pair`: the blocks of each changed file from before the commits to after them, as the diffs show the range, and
//!   the lines at the head they change, which tell a use updated from one left outside
//! - `units`: those blocks cut into the changes a reviewer reads once (openspec/specs/branch/review-units/spec.md), in the order
//!   of the story
//! - `order`: the changed files in dependency order, a declaration before its uses, a test after its subject
//! - `links`: which changed file uses which, added, removed or kept by the commits
//! - `dependencies`: the entries the commits add to, remove from or change in the build files, and the build files they
//!   add or delete
//! - `modules`: the modules the commits add, change or delete, the shape of each part and what each newly uses
//! - `registrations`: what the commits register in the platform, the services, extension points, listeners,
//!   extensions, registry keys, actions and content modules their plugin descriptors add, remove or change, from accio
//! - `files`: per changed file, its links, why its change is mechanical, its repeated edit, the usages of the API it
//!   removes or changes and whether its tests changed with it
//! - `steps`: the story of the review (openspec/specs/branch/review-tour/spec.md): the steps the commits tell, of one commit the
//!   parts of its message that name its code (`message_steps`), else the concerns, each with the parts of the units it
//!   shows and the tests that name its code
//! - `brief`: the issue the commits name and the prose of the specs and the docs the change adds or changes, whole
//!   paragraphs changed by word (`prose`)
//! - `story`: the body of each commit once, its code names resolved to the units and the files of the range, and the
//!   new API and the new links between modules of each step
//! - `texts`: the texts of the UI, the keys the commits add to, remove from or change in the message bundles
//! - `reading`: what the pair leaves to read, from the full marks of its diffs (openspec/specs/review/range-reading/spec.md),
//!   and of each layer of a stack
//!
//! The types of the plan: `ReviewPlanParams` names what is reviewed (`ReviewSubject`), `ReviewPlan` is the answer of
//! `review_plan_core`, the one entry of `brd review-plan` and the app.

pub mod api;
pub mod brief;
pub mod concerns;
pub mod dependencies;
pub mod entries;
pub mod fan_in;
pub mod files;
pub mod groups;
pub mod interdiff;
pub mod links;
pub mod mark_reviewed;
pub mod message_steps;
pub mod modules;
pub mod order;
pub mod pair;
pub mod prose;
pub mod reading;
pub mod registrations;
mod renamed_uses;
pub mod shown;
pub mod steps;
pub mod story;
pub mod texts;
pub mod units;
mod whitespace;

use crate::api::{ApiChange, ApiChangeKind, BreakVerdict, api_changes};
use crate::brief::{ReviewBrief, review_brief};
use crate::concerns::{Concern, ConcernEdges, review_concerns};
use crate::dependencies::{Dependencies, DependencyChange, dependency_changes, manifest_blobs};
use crate::entries::add_entries;
use crate::fan_in::{add_usages, verdict_of};
use crate::files::{FileFacts, RepeatedEditGroup, changed_files, file_facts, file_kinds, generated_by_name, generated_of, header_blobs, hunk_blobs};
use crate::groups::order_by_entries;
use crate::interdiff::{Interdiff, MessageChange, PairState, commits_of, interdiff, message_change};
use crate::links::{FileLink, link_files};
use crate::mark_reviewed::{BranchOnBase, branch_on_base, resolve_branch};
use crate::modules::{DeclaredRoots, ModuleShape, Modules, ModulesInput, module_shapes};
use crate::order::{ReviewOrder, review_order};
use crate::pair::{ChangedLines, pair_blocks};
use crate::reading::PlanReading;
use crate::registrations::{Registration, RegistrationsInput, changed_descriptors, registrations_of};
use crate::renamed_uses::DeclaredRenames;
use crate::steps::{ReviewSteps, StepsInput, review_steps};
use crate::texts::{UiText, bundle_blobs, ui_texts};
use crate::units::{FreeOrder, PairUnits, ReviewUnit, review_units};
use crate::whitespace::TextTokens;
use anyhow::Result;
use domain_types::branch_names::{BranchKind, kept_branch_name, strip_branch_namespace};
use domain_types::range_reading::RangeReading;
use file_diffs::accio::SymbolIndex;
use file_diffs::model::DiffSource;
use file_diffs::service::FileDiffs;
use generated_files::attributes::attribute_generated_paths;
use generated_files::generated_by::GeneratedBy;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::landed_pairing::walk_commits_with_messages;
use git_ops::object_texts::into_text;
use git_ops::reviewed_refs::read_reviewed_refs;
use git_ops::revs::resolve_commit;
use object_cache::cache::ObjectCache;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use sync_utils::span::in_current_span;
use tracing::instrument;
use trial_check::diff_scan::{read_commit_diffs_cached, read_pair_diff};
use trial_check::symbol_scan::read_symbol_input_with;

/// What a plan is asked for
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlanParams {
  pub repository_path: String,
  pub subject: ReviewSubject,
  /// Only the commits added or rewritten since the reviewed tip of the branch; `SinceReviewedOfRange` for a range
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub since_reviewed: bool,
  /// The edges the concerns join their parts by: all of them, unless `brd review-plan --concern-edges` measures a part
  #[serde(skip)]
  #[cfg_attr(feature = "specta", specta(skip))]
  pub concern_edges: ConcernEdges,
}

/// What is reviewed
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReviewSubject {
  /// A local branch, by the name `mark_reviewed::resolve_branch` takes, on its base as `mark_reviewed::branch_on_base`
  /// finds it: the reviewed pair is that branch's
  #[serde(rename_all = "camelCase")]
  Branch {
    name: String,
    /// The commit the branch is reviewed on (the head of its stack parent); `None` takes the baseline
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(optional))]
    base: Option<String>,
    /// The baseline as the sync detected it, used without a `base`; `None` detects that of the checked out branch
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(optional))]
    baseline_branch: Option<String>,
  },
  /// The commits of `base..head`; no reviewed pair
  Range { base: String, head: String },
  /// One commit against its first parent, as the diffs of a commit show it; no reviewed pair, only the reviewed commit
  /// its message is compared with
  Commit {
    id: String,
    /// The reviewed commit it pairs with in the interdiff of its branch (`PairedCommit::reviewed_id`): its story then
    /// marks how its message changed since (`StoryCommit::change`)
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(optional))]
    reviewed: Option<String>,
  },
}

/// The plan of a review
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlan {
  /// The full name of the reviewed branch, `None` for a range
  pub branch: Option<String>,
  /// The name of what is reviewed, the headline of its overview: a branch of the prefix by its name as the title bar
  /// shows it (`layout` of `jane/virtual/layout`, `docs` of `jane/unapplied/docs`), another branch by its full name, a
  /// range by its short ids (`795c45d5..ccf33d6d`), a commit by its short id. The page shows the title of the issue the
  /// brief names in its place once YouTrack told it, and none under the title of a pull request
  pub title: String,
  /// The commits reviewed are `base..head`: for a branch the merge base of its tip and its base, and its tip
  pub base: String,
  pub head: String,
  /// What changed since the review of the branch; `None` for a range and for a branch never marked reviewed. With
  /// `since_reviewed`, only its `added` and `rewritten` commits
  pub interdiff: Option<Interdiff>,
  /// The public declarations the reviewed commits add, remove or change (`reviewed_commits`), the most usages left
  /// outside the change first
  pub api: Vec<ApiChange>,
  /// Whether a use left outside the change breaks, over the removed and changed entries of `api` (`fan_in::verdict_of`);
  /// `Pending` in the plan sent before the index answered (`PlanWait::Index`)
  pub verdict: BreakVerdict,
  /// Every file the reviewed commits change, once, at its last path (a deleted one too, none added and deleted again):
  /// the most `breaks` first, then the `untested` ones, then in tree order
  pub files: Vec<FileFacts>,
  /// Which of these files uses which, before and after the commits, by the file that uses, then the one that declares
  pub links: Vec<FileLink>,
  /// The entries the commits add to, remove from or change in the build files, and the build files they add or delete:
  /// by build file in tree order, then by kind and name
  pub dependencies: Vec<DependencyChange>,
  /// The modules of the changed files (`modules`): one the commits add first, then the most changed lines of code and
  /// tests; `FileFacts::module` of each file names its own
  pub modules: Vec<ModuleShape>,
  /// What the commits register in the platform (`registrations`): the services, extension points, listeners,
  /// extensions, registry keys, actions and content modules their plugin descriptors add, remove or change, read by
  /// accio at the base and the head; by the module of the descriptor, the descriptor, then the kind. None without accio
  /// (`index_note`), and in the plan sent before the index answered (`PlanWait::Index`)
  pub registrations: Vec<Registration>,
  /// The texts of the UI the commits add, remove or change: the keys of the message bundles (`texts`), the bundles in
  /// tree order, then by line
  pub texts: Vec<UiText>,
  /// The groups of files that get the same edit, the most files first
  pub repeated_edits: Vec<RepeatedEditGroup>,
  /// The changes of the pair the plan reads (`units`): every block of its diffs in one unit, every changed file in one
  /// at least, in the order of the story
  pub units: Vec<ReviewUnit>,
  /// The tasks the range does, each a set of parts of the units joined by evidence (`concerns`), in the order of the
  /// story; the parts no evidence joined last (`ConcernName::Rest`). No mechanical part but a consequence that follows
  /// its cause: a generated file, a build file, a file of imports alone
  pub concerns: Vec<Concern>,
  /// The story of the review: the steps its commits tell, else its concerns, each with what it shows of the units in
  /// its order and the tests that name its code; every block of the diffs in one stop or in a mechanical unit
  pub steps: ReviewSteps,
  /// The issue the commits name and the prose of the specs and the docs the change adds or changes
  pub brief: ReviewBrief,
  /// Every path the reviewed commits change, once, at its last path (a deleted one too, none added and deleted
  /// again), in dependency order (`order`): a file that declares what another uses before it, a test right after its
  /// subject, the tree order inside a cycle and between files with no edge
  pub order: Vec<String>,
  /// What the pair leaves to read (`reading`), from the full marks of the diffs the window shows of it: of `base..head`,
  /// of a commit against its parent, of the scope of Since Your Review; its renames wait for the index in the plan sent
  /// before the index answered (`Unproven::RenamesPending`)
  pub reading: RangeReading,
  /// Why a removed or changed declaration has no usages: no accio, no index of the repository, a failed query
  pub index_note: Option<String>,
}

impl ReviewPlan {
  /// Whether the plan a reading answered (`review_plan_of`) is what it would be whenever it is asked: every query of the
  /// index answered (`index_note` null), none cut short (`ModuleEntries::cut`) and no rename of its reading waits for
  /// it (`RangeReading::pending`: its query was canceled). Only such a plan is kept in the memory of the app; never
  /// asked of a plan sent while a query runs (`PlanWait`), which is no answer of a reading
  pub fn is_final(&self) -> bool {
    self.index_note.is_none() && self.modules.iter().all(|module| module.entries.as_ref().is_none_or(|entries| !entries.cut)) && !self.reading.pending()
  }
}

/// The commit `rev` names (`rev-parse --verify`), `UnknownRevision` when none
fn resolved(git: &GitCommandExecutor, repository_path: &str, rev: &str) -> Result<String> {
  resolve_commit(git, repository_path, rev)?.ok_or_else(|| UnknownRevision(rev.to_string()).into())
}

/// What the plan of a window tells it, in order (`get_review_plan` of the app)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReviewPlanEvent {
  /// The plan as it stands; `waits` names what the index still answers, and the plan with it follows: a fact of the
  /// plan shows when its query answered. `None` is the plan of the reading, whole
  Plan { plan: Box<ReviewPlan>, waits: Option<PlanWait> },
  /// The repository turns the review plan off (`branchdeck.reviewExperimental`): the window shows none
  Disabled,
}

/// What a plan sent while the index is asked still waits for (`ReviewPlanEvent::Plan::waits`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PlanWait {
  /// Every answer of the index: no entry of its API has usages yet (`BreakVerdict::Pending`), no file counts `breaks`,
  /// it has no registrations, no new module what enters it, and no step the tests that reach its code; the first such
  /// plan has no module and no file of one either (accio's `modules` is asked beside git)
  Index,
  /// The tests of the steps alone (`StepTests::awaited` of each step whose code is asked): the usages, the
  /// registrations and the entry points answered and are in the plan
  Tests,
}

/// What a plan reads beside git: the caches of the diffs and the summaries, and the index of the repository for the
/// usages (`None`: no accio on this machine)
#[derive(Clone, Copy)]
pub struct PlanSources<'a> {
  pub cache: &'a ObjectCache,
  pub file_diffs: &'a FileDiffs,
  pub index: Option<&'a dyn SymbolIndex>,
}

/// A rev of a range names no commit
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("No commit {0:?}")]
pub struct UnknownRevision(pub String);

/// `since_reviewed` was asked of a range, which has no reviewed pair
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Only a branch has a review to compare with, not {0}")]
pub struct SinceReviewedOfRange(pub String);

/// The plan of the subject of `params`. For a branch: one `for-each-ref` of the reviewed refs with the branches, its base
/// (`branch_on_base`), then its interdiff when it was marked (`interdiff::interdiff`); for a range: one `rev-parse
/// --verify` per rev. Then the API, its usages, the order, the links, the dependency changes and the facts of the files
/// of the reviewed commits (`reviewed_commits`): the order and the links from the summaries the API read, no git; the
/// text of the hunks, of the build files and of the message bundles before and after the commits (`manifest_blobs`,
/// `bundle_blobs`) from the memory of the diffs when they read it in this process, else in the batch of the summaries
/// (`read_symbol_input_with`), which then runs for those blobs alone when it has no summary or manifest to read. Reads
/// only;
/// `UnknownReviewBranch`, `UnknownReviewBase`, `NoCommonHistory`, `UnknownRevision` and `SinceReviewedOfRange` are the
/// caller's errors, `SearchCanceled` the cancel of `sources.index`
#[instrument(skip(git, sources), fields(subject = ?params.subject))]
pub fn review_plan_core(git: &GitCommandExecutor, sources: PlanSources<'_>, branch_prefix: &str, params: &ReviewPlanParams) -> Result<ReviewPlan> {
  let subject = resolve_subject(git, branch_prefix, params)?;
  review_plan_of(git, sources, params, subject, |_, _| {})
}

/// What a plan is of, resolved: the branch, `base..head`, and the interdiff of a branch marked reviewed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSubject {
  pub branch: Option<String>,
  /// `ReviewPlan::title`
  pub title: String,
  pub base: String,
  pub head: String,
  pub interdiff: Option<Interdiff>,
  /// How the message of each commit differs from its reviewed version, by its id: of the interdiff of a branch, of a
  /// commit read with its reviewed one
  pub message_changes: HashMap<String, MessageChange>,
  /// The reviewed commit a commit was read with (`ReviewSubject::Commit::reviewed`), by its full id
  pub reviewed_commit: Option<String>,
  /// The virtual branch of the prefix whose head the base of a branch is (its stack parent, as the review window names
  /// the base by that commit), by its name: `ReviewSubject::Branch::base` of the same plan once a rewrite moved both
  /// heads; `None` for a branch on the baseline, a range, a commit
  pub stack_parent: Option<String>,
}

/// The subject of `params` resolved (`review_plan_core`): for a branch one `for-each-ref` of the reviewed refs with
/// the branches, its base, then its interdiff when it was marked; for a range one `rev-parse --verify` per rev, for a
/// commit one for it and one for its first parent, and with its reviewed commit one `rev-list` of both messages
pub fn resolve_subject(git: &GitCommandExecutor, branch_prefix: &str, params: &ReviewPlanParams) -> Result<ResolvedSubject> {
  let repository_path = params.repository_path.as_str();
  match &params.subject {
    ReviewSubject::Branch {
      name,
      base: named,
      baseline_branch,
    } => {
      let refs = read_reviewed_refs(git, repository_path)?;
      let BranchOnBase { branch, tip, base } = branch_on_base(git, repository_path, &refs.branches, branch_prefix, name, (named.as_deref(), baseline_branch.as_deref()))?;
      let stack_parent = named.as_deref().and_then(|named| stack_parent(&refs.branches, branch_prefix, named, &base));
      let interdiff = refs.reviewed(&branch).map(|reviewed| interdiff(git, repository_path, &reviewed, &base, &tip)).transpose()?;
      let message_changes = interdiff
        .iter()
        .flat_map(|found| &found.commits)
        .filter_map(|commit| Some((commit.id.clone()?, commit.message_change.clone()?)))
        .collect();
      Ok(ResolvedSubject {
        title: branch_title(&branch, branch_prefix),
        branch: Some(branch),
        base,
        head: tip,
        interdiff,
        message_changes,
        reviewed_commit: None,
        stack_parent,
      })
    }
    ReviewSubject::Range { base, head } => {
      if params.since_reviewed {
        return Err(SinceReviewedOfRange(format!("{base}..{head}")).into());
      }
      let (base, head) = (resolved(git, repository_path, base)?, resolved(git, repository_path, head)?);
      Ok(ResolvedSubject {
        branch: None,
        title: format!("{}..{}", short_id(&base), short_id(&head)),
        base,
        head,
        interdiff: None,
        message_changes: HashMap::new(),
        reviewed_commit: None,
        stack_parent: None,
      })
    }
    ReviewSubject::Commit { id, reviewed } => {
      if params.since_reviewed {
        return Err(SinceReviewedOfRange(id.clone()).into());
      }
      // a root commit has no parent to read it against: `UnknownRevision` of `<id>^`
      let head = resolved(git, repository_path, id)?;
      let base = resolved(git, repository_path, &format!("{id}^"))?;
      let (reviewed_commit, message_changes) = match reviewed {
        Some(reviewed) => reviewed_message_change(git, repository_path, &head, reviewed)?,
        None => (None, HashMap::new()),
      };
      Ok(ResolvedSubject {
        branch: None,
        title: short_id(&head).to_string(),
        base,
        head,
        interdiff: None,
        message_changes,
        reviewed_commit,
        stack_parent: None,
      })
    }
  }
}

/// The stack parent a branch is reviewed on, by its name (`ResolvedSubject::stack_parent`): the branch `named` names, or
/// the virtual branch of the prefix whose tip is `base` (the merge base of the branch and the commit `named`); the first
/// by name should two have that tip
fn stack_parent(branches: &HashMap<String, String>, branch_prefix: &str, named: &str, base: &str) -> Option<String> {
  if resolve_branch(branches, branch_prefix, named).is_some() {
    return Some(named.to_string());
  }
  branches
    .iter()
    .filter(|(_, tip)| *tip == base)
    .filter_map(|(full, _)| strip_branch_namespace(full, branch_prefix, BranchKind::Virtual))
    .min()
    .map(str::to_string)
}

/// The reviewed commit of `head` by its full id and how the message of `head` differs from its message: one `rev-list`
/// of both. A reviewed commit git no longer has (a `git gc` after its pair was replaced) is none, and so is its change
fn reviewed_message_change(git: &GitCommandExecutor, repository_path: &str, head: &str, reviewed: &str) -> Result<(Option<String>, HashMap<String, MessageChange>)> {
  let read = walk_commits_with_messages(git, repository_path, &["--no-walk", "--ignore-missing"], &[head, reviewed])?;
  let message_of = |found: &dyn Fn(&str) -> bool| {
    read
      .iter()
      .find(|(commit, _)| found(&commit.id))
      .map(|(commit, message)| (commit.id.as_str(), message.as_str()))
  };
  let (Some((_, now)), Some((reviewed_id, before))) = (message_of(&|id| id == head), message_of(&|id| id != head && id.starts_with(reviewed))) else {
    return Ok((None, HashMap::new()));
  };
  let changes = message_change(before, now).map(|change| (head.to_string(), change)).into_iter().collect();
  Ok((Some(reviewed_id.to_string()), changes))
}

/// The name of a reviewed branch (`ReviewPlan::title`): a virtual branch of the prefix by its name, a kept one by the
/// branch it was, another by its full name
fn branch_title(branch: &str, branch_prefix: &str) -> String {
  strip_branch_namespace(branch, branch_prefix, BranchKind::Virtual)
    .or_else(|| kept_branch_name(branch, branch_prefix))
    .unwrap_or(branch)
    .to_string()
}

/// The short form of a commit id in the title of a range
fn short_id(id: &str) -> &str {
  id.get(..8).unwrap_or(id)
}

/// What the plan tells of its pair: its units, its concerns, its brief and its steps, each read from the ones before
struct Story {
  units: PairUnits,
  concerns: Vec<Concern>,
  brief: ReviewBrief,
  steps: ReviewSteps,
}

/// The plan of `subject` (`resolve_subject` of `params`), a fact of it shown when its query answered: `stage` gets the
/// plan as it stands with what it still waits for, and the answer is the plan whole.
/// - `PlanWait::Index`, before the index answered, when there is a query to ask: the plan of git alone. The modules
///   are a query of the index too (`DeclaredRoots::ask`, accio's `modules` at the base and the head), asked while git
///   reads: of a plan with an index and a changed file, the plan of git is sent without them (no module, every file of
///   none), then once again with them while another query is left to ask. A cold answer takes several times what git
///   takes; the second telling of the concerns and the steps costs a fraction of a second on a range of tens of
///   commits, a whole telling (tens of seconds) on one of thousands of files (decision 30 of change-review.md).
/// - `PlanWait::Tests`, once the usages, the registrations and the entry points answered, when the tests of the steps
///   are a query of the index (`TestsAsk::asks_index`) that still runs: the plan with all but them. The usages and the
///   moves need no module: they are asked with the modules, once git read the plan. The tests, the registrations and
///   the entry points are of the modules, asked once those answered; the tests are the slow one (cold after each new
///   tree of a branch), so they hold none of the others.
///
/// A plan `stage` gets is no answer of a reading: only the returned one may be kept (`ReviewPlan::is_final`)
pub fn review_plan_of(
  git: &GitCommandExecutor,
  sources: PlanSources<'_>,
  params: &ReviewPlanParams,
  subject: ResolvedSubject,
  mut stage: impl FnMut(&ReviewPlan, PlanWait),
) -> Result<ReviewPlan> {
  let repository_path = params.repository_path.as_str();
  let ResolvedSubject {
    branch,
    title,
    base,
    head,
    interdiff,
    message_changes,
    reviewed_commit: _,
    stack_parent: _,
  } = subject;
  let carried = interdiff
    .as_ref()
    .and_then(|found| found.scope.as_ref())
    .filter(|scope| params.since_reviewed && scope.carried);
  // the commits read, each with its message: the carried scope is one diff of no commit, which tells no step
  let reviewed = if carried.is_some() {
    Vec::new()
  } else {
    reviewed_commits(git, repository_path, &base, &head, interdiff.as_ref(), params.since_reviewed)?
  };
  let diffs = if let Some(scope) = carried {
    vec![read_pair_diff(git, repository_path, sources.cache, sources.file_diffs, &scope.base, &scope.head)?]
  } else {
    let ids: Vec<String> = reviewed.iter().map(|(id, _)| id.clone()).collect();
    if ids.is_empty() {
      Vec::new()
    } else {
      read_commit_diffs_cached(git, repository_path, sources.cache, sources.file_diffs, &ids)?
    }
  };
  let changed = changed_files(&diffs);
  // the generated files by their name, else by the attributes of the head: one `check-attr` for the others
  let by_attributes = attribute_generated_paths(
    git,
    repository_path,
    &head,
    changed.iter().filter(|file| generated_by_name(file.path).is_none()).map(|file| file.path),
  )?;
  let known: Vec<_> = changed
    .iter()
    .map(|file| generated_by_name(file.path).or_else(|| by_attributes.contains(file.path).then_some(GeneratedBy::Attribute)))
    .collect();
  let mut seen: HashSet<String> = HashSet::new();
  let wanted: Vec<String> = hunk_blobs(&changed, &known)
    .into_iter()
    .chain(manifest_blobs(&changed))
    .chain(header_blobs(&changed, &known))
    .chain(bundle_blobs(&changed))
    .filter(|id| seen.insert(id.clone()))
    .collect();
  // what the pair leaves to read, of the diffs the window shows of it, its renames asked of the index last: read first,
  // the blobs its diffs read are in the memory of the diffs for the texts of the plan
  let source = reading_source(params, &base, &head, interdiff.as_ref());
  let index = sources.index;
  let modules_base = modules_base(params, &base, interdiff.as_ref());
  let modules_head = head.clone();
  let module_paths: Vec<String> = changed.iter().map(|file| file.path.to_string()).collect();
  let asks_modules = index.is_some() && !module_paths.is_empty();
  // the revisions the registrations compare: of Since Your Review the ends of its scope, none when its carry conflicted
  let sides = match interdiff.as_ref().filter(|_| params.since_reviewed) {
    Some(found) => found.scope.as_ref().map(|scope| (scope.base.clone(), scope.head.clone())),
    None => Some((base.clone(), head.clone())),
  };
  let interdiff = interdiff.map(|mut found| {
    if params.since_reviewed {
      found.retain_unreviewed();
    }
    found
  });
  std::thread::scope(|threads| -> Result<ReviewPlan> {
    // the modules of the files as accio reads the build files of the head reviewed and of the base, not of the
    // checkout (a pull request is not checked out), while git reads the rest
    let roots_query = threads.spawn(in_current_span(move || {
      DeclaredRoots::ask(index, repository_path, &modules_base, &modules_head, &module_paths)
    }));
    let mut pair_reading = PlanReading::collect(git, sources, repository_path, source.clone())?;
    let (mut texts, unread) = sources.file_diffs.texts_in_memory(&wanted);
    let input = if diffs.is_empty() {
      None
    } else {
      let (input, read) = read_symbol_input_with(git, repository_path, &head, sources.cache, &diffs, &unread)?;
      // as the memory of the diffs holds a text, an invalid sequence replaced: the plan is the same whether the diffs
      // came from git in this process or from the object cache
      texts.extend(unread.into_iter().zip(read).filter_map(|(id, bytes)| Some((id, into_text(bytes?)))));
      Some(input)
    };
    let pairs = pair_blocks(git, repository_path, sources.cache, sources.file_diffs, &changed, &texts)?;
    let (mut api, order, uses) = input.as_ref().map_or_else(
      || (Vec::new(), Vec::new(), Vec::new()),
      |input| {
        let ReviewOrder { order, links: uses } = review_order(input, &changed);
        (api_changes(input, &diffs), order, uses)
      },
    );
    let Dependencies {
      changes: dependencies,
      states,
      added: added_manifests,
    } = dependency_changes(&changed, &texts);
    let generated = generated_of(&changed, &known, &texts);
    // the tokens of the texts, read once: what tells a change of whitespace alone in a file of a grammar
    let tokens = TextTokens::new(&texts);
    let kinds = file_kinds(&changed, &texts, &tokens, &states, &generated);
    let declared = input.as_ref().map_or_else(HashSet::new, |input| declared_names(input, &changed));
    // what the roots of the modules shape: the links, which cross modules by them, the modules and their texts. Before
    // the roots answer (`None`) the plan has no module, not one module of every file at the root
    let shaped = |roots: Option<&DeclaredRoots>| -> (Vec<FileLink>, Modules, Vec<UiText>) {
      let none = DeclaredRoots::default();
      let links = input.as_ref().map_or_else(Vec::new, |input| link_files(input, &changed, &uses, roots.unwrap_or(&none)));
      let modules = roots.map_or_else(Modules::default, |roots| {
        module_shapes(&ModulesInput {
          changed: &changed,
          roots,
          mechanical: &kinds.mechanical,
          pairs: &pairs,
          links: &links,
          dependencies: &dependencies,
          added_manifests: &added_manifests,
        })
      });
      let ui = ui_texts(&changed, &texts, &modules);
      (links, modules, ui)
    };
    // the API as the plan of git has it: the story is told by it, also when the moves the index answers tell it again
    let story_api = api.clone();
    // the units of the pair, which the free reorders the index answered fold and the renames of declarations it has not
    // proven pair: read once of what git read, whatever the modules
    let units_of = |free: &FreeOrder, renames: &DeclaredRenames| {
      input.as_ref().map_or_else(PairUnits::default, |input| {
        review_units(input, &changed, &pairs, &texts, &tokens, &kinds, &order, free, renames)
      })
    };
    // the concerns, the brief and the steps of the units, which the links and the modules shape
    let story = |units: PairUnits, links: &[FileLink], modules: &Modules| -> Story {
      let concerns = review_concerns(&units.units, &changed, &kinds.generated, &texts, links, &declared, params.concern_edges);
      let brief = review_brief(reviewed.iter().map(|(_, message)| message.as_str()), &units.units, &changed, &texts);
      let steps = review_steps(&StepsInput {
        commits: &reviewed,
        message_changes: &message_changes,
        diffs: &diffs,
        changed: &changed,
        pairs: &pairs,
        units: &units.units,
        concerns: &concerns,
        input: input.as_ref(),
        texts: &texts,
        links,
        contract: &brief.contract,
        api: &story_api,
        issue: brief.issue.as_deref(),
        modules,
      });
      Story { units, concerns, brief, steps }
    };
    // the plan of the roots, if they answered, with no other answer of the index in it, and the paths whose commits
    // change a test inside them
    let renames = pair_reading.declared_renames();
    let units = units_of(&FreeOrder::default(), &renames);
    // a declaration that moved with its code the same is folded where the index says its order is free
    let asks_moves = units.moved && index.is_some();
    let plan_of = |roots: Option<&DeclaredRoots>, pair_reading: &PlanReading| -> (ReviewPlan, Modules, HashSet<String>) {
      let (links, modules, ui) = shaped(roots);
      let Story {
        units: PairUnits { units, tests, moved: _ },
        concerns,
        brief,
        steps,
      } = story(units.clone(), &links, &modules);
      let mut plan = ReviewPlan {
        branch: branch.clone(),
        title: title.clone(),
        base: base.clone(),
        head: head.clone(),
        interdiff: interdiff.clone(),
        api: Vec::new(),
        verdict: BreakVerdict::Pending,
        files: Vec::new(),
        links,
        dependencies: dependencies.clone(),
        modules: modules.shapes.clone(),
        registrations: Vec::new(),
        texts: ui,
        repeated_edits: Vec::new(),
        units,
        concerns,
        steps,
        brief,
        order: order.clone(),
        reading: pair_reading.reading(),
        index_note: None,
      };
      pair_reading.mark_renamed_uses(&mut plan.units);
      (plan, modules, tests)
    };
    // the usages of the API and the moves of the pairs need no module: each one query of the index, asked as soon as
    // git read the plan, beside the modules, and joined where the plan takes their answers
    std::thread::scope(|early| -> Result<ReviewPlan> {
      // the plan sent before the index answered (`PlanWait::Index`): its files with their modules and the API of git
      let mut stage_index = |plan: &ReviewPlan, modules: &Modules, inline_tests: &HashSet<String>| {
        let mut staged = plan.clone();
        (staged.files, staged.repeated_edits) = file_facts(&changed, &kinds, &staged.links, &story_api, inline_tests);
        with_modules(&mut staged.files, &changed, modules);
        staged.api.clone_from(&story_api);
        stage(&staged, PlanWait::Index);
      };
      let usages = {
        let (head, changed, pairs) = (&head, &changed, &pairs);
        early.spawn(in_current_span(move || -> Result<_> {
          let note = add_usages(index, repository_path, head, || Ok(ChangedLines::of(changed, pairs)), &mut api)?;
          Ok((api, note))
        }))
      };
      let moves = index
        .filter(|_| asks_moves)
        .map(|index| early.spawn(in_current_span(|| sources.file_diffs.moves(index, repository_path, &source))));
      // the modules are a query of the index too, cold after each new tree several times what git took: the plan of
      // git does not wait for them. It is sent without them, each step whose code is asked awaiting its tests, and
      // they come with the next plan. Decided by what is asked, never by which thread ended first
      if asks_modules {
        let (mut plan, modules, inline_tests) = plan_of(None, &pair_reading);
        if plan.steps.tests_ask().asks_index(index) {
          plan.steps.await_tests();
        }
        stage_index(&plan, &modules, &inline_tests);
      }
      let roots = roots_query.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the query of the modules panicked")))?;
      let (mut plan, modules, mut inline_tests) = plan_of(Some(&roots), &pair_reading);
      let descriptors = changed_descriptors(&changed);
      let asks = story_api.iter().any(|change| change.kind != ApiChangeKind::Added)
        || asks_moves
        || plan.steps.steps.iter().any(|step| !step.code.is_empty())
        || (sides.is_some() && !descriptors.is_empty())
        || modules.shapes.iter().any(|module| module.state == modules::ModuleState::Added);
      // what the steps ask of their tests owns its places: its query runs beside the plan for the whole reading
      let tests_ask = plan.steps.tests_ask();
      let tests_asked = tests_ask.asks_index(index);
      if tests_asked {
        plan.steps.await_tests();
      }
      if index.is_some() && asks {
        stage_index(&plan, &modules, &inline_tests);
      }
      let head = plan.head.clone();
      let registrations_input = RegistrationsInput {
        repository_path,
        sides: sides.as_ref().map(|(base, head)| (base.as_str(), head.as_str())),
        files: &descriptors,
        changed: &changed,
        modules: &modules,
      };
      // the tests of the steps, the descriptors of each side and what enters each new module, each one query of the
      // index of the modules, at once; the plan waits for the tests last, with the answers of the others in it
      std::thread::scope(|reading| {
        let tests = reading.spawn(in_current_span(|| tests_ask.ask(index, repository_path, &head)));
        let shapes = &mut plan.modules;
        let (usages, registrations, entries) = std::thread::scope(|scope| {
          let registrations = scope.spawn(in_current_span(|| registrations_of(index, &registrations_input)));
          let entries = scope.spawn(in_current_span(|| add_entries(index, repository_path, &head, shapes)));
          (
            usages.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the query of the usages panicked"))),
            registrations.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the query of the descriptors panicked"))),
            entries.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the query of the entry points panicked"))),
          )
        });
        let (registrations, registrations_note) = registrations?;
        entries?;
        let (api, usages_note) = usages?;
        // the story again where the index says the order of a declaration that moved is free: it folds that move
        let mut free = FreeOrder::default();
        if let Some(moves) = moves {
          let answered = moves.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the query of the moves panicked")))?;
          free = answered.map_or_default(|moves| FreeOrder::of(&moves));
          if !free.is_empty() {
            let told = story(units_of(&free, &renames), &plan.links, &modules);
            (plan.units, inline_tests, plan.concerns, plan.brief, plan.steps) = (told.units.units, told.units.tests, told.concerns, told.brief, told.steps);
            pair_reading.mark_renamed_uses(&mut plan.units);
            if tests_asked {
              plan.steps.await_tests();
            }
          }
        }
        // the group of a new module reads from its entry points
        order_by_entries(&mut plan.steps.steps, &plan.modules);
        plan.registrations = registrations;
        (plan.files, plan.repeated_edits) = file_facts(&changed, &kinds, &plan.links, &api, &inline_tests);
        with_modules(&mut plan.files, &changed, &modules);
        plan.verdict = verdict_of(&api);
        plan.api = api;
        // decided by the ask, not by the threads: a reading whose tests are no query of the index has no such stage,
        // and one whose query answered already has its plan whole
        if tests_asked && !tests.is_finished() {
          plan.index_note = usages_note.clone().or_else(|| registrations_note.clone());
          stage(&plan, PlanWait::Tests);
        }
        // the renames last, while the tests are asked, when the window that shows the pair asked them already: memory
        // keeps its final answer. Where the answer pairs other declarations renamed (`DeclaredRenames::pairs`: the index
        // proved one), the story is told again; else only what each rename waits on changes
        pair_reading.answer(git, sources)?;
        plan.reading = pair_reading.reading();
        let answered = pair_reading.declared_renames();
        if answered.pairs() == renames.pairs() {
          answered.mark_waits(&mut plan.units);
        } else {
          let told = story(units_of(&free, &answered), &plan.links, &modules);
          (plan.units, inline_tests, plan.concerns, plan.brief, plan.steps) = (told.units.units, told.units.tests, told.concerns, told.brief, told.steps);
          order_by_entries(&mut plan.steps.steps, &plan.modules);
          let (files, repeated_edits) = file_facts(&changed, &kinds, &plan.links, &plan.api, &inline_tests);
          (plan.files, plan.repeated_edits) = (files, repeated_edits);
          with_modules(&mut plan.files, &changed, &modules);
        }
        pair_reading.mark_renamed_uses(&mut plan.units);
        let answer = tests.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the query of the tests panicked")))?;
        // after `order_by_entries`: it moves the stops of a step, never a step, so `proves` are of the final order
        // either way
        let tests_note = plan.steps.apply_tests(answer);
        plan.index_note = usages_note.or(tests_note).or(registrations_note);
        Ok(plan)
      })
    })
  })
}

/// The diffs a plan reads what is left to read of: those the window shows of its subject. A commit against its first
/// parent; Since Your Review its scope (the reviewed tip, carried, to the tip), else `base..head`
fn reading_source(params: &ReviewPlanParams, base: &str, head: &str, interdiff: Option<&Interdiff>) -> DiffSource {
  if matches!(params.subject, ReviewSubject::Commit { .. }) {
    return DiffSource::Commit { id: head.to_string() };
  }
  match interdiff.and_then(|found| found.scope.as_ref()).filter(|_| params.since_reviewed) {
    Some(scope) => DiffSource::Range {
      base: scope.base.clone(),
      head: scope.head.clone(),
    },
    None => DiffSource::Range {
      base: base.to_string(),
      head: head.to_string(),
    },
  }
}

/// The side the modules of the files are compared with, a module of the head alone added: `base`, of Since Your Review
/// the start of its scope, and the reviewed tip where its carry conflicted, so a module the reviewer read is not added
/// again by the new commits that touch it
fn modules_base(params: &ReviewPlanParams, base: &str, interdiff: Option<&Interdiff>) -> String {
  match interdiff.filter(|_| params.since_reviewed) {
    Some(found) => found.scope.as_ref().map_or_else(|| found.reviewed_tip.clone(), |scope| scope.base.clone()),
    None => base.to_string(),
  }
}

/// `FileFacts::module` and `FileFacts::folder` of each file
fn with_modules(files: &mut [FileFacts], changed: &[files::ChangedFile<'_>], modules: &Modules) {
  for facts in files {
    facts.module = modules.of_path(changed, &facts.path);
    facts.folder = modules.folder_of_path(changed, &facts.path);
  }
}

/// The names the changed files declare on either side, lowercase: what a rare token of code may be (`concerns`)
fn declared_names(input: &trial_check::symbol_scan::SymbolInput, changed: &[files::ChangedFile<'_>]) -> HashSet<String> {
  changed
    .iter()
    .flat_map(|file| [file.before, file.after])
    .flatten()
    .filter_map(|blob| input.summaries.get(blob))
    .flat_map(|summary| summary.declarations.iter().map(|declaration| summary.name(declaration.name).to_lowercase()))
    .collect()
}

/// The commits the API, the order and the files are read from, oldest first, each with its message: of `base..head`,
/// or with `since_reviewed` (and no carried scope, whose one diff the plan reads instead) only those added or rewritten
/// since the review, adjacent or not, each read against its own parent. A branch never marked reviewed has no reviewed
/// commit, so all of its commits are new. The commits of the interdiff are those of `base..head` already; else one
/// `rev-list`
fn reviewed_commits(git: &GitCommandExecutor, repository_path: &str, base: &str, head: &str, interdiff: Option<&Interdiff>, since_reviewed: bool) -> Result<Vec<(String, String)>> {
  match interdiff {
    Some(interdiff) => Ok(
      interdiff
        .commits
        .iter()
        // a reworded commit is unread for its message alone: its code is the code the reviewer read
        .filter(|commit| !since_reviewed || matches!(commit.state, PairState::Added | PairState::Rewritten))
        .filter_map(|commit| Some((commit.id.clone()?, commit.message.clone())))
        .collect(),
    ),
    None => Ok(
      commits_of(git, repository_path, base, head)?
        .into_iter()
        .map(|(commit, message)| (commit.id, message))
        .collect(),
    ),
  }
}

#[cfg(test)]
mod api_test;
#[cfg(test)]
mod concerns_test;
#[cfg(test)]
mod dependencies_test;
#[cfg(test)]
mod fan_in_test;
#[cfg(test)]
mod files_test;
#[cfg(test)]
mod groups_test;
#[cfg(test)]
mod interdiff_test;
#[cfg(test)]
mod links_test;
#[cfg(test)]
mod mark_reviewed_test;
#[cfg(test)]
mod message_steps_test;
#[cfg(test)]
mod modules_test;
#[cfg(test)]
mod order_test;
#[cfg(test)]
mod plan_sources_test;
#[cfg(test)]
mod prose_test;
#[cfg(test)]
mod reading_test;
#[cfg(test)]
mod registrations_test;
#[cfg(test)]
mod renamed_uses_test;
#[cfg(test)]
mod shown_test;
#[cfg(test)]
mod stages_test;
#[cfg(test)]
mod steps_test;
#[cfg(test)]
mod story_test;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod texts_test;
#[cfg(test)]
mod units_test;
