//! The analysis of a conflict view (`merge_conflict.rs`): the commits of the source that the target lacks and that
//! change a conflicting file, where the two diverged, and the contents of files at revisions, each read in one
//! process, the paths as literal pathspecs.

use crate::commit_files::MAX_BLOB_BYTES;
use crate::object_texts::read_texts;
use crate::revs::{is_object_id, merge_base};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::collections::{HashMap, HashSet};
use tracing::{debug, instrument};

/// Represents a commit that exists in the source branch but is missing from the target branch.
/// These commits might be causing merge conflicts. The view reads their diffs with `get_file_diffs`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct MissingCommit {
  pub hash: String,
  pub subject: String,
  pub message: String,
  pub author_time: u32,
  pub committer_time: u32,
  pub author: String,
  /// The conflicting files the commit changes
  pub files_touched: Vec<String>,
  /// The branch the commit is in, `None` for an unassigned commit; set by the sync, which knows the grouping
  pub branch: Option<String>,
}

/// Analysis results for a merge conflict, including missing commits and divergence information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictAnalysis {
  pub missing_commits: Vec<MissingCommit>,
  pub merge_base_hash: String,
  pub merge_base_subject: String,
  pub merge_base_message: String,
  pub merge_base_time: u32,
  pub merge_base_author: String,
  pub divergence_summary: DivergenceSummary,
}

/// Summary of how two branches have diverged from their common ancestor.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct DivergenceSummary {
  pub commits_ahead_in_source: u32,  // How many commits the source branch is ahead
  pub commits_ahead_in_target: u32,  // How many commits the target branch is ahead
  pub common_ancestor_distance: u32, // How far back is the common ancestor
}

/// `\x01` starts a commit, NUL ends a field; `%B` is the last one, so a message may hold anything but NUL
const MISSING_COMMIT_FORMAT: &str = "--format=%x01%H%x00%at%x00%ct%x00%an%x00%s%x00%B";

/// The commits of `source` that `target` lacks and that change one of `conflicting_files`, newest first, from one
/// `log -z --name-only`: the paths verbatim and the whole message of each. Their diffs are not read: the views read them
/// with `get_file_diffs`
#[instrument(skip(git_executor, conflicting_files))]
pub(crate) fn list_missing_commits(git_executor: &GitCommandExecutor, repo_path: &str, source: &str, target: &str, conflicting_files: &[String]) -> Result<Vec<MissingCommit>> {
  if conflicting_files.is_empty() {
    return Ok(Vec::new());
  }
  let exclude_target = format!("^{target}");
  // the paths are names, never globs (`a*.txt`, `[x].md`) or magic (`:name`)
  let mut args = vec![
    "--literal-pathspecs",
    "log",
    "-z",
    "--name-only",
    "--no-renames",
    "--no-merges",
    MISSING_COMMIT_FORMAT,
    source,
    &exclude_target,
    "--",
  ];
  args.extend(conflicting_files.iter().map(String::as_str));
  let output = git_executor.cmd(&args, repo_path).bytes()?;
  let output = String::from_utf8(output).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
  let conflicting: HashSet<&str> = conflicting_files.iter().map(String::as_str).collect();
  let listed = parse_missing_commits(&output, &conflicting);
  for commit in &listed {
    debug!(commit_hash = %commit.hash, files_touched = ?commit.files_touched, "found missing commit that touches conflicting files");
  }
  Ok(listed)
}

/// The commits of `log -z --name-only` with `MISSING_COMMIT_FORMAT` that change a `conflicting` path, without their
/// diffs. A commit is `\x01<id>`, its fields and its message,
/// each ended by NUL, then a newline before the first path and `<path>\0` per changed path
fn parse_missing_commits(output: &str, conflicting: &HashSet<&str>) -> Vec<MissingCommit> {
  let is_commit_start = |field: &str| field.strip_prefix('\x01').is_some_and(is_object_id);
  let mut commits = Vec::new();
  let mut fields = output.split('\0').peekable();
  while let Some(hash) = fields.next().and_then(|field| field.strip_prefix('\x01')) {
    let [author_time, committer_time, author, subject, message] = std::array::from_fn(|_| fields.next().unwrap_or_default());
    let mut files_touched = Vec::new();
    let mut first = true;
    while let Some(path) = fields.next_if(|field| !is_commit_start(field)) {
      let path = if first { path.strip_prefix('\n').unwrap_or(path) } else { path };
      first = false;
      if conflicting.contains(path) {
        files_touched.push(path.to_string());
      }
    }
    if files_touched.is_empty() {
      continue;
    }
    let commit = MissingCommit {
      hash: hash.to_string(),
      subject: subject.to_string(),
      message: message.trim().to_string(),
      author_time: author_time.parse().unwrap_or(0),
      committer_time: committer_time.parse().unwrap_or(0),
      author: author.to_string(),
      files_touched,
      branch: None,
    };
    commits.push(commit);
  }
  commits
}

/// Where two commits diverged: their merge base, and the commits each has that the other lacks
#[derive(Debug, Clone)]
pub(crate) struct Divergence {
  pub merge_base: String,
  pub summary: DivergenceSummary,
}

/// The merge base and the divergence of `source` and `target` from one `rev-list --left-right --boundary
/// <source>...<target>`: the commits of each side are counted, and the boundary, the commits outside both sides that a
/// commit of a side has as a parent, holds every merge base (the child of a merge base on the way to a side is on that
/// side), so a boundary of one commit is the merge base. Any other boundary (criss-cross merges, a side that merged the
/// other, unrelated histories) leaves the merge base to one `merge-base`; `None` without one
#[instrument(skip(git_executor))]
pub(crate) fn find_divergence(git_executor: &GitCommandExecutor, repo_path: &str, source: &str, target: &str) -> Result<Option<Divergence>> {
  let range = format!("{source}...{target}");
  let output = git_executor.execute_command(&["rev-list", "--left-right", "--boundary", "--end-of-options", &range, "--"], repo_path)?;
  let (mut ahead_in_source, mut ahead_in_target) = (0u32, 0u32);
  let mut boundary = Vec::new();
  for line in output.lines() {
    match line.as_bytes().first() {
      Some(b'<') => ahead_in_source += 1,
      Some(b'>') => ahead_in_target += 1,
      Some(b'-') => boundary.push(&line[1..]),
      _ => return Err(anyhow!("Unexpected rev-list line: {line}")),
    }
  }
  let base = match boundary.as_slice() {
    [base] => (*base).to_string(),
    _ => match merge_base(git_executor, repo_path, source, target)? {
      Some(base) => base,
      None => return Ok(None),
    },
  };
  Ok(Some(Divergence {
    merge_base: base,
    summary: DivergenceSummary {
      commits_ahead_in_source: ahead_in_source,
      commits_ahead_in_target: ahead_in_target,
      common_ancestor_distance: ahead_in_source.min(ahead_in_target),
    },
  }))
}

/// Content of files at several revisions with one `git cat-file --batch` reading `<rev>:<path>` names, keyed by
/// revision and path (`read_texts`). A file that is absent at a revision, is not a blob, is binary or is larger than the
/// diff view shows (`MAX_BLOB_BYTES`, its bytes skipped unread) has empty content.
#[instrument(skip(git_executor, requests), fields(revisions = requests.len()))]
pub(crate) fn get_files_content_at_revisions(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  requests: &[(&str, &[String])],
) -> Result<HashMap<String, HashMap<String, String>>> {
  let mut result: HashMap<String, HashMap<String, String>> = HashMap::with_capacity(requests.len());
  let mut names = Vec::new();
  let mut keys = Vec::new();
  for (rev, file_paths) in requests {
    let contents = result.entry((*rev).to_string()).or_default();
    for file_path in *file_paths {
      // absent files keep this empty content; a file asked for twice at a revision is read once
      if contents.insert(file_path.clone(), String::new()).is_none() {
        names.push(format!("{rev}:{file_path}"));
        keys.push((*rev, file_path.as_str()));
      }
    }
  }
  let texts = read_texts(git_executor, repo_path, &names, MAX_BLOB_BYTES).map_err(|e| anyhow!("Failed to cat-file batch: {e}"))?;
  for ((rev, file_path), text) in keys.into_iter().zip(texts) {
    if let Some(text) = text
      && let Some(contents) = result.get_mut(rev)
    {
      contents.insert(file_path.to_string(), text);
    }
  }
  Ok(result)
}
