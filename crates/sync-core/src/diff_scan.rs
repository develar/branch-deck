//! The changes of many commits from one `git log -p -U0 --raw -z` process: per file the status and the paths (from the
//! raw part, NUL-separated and never quoted), the added and deleted line counts (what `--numstat` reports), whether
//! the file is binary, and the hunk ranges the dependency edges are computed from. And the patches of commits with
//! three lines of context, whose hunks `bd commits --patch` prints and `bd move-hunk --hunk` numbers (`file_patch`).

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

/// The options that fix where the hunks start and end and which lines they hold, whatever `diff.algorithm`,
/// `diff.interHunkContext` and `diff.indentHeuristic` say. They matter three times: the `-U0` hunks give the
/// dependency edges, hunk N of `bd commits --patch` is hunk N of `bd move-hunk --hunk N`, and the changed lines of the
/// dangling-reference check (`read_commit_lines`) are cached
pub(crate) const HUNK_OPTIONS: [&str; 3] = ["--diff-algorithm=myers", "--inter-hunk-context=0", "--indent-heuristic"];

/// Diff options that, with `HUNK_OPTIONS`, make the output independent of the user's diff config
const DIFF_OPTIONS: [&str; 7] = ["-M", "--no-color", "--no-ext-diff", "--no-textconv", "--no-relative", "--src-prefix=a/", "--dst-prefix=b/"];

/// Read the changes of `ids` (in this order) with zero context lines, in one process
#[instrument(skip(git, ids), fields(commits = ids.len()))]
pub fn read_commit_diffs(git: &GitCommandExecutor, repository: &str, ids: &[String]) -> Result<Vec<CommitDiff>> {
  if ids.is_empty() {
    return Ok(Vec::new());
  }
  // full blob ids in the raw part, whatever the hash: `--abbrev=40` would cut a SHA-256 id
  let mut args = vec!["log", "--no-walk=unsorted", "--stdin", "--format=%x1e%H", "-z", "--raw", "--no-abbrev", "-p", "-U0"];
  args.extend(HUNK_OPTIONS);
  args.extend(DIFF_OPTIONS);
  let output = git.cmd(&args, repository).stdin(stdin_ids(ids).as_bytes()).bytes()?;
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

/// A patch with three lines of context, whatever `diff.context` says; `HUNK_OPTIONS` and `DIFF_OPTIONS` fix the rest
const PATCH_OPTIONS: [&str; 2] = ["-p", "-U3"];

/// The patch of each of `ids` (in this order) with three lines of context, in one process
#[instrument(skip(git, ids), fields(commits = ids.len()))]
pub fn read_commit_patches(git: &GitCommandExecutor, repository: &str, ids: &[String]) -> Result<Vec<(String, String)>> {
  if ids.is_empty() {
    return Ok(Vec::new());
  }
  let mut args = vec!["log", "--no-walk=unsorted", "--stdin", "--format=%x1e%H"];
  args.extend(PATCH_OPTIONS);
  args.extend(HUNK_OPTIONS);
  args.extend(DIFF_OPTIONS);
  let output = git.cmd(&args, repository).stdin(stdin_ids(ids).as_bytes()).bytes()?;
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

/// The section of one file in the patch of a commit (`read_commit_patches`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePatch {
  /// The path before the commit; `None` for an added file
  pub old_path: Option<String>,
  /// The path after the commit; `None` for a deleted file
  pub new_path: Option<String>,
  /// The modes before and after, e.g. `100644`; `None` where the file doesn't exist or the patch doesn't say
  pub old_mode: Option<String>,
  pub new_mode: Option<String>,
  /// A rename or a copy (`-M`): `old_path` and `new_path` differ
  pub renamed: bool,
  pub copied: bool,
  /// `Binary files ... differ`: a change without hunks
  pub binary: bool,
  pub hunks: Vec<PatchHunk>,
}

/// One `@@` hunk of a file's patch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchHunk {
  /// `@@ -a,b +c,d @@ context`
  pub header: String,
  pub range: Hunk,
  /// The lines after the header, without their line ends: ` context`, `-deleted`, `+added`, and `\ No newline at end
  /// of file`
  pub lines: Vec<String>,
}

impl PatchHunk {
  /// The lines, each ended by a newline
  pub fn text(&self) -> String {
    self
      .lines
      .iter()
      .fold(String::with_capacity(self.lines.iter().map(|line| line.len() + 1).sum()), |mut text, line| {
        text.push_str(line);
        text.push('\n');
        text
      })
  }
}

/// The hunks of `path` in a commit's patch, numbered from 1 in this order; empty when the commit doesn't change it
pub fn file_hunks(patch: &str, path: &str) -> Vec<PatchHunk> {
  file_patch(patch, path).map_or_default(|file| file.hunks)
}

/// The section of the patch whose old or new path is `path`; `None` when the commit doesn't change it
pub fn file_patch(patch: &str, path: &str) -> Option<FilePatch> {
  split_file_sections(patch)
    .map(|lines| parse_file_section(&lines))
    .find(|file| [&file.old_path, &file.new_path].iter().any(|side| side.as_deref() == Some(path)))
}

/// The sections of a patch, each starting with its `diff --git` line
fn split_file_sections(patch: &str) -> impl Iterator<Item = Vec<&str>> {
  let mut sections: Vec<Vec<&str>> = Vec::new();
  for line in patch.split('\n') {
    if line.starts_with("diff --git ") {
      sections.push(vec![line]);
    } else if let Some(section) = sections.last_mut() {
      section.push(line);
    }
  }
  sections.into_iter()
}

fn parse_file_section(lines: &[&str]) -> FilePatch {
  let mut file = FilePatch {
    old_path: None,
    new_path: None,
    old_mode: None,
    new_mode: None,
    renamed: false,
    copied: false,
    binary: false,
    hunks: Vec::new(),
  };
  // `--- /dev/null` and `+++ /dev/null` name a side that doesn't exist; a section without them (binary, mode only)
  // takes the paths from its `diff --git` line
  let (mut old_seen, mut new_seen, mut added, mut deleted) = (false, false, false, false);
  let mut rest = lines.iter().skip(1).copied().peekable();
  while let Some(line) = rest.next() {
    if line.starts_with("@@ ") {
      let Some(range) = parse_hunk_header(line.as_bytes()) else { continue };
      let (mut old_left, mut new_left) = (range.old_lines, range.new_lines);
      let mut hunk_lines = Vec::new();
      // the counts of the header end the hunk; an empty line is an empty context line (`diff.suppressBlankEmpty`)
      while old_left > 0 || new_left > 0 || rest.peek().is_some_and(|next| next.starts_with('\\')) {
        let Some(hunk_line) = rest.next() else { break };
        match hunk_line.as_bytes().first() {
          Some(b'-') => old_left = old_left.saturating_sub(1),
          Some(b'+') => new_left = new_left.saturating_sub(1),
          Some(b'\\') => {}
          _ => {
            old_left = old_left.saturating_sub(1);
            new_left = new_left.saturating_sub(1);
          }
        }
        hunk_lines.push(hunk_line.to_string());
      }
      file.hunks.push(PatchHunk {
        header: line.to_string(),
        range,
        lines: hunk_lines,
      });
    } else if let Some(path) = line.strip_prefix("--- ") {
      old_seen = true;
      file.old_path = patch_path(path, "a/");
    } else if let Some(path) = line.strip_prefix("+++ ") {
      new_seen = true;
      file.new_path = patch_path(path, "b/");
    } else if let Some(path) = line.strip_prefix("rename from ") {
      file.renamed = true;
      file.old_path = Some(unquote(path));
    } else if let Some(path) = line.strip_prefix("rename to ") {
      file.new_path = Some(unquote(path));
    } else if let Some(path) = line.strip_prefix("copy from ") {
      file.copied = true;
      file.old_path = Some(unquote(path));
    } else if let Some(path) = line.strip_prefix("copy to ") {
      file.new_path = Some(unquote(path));
    } else if let Some(mode) = line.strip_prefix("old mode ") {
      file.old_mode = Some(mode.to_string());
    } else if let Some(mode) = line.strip_prefix("new mode ") {
      file.new_mode = Some(mode.to_string());
    } else if let Some(mode) = line.strip_prefix("new file mode ") {
      added = true;
      file.new_mode = Some(mode.to_string());
    } else if let Some(mode) = line.strip_prefix("deleted file mode ") {
      deleted = true;
      file.old_mode = Some(mode.to_string());
    } else if let Some(index) = line.strip_prefix("index ") {
      // `index <old>..<new> <mode>`: the mode of both sides
      if let Some((_, mode)) = index.split_once(' ') {
        file.old_mode.get_or_insert_with(|| mode.to_string());
        file.new_mode.get_or_insert_with(|| mode.to_string());
      }
    } else if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
      file.binary = true;
    }
  }
  if !file.renamed && !file.copied && (!old_seen || !new_seen) {
    let path = lines.first().and_then(|line| git_line_path(line.strip_prefix("diff --git ").unwrap_or_default()));
    if !old_seen && !added {
      file.old_path.clone_from(&path);
    }
    if !new_seen && !deleted {
      file.new_path = path;
    }
  }
  if added {
    file.old_mode = None;
  }
  if deleted {
    file.new_mode = None;
  }
  file
}

/// The path of a `---`/`+++` line without its `a/` or `b/` prefix and the tab git adds after a name with a space;
/// `None` for `/dev/null`
fn patch_path(text: &str, prefix: &str) -> Option<String> {
  let text = text.strip_suffix('\t').unwrap_or(text);
  if text == "/dev/null" {
    return None;
  }
  let path = unquote(text);
  Some(path.strip_prefix(prefix).map(str::to_string).unwrap_or(path))
}

/// The path of `a/<path> b/<path>` (both quoted or not), for a section that doesn't rename
fn git_line_path(text: &str) -> Option<String> {
  if text.starts_with('"') {
    let end = quoted_end(text)?;
    return unquote(&text[..end]).strip_prefix("a/").map(str::to_string);
  }
  // `a/P b/P`: both halves have the same length
  let length = text.len().checked_sub(5)? / 2;
  let (old, new) = (text.get(2..2 + length)?, text.get(2 + length..)?);
  (text.starts_with("a/") && new.strip_prefix(" b/") == Some(old)).then(|| old.to_string())
}

/// The end of the C-quoted string at the start of `text`, after its closing quote
fn quoted_end(text: &str) -> Option<usize> {
  let mut escaped = false;
  for (index, c) in text.char_indices().skip(1) {
    match c {
      _ if escaped => escaped = false,
      '\\' => escaped = true,
      '"' => return Some(index + 1),
      _ => {}
    }
  }
  None
}

/// A path as git prints it: as is, or C-quoted (`"a/x\ty"`, octal escapes for bytes)
fn unquote(text: &str) -> String {
  let Some(inner) = text.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) else {
    return text.to_string();
  };
  let mut bytes = Vec::with_capacity(inner.len());
  let mut chars = inner.bytes().peekable();
  while let Some(b) = chars.next() {
    if b != b'\\' {
      bytes.push(b);
      continue;
    }
    match chars.next() {
      Some(b'n') => bytes.push(b'\n'),
      Some(b't') => bytes.push(b'\t'),
      Some(b'r') => bytes.push(b'\r'),
      Some(b'a') => bytes.push(0x07),
      Some(b'b') => bytes.push(0x08),
      Some(b'f') => bytes.push(0x0c),
      Some(b'v') => bytes.push(0x0b),
      Some(digit @ b'0'..=b'7') => {
        let mut value = u32::from(digit - b'0');
        for _ in 0..2 {
          if let Some(&next @ b'0'..=b'7') = chars.peek() {
            value = value * 8 + u32::from(next - b'0');
            chars.next();
          }
        }
        bytes.push(u8::try_from(value).unwrap_or(b'?'));
      }
      Some(other) => bytes.push(other),
      None => bytes.push(b'\\'),
    }
  }
  String::from_utf8_lossy(&bytes).into_owned()
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
