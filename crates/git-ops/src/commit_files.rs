//! The changed files of commits, read-only: the files with their statuses and line counts of many commits from one
//! `diff-tree --stdin --raw --numstat -z` process, through the object cache; the patch of a whole commit from one
//! `diff-tree -p`, and the contents of the blobs a diff expands from one `cat-file --batch`. A range of two commits
//! (`base..head`, the review of a pull request) has its files from one `diff-tree` of both, never through the cache:
//! a `commit_files` entry is keyed by one commit.
//!
//! The patch of a commit is read through the `commit_patch` store (`read_patch_cached`), keyed by the commit id and
//! the version of the limits that cut them; a range is never written there. A range has its patch and its files from
//! one `diff-tree --raw --numstat -z -p` (`read_patch_and_files`): one git process, whose start is what costs most in a
//! large repository. `warm_patches` fills it for the commits of a list ahead of their first
//! review: one `diff-tree --stdin -p` for all of them, cut per commit at the id line git prints before each.

use crate::cat_file_batch::{BatchEntry, for_each_object};
use crate::object_texts::{into_text, is_binary};
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use object_cache::commit_files::{CommitFile, FileStatus};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
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
pub struct GetCommitPatchParams {
  pub repository_path: String,
  pub commit_id: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetBlobContentsParams {
  pub repository_path: String,
  /// Full blob ids, at most `MAX_BLOB_IDS`
  pub object_ids: Vec<String>,
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

/// A full object id (SHA-1 or SHA-256), so an id never reads as an option or a revision expression
pub fn is_object_id(id: &str) -> bool {
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
  let input = stdin_lines(ids);
  let output = git.execute_command_with_input_raw(&args, repository_path, &input)?;
  parse_commit_files(&output)
}

/// The ids, a line each, for the stdin of `diff-tree --stdin`
fn stdin_lines(ids: &[String]) -> String {
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
  let output = git.execute_command_raw_accepting(&args, repository_path, &[])?.0;
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

/// The patch of a commit against its first parent (or of a range), one section per file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitPatch {
  /// The commit id, or `<base>..<head>` for a range (`DiffSubject::id`)
  pub commit_id: String,
  /// In git's order (by path), a section per `diff --git` line
  pub files: Vec<FilePatch>,
}

/// The patch of one file of a commit
// the serde names are the app's; the cache encoding (postcard) is positional and doesn't see them
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FilePatch {
  /// The section as git prints it, from its `diff --git` line; only the lines before the first hunk when
  /// `omitted_lines` is set
  pub patch: String,
  /// The hunk lines left out: the file passed `PatchLimits::file_bytes` or `file_lines`, or the commit passed
  /// `total_bytes` before it
  pub omitted_lines: Option<u32>,
}

/// How much of a patch is kept: a file over a limit, and every file after the commit's total, keeps only its header
#[derive(Debug, Clone, Copy)]
pub struct PatchLimits {
  pub file_bytes: usize,
  pub file_lines: u32,
  pub total_bytes: usize,
}

pub const PATCH_LIMITS: PatchLimits = PatchLimits {
  file_bytes: 1 << 20,
  file_lines: 10_000,
  total_bytes: 16 << 20,
};

/// The version of the patches in the `commit_patch` store: bump it when `PATCH_LIMITS`, `PATCH_OPTIONS` or the diff
/// options change, so that a patch cut otherwise is never read
pub const PATCH_CACHE_VERSION: u32 = 1;

/// A patch over this many bytes is not written to the `commit_patch` store: the app's memory cache holds it
pub const MAX_CACHED_PATCH_BYTES: usize = 2 << 20;

/// `warm_patches` skips a commit that changes more lines (added and deleted) or more files: its first review reads git
pub const MAX_WARM_LINES: u32 = 5000;
pub const MAX_WARM_FILES: usize = 300;

/// `warm_patches` writes what it read in one transaction per this many bytes, so its memory stays bounded
const WARM_WRITE_BYTES: usize = 32 << 20;

/// The options of a patch for the diff view, after `-p`
const PATCH_OPTIONS: [&str; 5] = ["--full-index", "--unified=3", "--no-color", "--src-prefix=a/", "--dst-prefix=b/"];

/// Where `read_patch_cached` found a patch
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchSource {
  Disk,
  Git,
}

impl PatchSource {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Disk => "disk",
      Self::Git => "git",
    }
  }
}

/// The bytes of the sections of a patch
pub fn patch_bytes(files: &[FilePatch]) -> usize {
  files.iter().map(|file| file.patch.len()).sum()
}

/// The patch of `commit_id` against its first parent from one `diff-tree -p` process, read as it streams, so a huge
/// commit costs the limits, not its size. `--full-index` gives the blob ids that `read_blob_contents` reads to expand
/// the unchanged lines. The paths are left to the diff view, which reads the `---`/`+++` lines
pub fn read_commit_patch(git: &GitCommandExecutor, repository_path: &str, commit_id: &str, limits: &PatchLimits) -> Result<CommitPatch> {
  read_patch(git, repository_path, &DiffSubject::Commit(commit_id.to_string()), limits)
}

/// `read_commit_patch` of any subject: a range diffs its two commits
#[instrument(skip(git, limits))]
pub fn read_patch(git: &GitCommandExecutor, repository_path: &str, subject: &DiffSubject, limits: &PatchLimits) -> Result<CommitPatch> {
  let mut options = vec!["-p", "--no-commit-id"];
  options.extend(PATCH_OPTIONS);
  let args = subject.diff_tree_args(&options)?;
  let mut splitter = PatchSplitter::new(*limits);
  git.execute_command_streaming(&args, repository_path, |chunk| {
    splitter.push(chunk);
    Ok(())
  })?;
  Ok(CommitPatch {
    commit_id: subject.id(),
    files: splitter.finish(),
  })
}

/// The patch of `subject` as `read_patch` cuts it with `limits`, and its files as `read_diff_files` reads them, from one
/// `diff-tree --raw --numstat -z -p`: git prints the files first, NUL-separated, then an empty field, then the patch
/// from its first `diff --git` line. No path holds a NUL and no entry is empty, so the first two NULs in a row end the
/// files
pub fn read_patch_and_files(git: &GitCommandExecutor, repository_path: &str, subject: &DiffSubject, limits: &PatchLimits) -> Result<(CommitPatch, CommitFiles)> {
  let mut options = vec!["--raw", "--numstat", "-z", "-p", "--no-commit-id"];
  options.extend(PATCH_OPTIONS);
  let args = subject.diff_tree_args(&options)?;
  let mut files_output = Vec::new();
  let mut in_patch = false;
  let mut splitter = PatchSplitter::new(*limits);
  git.execute_command_streaming(&args, repository_path, |chunk| {
    if in_patch {
      splitter.push(chunk);
      return Ok(());
    }
    // a NUL that ended the previous chunk may be the first of the two
    let from = files_output.len().saturating_sub(1);
    files_output.extend_from_slice(chunk);
    if let Some(end) = memchr::memmem::find(&files_output[from..], b"\0\0").map(|index| from + index) {
      let patch = files_output.split_off(end + 2);
      in_patch = true;
      splitter.push(&patch);
    }
    Ok(())
  })?;
  let id = subject.id();
  let mut files = parse_files(&files_output, Some(id.clone()))?;
  let files = CommitFiles::new(id.clone(), files.remove(&id).unwrap_or_default());
  Ok((
    CommitPatch {
      commit_id: id,
      files: splitter.finish(),
    },
    files,
  ))
}

/// `read_patch` with `PATCH_LIMITS` through the `commit_patch` store: a hit runs no git; what git reads is written
/// back unless it is over `MAX_CACHED_PATCH_BYTES`. A range is read, never cached: the store is keyed by commits
pub fn read_patch_cached(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, subject: &DiffSubject) -> Result<(CommitPatch, PatchSource)> {
  if matches!(subject, DiffSubject::Range { .. }) {
    return Ok((read_patch(git, repository_path, subject, &PATCH_LIMITS)?, PatchSource::Git));
  }
  let commit_id = subject.id();
  if let Some(files) = cache.get_commit_patch::<Vec<FilePatch>>(PATCH_CACHE_VERSION, &commit_id) {
    return Ok((CommitPatch { commit_id, files }, PatchSource::Disk));
  }
  let patch = read_patch(git, repository_path, subject, &PATCH_LIMITS)?;
  if patch_bytes(&patch.files) <= MAX_CACHED_PATCH_BYTES {
    cache.put_commit_patches(PATCH_CACHE_VERSION, &[(commit_id.as_str(), &patch.files)]);
  }
  Ok((patch, PatchSource::Git))
}

/// The commits of `files` that `warm_patches` reads: a commit, not a range, that changes something and at most
/// `MAX_WARM_FILES` files and `MAX_WARM_LINES` lines; each once, in the order of `files`
pub fn patch_warm_ids(files: &[CommitFiles]) -> Vec<String> {
  let mut seen = HashSet::new();
  files
    .iter()
    .filter(|files| {
      is_object_id(&files.commit_id)
        && !files.files.is_empty()
        && files.files.len() <= MAX_WARM_FILES
        && files.additions.saturating_add(files.deletions) <= MAX_WARM_LINES
        && seen.insert(files.commit_id.as_str())
    })
    .map(|files| files.commit_id.clone())
    .collect()
}

/// What `warm_patches` did
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PatchWarm {
  /// The ids whose patch the store had
  pub cached: usize,
  /// The patches git read
  pub read: usize,
  /// The patches written: those not over `MAX_CACHED_PATCH_BYTES`
  pub written: usize,
}

/// Fill the `commit_patch` store with the patches of `ids` it misses (`patch_warm_ids`), cut by `PATCH_LIMITS`, from
/// one `read_patches`; written in one transaction per `WARM_WRITE_BYTES`
#[instrument(skip(git, cache, ids), fields(commits = ids.len()))]
pub fn warm_patches(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, ids: &[String]) -> Result<PatchWarm> {
  if !cache.is_enabled() || ids.is_empty() {
    return Ok(PatchWarm::default());
  }
  let missing: Vec<String> = ids
    .iter()
    .zip(cache.has_commit_patches(PATCH_CACHE_VERSION, ids))
    .filter(|(_, cached)| !cached)
    .map(|(id, _)| id.clone())
    .collect();
  let mut warm = PatchWarm {
    cached: ids.len() - missing.len(),
    ..PatchWarm::default()
  };
  if missing.is_empty() {
    return Ok(warm);
  }
  let started = Instant::now();
  let write = |batch: &mut Vec<(String, Vec<FilePatch>)>| {
    let entries: Vec<(&str, &Vec<FilePatch>)> = batch.iter().map(|(id, files)| (id.as_str(), files)).collect();
    cache.put_commit_patches(PATCH_CACHE_VERSION, &entries);
    batch.clear();
  };
  let mut batch = Vec::new();
  let mut batch_bytes = 0;
  let mut written = 0;
  read_patches(git, repository_path, &missing, &PATCH_LIMITS, |id, files| {
    warm.read += 1;
    let bytes = patch_bytes(&files);
    if bytes > MAX_CACHED_PATCH_BYTES {
      return;
    }
    batch.push((id, files));
    batch_bytes += bytes;
    written += 1;
    if batch_bytes >= WARM_WRITE_BYTES {
      write(&mut batch);
      batch_bytes = 0;
    }
  })?;
  write(&mut batch);
  warm.written = written;
  debug!(
    cached = warm.cached,
    read = warm.read,
    written = warm.written,
    elapsed_ms = started.elapsed().as_millis(),
    "warmed patches"
  );
  Ok(warm)
}

/// The patches of the commits `ids` against their first parents, as `read_patch` reads each, from one
/// `diff-tree --stdin -p`: git prints the id of each commit on a line of its own before its patch, and nothing for a
/// commit without changes. `on_patch` gets each patch once it is whole, in the order of `ids`
pub fn read_patches(git: &GitCommandExecutor, repository_path: &str, ids: &[String], limits: &PatchLimits, mut on_patch: impl FnMut(String, Vec<FilePatch>)) -> Result<()> {
  if ids.is_empty() {
    return Ok(());
  }
  check_object_ids(ids)?;
  let mut args = vec!["diff-tree", "--stdin", "-p"];
  args.extend(PATCH_OPTIONS);
  args.extend(DIFF_OPTIONS);
  let input = stdin_lines(ids);
  let requested: HashSet<&[u8]> = ids.iter().map(String::as_bytes).collect();
  let mut partial = Vec::new();
  let mut current: Option<(String, PatchSplitter)> = None;
  let mut push_line = |line: &[u8]| {
    let id = line.strip_suffix(b"\n").unwrap_or(line);
    if requested.contains(id) {
      if let Some((previous, splitter)) = current.take() {
        on_patch(previous, splitter.finish());
      }
      current = Some((String::from_utf8_lossy(id).into_owned(), PatchSplitter::new(*limits)));
    } else if let Some((_, splitter)) = current.as_mut() {
      splitter.push_line(line);
    }
  };
  git.execute_command_streaming_with_input(&args, repository_path, input.as_bytes(), |chunk| {
    for_each_line(&mut partial, chunk, &mut push_line);
    Ok(())
  })?;
  if !partial.is_empty() {
    push_line(&partial);
  }
  if let Some((id, splitter)) = current.take() {
    on_patch(id, splitter.finish());
  }
  Ok(())
}

/// Pass each whole line of `chunk` (with its newline) to `on_line`, `partial` holding the bytes of a line that the
/// previous chunk cut
fn for_each_line(partial: &mut Vec<u8>, mut chunk: &[u8], on_line: &mut impl FnMut(&[u8])) {
  while let Some(newline) = memchr::memchr(b'\n', chunk) {
    let (line, rest) = chunk.split_at(newline + 1);
    if partial.is_empty() {
      on_line(line);
    } else {
      partial.extend_from_slice(line);
      on_line(partial);
      partial.clear();
    }
    chunk = rest;
  }
  partial.extend_from_slice(chunk);
}

/// Cuts a patch into its file sections at `diff --git` lines, as the bytes arrive. Inside a hunk every line starts
/// with ` `, `+`, `-` or `\`, so a cut never falls into the content of a file
pub struct PatchSplitter {
  limits: PatchLimits,
  files: Vec<FilePatch>,
  section: Option<Section>,
  /// The bytes after the last newline
  partial_line: Vec<u8>,
  /// The bytes kept so far, of all sections
  total: usize,
}

struct Section {
  bytes: Vec<u8>,
  /// The length of the header (the lines before the first hunk), once a hunk started
  header_len: Option<usize>,
  hunk_lines: u32,
  omitted: bool,
}

impl PatchSplitter {
  pub const fn new(limits: PatchLimits) -> Self {
    Self {
      limits,
      files: Vec::new(),
      section: None,
      partial_line: Vec::new(),
      total: 0,
    }
  }

  pub fn push(&mut self, chunk: &[u8]) {
    let mut partial = std::mem::take(&mut self.partial_line);
    for_each_line(&mut partial, chunk, &mut |line| self.push_line(line));
    self.partial_line = partial;
  }

  pub fn finish(mut self) -> Vec<FilePatch> {
    if !self.partial_line.is_empty() {
      let line = std::mem::take(&mut self.partial_line);
      self.push_line(&line);
    }
    self.close_section();
    self.files
  }

  fn push_line(&mut self, line: &[u8]) {
    if line.starts_with(b"diff --git ") {
      self.close_section();
      self.total += line.len();
      self.section = Some(Section {
        bytes: line.to_vec(),
        header_len: None,
        hunk_lines: 0,
        omitted: false,
      });
      return;
    }
    let limits = self.limits;
    let Some(section) = self.section.as_mut() else {
      // nothing before the first section: --no-commit-id prints no commit line
      return;
    };
    if section.header_len.is_none() {
      if !line.starts_with(b"@@") {
        // the header is kept whatever the limits: it names the file and says what changed
        section.bytes.extend_from_slice(line);
        self.total += line.len();
        return;
      }
      section.header_len = Some(section.bytes.len());
    }
    if !line.starts_with(b"@@") && !line.starts_with(b"\\") {
      section.hunk_lines += 1;
    }
    if section.omitted {
      return;
    }
    let over = section.bytes.len() + line.len() > limits.file_bytes || section.hunk_lines > limits.file_lines || self.total + line.len() > limits.total_bytes;
    if over {
      // all or nothing: a file shows its hunks whole, or its header alone
      let header_len = section.header_len.unwrap_or(section.bytes.len());
      self.total -= section.bytes.len() - header_len;
      section.bytes.truncate(header_len);
      section.omitted = true;
    } else {
      section.bytes.extend_from_slice(line);
      self.total += line.len();
    }
  }

  fn close_section(&mut self) {
    if let Some(section) = self.section.take() {
      self.files.push(FilePatch {
        patch: String::from_utf8_lossy(&section.bytes).into_owned(),
        omitted_lines: section.omitted.then_some(section.hunk_lines),
      });
    }
  }
}

/// The most blobs one `read_blob_contents` reads: the two sides of a few files
pub const MAX_BLOB_IDS: usize = 16;

/// The largest blob whose text is sent to the diff view
pub const MAX_BLOB_BYTES: usize = 8 << 20;

/// The contents of a blob for the diff view
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum BlobContents {
  /// The bytes as UTF-8, an invalid sequence replaced
  Text { object_id: String, content: String },
  /// A NUL in the first 8000 bytes, as git decides
  Binary { object_id: String },
  /// Over `MAX_BLOB_BYTES`
  TooLarge { object_id: String, size: u32 },
  /// Not a blob of the repository
  Missing { object_id: String },
}

/// The contents of blobs by full id, in this order, from one `cat-file --batch` process; the bytes of a blob over
/// `MAX_BLOB_BYTES` are skipped unread. Ids, not `<rev>:<path>` names: no path is quoted, and the root commit has no
/// parent to name
#[instrument(skip(git, object_ids), fields(blobs = object_ids.len()))]
pub fn read_blob_contents(git: &GitCommandExecutor, repository_path: &str, object_ids: &[String]) -> Result<Vec<BlobContents>> {
  if object_ids.len() > MAX_BLOB_IDS {
    bail!("Too many blobs: {} (at most {MAX_BLOB_IDS})", object_ids.len());
  }
  if let Some(id) = object_ids.iter().find(|id| !is_object_id(id)) {
    bail!("Not an object id: {id}");
  }
  let mut contents = Vec::with_capacity(object_ids.len());
  for_each_object(git, repository_path, object_ids, MAX_BLOB_BYTES, |index, entry| {
    let object_id = object_ids[index].clone();
    contents.push(match entry {
      BatchEntry::Object { kind: "blob", content, .. } if is_binary(&content) => BlobContents::Binary { object_id },
      BatchEntry::Object { kind: "blob", content, .. } => BlobContents::Text {
        object_id,
        content: into_text(content),
      },
      BatchEntry::TooLarge { kind: "blob", size, .. } => BlobContents::TooLarge {
        object_id,
        size: u32::try_from(size).unwrap_or(u32::MAX),
      },
      _ => BlobContents::Missing { object_id },
    });
    Ok(())
  })?;
  Ok(contents)
}
