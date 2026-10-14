//! The changed files of commits, read-only: the files with their statuses and line counts of many commits from one
//! `diff-tree --stdin --raw --numstat -z` process, through the object cache, and the patch of one file on demand.

use crate::conflict_analysis::{FileDiff, FileInfo};
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use object_cache::commit_files::{CommitFile, FileStatus};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{debug, instrument};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetCommitFilesParams {
  pub repository_path: String,
  pub commit_id: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetCommitsFilesParams {
  pub repository_path: String,
  pub commit_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetCommitFileDiffParams {
  pub repository_path: String,
  pub commit_id: String,
  /// The path after the commit
  pub path: String,
  /// The path before a rename or copy, so the patch shows the changes and not a new file
  pub old_path: Option<String>,
}

/// The changed files of a commit against its first parent, with the sums of their line counts
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitFiles {
  pub commit_id: String,
  /// In git's order: by path
  pub files: Vec<CommitFile>,
  pub additions: u32,
  pub deletions: u32,
}

impl CommitFiles {
  pub fn new(commit_id: String, files: Vec<CommitFile>) -> Self {
    let (additions, deletions) = files.iter().fold((0u32, 0u32), |(additions, deletions), file| {
      (additions.saturating_add(file.additions), deletions.saturating_add(file.deletions))
    });
    Self {
      commit_id,
      files,
      additions,
      deletions,
    }
  }
}

/// Diff options that make the result independent of the user's diff config: renames, the root commit as all added, a
/// merge against its first parent, and no external diff or textconv
const DIFF_OPTIONS: [&str; 7] = ["-r", "-M", "--root", "--diff-merges=first-parent", "--no-ext-diff", "--no-textconv", "--no-relative"];

/// A full object id (SHA-1 or SHA-256), so an id never reads as an option or a revision expression
fn is_object_id(id: &str) -> bool {
  matches!(id.len(), 40 | 64) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn check_object_ids<'a>(ids: impl IntoIterator<Item = &'a String>) -> Result<()> {
  match ids.into_iter().find(|id| !is_object_id(id)) {
    Some(id) => bail!("Not a commit id: {id}"),
    None => Ok(()),
  }
}

/// The files of `ids`, by id, in one process. A commit without changes has no entry
#[instrument(skip(git, ids), fields(commits = ids.len()))]
pub fn read_commit_files(git: &GitCommandExecutor, repository_path: &str, ids: &[String]) -> Result<HashMap<String, Vec<CommitFile>>> {
  if ids.is_empty() {
    return Ok(HashMap::new());
  }
  check_object_ids(ids)?;
  let mut args = vec!["diff-tree", "--stdin", "--raw", "--numstat", "-z"];
  args.extend(DIFF_OPTIONS);
  let input = ids.iter().fold(String::new(), |mut input, id| {
    input.push_str(id);
    input.push('\n');
    input
  });
  let output = git.execute_command_with_input_raw(&args, repository_path, &input)?;
  parse_commit_files(&output)
}

/// The files of `ids` (in this order, an entry per id) through the cache: git runs once for the ids the cache misses,
/// and what it lists is written back. A commit git lists nothing for is not cached: it has no changes, or it isn't
/// there yet
pub fn read_commit_files_cached(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, ids: &[String]) -> Result<Vec<CommitFiles>> {
  let mut known = cache.get_commit_files(ids);
  let mut seen = HashSet::new();
  let missing: Vec<String> = ids.iter().filter(|id| !known.contains_key(*id) && seen.insert(id.as_str())).cloned().collect();
  debug!(cached = known.len(), missing = missing.len(), "commit files");
  if !missing.is_empty() {
    let read = read_commit_files(git, repository_path, &missing)?;
    cache.put_commit_files(read.iter().map(|(id, files)| (id.as_str(), files)));
    known.extend(read);
  }
  Ok(ids.iter().map(|id| CommitFiles::new(id.clone(), known.get(id).cloned().unwrap_or_default())).collect())
}

/// Parse NUL-separated fields: a commit id, then its `--raw` entries (`:<modes> <oids> <status>` NUL `<path>`, and a
/// second path for a rename or copy), then its `--numstat` entries (`<added>\t<deleted>\t<path>`, or
/// `<added>\t<deleted>\t` NUL `<old>` NUL `<new>`; `-` counts for a binary file) in the same order. A field that is
/// neither is the id of the next commit
pub fn parse_commit_files(output: &[u8]) -> Result<HashMap<String, Vec<CommitFile>>> {
  let mut result: HashMap<String, Vec<CommitFile>> = HashMap::new();
  let mut fields = output.split(|&b| b == 0).map(|field| field.strip_prefix(b"\n").unwrap_or(field));
  let mut current: Option<(String, Vec<CommitFile>)> = None;
  // the next raw entry a numstat entry applies to
  let mut counted = 0;
  while let Some(field) = fields.next() {
    if field.is_empty() {
      continue;
    }
    if field[0] == b':' {
      let (_, files) = current.as_mut().ok_or_else(|| anyhow!("raw entry before a commit id"))?;
      let letter = field.rsplit(|&b| b == b' ').next().and_then(|status| status.first().copied()).unwrap_or(b'M');
      let status = FileStatus::from_letter(letter);
      let first_path = next_path(&mut fields, "raw entry")?;
      let (path, old_path) = if matches!(status, FileStatus::Renamed | FileStatus::Copied) {
        (next_path(&mut fields, "rename")?, Some(first_path))
      } else {
        (first_path, None)
      };
      files.push(CommitFile {
        path,
        status,
        old_path,
        additions: 0,
        deletions: 0,
        binary: false,
      });
    } else if let Some((additions, deletions, path)) = split_numstat(field) {
      let (_, files) = current.as_mut().ok_or_else(|| anyhow!("numstat entry before a commit id"))?;
      let path = if path.is_empty() {
        next_path(&mut fields, "rename")?;
        next_path(&mut fields, "rename")?
      } else {
        String::from_utf8_lossy(path).into_owned()
      };
      // the entries come in the order of the raw entries; look the path up only if they don't
      let index = if files.get(counted).is_some_and(|file| file.path == path) {
        Some(counted)
      } else {
        files.iter().position(|file| file.path == path)
      };
      let Some(index) = index else {
        debug!(path, "numstat entry without a raw entry");
        continue;
      };
      let file = &mut files[index];
      file.binary = additions.is_none() || deletions.is_none();
      file.additions = additions.unwrap_or(0);
      file.deletions = deletions.unwrap_or(0);
      counted = index + 1;
    } else {
      let id = String::from_utf8_lossy(field).trim().to_string();
      if !is_object_id(&id) {
        bail!("unexpected diff-tree output: {id}");
      }
      if let Some((previous, files)) = current.replace((id, Vec::new())) {
        result.insert(previous, files);
      }
      counted = 0;
    }
  }
  if let Some((id, files)) = current {
    result.insert(id, files);
  }
  Ok(result)
}

fn next_path<'a>(fields: &mut impl Iterator<Item = &'a [u8]>, what: &str) -> Result<String> {
  fields
    .next()
    .map(|path| String::from_utf8_lossy(path).into_owned())
    .ok_or_else(|| anyhow!("{what} without a path"))
}

/// `<added>\t<deleted>\t<path>` with each count a number or `-` (`None`)
fn split_numstat(field: &[u8]) -> Option<(Option<u32>, Option<u32>, &[u8])> {
  let mut parts = field.splitn(3, |&b| b == b'\t');
  let (added, deleted, path) = (parts.next()?, parts.next()?, parts.next()?);
  let count = |part: &[u8]| -> Option<Option<u32>> {
    if part == b"-" {
      Some(None)
    } else {
      std::str::from_utf8(part).ok()?.parse().ok().map(Some)
    }
  };
  Some((count(added)?, count(deleted)?, path))
}

/// The patch of one file of a commit against its first parent, in the shape the diff view takes: the whole
/// `diff-tree -p` output of the file as one hunk text, as for an uncommitted file
#[instrument(skip(git))]
pub fn read_commit_file_diff(git: &GitCommandExecutor, repository_path: &str, commit_id: &str, path: &str, old_path: Option<&str>) -> Result<FileDiff> {
  if !is_object_id(commit_id) {
    bail!("Not a commit id: {commit_id}");
  }
  let mut args = vec!["diff-tree", "-p", "--no-commit-id", "--unified=3", "--src-prefix=a/", "--dst-prefix=b/"];
  args.extend(DIFF_OPTIONS);
  // literal pathspecs: a path such as `:(glob)*` or `*.rs` names only itself
  let pathspecs: Vec<String> = std::iter::once(path).chain(old_path).map(|path| format!(":(literal){path}")).collect();
  args.extend(["--end-of-options", commit_id, "--"]);
  args.extend(pathspecs.iter().map(String::as_str));
  // untrimmed: the last line of a patch may be a context line of a blank line, a single space
  let patch = git.execute_command_raw(&args, repository_path)?;

  let file_lang = std::path::Path::new(path).extension().and_then(|ext| ext.to_str()).unwrap_or("txt").to_string();
  Ok(FileDiff {
    old_file: FileInfo {
      file_name: old_path.unwrap_or(path).to_string(),
      file_lang: file_lang.clone(),
      content: String::new(),
    },
    new_file: FileInfo {
      file_name: path.to_string(),
      file_lang,
      content: String::new(),
    },
    hunks: if patch.trim().is_empty() { Vec::new() } else { vec![patch] },
  })
}
