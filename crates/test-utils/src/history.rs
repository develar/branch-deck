//! A history of a fixture repository in two git processes: one `git fast-import` writes its commits (and any other ref
//! a fixture needs), one `read-tree -u --reset HEAD` gives the index and the working tree the files of HEAD. `add` and
//! `commit` cost two processes per commit, and a fixture of 50 commits 100 of them. The commits are those
//! `git commit -m` writes with the test identity (`Test User <test@example.com>`) or the commit's own (`by`): the
//! message cleaned up as `-m` does, the author and the committer at the commit's time (`+0000`), else now.

use crate::git_test_utils::git_command;
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::Stdio;

/// The identity of the commits of a fixture, the `user.name` and `user.email` of its config
pub const TEST_IDENTITY: &str = "Test User <test@example.com>";

/// A commit of a history: its message, the files it writes and the paths it deletes, its time (Unix seconds) and its
/// author and committer (`Name <email>`, else `TEST_IDENTITY`)
#[derive(Debug, Clone, Default)]
pub struct HistoryCommit {
  pub message: String,
  pub files: Vec<(String, String)>,
  pub deleted: Vec<String>,
  pub timestamp: Option<i64>,
  pub identity: Option<String>,
}

impl HistoryCommit {
  /// A commit of `files` at the current time
  pub fn new(message: impl Into<String>, files: &[(&str, &str)]) -> Self {
    Self {
      message: message.into(),
      files: files.iter().map(|(path, content)| ((*path).to_string(), (*content).to_string())).collect(),
      ..Self::default()
    }
  }

  /// This commit at `time`: two commits of one author with one subject at one time share their identity (`CommitKey`)
  #[must_use]
  pub fn at(self, time: i64) -> Self {
    Self { timestamp: Some(time), ..self }
  }

  /// This commit by `identity` (`Name <email>`), its author and its committer, as `git commit` of a repository whose
  /// `user.name` and `user.email` name them writes it
  #[must_use]
  pub fn by(self, identity: &str) -> Self {
    Self {
      identity: Some(identity.to_string()),
      ..self
    }
  }

  /// This commit deleting `paths` too, as a `git rm` of them before it
  #[must_use]
  pub fn deleting(mut self, paths: &[&str]) -> Self {
    self.deleted.extend(paths.iter().map(|path| (*path).to_string()));
    self
  }
}

/// Commits of one file each, `(message, path, content)`, as `create_commit` of each writes them
pub fn one_file_each<const N: usize>(commits: [(&str, &str, &str); N]) -> [HistoryCommit; N] {
  commits.map(|(message, path, content)| HistoryCommit::new(message, &[(path, content)]))
}

/// Nine lines, each its number, with `changes` (line number, text) in place: a change of one line keeps the others as
/// the context of its diff
pub fn numbered_lines(changes: &[(usize, &str)]) -> String {
  (1..=9).fold(String::new(), |mut text, line| {
    match changes.iter().find(|(number, _)| *number == line) {
      Some((_, changed)) => text.push_str(changed),
      None => text.push_str(&line.to_string()),
    }
    text.push('\n');
    text
  })
}

/// A `fast-import` stream that the commits of `branch` and the refs of a fixture go into, with the files each commit
/// leaves, so a commit that changes nothing is left out as `git commit` refuses it
pub struct HistoryStream {
  stream: String,
  /// The files of the branch's tip, path -> content
  files: BTreeMap<String, String>,
  next_mark: usize,
  /// The mark of each commit of `commits`, `None` for one left out
  marks: Vec<Option<usize>>,
  branch: String,
  /// The mark of the branch's tip, `None` before its first commit
  tip: Option<usize>,
  /// The first commit continues the branch as the repository has it (`on_branch`), whose files the stream doesn't know:
  /// every commit counts as a change then
  continues: bool,
}

impl HistoryStream {
  /// A stream of new commits on `branch` (`refs/heads/master`), which has none yet
  pub fn new(branch: &str) -> Self {
    Self {
      stream: String::new(),
      files: BTreeMap::new(),
      next_mark: 1,
      marks: Vec::new(),
      branch: branch.to_string(),
      tip: None,
      continues: false,
    }
  }

  /// A stream of commits on top of `branch` as the repository has it (`from <branch>^0`)
  pub fn on_branch(branch: &str) -> Self {
    Self {
      continues: true,
      ..Self::new(branch)
    }
  }

  /// Adds `commit` on the branch; its index in the ids of `write`
  pub fn commit(&mut self, commit: &HistoryCommit) -> usize {
    let index = self.marks.len();
    let mut changed = self.continues || commit.deleted.iter().any(|path| self.files.contains_key(path));
    changed |= commit.files.iter().any(|(path, content)| self.files.get(path) != Some(content));
    if !changed {
      self.marks.push(None);
      return index;
    }
    let mark = self.next_mark;
    self.next_mark += 1;
    let when = format!("{} +0000", commit.timestamp.unwrap_or_else(now));
    let identity = commit.identity.as_deref().unwrap_or(TEST_IDENTITY);
    let _ = writeln!(self.stream, "commit {}\nmark :{mark}", self.branch);
    let _ = writeln!(self.stream, "author {identity} {when}\ncommitter {identity} {when}");
    push_data(&mut self.stream, &cleaned_message(&commit.message));
    match self.tip {
      Some(tip) => {
        let _ = writeln!(self.stream, "from :{tip}");
      }
      None if self.continues => {
        let _ = writeln!(self.stream, "from {}^0", self.branch);
      }
      None => {}
    }
    for path in &commit.deleted {
      if self.files.remove(path).is_some() || self.continues {
        let _ = writeln!(self.stream, "D {path}");
      }
    }
    for (path, content) in &commit.files {
      let _ = writeln!(self.stream, "M 100644 inline {path}");
      push_data(&mut self.stream, content);
      self.files.insert(path.clone(), content.clone());
    }
    self.stream.push('\n');
    self.tip = Some(mark);
    self.marks.push(Some(mark));
    index
  }

  /// Points `reference` at the first commit of the branch, its root, if it has one
  pub fn reset_to_root(&mut self, reference: &str) {
    if let Some(mark) = self.marks.iter().flatten().next() {
      let _ = writeln!(self.stream, "reset {reference}\nfrom :{mark}\n");
    }
  }

  /// Points `reference` at the commit `index` of `commit` (its index in the ids of `write`), as an `update-ref` of it
  /// between two commits moves a baseline
  ///
  /// # Panics
  /// When the stream has no such commit, or left it out as one that changed nothing
  pub fn reset_at(&mut self, reference: &str, index: usize) {
    let mark = self.marks.get(index).copied().flatten().unwrap_or_else(|| panic!("no commit {index} in the stream"));
    let _ = writeln!(self.stream, "reset {reference}\nfrom :{mark}\n");
  }

  /// A root commit on `reference` of its one file `file` with `content`, as `commit-tree` writes a store of Branch Deck
  pub fn root_commit(&mut self, reference: &str, message: &str, file: &str, content: &str) {
    let mark = self.next_mark;
    self.next_mark += 1;
    let when = format!("{} +0000", now());
    let _ = writeln!(self.stream, "commit {reference}\nmark :{mark}");
    let _ = writeln!(self.stream, "author {TEST_IDENTITY} {when}\ncommitter {TEST_IDENTITY} {when}");
    push_data(&mut self.stream, &cleaned_message(message));
    let _ = writeln!(self.stream, "M 100644 inline {file}");
    push_data(&mut self.stream, content);
    self.stream.push('\n');
  }

  /// Writes the stream into the repository at `repo_path` (one `fast-import`), then the index and the working tree of
  /// HEAD (one `read-tree -u --reset`); the ids of the commits of `commit`, `None` for one that changed nothing
  ///
  /// # Errors
  /// When git refuses the stream or the checkout
  pub fn write(self, repo_path: &Path) -> Result<Vec<Option<String>>> {
    let marks_file = tempfile::NamedTempFile::new()?;
    let export = format!("--export-marks={}", marks_file.path().display());
    let mut child = git_command()
      .args(["fast-import", "--quiet", "--done", "--date-format=raw", &export])
      .current_dir(repo_path)
      .stdin(Stdio::piped())
      .stdout(Stdio::null())
      .stderr(Stdio::piped())
      .spawn()
      .context("cannot start git fast-import")?;
    let mut input = child.stdin.take().context("piped stdin")?;
    input.write_all(self.stream.as_bytes())?;
    input.write_all(b"done\n")?;
    drop(input);
    let output = child.wait_with_output()?;
    if !output.status.success() {
      bail!("git fast-import failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    let ids: HashMap<usize, String> = std::fs::read_to_string(marks_file.path())?
      .lines()
      .filter_map(|line| {
        let (mark, id) = line.strip_prefix(':')?.split_once(' ')?;
        Some((mark.parse().ok()?, id.to_string()))
      })
      .collect();
    if self.tip.is_some() {
      let checkout = git_command().args(["read-tree", "-u", "--reset", "HEAD"]).current_dir(repo_path).output()?;
      if !checkout.status.success() {
        bail!("git read-tree failed: {}", String::from_utf8_lossy(&checkout.stderr));
      }
    }
    Ok(self.marks.iter().map(|mark| mark.and_then(|mark| ids.get(&mark).cloned())).collect())
  }
}

/// `text` as a `data` command of exactly its bytes
fn push_data(stream: &mut String, text: &str) {
  let _ = write!(stream, "data {}\n{text}\n", text.len());
}

/// The message `git commit -m <message>` writes (`--cleanup=whitespace`): no trailing whitespace on a line, no leading
/// or trailing empty lines, empty lines in a row as one, a newline at the end; an empty message stays empty
pub fn cleaned_message(message: &str) -> String {
  let mut cleaned = String::new();
  let mut empty_lines = 0;
  for line in message.lines().map(str::trim_end) {
    if line.is_empty() {
      empty_lines += 1;
      continue;
    }
    if !cleaned.is_empty() && empty_lines > 0 {
      cleaned.push('\n');
    }
    empty_lines = 0;
    cleaned.push_str(line);
    cleaned.push('\n');
  }
  cleaned
}

fn now() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
#[path = "history_test.rs"]
mod history_test;
