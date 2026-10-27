//! The reword of commits ahead of the baseline (`bd reword` and the app's edit of a message), as one regroup
//! (`regroup.rs`). The regroup's guards, its repository lock, its expected HEAD and its validation hold: a new
//! `(prefix)` moves the commit to that branch as `bd group` does, with the fixups of a target whose prefix changed, and
//! the stack problems it would introduce, the fixups it would strand (`fixup_targets.rs`) and the issue groups and the
//! stack parents stay as the regroup keeps them. Message-only: the trees, the index and the working tree stay as they
//! are, and the undo token restores HEAD. Also the writes of every regroup (`reword_read_commits_at`): the messages
//! cleaned up as `git commit` does, written with one `fast-import`.

use crate::autosquash::plan_autosquash;
use crate::baseline::detect_baseline_branch;
use crate::commit_ids::{IdMatch, match_commit_id};
use crate::regroup::{Assignment, Change, FixupRefusal, RegroupError, RegroupOptions, RegroupOutcome, UndoToken, refuse_unresolved, regroup_with};
use crate::snapshot::Snapshot;
use anyhow::{Result, anyhow};
use domain_types::commit::Commit;
use domain_types::rewrite::{CommitReword, RewordResult, RewordText};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_utils::find_commit;
use git_ops::commit_writer::{StoreUpdate, write_store_updates};
use git_ops::revs::{checked_out_branch, short_id};
use git_ops::reword_commits::{RewordCommitParams, reword_commits_batch_at};
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use sync_utils::commit_subject::{parse_subject, stripped_subject};
use tracing::{info, instrument};

/// Rewords commits ahead of the baseline in one history rewrite, computed for the HEAD it rewrites, under the
/// repository lock (`RegroupOptions`). Refuses an id that names no commit ahead of the baseline (`NotAheadOfBaseline`),
/// an ambiguous or repeated one, an empty subject and a subject of several lines (`InvalidPlan`)
#[instrument(skip(git_executor, rewords, options), fields(count = rewords.len(), dry_run = options.dry_run))]
pub fn reword_core(git_executor: &GitCommandExecutor, repository_path: &str, rewords: &[CommitReword], options: &RegroupOptions) -> Result<RegroupOutcome, RegroupError> {
  regroup_with(git_executor, repository_path, options, |snapshot| reword_change(snapshot, rewords))
}

/// The app's reword: the commits of `rewords` (the app sends full ids and whole messages) in the repository
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RewordRequest {
  pub repository_path: String,
  pub rewords: Vec<CommitReword>,
}

/// The answer of the reword `request` that wrote `outcome`: the new id of each commit it named and the undo token
pub fn reword_result(outcome: &RegroupOutcome, request: &RewordRequest) -> RewordResult {
  let new_commit_ids = request
    .rewords
    .iter()
    .filter_map(|reword| {
      let commit = outcome.commits.iter().find(|commit| commit.id.starts_with(reword.commit.as_str()))?;
      Some((reword.commit.clone(), commit.new_id.clone()))
    })
    .collect();
  RewordResult {
    new_commit_ids,
    previous_head: outcome.previous_head.clone(),
    new_head: outcome.new_head.clone(),
    undo_token: UndoToken::of(outcome).map(|token| token.to_string()),
  }
}

/// `reword_core` for the app (the Tauri command `reword` and the E2E server): its result with the undo token, and its
/// refusals told as the app's user can act on them
pub fn reword_request_core(git_executor: &GitCommandExecutor, request: &RewordRequest, options: &RegroupOptions) -> Result<RewordResult> {
  let outcome = reword_core(git_executor, &request.repository_path, &request.rewords, options).map_err(app_error)?;
  Ok(reword_result(&outcome, request))
}

/// A refusal of `reword_core` with what the app offers: a commit gone from the list it showed is a stale view, and a
/// stranded fixup is folded by Squash Fixups
fn app_error(error: RegroupError) -> anyhow::Error {
  match &error {
    RegroupError::NotAheadOfBaseline { commits } => {
      let ids: Vec<&str> = commits.iter().map(|id| short_id(id)).collect();
      // e.g. reworded or dropped in a terminal while the message was edited here
      match ids.as_slice() {
        [id] => anyhow!("Commit {id} is no longer on the current branch, sync and try again"),
        _ => anyhow!("Commits {} are no longer on the current branch, sync and try again", ids.join(", ")),
      }
    }
    RegroupError::FixupsStranded { refusal, .. } => match refusal {
      FixupRefusal::MarkerChanged => error.into(),
      FixupRefusal::Retargeted | FixupRefusal::NamedById => anyhow!("{error} (squash the fixups into their target first)"),
      FixupRefusal::AmendWaits => anyhow!("{error} (squash the amend! into its target first)"),
    },
    _ => error.into(),
  }
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

/// What `reword_read_commits_at` wrote
pub(crate) struct Reworded {
  /// Old -> new full id of every recreated commit, the ones after the oldest reworded one included
  pub recreated: HashMap<String, String>,
  /// The new commits of the stores, in input order
  pub store_commits: Vec<String>,
}

/// Rewords `(commit id, whole new message)` pairs of `commits` (the regroup's snapshot), for messages
/// computed from the history at `expected_head` (`reword_commits_batch_at`), with `stores` written in the same run and
/// moved in the transaction that moves the branch. The caller holds the repository lock
pub(crate) fn reword_read_commits_at<'a>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  commits: &HashMap<String, Commit>,
  rewrites: impl IntoIterator<Item = (&'a str, &'a str)>,
  expected_head: Option<&str>,
  stores: &[StoreUpdate<'_>],
) -> Result<Reworded> {
  let rewrites: Vec<(&str, String)> = rewrites.into_iter().map(|(id, message)| (id, normalize_message(message))).collect();
  if rewrites.iter().any(|(_, message)| message.lines().next().is_none_or(|subject| subject.trim().is_empty())) {
    return Err(anyhow!("The commit message must start with a subject line"));
  }

  let mut params = Vec::new();
  for (id, message) in rewrites {
    let commit = find_commit(commits, id).ok_or_else(|| anyhow!("Commit {} not found", short_id(id)))?;
    if normalize_message(&commit.message) != message {
      params.push(RewordCommitParams {
        commit_id: commit.id.clone(),
        keeps_virtual_commit: keeps_virtual_commit(&commit.message, &message),
        new_message: message,
      });
    }
  }

  let reworded_count = params.len();
  // also maps the commits recreated on top of a reworded one, unchanged messages included;
  // `reword_commits_batch` refuses a commit that is no longer on the current branch
  let (mapping, store_commits) = if params.is_empty() {
    // no message changes once cleaned up: the stores still move, on the same HEAD
    let store_commits = if stores.is_empty() {
      Vec::new()
    } else {
      let branch_ref = format!("refs/heads/{}", checked_out_branch(git_executor, repository_path)?);
      let verify = expected_head.map(|head| (branch_ref.as_str(), head));
      write_store_updates(git_executor, repository_path, stores, verify, "reword")?
    };
    (HashMap::new(), store_commits)
  } else {
    let baseline_branch = detect_baseline_branch(git_executor, repository_path)?;
    reword_commits_batch_at(git_executor, repository_path, &baseline_branch, params, expected_head, stores)?
  };
  info!(reworded_count, "Reworded commits");
  Ok(Reworded {
    recreated: mapping,
    store_commits,
  })
}

/// Whether the virtual branch commit of a commit stays the same when its message changes from `old` to `new`: the
/// subject without its grouping prefix and the body are the same, so a move to another branch keeps it
pub fn keeps_virtual_commit(old: &str, new: &str) -> bool {
  fn virtual_message(message: &str) -> (Cow<'_, str>, &str) {
    let message = message.trim();
    let (subject, body) = message.split_once('\n').unwrap_or((message, ""));
    (stripped_subject(subject), body)
  }
  virtual_message(old) == virtual_message(new)
}

/// LF line endings, no trailing whitespace on a line, no blank lines at the start or the end (as `git commit` cleans up)
pub(crate) fn normalize_message(message: &str) -> String {
  let lines: Vec<&str> = message.lines().map(str::trim_end).collect();
  lines.join("\n").trim_matches('\n').to_string()
}
