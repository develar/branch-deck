//! The merges and the commits of one branch of a sync, each in one git process for the whole branch instead of one per
//! commit (docs/design/git-operations.md, "Sync: one merge and one commit process per branch").
//!
//! - `merge-tree --write-tree --stdin` gets one line per cherry-pick, `<base tree> -- <target tree> <commit tree>`, and
//!   answers it before the next line comes: the target of a merge is the tree the merge before it wrote.
//! - `fast-import` writes each commit and answers its id (`get-mark`), so the next commit and the reuse checks of
//!   `create_or_update_commit` have it at once. The commit object has the bytes `commit-tree` would write (`ident`,
//!   the message completed with a newline), so a commit copied again has the id it had.
//!
//! The commits of the writer are readable by other git processes only after `flush`. The chain flushes before a
//! conflict is analysed and at `finish`, before the branch's head goes to the branches built on it. The writer's
//! commits stay on no ref (`reset` without `from` before `done`): the sync's ref transaction makes them reachable, as
//! it did the commits of `commit-tree`.

use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::interactive_command::InteractiveCommand;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, instrument, warn};

/// How long a merge or a commit of the chain may take: a merge of a large tree takes seconds, never minutes
const REPLY_TIMEOUT: Duration = Duration::from_mins(10);

/// The ref the writer's commits are written on; it is reset before `done`, so git never writes it
const WRITER_REF: &str = "refs/branch-deck/tmp/chain";

/// The result of a merge of the chain
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainMerge {
  Clean {
    tree: String,
  },
  /// The conflicting paths, each once, in git's order
  Conflict {
    files: Vec<String>,
  },
}

/// A commit to write: what `commit-tree` gets from its arguments and the `GIT_AUTHOR_*` and `GIT_COMMITTER_*`
/// variables
#[derive(Debug, Clone, Copy)]
pub struct ChainCommit<'a> {
  pub tree: &'a str,
  pub parent: &'a str,
  pub author_name: &'a str,
  pub author_email: &'a str,
  pub author_timestamp: u32,
  /// `+hhmm`; empty for UTC
  pub author_timezone: &'a str,
  pub committer_name: &'a str,
  pub committer_email: &'a str,
  pub committer_timestamp: u32,
  pub committer_timezone: &'a str,
  pub message: &'a str,
}

pub struct CommitChain {
  git_executor: GitCommandExecutor,
  repo_path: String,
  merger: Option<InteractiveCommand>,
  writer: Option<InteractiveCommand>,
  /// The commits of the running writer by id, with their marks: a commit of the same stream is a parent by its mark
  marks: HashMap<String, usize>,
}

impl CommitChain {
  pub fn new(git_executor: &GitCommandExecutor, repo_path: &str) -> Self {
    Self {
      git_executor: git_executor.clone(),
      repo_path: repo_path.to_string(),
      merger: None,
      writer: None,
      marks: HashMap::new(),
    }
  }

  /// Merge `commit` (a tree) into `target` (a tree) with `base` (a tree) as the merge base, as
  /// `merge-tree --write-tree --merge-base <base> <target> <commit>` does with the commits of these trees
  #[instrument(skip(self))]
  pub fn merge(&mut self, base: &str, target: &str, commit: &str) -> Result<ChainMerge> {
    let merger = match &mut self.merger {
      Some(merger) => merger,
      None => self.merger.insert(
        self
          .git_executor
          .spawn_interactive_with_delimiter(&["merge-tree", "--write-tree", "--stdin", "-z", "--no-messages"], &self.repo_path, b'\0')?,
      ),
    };
    merger.send(&format!("{base} -- {target} {commit}"))?;
    // `<status>\0<tree>\0`, then `<mode> <object> <stage>\t<path>\0` per conflicted stage and an empty field
    let status = merger.read_record(REPLY_TIMEOUT)?;
    let tree = String::from_utf8(merger.read_record(REPLY_TIMEOUT)?)?;
    let mut files: Vec<String> = Vec::new();
    loop {
      let entry = merger.read_record(REPLY_TIMEOUT)?;
      if entry.is_empty() {
        break;
      }
      let tab = memchr::memchr(b'\t', &entry).ok_or_else(|| anyhow!("Unexpected merge-tree entry: {}", String::from_utf8_lossy(&entry)))?;
      let path = String::from_utf8_lossy(&entry[tab + 1..]).into_owned();
      if files.last() != Some(&path) {
        files.push(path);
      }
    }
    match status.as_slice() {
      b"1" if files.is_empty() => Ok(ChainMerge::Clean { tree }),
      b"0" => Ok(ChainMerge::Conflict { files }),
      _ => bail!("Unexpected merge-tree status {:?} with {} conflicted paths", String::from_utf8_lossy(&status), files.len()),
    }
  }

  /// Write `commit` and return its id
  #[instrument(skip(self, commit), fields(tree = commit.tree, parent = commit.parent))]
  pub fn write_commit(&mut self, commit: &ChainCommit<'_>) -> Result<String> {
    let author = ident(commit.author_name, commit.author_email, commit.author_timestamp, commit.author_timezone)?;
    let committer = ident(commit.committer_name, commit.committer_email, commit.committer_timestamp, commit.committer_timezone)?;
    let mark = self.marks.len() + 1;
    let from = self
      .marks
      .get(commit.parent)
      .map_or_else(|| commit.parent.to_string(), |parent_mark| format!(":{parent_mark}"));
    let message = complete_line(commit.message);

    let mut stream = Vec::with_capacity(256 + message.len());
    stream.extend_from_slice(format!("commit {WRITER_REF}\nmark :{mark}\nauthor {author}\ncommitter {committer}\ndata {}\n", message.len()).as_bytes());
    stream.extend_from_slice(&message);
    // a tree at the empty path replaces the whole tree of the commit
    stream.extend_from_slice(format!("\nfrom {from}\nM 040000 {} \"\"\n\nget-mark :{mark}\n", commit.tree).as_bytes());

    let writer = match &mut self.writer {
      Some(writer) => writer,
      None => self.writer.insert(
        self
          .git_executor
          .spawn_interactive(&["fast-import", "--quiet", "--done", "--date-format=raw"], &self.repo_path)?,
      ),
    };
    writer.send_bytes(&stream)?;
    let id = writer.read_line(REPLY_TIMEOUT)?;
    if id.len() < 40 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
      bail!("fast-import answered {id:?} for a commit");
    }
    self.marks.insert(id.clone(), mark);
    Ok(id)
  }

  /// End the writer, so other git processes read its commits; a later commit starts a new one
  pub fn flush(&mut self) -> Result<()> {
    self.marks.clear();
    let Some(mut writer) = self.writer.take() else {
      return Ok(());
    };
    debug!("ending the commit writer of the chain");
    writer.send(&format!("reset {WRITER_REF}\n\ndone"))?;
    writer.finish()
  }

  /// Flush the commits and end both processes
  pub fn finish(mut self) -> Result<()> {
    let flushed = self.flush();
    if let Some(merger) = self.merger.take() {
      merger.finish()?;
    }
    flushed
  }
}

impl Drop for CommitChain {
  /// A chain left on an error still ends its writer with `done`: without it, `fast-import` dies on the early end of its
  /// stream and leaves a crash report in the git directory
  fn drop(&mut self) {
    if let Err(e) = self.flush() {
      warn!(error = %e, "cannot end the commit writer of the chain");
    }
  }
}

/// An identity line as `commit-tree` writes it from `GIT_*_NAME`, `GIT_*_EMAIL` and `GIT_*_DATE`: `Name <email>
/// <seconds> <tz>`, name and email without git's "crud" (`strbuf_addstr_without_crud` in `ident.c`); an empty name is an
/// error, as for `commit-tree`. A date without a timezone is UTC
fn ident(name: &str, email: &str, timestamp: u32, timezone: &str) -> Result<String> {
  let name = without_crud(name);
  if name.is_empty() {
    bail!("empty ident name (for <{email}>) not allowed");
  }
  let timezone = if timezone.is_empty() { "+0000" } else { timezone };
  Ok(format!("{name} <{}> {timestamp} {timezone}", without_crud(email)))
}

/// git's `crud`: control characters, the space and `,:;<>"\'`
const fn is_crud(c: u8) -> bool {
  c <= 32 || matches!(c, b',' | b':' | b';' | b'<' | b'>' | b'"' | b'\\' | b'\'')
}

/// `strbuf_addstr_without_crud`: crud cut at both ends, then `\n`, `<` and `>` dropped
fn without_crud(text: &str) -> String {
  let bytes = text.as_bytes();
  let start = bytes.iter().position(|&c| !is_crud(c)).unwrap_or(bytes.len());
  let end = bytes.iter().rposition(|&c| !is_crud(c)).map_or(start, |last| last + 1);
  let kept: Vec<u8> = bytes[start..end.max(start)].iter().copied().filter(|c| !matches!(c, b'\n' | b'<' | b'>')).collect();
  // only ASCII bytes are cut, so the rest is still UTF-8
  String::from_utf8(kept).unwrap_or_default()
}

/// The message as `commit-tree -m` stores it (`strbuf_complete_line`): a newline at the end unless it has one; an
/// empty message stays empty
fn complete_line(message: &str) -> Vec<u8> {
  let mut bytes = message.as_bytes().to_vec();
  if !bytes.is_empty() && !bytes.ends_with(b"\n") {
    bytes.push(b'\n');
  }
  bytes
}

#[cfg(test)]
#[path = "commit_chain_test.rs"]
mod commit_chain_test;
