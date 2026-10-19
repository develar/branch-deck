//! The changed files of commits, read-only: the files with their statuses and line counts of many commits from one
//! `diff-tree --stdin --raw --numstat -z` process, through the object cache. A range of two commits (`base..head`, the
//! review of a pull request) has its files from one `diff-tree` of both, never through the cache: a `commit_files`
//! entry is keyed by one commit. The diffs of the files come from `file_diffs` (`get_file_diffs`).

use crate::revs::is_object_id;
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

/// The changed files of a commit against its first parent (or of a range), with the sums of their line counts
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitFiles {
  /// The commit id, or `<base>..<head>` for a range (`DiffSubject::id`)
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

/// Diff options of a range only: a rename limit, since a range may change thousands of files, and files over
/// `MAX_BLOB_BYTES` diffed as binary (`core.bigFileThreshold`), since the diff view would not show them
const RANGE_CONFIG: [&str; 2] = ["-c", "core.bigFileThreshold=8m"];
const RANGE_DIFF_OPTIONS: [&str; 1] = ["-l1000"];

/// What a diff compares
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffSubject {
  /// A commit with its first parent
  Commit(String),
  /// Two commits, as `git diff base head`
  Range { base: String, head: String },
}

impl DiffSubject {
  /// The id of the result: the commit id, or `<base>..<head>`
  pub fn id(&self) -> String {
    match self {
      Self::Commit(id) => id.clone(),
      Self::Range { base, head } => format!("{base}..{head}"),
    }
  }

  /// The `diff-tree` arguments: `config` (before the subcommand), the subcommand with `options`, then the diff options
  /// and the ids; fails unless every id is a full object id
  fn diff_tree_args<'a>(&'a self, options: &[&'a str]) -> Result<Vec<&'a str>> {
    let ids: Vec<&str> = match self {
      Self::Commit(id) => vec![id],
      Self::Range { base, head } => vec![base, head],
    };
    if let Some(id) = ids.iter().find(|id| !is_object_id(id)) {
      bail!("Not a commit id: {id}");
    }
    let mut args = Vec::new();
    if matches!(self, Self::Range { .. }) {
      args.extend(RANGE_CONFIG);
    }
    args.push("diff-tree");
    args.extend_from_slice(options);
    args.extend(DIFF_OPTIONS);
    if matches!(self, Self::Range { .. }) {
      args.extend(RANGE_DIFF_OPTIONS);
    }
    args.push("--end-of-options");
    args.extend(ids);
    Ok(args)
  }
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
  let input = stdin_lines(ids);
  let output = git.cmd(&args, repository_path).stdin(input.as_bytes()).bytes()?;
  parse_commit_files(&output)
}

/// The files of the diff from `base` to each of `tips` (full commit ids, as `git diff <base> <tip>`), by tip, in one
/// `diff-tree --stdin --raw --numstat -z` process: a `<tip> <base>` line compares the two. Not cached: a pair is no
/// commit. A pair without changes has no entry
#[instrument(skip(git, tips), fields(tips = tips.len()))]
pub fn read_files_from_base(git: &GitCommandExecutor, repository_path: &str, base: &str, tips: &[&str]) -> Result<HashMap<String, Vec<CommitFile>>> {
  if tips.is_empty() {
    return Ok(HashMap::new());
  }
  if let Some(id) = std::iter::once(&base).chain(tips).find(|id| !is_object_id(id)) {
    bail!("Not a commit id: {id}");
  }
  let mut args = vec!["diff-tree", "--stdin", "--raw", "--numstat", "-z"];
  args.extend(DIFF_OPTIONS);
  let input = tips.iter().fold(String::new(), |mut input, tip| {
    input.push_str(tip);
    input.push(' ');
    input.push_str(base);
    input.push('\n');
    input
  });
  let output = git.cmd(&args, repository_path).stdin(input.as_bytes()).bytes()?;
  parse_commit_files(&output)
}

/// The ids, a line each, for the stdin of `diff-tree --stdin`
pub(crate) fn stdin_lines(ids: &[String]) -> String {
  ids.iter().fold(String::new(), |mut input, id| {
    input.push_str(id);
    input.push('\n');
    input
  })
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

/// The files of one subject, not cached, from one `diff-tree --raw --numstat -z` without `--stdin`: a range never lands in
/// the cache entry of its head
#[instrument(skip(git))]
pub fn read_diff_files(git: &GitCommandExecutor, repository_path: &str, subject: &DiffSubject) -> Result<CommitFiles> {
  let args = subject.diff_tree_args(&["--raw", "--numstat", "-z", "--no-commit-id"])?;
  let output = git.cmd(&args, repository_path).bytes()?;
  let id = subject.id();
  let mut files = parse_files(&output, Some(id.clone()))?;
  Ok(CommitFiles::new(id.clone(), files.remove(&id).unwrap_or_default()))
}

/// Parse NUL-separated fields: a commit id, then its `--raw` entries (`:<modes> <oids> <status>` NUL `<path>`, and a
/// second path for a rename or copy), then its `--numstat` entries (`<added>\t<deleted>\t<path>`, or
/// `<added>\t<deleted>\t` NUL `<old>` NUL `<new>`; `-` counts for a binary file) in the same order. A field that is
/// neither is the id of the next commit
pub fn parse_commit_files(output: &[u8]) -> Result<HashMap<String, Vec<CommitFile>>> {
  parse_files(output, None)
}

/// The ids of a raw entry (`:<old mode> <new mode> <old id> <new id> <status>`), `None` for the zero id of a missing
/// side, and whether a side is a submodule, whose id is a commit
fn raw_ids(entry: &[u8]) -> (Option<String>, Option<String>, bool) {
  let fields: Vec<&[u8]> = entry.strip_prefix(b":").unwrap_or(entry).split(|&b| b == b' ').collect();
  let side = |id: usize| fields.get(id).filter(|id| id.iter().any(|&b| b != b'0')).map(|id| String::from_utf8_lossy(id).into_owned());
  let submodule = fields.iter().take(2).any(|mode| *mode == b"160000");
  (side(2), side(3), submodule)
}

/// `parse_commit_files` of output whose entries before the first commit id are of `first`: the output of one subject
/// with `--no-commit-id` has no id at all
fn parse_files(output: &[u8], first: Option<String>) -> Result<HashMap<String, Vec<CommitFile>>> {
  let mut result: HashMap<String, Vec<CommitFile>> = HashMap::new();
  let mut fields = output.split(|&b| b == 0).map(|field| field.strip_prefix(b"\n").unwrap_or(field));
  let mut current: Option<(String, Vec<CommitFile>)> = first.map(|id| (id, Vec::new()));
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
      let (old_id, new_id, submodule) = raw_ids(field);
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
        old_id,
        new_id,
        submodule,
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

/// The largest blob whose text is sent to the diff view
pub const MAX_BLOB_BYTES: usize = 8 << 20;
