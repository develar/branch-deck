//! Squashing `fixup!`, `squash!` and `amend!` commits into their targets, as `git rebase --autosquash` does, without
//! touching the working tree (docs/design/git-operations.md, "Squash fixups"). The plan is pure; the rewrite is one
//! `prepare_squash` with a group per target.

use crate::baseline::detect_baseline_branch;
use crate::commit_grouper::{AUTOSQUASH_MARKERS, GroupingInputs, parse_subject};
use crate::issue_groups::read_grouping_inputs;
use crate::repository_lock::RewriteOptions;
use crate::stack::leaf_of;
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, get_commit_list};
use git_ops::commit_writer::read_raw_commits;
use git_ops::history::History;
use git_ops::ref_transaction::RefTransaction;
use git_ops::rewrite_guard::{HeadMoved, get_current_branch};
use git_ops::squash_commits::{SquashGroup, SquashMessage, prepare_squash};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
  fn of(subject: &str) -> Option<Self> {
    let marker = AUTOSQUASH_MARKERS.iter().position(|marker| subject.starts_with(marker))?;
    Some([Self::Fixup, Self::Squash, Self::Amend][marker])
  }
}

/// A commit folded into its target
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlannedFixup {
  pub commit_id: String,
  pub subject: String,
  pub kind: FixupKind,
}

/// The fixups of one target, oldest first
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlannedGroup {
  pub target: String,
  pub target_subject: String,
  /// The virtual branch the target is grouped into; `None` for an unassigned target
  pub branch: Option<String>,
  pub folded: Vec<PlannedFixup>,
}

/// A fixup without a target among the older commits of the range: it is not squashed
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OrphanFixup {
  pub commit_id: String,
  pub subject: String,
  /// The virtual branch the fixup itself is grouped into; `None` if it is unassigned
  pub branch: Option<String>,
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

/// How a commit of the range was resolved, by position
enum Resolution {
  /// Not a fixup
  Commit,
  /// A fixup of the commit at this position (the final target of a chain)
  Fixup(usize),
  Orphan,
}

/// Plans squashing every fixup of `commits` (oldest first, ending at `head`) into its target, with git's rules
/// (`todo_list_rearrange_squash`): all leading markers are stripped, then the target is the oldest older commit with
///   1. exactly that subject;
///   2. an id starting with it, if it is a hash prefix of at least 4 hex digits that matches one commit;
///   3. the same subject without the `(prefix)`, if the prefixes agree when the fixup names one;
///   4. a subject (or, prefixes agreeing, a subject without the `(prefix)`) starting with it.
///
/// A fixup of a fixup (by hash) goes to the final target. A fixup without a target is an orphan. With stacked branches
/// on (`grouping.stacks`), prefixes agree when they name the same branch, so `fixup! (login) X` finds `(auth/login) X`,
/// and the branches are the leaves. The branches follow the issue groups of `grouping` too.
#[must_use]
pub fn plan_autosquash(head: String, commits: &[Commit], grouping: &GroupingInputs) -> AutosquashPlan {
  let stacks = grouping.stacks;
  let branch_of = |subject: &str| grouping.branch_of(subject);
  let mut resolutions: Vec<Resolution> = Vec::with_capacity(commits.len());
  // full subject -> oldest position
  let mut by_subject: HashMap<&str, usize> = HashMap::new();
  for (index, commit) in commits.iter().enumerate() {
    let resolution = match FixupKind::of(&commit.subject) {
      None => Resolution::Commit,
      Some(_) => match find_target(commits, &by_subject, index, stacks) {
        None => Resolution::Orphan,
        Some(target) => match resolutions[target] {
          Resolution::Commit => Resolution::Fixup(target),
          Resolution::Fixup(final_target) => Resolution::Fixup(final_target),
          Resolution::Orphan => Resolution::Orphan,
        },
      },
    };
    resolutions.push(resolution);
    by_subject.entry(commit.subject.as_str()).or_insert(index);
  }

  // by target position, so the groups come out oldest target first
  let mut groups: BTreeMap<usize, PlannedGroup> = BTreeMap::new();
  let mut orphans = Vec::new();
  for (commit, resolution) in commits.iter().zip(&resolutions) {
    match *resolution {
      Resolution::Commit => {}
      Resolution::Orphan => orphans.push(OrphanFixup {
        commit_id: commit.id.clone(),
        subject: commit.subject.clone(),
        branch: branch_of(&commit.subject),
      }),
      Resolution::Fixup(target) => {
        let group = groups.entry(target).or_insert_with(|| PlannedGroup {
          target: commits[target].id.clone(),
          target_subject: commits[target].subject.clone(),
          branch: branch_of(&commits[target].subject),
          folded: Vec::new(),
        });
        group.folded.push(PlannedFixup {
          commit_id: commit.id.clone(),
          subject: commit.subject.clone(),
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

/// The position of the target of the fixup at `index`, among the older commits
fn find_target(commits: &[Commit], by_subject: &HashMap<&str, usize>, index: usize, stacks: bool) -> Option<usize> {
  let parsed = parse_subject(&commits[index].subject);
  let wanted = &commits[index].subject[parsed.autosquash.len()..];
  if wanted.is_empty() {
    return None;
  }
  if let Some(&target) = by_subject.get(wanted) {
    return Some(target);
  }

  let older = &commits[..index];
  if wanted.len() >= 4 && wanted.bytes().all(|b| b.is_ascii_hexdigit()) {
    let wanted = wanted.to_ascii_lowercase();
    let mut matches = older.iter().enumerate().filter(|(_, commit)| commit.id.starts_with(&wanted));
    if let (Some((target, _)), None) = (matches.next(), matches.next()) {
      return Some(target);
    }
  }

  // fixups are not targets of a subject match: their own subjects start with a marker
  let candidates = || {
    older
      .iter()
      .enumerate()
      .map(|(position, commit)| (position, parse_subject(&commit.subject)))
      .filter(|(_, candidate)| candidate.autosquash.is_empty())
  };
  let prefixes_agree = |candidate: Option<&str>| {
    parsed.prefix.is_none_or(|prefix| match candidate {
      Some(candidate) if stacks => leaf_of(prefix).is_some_and(|leaf| leaf_of(candidate).as_ref() == Some(&leaf)),
      candidate => Some(prefix) == candidate,
    })
  };
  if let Some((target, _)) = candidates().find(|(_, candidate)| candidate.rest == parsed.rest && prefixes_agree(candidate.prefix)) {
    return Some(target);
  }
  candidates()
    .find(|&(position, ref candidate)| {
      older[position].subject.starts_with(wanted) || (!parsed.rest.is_empty() && prefixes_agree(candidate.prefix) && candidate.rest.starts_with(parsed.rest))
    })
    .map(|(target, _)| target)
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

/// Which planned groups a squash takes
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum AutosquashScope {
  All,
  /// The fixups whose target is grouped into this virtual branch
  Branch(String),
  /// The fixups whose target is unassigned
  Unassigned,
  /// Only this fixup commit, into its target
  Fixup(String),
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

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SquashedTarget {
  pub target: String,
  pub new_id: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SquashFixupsResult {
  /// One entry per target, oldest first
  pub squashed: Vec<SquashedTarget>,
  /// Number of fixup commits folded into their targets
  pub squashed_count: u32,
  /// Fixups of the scope left as they are: no target in this range
  pub orphans: Vec<OrphanFixup>,
  /// HEAD after the squash and before it, for an undo
  pub new_head: String,
  pub previous_head: String,
}

/// The commits ahead of the baseline (oldest first), HEAD, the stack setting and the issue groups, as the sync reads them
fn read_range(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<(String, Vec<Commit>, GroupingInputs)> {
  let baseline_branch = detect_baseline_branch(git_executor, repository_path, "master")?;
  let head = git_executor.execute_command(&["rev-parse", "HEAD"], repository_path)?.trim().to_string();
  let commits = get_commit_list(git_executor, repository_path, &baseline_branch)?;
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
/// the working tree are not touched, under the repository lock. The plan is computed again and must be for
/// `expected_head` (and `options.expect_head`): HEAD, the commits the plan is made of and the history the rewrite replays
/// must all end at it, else the error is `HeadMoved`. The rewrite itself moves the branch only from that head
/// (compare-and-swap).
#[instrument(skip(git_executor, options))]
pub fn squash_fixup_commits_core(git_executor: &GitCommandExecutor, params: &SquashFixupsParams, options: &RewriteOptions) -> Result<SquashFixupsResult> {
  let repo_path = &params.repository_path;
  let _lock = options.lock(git_executor, repo_path)?;
  let (head, commits, grouping) = read_range(git_executor, repo_path)?;
  options.ensure_head(git_executor, repo_path, &head)?;
  let head_moved = |actual: &str| {
    anyhow::Error::new(HeadMoved {
      branch: get_current_branch(git_executor, repo_path).unwrap_or_else(|_| "HEAD".to_string()),
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
  let history = History::read_to(git_executor, repo_path, &rewritten)?;
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
        bail!("Can't combine messages in encoding {encoding}: commit {} is not UTF-8", &id[..id.len().min(8)]);
      }
    }
  }

  let prepared = prepare_squash(git_executor, repo_path, &history, &squash_groups)?;
  prepared.apply(git_executor, repo_path, RefTransaction::new(), "squash fixups")?;

  let squashed: Vec<SquashedTarget> = groups
    .iter()
    .map(|group| SquashedTarget {
      target: group.target.clone(),
      new_id: prepared.replacement_of(&group.target).unwrap_or_else(|| prepared.new_head()).to_string(),
    })
    .collect();
  let squashed_count = groups.iter().map(|group| group.folded.len()).sum::<usize>();
  info!(squashed_count, targets = squashed.len(), "Squashed fixup commits");
  Ok(SquashFixupsResult {
    squashed,
    squashed_count: u32::try_from(squashed_count)?,
    orphans,
    new_head: prepared.new_head().to_string(),
    previous_head: prepared.old_head().to_string(),
  })
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
