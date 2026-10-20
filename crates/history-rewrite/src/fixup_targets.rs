//! The `fixup!`, `squash!` and `amend!` commits of a regroup or a reword: a rewrite of messages may not change what they
//! fold into. A fixup finds its target by subject or by id (`autosquash.rs`), so a new subject, a new id of a recreated
//! target or a marker added or removed would fold it elsewhere or nowhere, and a waiting `amend!` would replace a new
//! message at the next `bd fold`

use crate::autosquash::{AutosquashPlan, FixupKind, plan_autosquash};
use crate::commit_grouper::{GroupingInputs, parse_subject};
use crate::regroup::{FixupRefusal, RegroupError};
use crate::reword::keeps_virtual_commit;
use crate::snapshot::Snapshot;
use git_ops::commit_list::Commit;
use git_ops::revs::short_id;
use std::collections::HashMap;

/// Refuses `regrouped` (the commits of `snapshot` with their new messages, by position) when a fixup would fold into
/// another target than in `before` (the plan of the messages now) or into none, when a rewrite adds or removes a marker,
/// when a target that a fixup names by its id gets a new id, or when the text of a target changes while an `amend!` of
/// it waits
pub(crate) fn check_fixup_targets(snapshot: &Snapshot, regrouped: &[Commit], before: &AutosquashPlan, grouping: &GroupingInputs) -> Result<(), RegroupError> {
  let commits = &snapshot.commits;
  let Some(oldest_changed) = commits.iter().zip(regrouped).position(|(old, new)| old.message != new.message) else {
    return Ok(());
  };

  let marker_changed: Vec<String> = commits
    .iter()
    .zip(regrouped)
    .filter(|(old, new)| FixupKind::of(&old.subject) != FixupKind::of(&new.subject))
    .map(|(old, _)| short_id(&old.id).to_string())
    .collect();
  if !marker_changed.is_empty() {
    return Err(stranded(FixupRefusal::MarkerChanged, marker_changed));
  }

  let after = plan_autosquash(snapshot.head.clone(), regrouped, grouping);
  let targets_before = fixup_targets(before);
  let targets_after = fixup_targets(&after);
  let mut retargeted: Vec<String> = targets_before
    .iter()
    .filter(|(fixup, target)| targets_after.get(*fixup) != Some(*target))
    .chain(targets_after.iter().filter(|(fixup, _)| !targets_before.contains_key(*fixup)))
    .map(|(fixup, _)| short_id(fixup).to_string())
    .collect();
  if !retargeted.is_empty() {
    retargeted.sort();
    retargeted.dedup();
    return Err(stranded(FixupRefusal::Retargeted, retargeted));
  }

  // the rewrite recreates every commit from the oldest changed one: a target named by its id loses that id
  let position: HashMap<&str, usize> = commits.iter().enumerate().map(|(index, commit)| (commit.id.as_str(), index)).collect();
  let by_id: Vec<String> = after
    .groups
    .iter()
    .filter(|group| position[group.target.as_str()] >= oldest_changed)
    .flat_map(|group| {
      group
        .folded
        .iter()
        .filter(|fixup| names_by_id(&fixup.subject, &group.target, &group.target_subject))
        .map(|fixup| short_id(&fixup.commit_id).to_string())
    })
    .collect();
  if !by_id.is_empty() {
    return Err(stranded(FixupRefusal::NamedById, by_id));
  }

  // a new text of a target, not only a new prefix, is replaced by the body of its waiting `amend!` at the next fold
  let reworded: HashMap<&str, (&str, &str)> = commits
    .iter()
    .zip(regrouped)
    .filter(|(old, new)| old.message != new.message && !keeps_virtual_commit(&old.message, &new.message))
    .map(|(old, new)| (old.id.as_str(), (old.message.as_str(), new.message.as_str())))
    .collect();
  let regrouped_messages: HashMap<&str, &str> = regrouped.iter().map(|commit| (commit.id.as_str(), commit.message.as_str())).collect();
  let amended: Vec<String> = before
    .groups
    .iter()
    .filter(|group| reworded.contains_key(group.target.as_str()))
    .flat_map(|group| group.folded.iter().filter(|fixup| fixup.kind == FixupKind::Amend))
    .filter(|fixup| {
      let old = commits[position[fixup.commit_id.as_str()]].message.as_str();
      // an amend! reworded in the same plan carries the new message the user meant
      regrouped_messages[fixup.commit_id.as_str()] == old && has_body(old)
    })
    .map(|fixup| short_id(&fixup.commit_id).to_string())
    .collect();
  if !amended.is_empty() {
    return Err(stranded(FixupRefusal::AmendWaits, amended));
  }
  Ok(())
}

const fn stranded(refusal: FixupRefusal, commits: Vec<String>) -> RegroupError {
  RegroupError::FixupsStranded { refusal, commits }
}

/// Fixup id -> its target id, by the groups of a plan
fn fixup_targets(plan: &AutosquashPlan) -> HashMap<&str, &str> {
  plan
    .groups
    .iter()
    .flat_map(|group| group.folded.iter().map(move |fixup| (fixup.commit_id.as_str(), group.target.as_str())))
    .collect()
}

/// Whether a fixup's subject names `target` by an abbreviation of its id rather than by its subject (rule 2 of
/// `plan_autosquash`)
fn names_by_id(fixup_subject: &str, target: &str, target_subject: &str) -> bool {
  let wanted = &fixup_subject[parse_subject(fixup_subject).autosquash.len()..];
  wanted != target_subject && wanted.len() >= 4 && wanted.bytes().all(|byte| byte.is_ascii_hexdigit()) && target.starts_with(&wanted.to_ascii_lowercase())
}

/// Whether a message has more than its subject line: the body an `amend!` puts in place of its target's message
fn has_body(message: &str) -> bool {
  message.split_once('\n').is_some_and(|(_, body)| !body.trim().is_empty())
}
