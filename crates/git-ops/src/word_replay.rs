//! The local commits of an update replayed onto the upstream, a commit whose conflict of git resolves by its words
//! (`word_resolve`) committed with the result (docs/design/git-operations.md, "Resolve by words"): the update check and
//! the rounds of `bd update` take the same rule as the sync.
//!
//! - **Commit by commit only where needed.** `git replay` replays each run of commits that applies; a binary search
//!   finds the commit that conflicts, which the words resolve or the replay stops at.
//! - **The resolved commit** keeps the author and the message of the local commit; the committer is the one of the
//!   replay. It is the tree of the merge with the resolved files (`write_edited_commit`). As the commits of `git replay`,
//!   it has no ref once the replay ends: the branch move of the update makes it reachable.

use crate::commit_writer::{EditedCommit, TreeEdit, WrittenEdit, discard_temp_refs, read_raw_commits, write_edited_commit};
use crate::merge_tree_output::{MergeTreeOutput, parse_merge_tree_output};
use crate::replay::{first_conflict, replay_range_with_env};
use crate::word_resolve::{RESOLVE_VERSION, WordResolution, WordResolveOutcome, resolve_by_words};
use anyhow::{Result, anyhow, bail};
use domain_types::conflict::ResolvedFileSummary;
use domain_types::update_project::UpdateResolvedCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::{debug, instrument};

fn resolved_commit_of(commit: &str, resolved: &WordsResolved) -> UpdateResolvedCommit {
  let WordsResolved { written, resolution, subject } = resolved;
  UpdateResolvedCommit {
    commit: commit.to_string(),
    subject: subject.clone(),
    new_id: written.commit.clone(),
    conflicts: resolution.conflicts(),
    files: resolution
      .files
      .iter()
      .map(|file| ResolvedFileSummary {
        path: file.path.clone(),
        conflicts: file.conflicts,
      })
      .collect(),
  }
}

/// The note on a resolved commit (`update_resolve::RESOLVE_NOTES_REF`): what it resolves and how, one line per file
pub fn resolve_note(resolved: &UpdateResolvedCommit) -> String {
  let mut note = format!(
    "branch-deck resolve\ncommit: {}\nresolved-by-words: {RESOLVE_VERSION} {}\n",
    resolved.commit, resolved.conflicts
  );
  for file in &resolved.files {
    note.push_str("words\t");
    note.push_str(&file.path);
    note.push('\n');
  }
  note
}

/// A commit that the words resolved, written onto a temporary ref
#[derive(Debug)]
pub(crate) struct WordsResolved {
  pub written: WrittenEdit,
  pub resolution: WordResolution,
  /// The subject of the local commit
  pub subject: String,
}

/// The cherry-pick of `commit` (onto its parent) onto `onto` resolved by words: one `merge-tree` with the messages, the
/// resolve, and the commit with the author and the message of `commit` and `committer` (`Name <email> <seconds> <tz>`).
/// `None` when git merges it cleanly or a file does not resolve
pub(crate) fn resolve_commit_onto(git_executor: &GitCommandExecutor, repo_path: &str, commit: &str, onto: &str, committer: &str) -> Result<Option<WordsResolved>> {
  let parent = format!("{commit}^");
  let output = git_executor
    .cmd(&["merge-tree", "--write-tree", "-z", "--merge-base", &parent, onto, commit], repo_path)
    .accept(&[1])
    .output()?;
  let merge = parse_merge_tree_output(&output.stdout)?;
  if output.code == 0 || merge.entries.is_empty() {
    return Ok(None);
  }
  resolve_merge_onto(git_executor, repo_path, commit, onto, committer, &merge)
}

/// `resolve_commit_onto` of a merge the caller ran already (`merge-tree --write-tree -z` with its messages)
pub(crate) fn resolve_merge_onto(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  commit: &str,
  onto: &str,
  committer: &str,
  merge: &MergeTreeOutput,
) -> Result<Option<WordsResolved>> {
  let WordResolveOutcome::Resolved(resolution) = resolve_by_words(git_executor, repo_path, merge)? else {
    return Ok(None);
  };
  let raw = read_raw_commits(git_executor, repo_path, &[commit])?
    .remove(commit)
    .ok_or_else(|| anyhow!("no commit {commit}"))?;
  let edits: Vec<TreeEdit<'_>> = resolution
    .files
    .iter()
    .map(|file| TreeEdit::Content {
      path: &file.path,
      mode: &file.mode,
      content: &file.content,
    })
    .collect();
  let written = write_edited_commit(
    git_executor,
    repo_path,
    &EditedCommit {
      tree: &merge.tree,
      edits: &edits,
      parent: onto,
      author: &raw.author,
      committer,
      encoding: raw.encoding.as_deref(),
      message: &raw.message,
    },
  )?;
  if written.tree != resolution.tree {
    let temp_ref = written.temp_ref.clone();
    discard_temp_refs(git_executor, repo_path, &temp_ref, |transaction| written.delete_temp_ref(transaction));
    bail!("fast-import wrote the tree {} for the resolve by words of {commit}, not {}", written.tree, resolution.tree);
  }
  debug!(commit, conflicts = resolution.conflicts(), "a commit of the update resolves by words");
  let subject = String::from_utf8_lossy(raw.message.split(|&byte| byte == b'\n').next().unwrap_or_default()).into_owned();
  Ok(Some(WordsResolved { written, resolution, subject }))
}

/// The local commits replayed onto the upstream with the commits that the words resolved
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WordsReplay {
  pub tip: String,
  /// Oldest first
  pub resolved: Vec<UpdateResolvedCommit>,
}

/// Replay `local` (linear history, oldest first) onto `onto` with `env` for `git replay`; a commit that conflicts is
/// resolved by words with `committer`. The replay of all of them conflicted already: the search for the first conflict
/// starts at once. `None` at the first commit that does not resolve. A resolved commit that changes nothing is dropped,
/// as `git replay` drops a commit that becomes empty. No temporary ref is left
#[instrument(skip(git_executor, local, env, committer), fields(commits = local.len()))]
pub(crate) fn replay_resolving_words(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  onto: &str,
  local: &[String],
  env: &[(&str, &str)],
  committer: &str,
) -> Result<Option<WordsReplay>> {
  let mut written = Vec::new();
  let replay = replay_words(git_executor, repo_path, onto, local, env, committer, &mut written);
  if let Some(first) = written.first() {
    discard_temp_refs(git_executor, repo_path, &first.temp_ref, |transaction| {
      for edit in &written {
        edit.delete_temp_ref(transaction);
      }
    });
  }
  replay
}

fn replay_words(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  onto: &str,
  local: &[String],
  env: &[(&str, &str)],
  committer: &str,
  written: &mut Vec<WrittenEdit>,
) -> Result<Option<WordsReplay>> {
  let mut replay = WordsReplay {
    tip: onto.to_string(),
    resolved: Vec::new(),
  };
  let mut next = 0;
  while next < local.len() {
    let (from, last) = (next, local.len() - 1);
    let replay_to = |tip: &str, end: usize| -> Result<Option<String>> {
      // linear history: `<end> ^<from>^` is exactly the commits from `from` to `end`
      let revs = [local[end].clone(), format!("^{}^", local[from])];
      let revs: Vec<&str> = revs.iter().map(String::as_str).collect();
      Ok(replay_range_with_env(git_executor, repo_path, tip, &revs, env)?)
    };
    if next > 0
      && let Some(tip) = replay_to(&replay.tip, last)?
    {
      replay.tip = tip;
      break;
    }
    let start = replay.tip.clone();
    let (index, clean) = first_conflict(from, last, start.clone(), |end| replay_to(&start, end))?;
    let Some(resolved) = resolve_commit_onto(git_executor, repo_path, &local[index], &clean, committer)? else {
      debug!(commit = %local[index], "a commit of the update does not resolve by words");
      return Ok(None);
    };
    if resolved.written.tree == resolved.written.parent_tree {
      replay.tip = clean;
    } else {
      replay.resolved.push(resolved_commit_of(&local[index], &resolved));
      replay.tip.clone_from(&resolved.written.commit);
    }
    written.push(resolved.written);
    next = index + 1;
  }
  Ok(Some(replay))
}

#[cfg(test)]
#[path = "word_replay_test.rs"]
mod word_replay_test;
