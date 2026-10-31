//! Squashing `fixup!`, `squash!` and `amend!` commits into their targets, as `git rebase --autosquash` does, without
//! touching the working tree (docs/design/git-operations.md, "Squash fixups"). The plan is pure; the rewrite is one
//! `prepare_squash` with a group per target.

use crate::commit_grouper::GroupingInputs;
use crate::repository_lock::{Prepared, RewriteOptions, Witness};
use crate::snapshot::{CommitsAhead, read_grouping_inputs};
use anyhow::{Result, anyhow, bail};
use domain_types::commit::{Commit, SubjectParts};
use domain_types::rewrite::{AutosquashScope, OrphanFixup, SquashFixupsResult, SquashedTarget};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::autosquash_targets::{FixupTarget, fixup_targets};
use git_ops::commit_writer::read_raw_commits;
use git_ops::history::History;
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::{checked_out_branch, short_id, with_dirs_of};
use git_ops::rewrite_guard::HeadMoved;
use git_ops::rewrite_plan::PreparedRewrite;
use git_ops::squash_commits::{SquashGroup, SquashMessage, prepare_squash};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use sync_utils::commit_subject::first_marker;
use tracing::{info, instrument};

/// How a commit is squashed into its target; the first marker of the subject decides (`fixup! squash! X` is a fixup)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum FixupKind {
  /// `fixup!`: the target keeps its message
  Fixup,
  /// `squash!`: the body of the squash commit is appended to the target's message
  Squash,
  /// `amend!`: the body of the amend commit replaces the target's message
  Amend,
}

impl FixupKind {
  pub(crate) fn of(subject: &str) -> Option<Self> {
    Some([Self::Fixup, Self::Squash, Self::Amend][first_marker(subject)?])
  }
}

/// A commit folded into its target
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlannedFixup {
  pub commit_id: String,
  pub subject: String,
  /// The parts of `subject` the page shows (`Commit::subject_parts`)
  pub subject_parts: SubjectParts,
  pub kind: FixupKind,
}

/// The fixups of one target, oldest first
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlannedGroup {
  pub target: String,
  pub target_subject: String,
  /// The parts of `target_subject` the page shows (`Commit::subject_parts`)
  pub target_parts: SubjectParts,
  /// The virtual branch the target is grouped into; `None` for an unassigned target
  pub branch: Option<String>,
  pub folded: Vec<PlannedFixup>,
}

/// What squashing the fixups of a range does, with the HEAD it was computed for
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutosquashPlan {
  pub head: String,
  /// Ordered by the target, oldest first
  pub groups: Vec<PlannedGroup>,
  pub orphans: Vec<OrphanFixup>,
}

/// Plans squashing every fixup of `commits` (oldest first, ending at `head`) into its target, with git's rules
/// (`git_ops::autosquash_targets::fixup_targets`). With stacked branches on (`grouping.stacks`), prefixes agree when
/// they name the same branch, and the branches are the leaves. The branches follow the issue groups of `grouping` too.
#[must_use]
pub fn plan_autosquash(head: String, commits: &[Commit], grouping: &GroupingInputs) -> AutosquashPlan {
  let branch_of = |subject: &str| grouping.branch_of(subject);
  let resolutions = fixup_targets(commits, grouping.stacks);

  // by target position, so the groups come out oldest target first
  let mut groups: BTreeMap<usize, PlannedGroup> = BTreeMap::new();
  let mut orphans = Vec::new();
  for (commit, resolution) in commits.iter().zip(&resolutions) {
    match *resolution {
      FixupTarget::Commit => {}
      FixupTarget::Orphan => orphans.push(OrphanFixup {
        commit_id: commit.id.clone(),
        subject: commit.subject.clone(),
        subject_parts: commit.subject_parts.clone(),
        branch: branch_of(&commit.subject),
      }),
      FixupTarget::Fixup(target) => {
        let group = groups.entry(target).or_insert_with(|| PlannedGroup {
          target: commits[target].id.clone(),
          target_subject: commits[target].subject.clone(),
          target_parts: commits[target].subject_parts.clone(),
          branch: branch_of(&commits[target].subject),
          folded: Vec::new(),
        });
        group.folded.push(PlannedFixup {
          commit_id: commit.id.clone(),
          subject: commit.subject.clone(),
          subject_parts: commit.subject_parts.clone(),
          kind: FixupKind::of(&commit.subject).expect("a fixup has a marker"),
        });
      }
    }
  }
  AutosquashPlan {
    head,
    groups: groups.into_values().collect(),
    orphans,
  }
}

/// The message of a squashed group, from the target's message and the folded commits' messages (oldest first), as
/// git builds it: a `squash!` appends its body, an `amend!` replaces the message with its body, a `fixup!` adds nothing.
/// `None` keeps the target's message and encoding byte for byte.
#[must_use]
pub fn build_fixup_message(target_message: &str, folded: &[(FixupKind, &str)]) -> Option<String> {
  let mut message: Option<String> = None;
  for &(kind, folded_message) in folded {
    let body = message_body(folded_message);
    if body.is_empty() {
      continue;
    }
    match kind {
      FixupKind::Fixup => {}
      FixupKind::Squash => {
        let mut combined = message.take().unwrap_or_else(|| target_message.trim_end().to_string());
        combined.push_str("\n\n");
        combined.push_str(body);
        message = Some(combined);
      }
      FixupKind::Amend => message = Some(body.to_string()),
    }
  }
  message
}

/// The message after its subject line and the blank lines that follow it, without trailing whitespace
fn message_body(message: &str) -> &str {
  let Some((_, mut body)) = message.split_once('\n') else {
    return "";
  };
  while let Some((line, rest)) = body.split_once('\n')
    && line.trim().is_empty()
  {
    body = rest;
  }
  body.trim_end()
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetAutosquashPlanParams {
  pub repository_path: String,
}

/// What `squash_fixup_commits_core` squashes; the HEAD it expects, if any, is the one of its `RewriteOptions`
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SquashFixupsParams {
  pub repository_path: String,
  pub scope: AutosquashScope,
}

/// The scope has nothing to squash: no fixup, or only fixups without a target in the range (`orphans`)
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("No fixup commits to squash{}", without_target(.orphans))]
pub struct NothingToSquash {
  /// HEAD the plan was made on
  pub head: String,
  /// The fixups of the scope without a target, oldest first
  pub orphans: Vec<OrphanFixup>,
}

/// ": 2 without a target in this range", empty without orphans
fn without_target(orphans: &[OrphanFixup]) -> String {
  match orphans.len() {
    0 => String::new(),
    count => format!(": {count} without a target in this range"),
  }
}

/// What a plan of a fold is made of: the commits ahead of the baseline (oldest first), HEAD, the stack setting and the
/// issue groups, as the sync reads them
pub struct FoldRange {
  head: String,
  commits: Vec<Commit>,
  grouping: GroupingInputs,
}

impl FoldRange {
  pub fn read(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Self> {
    let CommitsAhead { commits, .. } = CommitsAhead::read(git_executor, repository_path)?;
    let head = git_executor.execute_command(&["rev-parse", "HEAD"], repository_path)?.trim().to_string();
    let grouping = read_grouping_inputs(git_executor, repository_path)?;
    Ok(Self { head, commits, grouping })
  }

  #[must_use]
  pub fn plan(&self) -> AutosquashPlan {
    plan_autosquash(self.head.clone(), &self.commits, &self.grouping)
  }

  /// The branch of every commit of the range, named as the plan names them
  #[must_use]
  pub fn branches(&self) -> BTreeSet<String> {
    self.commits.iter().filter_map(|commit| self.grouping.branch_of(&commit.subject)).collect()
  }
}

/// The fixups of the commits ahead of the baseline and where they go
#[instrument(skip(git_executor))]
pub fn get_autosquash_plan_core(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<AutosquashPlan> {
  Ok(FoldRange::read(git_executor, repository_path)?.plan())
}

/// Squashes the fixups of `scope` into their targets in one history rewrite that keeps HEAD's tree, so the index and
/// the working tree are not touched, and answers what it squashed. The plan is computed again, on HEAD as the core
/// reads it, which must be `options.expect_head` when the request names one (the app: the HEAD its rewrite resolved;
/// `bd fold --expect-head`): HEAD, the commits the plan is made of and the history the rewrite replays must all end at
/// it, else the error is `HeadMoved`, which a request that named no HEAD computes once more under the lock. A scope with
/// nothing to squash is `NothingToSquash`. The plan, the new commits and the mapping notes the replayed commits carry
/// come before the repository lock; under it only the witness of HEAD and the compare-and-swap move of the branch (`RewriteOptions::lock_prepared`)
pub fn squash_fixup_commits_core(git_executor: &GitCommandExecutor, params: &SquashFixupsParams, options: &RewriteOptions) -> Result<SquashFixupsResult> {
  squash_fixup_commits_on(git_executor, params, options, None)
}

/// `squash_fixup_commits_core` whose first plan is made of `range` when the caller read it already (`bd fold`, which
/// answers a dry run from it); a plan computed again because HEAD moved reads the range anew
#[instrument(skip(git_executor, options, range))]
pub fn squash_fixup_commits_on(git_executor: &GitCommandExecutor, params: &SquashFixupsParams, options: &RewriteOptions, range: Option<FoldRange>) -> Result<SquashFixupsResult> {
  let repo_path = &params.repository_path;
  // the guards and the lock take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let first = prepare_fold(git_executor, params, options, range);
  let prepare = || prepare_fold(git_executor, params, options, None);
  let (prepared, _lock) = options.lock_prepared(git_executor, repo_path, first, prepare)?;
  let PreparedFold { rewrite, groups, orphans } = prepared;
  rewrite.apply(git_executor, repo_path, RefTransaction::new(), "squash fixups")?;

  let squashed: Vec<SquashedTarget> = groups
    .iter()
    .map(|group| SquashedTarget {
      target: group.target.clone(),
      subject: group.target_subject.clone(),
      new_id: rewrite.replacement_of(&group.target).unwrap_or_else(|| rewrite.new_head()).to_string(),
      fixups: group.folded.iter().map(|fixup| fixup.commit_id.clone()).collect(),
    })
    .collect();
  let squashed_count = groups.iter().map(|group| group.folded.len()).sum::<usize>();
  info!(squashed_count, targets = squashed.len(), "Squashed fixup commits");
  Ok(SquashFixupsResult {
    squashed,
    squashed_count: u32::try_from(squashed_count)?,
    orphans,
    new_head: rewrite.new_head().to_string(),
    previous_head: rewrite.old_head().to_string(),
  })
}

/// A fold computed before the repository lock: its new commits, the groups it squashes and the fixups without a target
struct PreparedFold {
  rewrite: PreparedRewrite,
  groups: Vec<PlannedGroup>,
  orphans: Vec<OrphanFixup>,
}

impl Prepared for PreparedFold {
  fn witness(&self) -> Witness {
    self.rewrite.witness()
  }

  fn discard(&self, git_executor: &GitCommandExecutor, repository_path: &str) {
    self.rewrite.discard(git_executor, repository_path);
  }
}

/// The plan of `params` on HEAD as it reads it (or as `range` read it), the one `options` expects, and the squashed
/// commits written on temporary refs (`prepare_squash`): the branch is not moved
fn prepare_fold(git_executor: &GitCommandExecutor, params: &SquashFixupsParams, options: &RewriteOptions, range: Option<FoldRange>) -> Result<PreparedFold> {
  let repo_path = &params.repository_path;
  let FoldRange { head, commits, grouping } = match range {
    Some(range) => range,
    None => FoldRange::read(git_executor, repo_path)?,
  };
  options.ensure_head(git_executor, repo_path, &head)?;
  // three reads, each of which must end at that HEAD: a commit between them would give a plan for one head and a
  // rewrite of another
  let head_moved = |actual: &str| {
    anyhow::Error::new(HeadMoved {
      branch: checked_out_branch(git_executor, repo_path).unwrap_or_else(|_| "HEAD".to_string()),
      expected: head.clone(),
      actual: actual.to_string(),
    })
  };
  if let Some(newest) = commits.last().filter(|newest| newest.id != head) {
    return Err(head_moved(&newest.id));
  }
  let plan = plan_autosquash(head.clone(), &commits, &grouping);
  let (groups, orphans) = select_scope(plan, &params.scope)?;
  // down to the oldest target: the history the squash replays, which must end at that HEAD too
  let rewritten: Vec<&str> = groups
    .iter()
    .flat_map(|group| std::iter::once(group.target.as_str()).chain(group.folded.iter().map(|fixup| fixup.commit_id.as_str())))
    .collect();
  let history = History::read_to_with_notes(git_executor, repo_path, &rewritten)?;
  if let Some(actual) = history.head().filter(|actual| *actual != head) {
    return Err(head_moved(actual));
  }

  let by_id: HashMap<&str, &Commit> = commits.iter().map(|commit| (commit.id.as_str(), commit)).collect();
  let message_of = |id: &str| by_id.get(id).map_or("", |commit| commit.message.as_str());
  let mut squash_groups = Vec::with_capacity(groups.len());
  // commits whose messages are combined into a new one: it is written as UTF-8
  let mut combined_ids: Vec<&str> = Vec::new();
  for group in &groups {
    let folded: Vec<(FixupKind, &str)> = group.folded.iter().map(|fixup| (fixup.kind, message_of(&fixup.commit_id))).collect();
    let message = match build_fixup_message(message_of(&group.target), &folded) {
      None => SquashMessage::Target,
      Some(text) => {
        combined_ids.push(&group.target);
        combined_ids.extend(group.folded.iter().map(|fixup| fixup.commit_id.as_str()));
        SquashMessage::Text(text)
      }
    };
    squash_groups.push(SquashGroup {
      target: group.target.clone(),
      folded: group.folded.iter().map(|fixup| fixup.commit_id.clone()).collect(),
      message,
    });
  }
  if !combined_ids.is_empty() {
    for (id, raw) in read_raw_commits(git_executor, repo_path, &combined_ids)? {
      if let Some(encoding) = raw.encoding.filter(|encoding| !is_utf8(encoding)) {
        bail!("Can't combine messages in encoding {encoding}: commit {} is not UTF-8", short_id(&id));
      }
    }
  }

  // ready before the lock: the carried mapping notes are written there
  let rewrite = prepare_squash(git_executor, repo_path, &history, &squash_groups)?.ready(git_executor, repo_path)?;
  Ok(PreparedFold { rewrite, groups, orphans })
}

const fn is_utf8(encoding: &str) -> bool {
  encoding.eq_ignore_ascii_case("utf-8") || encoding.eq_ignore_ascii_case("utf8")
}

/// The groups and orphans of `scope`; refused if nothing would be squashed (`NothingToSquash`)
fn select_scope(plan: AutosquashPlan, scope: &AutosquashScope) -> Result<(Vec<PlannedGroup>, Vec<OrphanFixup>)> {
  let AutosquashPlan { head, groups, orphans } = plan;
  let (groups, orphans): (Vec<PlannedGroup>, Vec<OrphanFixup>) = match scope {
    AutosquashScope::All => (groups, orphans),
    AutosquashScope::Branch(name) => (
      groups.into_iter().filter(|group| group.branch.as_ref() == Some(name)).collect(),
      orphans.into_iter().filter(|orphan| orphan.branch.as_ref() == Some(name)).collect(),
    ),
    AutosquashScope::Unassigned => (
      groups.into_iter().filter(|group| group.branch.is_none()).collect(),
      orphans.into_iter().filter(|orphan| orphan.branch.is_none()).collect(),
    ),
    AutosquashScope::Fixup(id) => {
      if orphans.iter().any(|orphan| &orphan.commit_id == id) {
        bail!("The fixup has no target in this range");
      }
      let group = groups
        .into_iter()
        .find_map(|mut group| {
          group.folded.retain(|fixup| &fixup.commit_id == id);
          (!group.folded.is_empty()).then_some(group)
        })
        .ok_or_else(|| anyhow!("The commit is not a fixup of a commit in this range, sync and try again"))?;
      (vec![group], Vec::new())
    }
  };
  if groups.is_empty() {
    return Err(anyhow::Error::new(NothingToSquash { head, orphans }));
  }
  Ok((groups, orphans))
}

#[cfg(test)]
#[path = "autosquash_test.rs"]
mod autosquash_test;
