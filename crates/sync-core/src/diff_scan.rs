//! The changes of many commits: per file the status and the paths, from one `git log --raw -z` process (NUL-separated
//! and never quoted), and the blocks of the diff views (`file_diffs::blocks`), from one `cat-file --batch` for the blobs
//! that no cache has. The blocks give the hunk ranges the dependency edges are computed from, and the counts.

use anyhow::{Result, anyhow};
use file_diffs::blocks::{BlobPair, PairDiff};
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use object_cache::commit_diff::{FileChange, Hunk};
use text_diff::model::DiffBlock;
use tracing::instrument;

/// The marker the `--format` puts before each commit id
const RECORD_START: u8 = 0x1e;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDiff {
  pub id: String,
  pub files: Vec<FileChange>,
}

/// The options that fix where the hunks start and end and which lines they hold, whatever `diff.algorithm`,
/// `diff.interHunkContext` and `diff.indentHeuristic` say, for the changed lines of the dangling-reference check
/// (`read_commit_lines`), which are cached
pub(crate) const HUNK_OPTIONS: [&str; 3] = ["--diff-algorithm=myers", "--inter-hunk-context=0", "--indent-heuristic"];

/// Diff options that, with `HUNK_OPTIONS`, make the output independent of the user's diff config
const DIFF_OPTIONS: [&str; 7] = ["-M", "--no-color", "--no-ext-diff", "--no-textconv", "--no-relative", "--src-prefix=a/", "--dst-prefix=b/"];

/// Read the changes of `ids` (in this order): the files from one `log --raw`, their blocks from the caches of the diff
/// views or one `cat-file --batch`
#[instrument(skip(git, cache, ids), fields(commits = ids.len()))]
pub fn read_commit_diffs(git: &GitCommandExecutor, repository: &str, cache: &ObjectCache, ids: &[String]) -> Result<Vec<CommitDiff>> {
  let mut diffs = read_commit_changes(git, repository, ids)?;
  add_blocks(git, repository, cache, &mut diffs)?;
  Ok(diffs)
}

/// The changed files of `ids` (in this order) with their statuses, paths and blob ids, from one `log --raw`: no hunks,
/// no counts
pub fn read_commit_changes(git: &GitCommandExecutor, repository: &str, ids: &[String]) -> Result<Vec<CommitDiff>> {
  if ids.is_empty() {
    return Ok(Vec::new());
  }
  // full blob ids in the raw part, whatever the hash: `--abbrev=40` would cut a SHA-256 id
  let mut args = vec!["log", "--no-walk=unsorted", "--stdin", "--format=%x1e%H", "-z", "--raw", "--no-abbrev"];
  args.extend(DIFF_OPTIONS);
  let output = git.cmd(&args, repository).stdin(stdin_ids(ids).as_bytes()).bytes()?;
  parse_commit_diffs(&output)
}

/// The hunks and the counts of the text files of `diffs`, from the blocks of the diff views under `Default`; a file
/// without text (binary, a symlink, a submodule, over 8 MiB) is binary. A change of only the line ends is one hunk of
/// the whole file, as git sees it
fn add_blocks(git: &GitCommandExecutor, repository: &str, cache: &ObjectCache, diffs: &mut [CommitDiff]) -> Result<()> {
  // a type change needs no lines: the edges take the whole file
  let compared = |change: &FileChange| change.status != 'T' && (change.old_blob.is_some() || change.new_blob.is_some());
  let pairs: Vec<BlobPair<'_>> = diffs.iter().flat_map(|diff| &diff.files).filter(|change| compared(change)).map(blob_pair).collect();
  let mut pair_diffs = FileDiffs::default().blocks_of(git, repository, cache, &pairs)?.into_iter();
  for change in diffs.iter_mut().flat_map(|diff| &mut diff.files) {
    if !compared(change) {
      // a symlink or a submodule: no text to compare
      change.binary = change.status != 'T';
      continue;
    }
    match pair_diffs.next() {
      Some(PairDiff::Text(diff)) => {
        change.additions = diff.additions;
        change.deletions = diff.deletions;
        change.hunks = if diff.blocks.is_empty() && diff.line_ends.is_some() {
          // only the line ends changed: the views show no block, but git and a merge take every line as changed
          vec![Hunk {
            old_start: 1,
            old_lines: diff.old_line_count,
            new_start: 1,
            new_lines: diff.new_line_count,
          }]
        } else {
          diff.blocks.iter().map(hunk_of).collect()
        };
      }
      Some(PairDiff::NoText) | None => change.binary = true,
    }
  }
  Ok(())
}

/// The blobs of a changed file, as `file_diffs` compares them
pub fn blob_pair(change: &FileChange) -> BlobPair<'_> {
  BlobPair {
    path: &change.path,
    old_path: change.old_path.as_deref(),
    old_id: change.old_blob.as_deref(),
    new_id: change.new_blob.as_deref(),
  }
}

/// A block as git names a `-U0` hunk: 1-based starts, an empty side names the line before it
fn hunk_of(block: &DiffBlock) -> Hunk {
  let start = |start: u32, end: u32| if start == end { start } else { start + 1 };
  Hunk {
    old_start: start(block.old_start, block.old_end),
    old_lines: block.old_end - block.old_start,
    new_start: start(block.new_start, block.new_end),
    new_lines: block.new_end - block.new_start,
  }
}

/// `read_commit_diffs` through the cache: git runs once for the ids the cache misses, and its results are written back
pub fn read_commit_diffs_cached(git: &GitCommandExecutor, repository: &str, cache: &ObjectCache, ids: &[String]) -> Result<Vec<CommitDiff>> {
  let mut cached = cache.get_commit_diffs(ids);
  let missing: Vec<String> = ids.iter().filter(|id| !cached.contains_key(*id)).cloned().collect();
  tracing::debug!(cached = cached.len(), missing = missing.len(), "commit diffs");
  let read = read_commit_diffs(git, repository, cache, &missing)?;
  cache.put_commit_diffs(read.iter().map(|diff| (diff.id.as_str(), &diff.files)));
  for diff in read {
    cached.insert(diff.id, diff.files);
  }
  Ok(
    ids
      .iter()
      .map(|id| CommitDiff {
        id: id.clone(),
        files: cached.remove(id).unwrap_or_default(),
      })
      .collect(),
  )
}

fn stdin_ids(ids: &[String]) -> String {
  let mut input = ids.join("\n");
  input.push('\n');
  input
}

/// The commit records of the output: each starts with `RECORD_START` at the start of the output or of a line, or after
/// the NUL that ends a raw part (a patch line never starts with it, and patch text holds no NUL)
pub(crate) fn split_records(output: &[u8]) -> impl Iterator<Item = &[u8]> {
  let starts: Vec<usize> = (0..output.len())
    .filter(|&i| output[i] == RECORD_START && (i == 0 || matches!(output[i - 1], b'\n' | 0)))
    .collect();
  let ends: Vec<usize> = starts.iter().skip(1).copied().chain(std::iter::once(output.len())).collect();
  starts.into_iter().zip(ends).map(move |(start, end)| &output[start + 1..end])
}

/// Parse the output of `log --format=%x1e%H -z --raw --no-abbrev -p`
pub fn parse_commit_diffs(output: &[u8]) -> Result<Vec<CommitDiff>> {
  split_records(output).map(parse_record).collect()
}

fn parse_record(record: &[u8]) -> Result<CommitDiff> {
  let mut fields = Fields { data: record, pos: 0 };
  let id = fields.until_nul().ok_or_else(|| anyhow!("commit record without an id"))?;
  let id = String::from_utf8_lossy(id).trim().to_string();
  fields.skip_while(b'\n');

  // the raw part: `:<old mode> <new mode> <old oid> <new oid> <status>` NUL `<path>` NUL, and a second path for a
  // rename or copy
  let mut files = Vec::new();
  while fields.peek() == Some(b':') {
    let meta = fields.until_nul().ok_or_else(|| anyhow!("truncated raw entry in {id}"))?;
    let meta = String::from_utf8_lossy(meta.strip_prefix(b":").unwrap_or(meta)).into_owned();
    let parts: Vec<&str> = meta.split(' ').collect();
    let status = parts.last().and_then(|status| status.chars().next()).unwrap_or('M');
    let blob = |mode: usize, oid: usize| match (parts.get(mode), parts.get(oid)) {
      (Some(&mode), Some(&oid)) if is_source_mode(mode) && !oid.bytes().all(|b| b == b'0') => Some(oid.to_string()),
      _ => None,
    };
    let (old_blob, new_blob) = (blob(0, 2), blob(1, 3));
    let first_path = fields.until_nul().ok_or_else(|| anyhow!("raw entry without a path in {id}"))?;
    let first_path = String::from_utf8_lossy(first_path).into_owned();
    let (path, old_path) = if matches!(status, 'R' | 'C') {
      let second_path = fields.until_nul().ok_or_else(|| anyhow!("rename without a target in {id}"))?;
      (String::from_utf8_lossy(second_path).into_owned(), Some(first_path))
    } else {
      (first_path, None)
    };
    files.push(FileChange {
      status,
      path,
      old_path,
      additions: 0,
      deletions: 0,
      binary: false,
      old_blob,
      new_blob,
      hunks: Vec::new(),
    });
  }
  fields.skip_while(0);

  parse_patch(&id, fields.rest(), &mut files);
  Ok(CommitDiff { id, files })
}

/// Whether a file of `mode` (`--raw`, octal) has content to read as source: a regular file, executable or not; not a
/// symlink (120000), a submodule (160000) or no file (000000)
fn is_source_mode(mode: &str) -> bool {
  mode.starts_with("100")
}

/// Fill in the counts, binary flags and hunks from the patch. The file sections come in the order of the raw entries;
/// a type change has two sections (the old file deleted, the new one added)
fn parse_patch(id: &str, patch: &[u8], files: &mut [FileChange]) {
  // the file of the current section and the sections still expected for it
  let mut current: Option<usize> = None;
  let mut remaining_sections = 0;
  let mut in_hunks = false;
  for line in patch.split(|&b| b == b'\n') {
    if line.starts_with(b"diff --git ") {
      in_hunks = false;
      if remaining_sections == 0 {
        let next = current.map_or(0, |index| index + 1);
        if next >= files.len() {
          tracing::warn!(commit = id, "more patch sections than raw entries");
          current = None;
          continue;
        }
        current = Some(next);
        remaining_sections = if files[next].status == 'T' { 2 } else { 1 };
      }
      remaining_sections -= 1;
      continue;
    }
    let Some(file) = current.map(|index| &mut files[index]) else { continue };
    if line.starts_with(b"@@ ") {
      in_hunks = true;
      if let Some(hunk) = parse_hunk_header(line) {
        file.hunks.push(hunk);
      }
    } else if in_hunks {
      match line.first() {
        Some(b'+') => file.additions += 1,
        Some(b'-') => file.deletions += 1,
        _ => {}
      }
    } else if line.starts_with(b"Binary files ") || line.starts_with(b"GIT binary patch") {
      file.binary = true;
    }
  }
}

/// `@@ -a[,b] +c[,d] @@ …`; an omitted count is 1
fn parse_hunk_header(line: &[u8]) -> Option<Hunk> {
  let line = std::str::from_utf8(line).ok()?;
  let mut parts = line.strip_prefix("@@ ")?.split(' ');
  let (old_start, old_lines) = parse_range(parts.next()?.strip_prefix('-')?)?;
  let (new_start, new_lines) = parse_range(parts.next()?.strip_prefix('+')?)?;
  Some(Hunk {
    old_start,
    old_lines,
    new_start,
    new_lines,
  })
}

fn parse_range(range: &str) -> Option<(u32, u32)> {
  match range.split_once(',') {
    Some((start, lines)) => Some((start.parse().ok()?, lines.parse().ok()?)),
    None => Some((range.parse().ok()?, 1)),
  }
}

struct Fields<'a> {
  data: &'a [u8],
  pos: usize,
}

impl<'a> Fields<'a> {
  fn peek(&self) -> Option<u8> {
    self.data.get(self.pos).copied()
  }

  fn skip_while(&mut self, byte: u8) {
    while self.peek() == Some(byte) {
      self.pos += 1;
    }
  }

  /// The bytes up to the next NUL, which is consumed; `None` at the end of the data
  fn until_nul(&mut self) -> Option<&'a [u8]> {
    let rest = self.data.get(self.pos..)?;
    let end = rest.iter().position(|&b| b == 0)?;
    self.pos += end + 1;
    Some(&rest[..end])
  }

  fn rest(&self) -> &'a [u8] {
    &self.data[self.pos.min(self.data.len())..]
  }
}
