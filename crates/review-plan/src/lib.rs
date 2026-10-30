//! The plan of a review (spec/branch/review-plan.spec.md, Status: Experimental): what the reviewer of a branch or a
//! range needs first, computed in one place for `bd` and the app (docs/design/git-operations.md, "Review plan").
//! - `mark_reviewed`: `mark_reviewed_core`, the base and the tip a local branch has now kept as its reviewed pair
//!   (`git_ops::reviewed_refs`), which a later review compares the branch with
//! - `interdiff`: the commits of a branch paired with those of its reviewed pair, and the range of the ones added on top
//! - `api`: the public declarations the reviewed commits add, remove or change in form
//! - `fan_in`: the usages of the removed and changed ones at the head, from accio, updated in this change or left
//!   outside
//! - `pair`: the blocks of each changed file from before the commits to after them, as the diffs show the range, and
//!   the lines at the head they change, which tell a use updated from one left outside
//! - `units`: those blocks cut into the changes a reviewer reads once (spec/branch/review-units.spec.md), in the order
//!   of the story
//! - `order`: the changed files in dependency order, a declaration before its uses, a test after its subject
//! - `links`: which changed file uses which, added, removed or kept by the commits
//! - `dependencies`: the entries the commits add to, remove from or change in the build files, and the build files they
//!   add or delete
//! - `files`: per changed file, its links, why its change is mechanical, its repeated edit, the usages of the API it
//!   removes or changes and whether its tests changed with it
//!
//! The types of the plan: `ReviewPlanParams` names what is reviewed (`ReviewSubject`), `ReviewPlan` is the answer of
//! `review_plan_core`, the one entry of `bd review-plan` and the app.

pub mod api;
pub mod dependencies;
pub mod fan_in;
pub mod files;
pub mod interdiff;
pub mod links;
pub mod mark_reviewed;
pub mod order;
pub mod pair;
pub mod units;

use crate::api::{ApiChange, ApiChangeKind, api_changes};
use crate::dependencies::{Dependencies, DependencyChange, dependency_changes, manifest_blobs};
use crate::fan_in::add_usages;
use crate::files::{FileFacts, RepeatedEditGroup, changed_files, file_facts, file_kinds, hunk_blobs};
use crate::interdiff::{Interdiff, PairState, commits_of, interdiff};
use crate::links::{FileLink, link_files};
use crate::mark_reviewed::{BranchOnBase, branch_on_base};
use crate::order::{ReviewOrder, review_order};
use crate::pair::{ChangedLines, pair_blocks};
use crate::units::{ReviewUnit, review_units};
use anyhow::Result;
use file_diffs::accio::SymbolIndex;
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::reviewed_refs::read_reviewed_refs;
use git_ops::revs::resolve_commit;
use object_cache::cache::ObjectCache;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
}

/// The plan of a review
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlan {
  /// The full name of the reviewed branch, `None` for a range
  pub branch: Option<String>,
  /// The commits reviewed are `base..head`: for a branch the merge base of its tip and its base, and its tip
  pub base: String,
  pub head: String,
  /// What changed since the review of the branch; `None` for a range and for a branch never marked reviewed. With
  /// `since_reviewed`, only its `added` and `rewritten` commits
  pub interdiff: Option<Interdiff>,
  /// The public declarations the reviewed commits add, remove or change (`reviewed_commits`), the most usages left
  /// outside the change first
  pub api: Vec<ApiChange>,
  /// Every file the reviewed commits change, once, at its last path (a deleted one too, none added and deleted again):
  /// the most `breaks` first, then the `untested` ones, then in tree order
  pub files: Vec<FileFacts>,
  /// Which of these files uses which, before and after the commits, by the file that uses, then the one that declares
  pub links: Vec<FileLink>,
  /// The entries the commits add to, remove from or change in the build files, and the build files they add or delete:
  /// by build file in tree order, then by kind and name
  pub dependencies: Vec<DependencyChange>,
  /// The groups of files that get the same edit, the most files first
  pub repeated_edits: Vec<RepeatedEditGroup>,
  /// The changes of the pair the plan reads (`units`): every block of its diffs in one unit, every changed file in one
  /// at least, in the order of the story
  pub units: Vec<ReviewUnit>,
  /// Every path the reviewed commits change, once, at its last path (a deleted one too, none added and deleted
  /// again), in dependency order (`order`): a file that declares what another uses before it, a test right after its
  /// subject, the tree order inside a cycle and between files with no edge
  pub order: Vec<String>,
  /// Why a removed or changed declaration has no usages: no accio, no index of the repository, a failed query
  pub index_note: Option<String>,
}

/// What the plan of a window tells it, in order (`get_review_plan` of the app)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReviewPlanEvent {
  /// The plan; `pending` while the index is asked: no entry of its API has usages yet, no file counts `breaks`, and
  /// the plan with them follows
  Plan { plan: Box<ReviewPlan>, pending: bool },
  /// The repository turns the review plan off (`branchdeck.reviewExperimental`): the window shows none
  Disabled,
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
#[error("Only a branch has a review to compare with, not the range {0}")]
pub struct SinceReviewedOfRange(pub String);

/// The plan of the subject of `params`. For a branch: one `for-each-ref` of the reviewed refs with the branches, its base
/// (`branch_on_base`), then its interdiff when it was marked (`interdiff::interdiff`); for a range: one `rev-parse
/// --verify` per rev. Then the API, its usages, the order, the links, the dependency changes and the facts of the files
/// of the reviewed commits (`reviewed_commits`): the order and the links from the summaries the API read, no git; the text of the hunks
/// and of the build files before and after the commits (`manifest_blobs`) from the memory of the diffs when they read
/// it in this process, else in the batch of the summaries (`read_symbol_input_with`), which then runs for those blobs
/// alone when it has no summary or manifest to read. Reads only;
/// `UnknownReviewBranch`, `UnknownReviewBase`, `NoCommonHistory`, `UnknownRevision` and `SinceReviewedOfRange` are the
/// caller's errors, `SearchCanceled` the cancel of `sources.index`
#[instrument(skip(git, sources), fields(subject = ?params.subject))]
pub fn review_plan_core(git: &GitCommandExecutor, sources: PlanSources<'_>, branch_prefix: &str, params: &ReviewPlanParams) -> Result<ReviewPlan> {
  let subject = resolve_subject(git, branch_prefix, params)?;
  review_plan_of(git, sources, params, subject, |_| {})
}

/// What a plan is of, resolved: the branch, `base..head`, and the interdiff of a branch marked reviewed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSubject {
  pub branch: Option<String>,
  pub base: String,
  pub head: String,
  pub interdiff: Option<Interdiff>,
}

/// The subject of `params` resolved (`review_plan_core`): for a branch one `for-each-ref` of the reviewed refs with
/// the branches, its base, then its interdiff when it was marked; for a range one `rev-parse --verify` per rev
pub fn resolve_subject(git: &GitCommandExecutor, branch_prefix: &str, params: &ReviewPlanParams) -> Result<ResolvedSubject> {
  let repository_path = params.repository_path.as_str();
  match &params.subject {
    ReviewSubject::Branch { name, base, baseline_branch } => {
      let refs = read_reviewed_refs(git, repository_path)?;
      let BranchOnBase { branch, tip, base } = branch_on_base(git, repository_path, &refs.branches, branch_prefix, name, (base.as_deref(), baseline_branch.as_deref()))?;
      let interdiff = refs.reviewed(&branch).map(|reviewed| interdiff(git, repository_path, &reviewed, &base, &tip)).transpose()?;
      Ok(ResolvedSubject {
        branch: Some(branch),
        base,
        head: tip,
        interdiff,
      })
    }
    ReviewSubject::Range { base, head } => {
      if params.since_reviewed {
        return Err(SinceReviewedOfRange(format!("{base}..{head}")).into());
      }
      let resolved = |rev: &str| -> Result<String> { resolve_commit(git, repository_path, rev)?.ok_or_else(|| UnknownRevision(rev.to_string()).into()) };
      Ok(ResolvedSubject {
        branch: None,
        base: resolved(base)?,
        head: resolved(head)?,
        interdiff: None,
      })
    }
  }
}

/// The plan of `subject` (`resolve_subject` of `params`): `pending` gets it once without the usages of the API, before
/// the index is asked, when there is a query to ask; the answer is the plan with them
pub fn review_plan_of(git: &GitCommandExecutor, sources: PlanSources<'_>, params: &ReviewPlanParams, subject: ResolvedSubject, pending: impl FnOnce(&ReviewPlan)) -> Result<ReviewPlan> {
  let repository_path = params.repository_path.as_str();
  let ResolvedSubject { branch, base, head, interdiff } = subject;
  let carried = interdiff
    .as_ref()
    .and_then(|found| found.scope.as_ref())
    .filter(|scope| params.since_reviewed && scope.carried);
  let diffs = if let Some(scope) = carried {
    vec![read_pair_diff(git, repository_path, sources.cache, sources.file_diffs, &scope.base, &scope.head)?]
  } else {
    let reviewed = reviewed_commits(git, repository_path, &base, &head, interdiff.as_ref(), params.since_reviewed)?;
    if reviewed.is_empty() {
      Vec::new()
    } else {
      read_commit_diffs_cached(git, repository_path, sources.cache, sources.file_diffs, &reviewed)?
    }
  };
  let changed = changed_files(&diffs);
  let mut seen: HashSet<String> = HashSet::new();
  let wanted: Vec<String> = hunk_blobs(&changed)
    .into_iter()
    .chain(manifest_blobs(&changed))
    .filter(|id| seen.insert(id.clone()))
    .collect();
  let (mut texts, unread) = sources.file_diffs.texts_in_memory(&wanted);
  let input = if diffs.is_empty() {
    None
  } else {
    let (input, read) = read_symbol_input_with(git, repository_path, sources.cache, &diffs, &unread)?;
    texts.extend(unread.into_iter().zip(read).filter_map(|(id, bytes)| Some((id, String::from_utf8(bytes?).ok()?))));
    Some(input)
  };
  let (mut api, order, links) = input.as_ref().map_or_else(
    || (Vec::new(), Vec::new(), Vec::new()),
    |input| {
      let ReviewOrder { order, links: uses } = review_order(input, &changed);
      (api_changes(input, &diffs), order, link_files(input, &changed, &uses))
    },
  );
  let pairs = pair_blocks(git, repository_path, sources.cache, sources.file_diffs, &changed, &texts)?;
  let Dependencies { changes: dependencies, states } = dependency_changes(&changed, &texts);
  let kinds = file_kinds(&changed, &texts, &states);
  let units = input.as_ref().map_or_else(Vec::new, |input| review_units(input, &changed, &pairs, &texts, &kinds, &order));
  let interdiff = interdiff.map(|mut found| {
    if params.since_reviewed {
      found.retain_unreviewed();
    }
    found
  });
  let mut plan = ReviewPlan {
    branch,
    base,
    head,
    interdiff,
    api: Vec::new(),
    files: Vec::new(),
    links,
    dependencies,
    repeated_edits: Vec::new(),
    units,
    order,
    index_note: None,
  };
  if sources.index.is_some() && api.iter().any(|change| change.kind != ApiChangeKind::Added) {
    (plan.files, plan.repeated_edits) = file_facts(&changed, &kinds, &plan.links, &api);
    plan.api.clone_from(&api);
    pending(&plan);
  }
  plan.index_note = add_usages(sources.index, repository_path, &plan.head, || Ok(ChangedLines::of(&changed, &pairs)), &mut api)?;
  (plan.files, plan.repeated_edits) = file_facts(&changed, &kinds, &plan.links, &api);
  plan.api = api;
  Ok(plan)
}

/// The commits the API, the order and the files are read from, oldest first: of `base..head`, or with `since_reviewed`
/// (and no carried scope, whose one diff the plan reads instead) only those added or rewritten since the review,
/// adjacent or not, each read against its own parent. A branch never
/// marked reviewed has no reviewed commit, so all of its commits are new. The commits of the interdiff are those of
/// `base..head` already; else one `rev-list`
fn reviewed_commits(git: &GitCommandExecutor, repository_path: &str, base: &str, head: &str, interdiff: Option<&Interdiff>, since_reviewed: bool) -> Result<Vec<String>> {
  match interdiff {
    Some(interdiff) => Ok(
      interdiff
        .commits
        .iter()
        .filter(|commit| !since_reviewed || matches!(commit.state, PairState::Added | PairState::Rewritten))
        .filter_map(|commit| commit.id.clone())
        .collect(),
    ),
    None => Ok(commits_of(git, repository_path, base, head)?.into_iter().map(|commit| commit.id).collect()),
  }
}

#[cfg(test)]
mod api_test;
#[cfg(test)]
mod dependencies_test;
#[cfg(test)]
mod fan_in_test;
#[cfg(test)]
mod files_test;
#[cfg(test)]
mod interdiff_test;
#[cfg(test)]
mod links_test;
#[cfg(test)]
mod mark_reviewed_test;
#[cfg(test)]
mod order_test;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod units_test;
