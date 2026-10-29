//! Squashing `fixup!`, `squash!` and `amend!` commits into their targets, as `git rebase --autosquash` does, without
//! touching the working tree (docs/design/git-operations.md, "Squash fixups"). The plan is pure; the rewrite is one
//! `prepare_squash` with a group per target.

use crate::commit_grouper::GroupingInputs;
use crate::repository_lock::{Prepared, RewriteOptions, Witness, lock_prepared};
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

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SquashFixupsParams {
  pub repository_path: String,
  /// HEAD of the plan the user confirmed; the squash is refused if the branch moved since
  pub expected_head: String,
  pub scope: AutosquashScope,
}

/// The commits ahead of the baseline (oldest first), HEAD, the stack setting and the issue groups, as the sync reads them
fn read_range(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<(String, Vec<Commit>, GroupingInputs)> {
  let CommitsAhead { commits, .. } = CommitsAhead::read(git_executor, repository_path)?;
  let head = git_executor.execute_command(&["rev-parse", "HEAD"], repository_path)?.trim().to_string();
  Ok((head, commits, read_grouping_inputs(git_executor, repository_path)?))
}

/// The fixups of the commits ahead of the baseline and where they go
#[instrument(skip(git_executor))]
pub fn get_autosquash_plan_core(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<AutosquashPlan> {
  let (head, commits, grouping) = read_range(git_executor, repository_path)?;
  Ok(plan_autosquash(head, &commits, &grouping))
}

/// `get_autosquash_plan_core` with the branch of every commit ahead of the baseline, named as the plan names them, from
/// the same read
pub fn get_autosquash_plan_with_branches(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<(AutosquashPlan, BTreeSet<String>)> {
  let (head, commits, grouping) = read_range(git_executor, repository_path)?;
  let branches = commits.iter().filter_map(|commit| grouping.branch_of(&commit.subject)).collect();
  Ok((plan_autosquash(head, &commits, &grouping), branches))
}

/// Squashes the fixups of `scope` into their targets in one history rewrite that keeps HEAD's tree, so the index and
/// the working tree are not touched. The plan is computed again and must be for `expected_head` (and
/// `options.expect_head`): HEAD, the commits the plan is made of and the history the rewrite replays must all end at
/// it, else the error is `HeadMoved`. The plan and the new commits come before the repository lock; under it only the
/// witness of HEAD and the compare-and-swap move of the branch (`RewriteOptions::lock_prepared`)
#[instrument(skip(git_executor, options))]
pub fn squash_fixup_commits_core(git_executor: &GitCommandExecutor, params: &SquashFixupsParams, options: &RewriteOptions) -> Result<SquashFixupsResult> {
  let repo_path = &params.repository_path;
  // the guards and the lock take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let prepare = || prepare_fold(git_executor, params, options);
  // the request names the HEAD of its plan (`expected_head`): another one is its answer, never computed once more
  let (prepared, _lock) = lock_prepared((options.lock_wait, options.held_lock), git_executor, repo_path, prepare()?, prepare)?;
  let PreparedFold { rewrite, groups, orphans } = prepared;
  rewrite.apply(git_executor, repo_path, RefTransaction::new(), "squash fixups")?;

  let squashed: Vec<SquashedTarget> = groups
    .iter()
    .map(|group| SquashedTarget {
      target: group.target.clone(),
      new_id: rewrite.replacement_of(&group.target).unwrap_or_else(|| rewrite.new_head()).to_string(),
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

/// The plan of `params` on the HEAD it expects, and the squashed commits written on temporary refs (`prepare_squash`):
/// the branch is not moved
fn prepare_fold(git_executor: &GitCommandExecutor, params: &SquashFixupsParams, options: &RewriteOptions) -> Result<PreparedFold> {
  let repo_path = &params.repository_path;
  let (head, commits, grouping) = read_range(git_executor, repo_path)?;
  options.ensure_head(git_executor, repo_path, &head)?;
  let head_moved = |actual: &str| {
    anyhow::Error::new(HeadMoved {
      branch: checked_out_branch(git_executor, repo_path).unwrap_or_else(|_| "HEAD".to_string()),
      expected: params.expected_head.clone(),
      actual: actual.to_string(),
    })
  };
  let read_heads = [Some(head.as_str()), commits.last().map(|commit| commit.id.as_str())];
  if let Some(actual) = read_heads.into_iter().flatten().find(|actual| *actual != params.expected_head) {
    return Err(head_moved(actual));
  }
  let plan = plan_autosquash(head, &commits, &grouping);
  let (groups, orphans) = select_scope(plan, &params.scope)?;
  // down to the oldest target: the history the squash replays, which must end at the expected head too
  let rewritten: Vec<&str> = groups
    .iter()
    .flat_map(|group| std::iter::once(group.target.as_str()).chain(group.folded.iter().map(|fixup| fixup.commit_id.as_str())))
    .collect();
  let history = History::read_to_with_notes(git_executor, repo_path, &rewritten)?;
  if let Some(actual) = history.head().filter(|actual| *actual != params.expected_head) {
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

  let rewrite = prepare_squash(git_executor, repo_path, &history, &squash_groups)?;
  Ok(PreparedFold { rewrite, groups, orphans })
}

const fn is_utf8(encoding: &str) -> bool {
  encoding.eq_ignore_ascii_case("utf-8") || encoding.eq_ignore_ascii_case("utf8")
}

/// The groups and orphans of `scope`; refused if nothing would be squashed
fn select_scope(plan: AutosquashPlan, scope: &AutosquashScope) -> Result<(Vec<PlannedGroup>, Vec<OrphanFixup>)> {
  let AutosquashPlan { groups, orphans, .. } = plan;
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
    if orphans.is_empty() {
      bail!("No fixup commits to squash");
    }
    bail!("No fixup commits to squash: {} without a target in this range", orphans.len());
  }
  Ok((groups, orphans))
}

#[cfg(test)]
#[path = "autosquash_test.rs"]
mod autosquash_test;
