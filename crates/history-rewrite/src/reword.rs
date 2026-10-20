//! `bd reword`: new messages of commits ahead of the baseline, as one regroup (`regroup.rs`). The regroup's guards, its
//! repository lock, its expected HEAD and its validation hold: a new `(prefix)` moves the commit to that branch as
//! `bd group` does, with the fixups of a target whose prefix changed, and the stack problems it would introduce, the
//! fixups it would strand (`fixup_targets.rs`) and the issue groups and the stack parents stay as the regroup keeps them.
//! Message-only: the trees, the index and the working tree stay as they are, and the undo token restores HEAD.

use crate::autosquash::plan_autosquash;
use crate::commit_grouper::parse_subject;
use crate::commit_ids::{IdMatch, match_commit_id};
use crate::regroup::{Assignment, Change, RegroupError, RegroupOptions, RegroupOutcome, refuse_unresolved, regroup_with};
use crate::reword_commit::normalize_message;
use crate::snapshot::Snapshot;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::short_id;
use std::collections::{HashMap, HashSet};
use tracing::instrument;

/// The new text of a commit
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewordText {
  /// The whole message, the `(prefix)` of the subject included
  Message(String),
  /// The subject line, the `(prefix)` included; the body stays
  Subject(String),
}

/// A commit ahead of the baseline (abbreviated ids are fine) and its new text
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitReword {
  pub commit: String,
  pub text: RewordText,
}

/// Rewords commits ahead of the baseline in one history rewrite, computed for the HEAD it rewrites, under the
/// repository lock (`RegroupOptions`). Refuses an id that names no commit ahead of the baseline (`NotAheadOfBaseline`),
/// an ambiguous or repeated one, an empty subject and a subject of several lines (`InvalidPlan`)
#[instrument(skip(git_executor, rewords, options), fields(count = rewords.len(), dry_run = options.dry_run))]
pub fn reword_core(git_executor: &GitCommandExecutor, repository_path: &str, rewords: &[CommitReword], options: &RegroupOptions) -> Result<RegroupOutcome, RegroupError> {
  regroup_with(git_executor, repository_path, options, |snapshot| reword_change(snapshot, rewords))
}

/// The messages of `rewords` by full id, and the fixups that follow a target whose prefix changed
fn reword_change(snapshot: &Snapshot, rewords: &[CommitReword]) -> Result<Change, RegroupError> {
  if rewords.is_empty() {
    return Err(RegroupError::invalid("No commit to reword", Vec::new()));
  }
  let mut messages: HashMap<String, String> = HashMap::with_capacity(rewords.len());
  let (mut unknown, mut ambiguous, mut repeated, mut empty) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
  for reword in rewords {
    let full = match match_commit_id(&reword.commit, snapshot.commits.iter().map(|commit| commit.id.as_str())) {
      IdMatch::One(full) => full,
      IdMatch::None => {
        unknown.push(reword.commit.clone());
        continue;
      }
      IdMatch::Ambiguous => {
        ambiguous.push(reword.commit.clone());
        continue;
      }
    };
    let message = match &reword.text {
      RewordText::Message(message) => message.clone(),
      RewordText::Subject(subject) if subject.contains('\n') => {
        return Err(RegroupError::invalid(
          format!("The new subject of {} has more than one line; give the whole message instead", short_id(full)),
          vec![short_id(full).to_string()],
        ));
      }
      RewordText::Subject(subject) => {
        let old = &snapshot.commits.iter().find(|commit| commit.id == full).expect("a matched commit").message;
        match old.split_once('\n') {
          Some((_, body)) => format!("{subject}\n{body}"),
          None => subject.clone(),
        }
      }
    };
    if normalize_message(&message).lines().next().is_none_or(|subject| subject.trim().is_empty()) {
      empty.push(short_id(full).to_string());
    }
    if messages.insert(full.to_string(), message).is_some() {
      repeated.push(short_id(full).to_string());
    }
  }
  refuse_unresolved(ambiguous, unknown)?;
  if !repeated.is_empty() {
    return Err(RegroupError::invalid(format!("Commits reworded more than once: {}", repeated.join(", ")), repeated));
  }
  if !empty.is_empty() {
    return Err(RegroupError::invalid(
      format!("The commit message must start with a subject line: {}", empty.join(", ")),
      empty,
    ));
  }
  Ok(Change {
    assignment: following_fixups(snapshot, &messages),
    messages,
    ..Change::default()
  })
}

/// The fixups of a reworded target whose `(prefix)` changes go with it, as `bd group` moves them (unless the reword
/// gives them their own message): the new prefix of the target, or none
fn following_fixups(snapshot: &Snapshot, messages: &HashMap<String, String>) -> Assignment {
  let plan = plan_autosquash(snapshot.head.clone(), &snapshot.commits, &snapshot.grouping);
  let prefix_of = |message: &str| parse_subject(message.lines().next().unwrap_or_default()).prefix.map(str::to_string);
  let old_messages: HashMap<&str, &str> = snapshot.commits.iter().map(|commit| (commit.id.as_str(), commit.message.as_str())).collect();
  let reworded: HashSet<&str> = messages.keys().map(String::as_str).collect();
  let mut assignment = Assignment::new();
  for group in &plan.groups {
    let Some(message) = messages.get(&group.target) else { continue };
    let new_prefix = prefix_of(&normalize_message(message));
    if new_prefix == prefix_of(old_messages[group.target.as_str()]) {
      continue;
    }
    for fixup in group.folded.iter().filter(|fixup| !reworded.contains(fixup.commit_id.as_str())) {
      assignment.push((fixup.commit_id.clone(), new_prefix.clone()));
    }
  }
  assignment
}
