//! Reads commits as raw objects and writes new ones with one `git fast-import` run, so a rewrite that only changes
//! messages keeps author and committer (name, email, date and timezone) and the bytes of every other message exactly.
//! `fast-import` objects are readable only after the stream ends, so this suits batches whose commits are all known
//! up front (reword, notes), not chains that must read each new commit back (the sync cherry-picks).

use crate::ref_transaction::RefTransaction;
use crate::reword_commits::complete_line;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::instrument;

/// A commit object as stored: identity lines verbatim (`Name <email> <seconds> <tz>`) and the message bytes
#[derive(Debug, Clone)]
pub struct RawCommit {
  pub tree: String,
  pub parents: Vec<String>,
  pub author: String,
  pub committer: String,
  pub encoding: Option<String>,
  pub message: Vec<u8>,
}

/// Read commit objects with one `cat-file --batch`. Signatures and other extra headers are not kept:
/// a rewritten commit can't keep a valid signature.
#[instrument(skip(git_executor, ids), fields(count = ids.len()))]
pub fn read_raw_commits(git_executor: &GitCommandExecutor, repo_path: &str, ids: &[&str]) -> Result<HashMap<String, RawCommit>> {
  let mut input = String::with_capacity(ids.len() * 41);
  for id in ids {
    input.push_str(id);
    input.push('\n');
  }
  let output = git_executor.execute_command_with_input_raw(&["cat-file", "--batch"], repo_path, &input)?;

  let mut commits = HashMap::with_capacity(ids.len());
  let mut rest = output.as_slice();
  for id in ids {
    let header_end = rest.iter().position(|&b| b == b'\n').ok_or_else(|| anyhow!("truncated cat-file output"))?;
    let header = std::str::from_utf8(&rest[..header_end])?;
    let mut fields = header.split(' ');
    let (Some(oid), Some(kind), Some(size)) = (fields.next(), fields.next(), fields.next()) else {
      return Err(anyhow!("commit {id} not found: {header}"));
    };
    if kind != "commit" {
      return Err(anyhow!("{id} is a {kind}, not a commit"));
    }
    let size: usize = size.parse()?;
    let content = rest.get(header_end + 1..header_end + 1 + size).ok_or_else(|| anyhow!("truncated cat-file output"))?;
    commits.insert((*id).to_string(), parse_commit_object(oid, content)?);
    // skip the content and its terminating LF
    rest = rest.get(header_end + 2 + size..).unwrap_or_default();
  }
  Ok(commits)
}

fn parse_commit_object(oid: &str, content: &[u8]) -> Result<RawCommit> {
  let split = content.windows(2).position(|w| w == b"\n\n");
  let (headers, message) = match split {
    Some(index) => (&content[..index], content[index + 2..].to_vec()),
    None => (content, Vec::new()),
  };
  let headers = std::str::from_utf8(headers).map_err(|e| anyhow!("commit {oid} has non-UTF-8 headers: {e}"))?;

  let mut commit = RawCommit {
    tree: String::new(),
    parents: Vec::new(),
    author: String::new(),
    committer: String::new(),
    encoding: None,
    message,
  };
  for line in headers.lines() {
    // continuation lines of multi-line headers (gpgsig, mergetag) start with a space
    let Some((key, value)) = line.split_once(' ') else { continue };
    match key {
      "tree" => commit.tree = value.to_string(),
      "parent" => commit.parents.push(value.to_string()),
      "author" => commit.author = value.to_string(),
      "committer" => commit.committer = value.to_string(),
      "encoding" => commit.encoding = Some(value.to_string()),
      _ => {}
    }
  }
  if commit.tree.is_empty() || commit.author.is_empty() || commit.committer.is_empty() {
    return Err(anyhow!("commit {oid} is missing tree, author or committer"));
  }
  Ok(commit)
}

/// Parent of a commit to write: an existing commit, or one written earlier in the same batch (by index)
#[derive(Debug, Clone, Copy)]
pub enum NewParent<'a> {
  Existing(&'a str),
  Written(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct NewCommit<'a> {
  pub tree: &'a str,
  /// `None` writes a root commit
  pub parent: Option<NewParent<'a>>,
  /// `Name <email> <seconds> <tz>`, as in `RawCommit`
  pub author: &'a str,
  pub committer: &'a str,
  pub encoding: Option<&'a str>,
  pub message: &'a [u8],
}

/// A commit of a small store whose tree holds one file, such as the issue groups
#[derive(Debug, Clone, Copy)]
pub struct FileCommit<'a> {
  pub path: &'a str,
  pub content: &'a [u8],
  /// The previous commit of the store; `None` writes a root commit
  pub parent: Option<&'a str>,
  pub message: &'a str,
}

/// A new commit of a store and the ref it moves, with compare-and-swap on the parent (or the create of the ref)
#[derive(Debug, Clone, Copy)]
pub struct StoreUpdate<'a> {
  pub store_ref: &'a str,
  pub commit: FileCommit<'a>,
}

impl StoreUpdate<'_> {
  /// Move the store's ref to `new_commit` in `transaction`, only if it is still at the parent (or does not exist)
  pub fn add_to(&self, transaction: &mut RefTransaction, new_commit: &str) {
    match self.commit.parent {
      Some(parent) => transaction.update(self.store_ref, new_commit, Some(parent)),
      None => transaction.create(self.store_ref, new_commit),
    };
  }
}

/// Commits written by `write_commits`, in input order. They are reachable from `temp_ref` until the caller deletes it,
/// normally in the same `RefTransaction` that moves the real branch, or with `discard` if the rewrite is abandoned.
#[derive(Debug)]
pub struct WrittenCommits {
  pub ids: Vec<String>,
  /// The commit of the `FileCommit` written in the same run
  pub file_commit: Option<String>,
  pub temp_ref: String,
}

impl WrittenCommits {
  /// Delete the temporary ref of an abandoned rewrite; the unreferenced objects are left to `git gc`
  pub fn discard(&self, git_executor: &GitCommandExecutor, repo_path: &str) {
    if let Err(e) = git_executor.execute_command(&["update-ref", "-d", &self.temp_ref], repo_path) {
      tracing::warn!(temp_ref = %self.temp_ref, error = %e, "failed to delete temporary ref");
    }
  }
}

/// A ref name no other operation of this or another Branch Deck process uses
fn unique_temp_ref() -> String {
  static COUNTER: AtomicU32 = AtomicU32::new(0);
  let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
  format!("refs/branch-deck/tmp/{}-{nanos}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// Append a `data` command with exact bytes
fn push_data(stream: &mut Vec<u8>, data: &[u8]) {
  stream.extend_from_slice(format!("data {}\n", data.len()).as_bytes());
  stream.extend_from_slice(data);
  stream.push(b'\n');
}

/// Notes for commits of a `write_commits_with_notes` batch, written to `notes_ref` in the same run
#[derive(Debug)]
pub struct NotesOfWritten<'a> {
  pub notes_ref: &'a str,
  /// `(index of the commit in the batch, note content)`
  pub notes: Vec<(usize, String)>,
}

/// Write `commits` with one `git fast-import` run and return their ids
pub fn write_commits(git_executor: &GitCommandExecutor, repo_path: &str, commits: &[NewCommit<'_>]) -> Result<WrittenCommits> {
  write_commits_with_notes(git_executor, repo_path, commits, None, None)
}

/// `write_commits` that also attaches notes to the new commits, in one notes commit of the same `fast-import` run, and
/// writes `file`, a commit of a store by the current user, after them on the temporary ref.
/// The notes ref moves even if the caller later abandons the commits; a note on an unreachable commit is harmless.
#[instrument(skip(git_executor, commits, notes, file), fields(count = commits.len()))]
pub fn write_commits_with_notes(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  commits: &[NewCommit<'_>],
  notes: Option<&NotesOfWritten<'_>>,
  file: Option<&FileCommit<'_>>,
) -> Result<WrittenCommits> {
  let temp_ref = unique_temp_ref();
  let mut stream = Vec::with_capacity(commits.len() * 256);
  for (index, commit) in commits.iter().enumerate() {
    if commit.parent.is_none() {
      // without `from`, a commit continues the ref's previous commit; reset it to write a root commit
      stream.extend_from_slice(format!("reset {temp_ref}\n\n").as_bytes());
    }
    stream.extend_from_slice(format!("commit {temp_ref}\nmark :{}\nauthor {}\ncommitter {}\n", index + 1, commit.author, commit.committer).as_bytes());
    if let Some(encoding) = commit.encoding {
      stream.extend_from_slice(format!("encoding {encoding}\n").as_bytes());
    }
    push_data(&mut stream, commit.message);
    match commit.parent {
      Some(NewParent::Existing(id)) => stream.extend_from_slice(format!("from {id}\n").as_bytes()),
      Some(NewParent::Written(parent_index)) => {
        if parent_index >= index {
          return Err(anyhow!("commit {index} refers to a parent written after it ({parent_index})"));
        }
        stream.extend_from_slice(format!("from :{}\n", parent_index + 1).as_bytes());
      }
      None => {}
    }
    // a tree at the empty path replaces the whole tree of the commit
    stream.extend_from_slice(format!("M 040000 {} \"\"\n\nget-mark :{}\n", commit.tree, index + 1).as_bytes());
  }
  let has_notes = notes.is_some_and(|notes| !notes.notes.is_empty());
  let committer = if has_notes || file.is_some() {
    Some(current_committer(git_executor, repo_path)?)
  } else {
    None
  };
  if let (Some(notes), Some(committer)) = (notes.filter(|notes| !notes.notes.is_empty()), &committer) {
    if let Some((index, _)) = notes.notes.iter().find(|(index, _)| *index >= commits.len()) {
      return Err(anyhow!("note for commit {index} of a batch of {}", commits.len()));
    }
    let targets = notes.notes.iter().map(|(index, content)| (format!(":{}", index + 1), content.as_str()));
    push_notes_commit(&mut stream, git_executor, repo_path, notes.notes_ref, committer, targets);
  }
  if let (Some(file), Some(committer)) = (file, &committer) {
    let mark = commits.len() + 1;
    if file.parent.is_none() {
      stream.extend_from_slice(format!("reset {temp_ref}\n\n").as_bytes());
    }
    stream.extend_from_slice(format!("commit {temp_ref}\nmark :{mark}\ncommitter {committer}\n").as_bytes());
    push_data(&mut stream, &complete_line(file.message));
    if let Some(parent) = file.parent {
      stream.extend_from_slice(format!("from {parent}\n").as_bytes());
    }
    stream.extend_from_slice(format!("deleteall\nM 100644 inline {}\n", file.path).as_bytes());
    push_data(&mut stream, file.content);
    stream.extend_from_slice(format!("\nget-mark :{mark}\n").as_bytes());
  }
  stream.extend_from_slice(b"done\n");

  let output = git_executor.execute_command_with_input_bytes(&["fast-import", "--quiet", "--done", "--date-format=raw", "--signed-commits=strip"], repo_path, &stream)?;
  let mut ids: Vec<String> = String::from_utf8(output)?.lines().map(str::to_string).collect();
  let file_commit = if file.is_some() { ids.pop() } else { None };
  if ids.len() != commits.len() || file_commit.as_ref().is_none_or(String::is_empty) != file.is_none() {
    return Err(anyhow!("fast-import wrote {} of {} commits", ids.len(), commits.len()));
  }
  Ok(WrittenCommits { ids, file_commit, temp_ref })
}

/// Write the commit of `store` with one `git fast-import` run and move its ref in one transaction, with `verify`
/// (`(full ref, expected id)`, e.g. the checked out branch at the HEAD the store was computed for) in the same
/// transaction. Returns the new commit of the store; on a failure nothing moves
#[instrument(skip(git_executor, store))]
pub fn write_store_commit(git_executor: &GitCommandExecutor, repo_path: &str, store: &StoreUpdate<'_>, verify: Option<(&str, &str)>, operation: &str) -> Result<String> {
  let written = write_commits_with_notes(git_executor, repo_path, &[], None, Some(&store.commit))?;
  let new_commit = written.file_commit.clone().ok_or_else(|| anyhow!("fast-import wrote no commit for {}", store.store_ref))?;
  let mut transaction = RefTransaction::new();
  if let Some((full_ref, expected)) = verify {
    transaction.verify(full_ref, expected);
  }
  store.add_to(&mut transaction, &new_commit);
  transaction.delete(&written.temp_ref, None);
  if let Err(e) = transaction.commit(git_executor, repo_path, operation) {
    written.discard(git_executor, repo_path);
    return Err(e);
  }
  Ok(new_commit)
}

/// Identity of the current user with the current time (`Name <email> <seconds> <tz>`), for new commits and notes
pub fn current_committer(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<String> {
  git_executor
    .execute_command(&["var", "GIT_COMMITTER_IDENT"], repo_path)
    .map_err(|e| anyhow!("Git user identity is not configured (set user.name and user.email): {e:#}"))
}

/// Add or overwrite notes on `notes_ref` with one `git fast-import` run (one notes commit for all of them).
/// Like `git notes add`, each note is stored with a trailing newline. fast-import refuses to move the notes ref
/// if it changed meanwhile, so a concurrent writer is not overwritten.
pub fn write_notes(git_executor: &GitCommandExecutor, repo_path: &str, notes_ref: &str, notes: &[(String, String)]) -> Result<()> {
  write_notes_to_refs(git_executor, repo_path, &[(notes_ref, notes)])
}

/// `write_notes` for several notes refs, `(notes ref, notes)`, with one `git fast-import` run: one notes commit per ref
/// that gets notes
#[instrument(skip(git_executor, refs), fields(refs = refs.len()))]
pub fn write_notes_to_refs(git_executor: &GitCommandExecutor, repo_path: &str, refs: &[(&str, &[(String, String)])]) -> Result<()> {
  if refs.iter().all(|(_, notes)| notes.is_empty()) {
    return Ok(());
  }
  let committer = current_committer(git_executor, repo_path)?;
  let mut stream = Vec::with_capacity(refs.iter().map(|(_, notes)| notes.len() * 128).sum());
  for (notes_ref, notes) in refs.iter().filter(|(_, notes)| !notes.is_empty()) {
    push_notes_commit(
      &mut stream,
      git_executor,
      repo_path,
      notes_ref,
      &committer,
      notes.iter().map(|(target, content)| (target.as_str(), content.as_str())),
    );
  }
  stream.extend_from_slice(b"done\n");
  git_executor.execute_command_with_input_bytes(&["fast-import", "--quiet", "--done"], repo_path, &stream)?;
  Ok(())
}

/// Append a notes commit on `notes_ref` that adds or overwrites a note per `(target, content)`; a target is a commit id
/// or a mark of the same stream. Like `git notes add`, each note is stored with a trailing newline.
fn push_notes_commit<'a, T: AsRef<str>>(
  stream: &mut Vec<u8>,
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  notes_ref: &str,
  committer: &str,
  notes: impl Iterator<Item = (T, &'a str)>,
) {
  stream.extend_from_slice(format!("commit {notes_ref}\ncommitter {committer}\n").as_bytes());
  push_data(stream, b"Notes added by Branch Deck\n");
  if git_executor.ref_exists(repo_path, notes_ref) {
    stream.extend_from_slice(format!("from {notes_ref}^0\n").as_bytes());
  }
  for (target, content) in notes {
    stream.extend_from_slice(format!("N inline {}\n", target.as_ref()).as_bytes());
    let mut data = content.as_bytes().to_vec();
    if !data.ends_with(b"\n") {
      data.push(b'\n');
    }
    push_data(stream, &data);
  }
  stream.push(b'\n');
}
