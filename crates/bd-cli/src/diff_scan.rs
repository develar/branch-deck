//! The changes of many commits from one `git log -p -U0 --raw -z` process: per file the status and the paths (from the
//! raw part, NUL-separated and never quoted), the added and deleted line counts (what `--numstat` reports), whether
//! the file is binary, and the hunk ranges the dependency edges are computed from.

use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use object_cache::commit_diff::{FileChange, Hunk};
use tracing::instrument;

/// The marker the `--format` puts before each commit id
const RECORD_START: u8 = 0x1e;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDiff {
  pub id: String,
  pub files: Vec<FileChange>,
}

/// Diff options that make the output independent of the user's diff config
const DIFF_OPTIONS: [&str; 7] = ["-M", "--no-color", "--no-ext-diff", "--no-textconv", "--no-relative", "--src-prefix=a/", "--dst-prefix=b/"];

/// Read the changes of `ids` (in this order) with zero context lines, in one process
#[instrument(skip(git, ids), fields(commits = ids.len()))]
pub fn read_commit_diffs(git: &GitCommandExecutor, repository: &str, ids: &[String]) -> Result<Vec<CommitDiff>> {
  if ids.is_empty() {
    return Ok(Vec::new());
  }
  let mut args = vec!["log", "--no-walk=unsorted", "--stdin", "--format=%x1e%H", "-z", "--raw", "-p", "-U0"];
  args.extend(DIFF_OPTIONS);
  let output = git.execute_command_with_input_raw(&args, repository, &stdin_ids(ids))?;
  parse_commit_diffs(&output)
}

/// `read_commit_diffs` through the cache: git runs once for the ids the cache misses, and its results are written back
pub fn read_commit_diffs_cached(git: &GitCommandExecutor, repository: &str, cache: &ObjectCache, ids: &[String]) -> Result<Vec<CommitDiff>> {
  let mut cached = cache.get_commit_diffs(ids);
  let missing: Vec<String> = ids.iter().filter(|id| !cached.contains_key(*id)).cloned().collect();
  tracing::debug!(cached = cached.len(), missing = missing.len(), "commit diffs");
  let read = read_commit_diffs(git, repository, &missing)?;
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

/// The patch of each of `ids` (in this order) with three lines of context, in one process
#[instrument(skip(git, ids), fields(commits = ids.len()))]
pub fn read_commit_patches(git: &GitCommandExecutor, repository: &str, ids: &[String]) -> Result<Vec<(String, String)>> {
  if ids.is_empty() {
    return Ok(Vec::new());
  }
  let mut args = vec!["log", "--no-walk=unsorted", "--stdin", "--format=%x1e%H", "-p", "-U3"];
  args.extend(DIFF_OPTIONS);
  let output = git.execute_command_with_input_raw(&args, repository, &stdin_ids(ids))?;
  let patches = split_records(&output)
    .map(|record| {
      let (id, patch) = record.split_at(record.iter().position(|&b| b == b'\n').unwrap_or(record.len()));
      (
        String::from_utf8_lossy(id).into_owned(),
        String::from_utf8_lossy(patch).trim_start_matches('\n').to_string(),
      )
    })
    .collect();
  Ok(patches)
}

fn stdin_ids(ids: &[String]) -> String {
  let mut input = ids.join("\n");
  input.push('\n');
  input
}

/// The commit records of the output: each starts with `RECORD_START` at the start of the output or of a line, or after
/// the NUL that ends a raw part (a patch line never starts with it, and patch text holds no NUL)
fn split_records(output: &[u8]) -> impl Iterator<Item = &[u8]> {
  let starts: Vec<usize> = (0..output.len())
    .filter(|&i| output[i] == RECORD_START && (i == 0 || matches!(output[i - 1], b'\n' | 0)))
    .collect();
  let ends: Vec<usize> = starts.iter().skip(1).copied().chain(std::iter::once(output.len())).collect();
  starts.into_iter().zip(ends).map(move |(start, end)| &output[start + 1..end])
}

/// Parse the output of `log --format=%x1e%H -z --raw -p`
pub fn parse_commit_diffs(output: &[u8]) -> Result<Vec<CommitDiff>> {
  split_records(output).map(parse_record).collect()
}

fn parse_record(record: &[u8]) -> Result<CommitDiff> {
  let mut fields = Fields { data: record, pos: 0 };
  let id = fields.until_nul().ok_or_else(|| anyhow!("commit record without an id"))?;
  let id = String::from_utf8_lossy(id).trim().to_string();
  fields.skip_while(b'\n');

  // the raw part: `:<modes> <oids> <status>` NUL `<path>` NUL, and a second path for a rename or copy
  let mut files = Vec::new();
  while fields.peek() == Some(b':') {
    let meta = fields.until_nul().ok_or_else(|| anyhow!("truncated raw entry in {id}"))?;
    let status_field = meta.rsplit(|&b| b == b' ').next().unwrap_or_default();
    let status = status_field.first().map_or('M', |&b| char::from(b));
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
      hunks: Vec::new(),
    });
  }
  fields.skip_while(0);

  parse_patch(&id, fields.rest(), &mut files);
  Ok(CommitDiff { id, files })
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
