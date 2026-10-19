//! Reads commits as raw objects and writes new ones with one `git fast-import` run, so a rewrite that only changes
//! messages keeps author and committer (name, email, date and timezone) and the bytes of every other message exactly.
//! `fast-import` objects are readable only after the stream ends, so this suits batches whose commits are all known
//! up front (reword, notes); the sync's copies, each written onto the one before it, go through the interactive writer
//! of `commit_chain.rs`.

use crate::cat_file_batch::{BatchEntry, NO_LIMIT, for_each_object};
use crate::ref_transaction::RefTransaction;
use crate::revs::ref_exists;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use std::fmt::Write as _;
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

/// Read commit objects with one `cat-file --batch` (`cat_file_batch`). Signatures and other extra headers are not kept:
/// a rewritten commit can't keep a valid signature.
#[instrument(skip(git_executor, ids), fields(count = ids.len()))]
pub fn read_raw_commits(git_executor: &GitCommandExecutor, repo_path: &str, ids: &[&str]) -> Result<HashMap<String, RawCommit>> {
  let mut commits = HashMap::with_capacity(ids.len());
  for_each_object(git_executor, repo_path, ids, NO_LIMIT, |index, entry| {
    let id = ids[index];
    match entry {
      BatchEntry::Object { id: oid, kind: "commit", content } => {
        commits.insert(id.to_string(), parse_raw_commit(oid, &content)?);
        Ok(())
      }
      BatchEntry::Object { kind, .. } | BatchEntry::TooLarge { kind, .. } => Err(anyhow!("{id} is a {kind}, not a commit")),
      BatchEntry::Missing => Err(anyhow!("commit {id} not found")),
    }
  })?;
  Ok(commits)
}

/// Parse the content of a commit object, as `cat-file --batch` prints it
pub fn parse_raw_commit(oid: &str, content: &[u8]) -> Result<RawCommit> {
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

/// A tree entry of a store commit that refers to an existing object, e.g. a blob (`100644`) or a tree (`040000`)
#[derive(Debug, Clone, Copy)]
pub struct StoreEntry<'a> {
  pub mode: &'a str,
  pub id: &'a str,
  pub path: &'a str,
}

/// A commit of a small store by the current user, such as the issue groups (one file) or the state of a resolve
/// session: its files (stored as `100644` blobs) and entries make its whole tree
#[derive(Debug, Clone)]
pub struct StoreCommit<'a> {
  /// `(path, content)`
  pub files: Vec<(&'a str, &'a [u8])>,
  pub entries: Vec<StoreEntry<'a>>,
  /// The previous commit of the store; `None` writes a root commit
  pub parent: Option<NewParent<'a>>,
  /// More parents, which keep commits reachable as long as the store points here (e.g. the commits a resolve session
  /// replayed): reachability through parents, which every git command follows
  pub merges: Vec<&'a str>,
  pub message: &'a str,
}

impl<'a> StoreCommit<'a> {
  /// A commit whose tree holds the one file `path`
  pub fn file(path: &'a str, content: &'a [u8], parent: Option<&'a str>, message: &'a str) -> Self {
    Self {
      files: vec![(path, content)],
      entries: Vec::new(),
      parent: parent.map(NewParent::Existing),
      merges: Vec::new(),
      message,
    }
  }

  /// The previous commit of the store, if it is an existing one
  const fn existing_parent(&self) -> Option<&'a str> {
    match self.parent {
      Some(NewParent::Existing(id)) => Some(id),
      _ => None,
    }
  }
}

/// A new commit of a store and the ref it moves, with compare-and-swap on the parent (or the create of the ref)
#[derive(Debug, Clone)]
pub struct StoreUpdate<'a> {
  pub store_ref: &'a str,
  pub commit: StoreCommit<'a>,
}

impl StoreUpdate<'_> {
  /// Move the store's ref to `new_commit` in `transaction`, only if it is still at the parent (or does not exist)
  pub fn add_to(&self, transaction: &mut RefTransaction, new_commit: &str) {
    match self.commit.existing_parent() {
      Some(parent) => transaction.update(self.store_ref, new_commit, Some(parent)),
      None => transaction.create(self.store_ref, new_commit),
    };
  }
}

/// A path for a `fast-import` command, C-style quoted, so any byte except NUL is safe
pub fn quote_path(path: &str) -> String {
  let mut quoted = String::with_capacity(path.len() + 2);
  quoted.push('"');
  for c in path.chars() {
    match c {
      '"' => quoted.push_str("\\\""),
      '\\' => quoted.push_str("\\\\"),
      '\n' => quoted.push_str("\\n"),
      c if c.is_ascii_control() => {
        let _ = write!(quoted, "\\{:03o}", u32::from(c));
      }
      c => quoted.push(c),
    }
  }
  quoted.push('"');
  quoted
}

/// Append the commit of a store on `temp_ref` with mark `mark`; `parent_mark` maps a `NewParent::Written` index to its mark
fn push_store_commit(stream: &mut Vec<u8>, temp_ref: &str, mark: usize, committer: &str, commit: &StoreCommit<'_>, parent_mark: impl Fn(usize) -> usize) {
  if commit.parent.is_none() {
    stream.extend_from_slice(format!("reset {temp_ref}\n\n").as_bytes());
  }
  stream.extend_from_slice(format!("commit {temp_ref}\nmark :{mark}\ncommitter {committer}\n").as_bytes());
  push_data(stream, &complete_line(commit.message));
  match commit.parent {
    Some(NewParent::Existing(parent)) => stream.extend_from_slice(format!("from {parent}\n").as_bytes()),
    Some(NewParent::Written(index)) => stream.extend_from_slice(format!("from :{}\n", parent_mark(index)).as_bytes()),
    None => {}
  }
  for merge in &commit.merges {
    stream.extend_from_slice(format!("merge {merge}\n").as_bytes());
  }
  stream.extend_from_slice(b"deleteall\n");
  for (path, content) in &commit.files {
    stream.extend_from_slice(format!("M 100644 inline {}\n", quote_path(path)).as_bytes());
    push_data(stream, content);
  }
  for entry in &commit.entries {
    stream.extend_from_slice(format!("M {} {} {}\n", entry.mode, entry.id, quote_path(entry.path)).as_bytes());
  }
  stream.extend_from_slice(format!("\nget-mark :{mark}\n").as_bytes());
}

/// Commits written by `write_commits`, in input order. They are reachable from `temp_ref` (the store commits from
/// `store_temp_refs`) until the caller deletes it, normally in the same `RefTransaction` that moves the real branch
/// (`delete_temp_refs`), or with `discard` if the rewrite is abandoned.
#[derive(Debug)]
pub struct WrittenCommits {
  pub ids: Vec<String>,
  /// The commits of the `StoreCommit`s written in the same run, in input order
  pub store_commits: Vec<String>,
  pub temp_ref: String,
  /// One temporary ref per store commit, so that none replaces the tip of `temp_ref` or of another store
  pub store_temp_refs: Vec<String>,
}

impl WrittenCommits {
  /// Delete the temporary refs in `transaction`, the one that moves the real refs, each with compare-and-swap on the
  /// commit written last on it (`temp_ref` exists only with commits)
  pub fn delete_temp_refs(&self, transaction: &mut RefTransaction) {
    if let Some(tip) = self.ids.last() {
      transaction.delete(&self.temp_ref, Some(tip));
    }
    for (temp_ref, commit) in self.store_temp_refs.iter().zip(&self.store_commits) {
      transaction.delete(temp_ref, Some(commit));
    }
  }

  /// Delete the temporary refs of an abandoned rewrite; the unreferenced objects are left to `git gc`
  pub fn discard(&self, git_executor: &GitCommandExecutor, repo_path: &str) {
    discard_temp_refs(git_executor, repo_path, &self.temp_ref, |transaction| self.delete_temp_refs(transaction));
  }
}

/// Delete temporary refs (`delete` adds them to the transaction) in a transaction of their own; a failure is only logged
pub(crate) fn discard_temp_refs(git_executor: &GitCommandExecutor, repo_path: &str, temp_ref: &str, delete: impl FnOnce(&mut RefTransaction)) {
  let mut transaction = RefTransaction::new();
  delete(&mut transaction);
  if let Err(e) = transaction.commit(git_executor, repo_path, "discard") {
    tracing::warn!(temp_ref = %temp_ref, error = %e, "failed to delete temporary refs");
  }
}

/// The prefix of the temporary refs: `<prefix><pid>-<nanos since the epoch>-<counter>`, a `-notes` suffix for notes
pub const TEMP_REF_PREFIX: &str = "refs/branch-deck/tmp/";

/// Temporary refs older than this belong to a process that is gone
pub const STALE_TEMP_REF_AGE: std::time::Duration = std::time::Duration::from_secs(3600);

/// Whether the temporary ref `name` (a full ref under `TEMP_REF_PREFIX`) was written more than `older_than` ago: the
/// leftover of a process killed before it deleted the ref, which keeps its objects alive. A name without the time of
/// `unique_temp_ref` is the shared `refs/branch-deck/tmp/chain` of Branch Deck 0.21 and earlier, a leftover too
pub fn is_stale_temp_ref(name: &str, older_than: std::time::Duration) -> bool {
  let Some(rest) = name.strip_prefix(TEMP_REF_PREFIX) else {
    return false;
  };
  let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
  rest
    .split('-')
    .nth(1)
    .and_then(|nanos| nanos.parse::<u128>().ok())
    .is_none_or(|written_at| now.saturating_sub(written_at) > older_than.as_nanos())
}

/// Delete the temporary refs written more than `older_than` ago (`is_stale_temp_ref`), with one `for-each-ref` and at
/// most one `update-ref`. Runs under the repository lock; a failure is only logged
pub fn sweep_stale_temp_refs(git_executor: &GitCommandExecutor, repo_path: &str, older_than: std::time::Duration) {
  match git_executor.execute_command_lines(&["for-each-ref", "--format=%(refname) %(objectname)", TEMP_REF_PREFIX], repo_path) {
    Ok(refs) => {
      let stale: Vec<(&str, &str)> = refs
        .iter()
        .filter_map(|line| line.split_once(' '))
        .filter(|(name, _)| is_stale_temp_ref(name, older_than))
        .collect();
      delete_stale_temp_refs(git_executor, repo_path, &stale);
    }
    Err(e) => tracing::warn!(error = %format!("{e:#}"), "failed to sweep stale temporary refs"),
  }
}

/// Delete the stale temporary refs `stale` (`(ref, tip)`, `is_stale_temp_ref`) in one CAS transaction, none for an
/// empty list; a failure is only logged: a ref that moved meanwhile is not stale
pub fn delete_stale_temp_refs(git_executor: &GitCommandExecutor, repo_path: &str, stale: &[(impl AsRef<str>, impl AsRef<str>)]) {
  if stale.is_empty() {
    return;
  }
  let mut transaction = RefTransaction::new();
  for (name, id) in stale {
    transaction.delete(name.as_ref(), Some(id.as_ref()));
  }
  match transaction.commit(git_executor, repo_path, "sweep temporary refs") {
    Ok(()) => tracing::debug!(count = stale.len(), "deleted stale temporary refs"),
    Err(e) => tracing::warn!(error = %format!("{e:#}"), "failed to sweep stale temporary refs"),
  }
}

/// A ref name no other operation of this or another Branch Deck process uses
pub(crate) fn unique_temp_ref() -> String {
  static COUNTER: AtomicU32 = AtomicU32::new(0);
  let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
  format!("{TEMP_REF_PREFIX}{}-{nanos}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// Append the start of a commit on `temp_ref` with mark `mark`: `(author, committer)`, the encoding and the message
fn push_commit_header(stream: &mut Vec<u8>, temp_ref: &str, mark: usize, (author, committer): (&str, &str), encoding: Option<&str>, message: &[u8]) {
  stream.extend_from_slice(format!("commit {temp_ref}\nmark :{mark}\nauthor {author}\ncommitter {committer}\n").as_bytes());
  if let Some(encoding) = encoding {
    stream.extend_from_slice(format!("encoding {encoding}\n").as_bytes());
  }
  push_data(stream, message);
}

/// The message as `commit-tree -m` stores it (`strbuf_complete_line`): a newline at the end unless it has one; an empty
/// message stays empty
pub(crate) fn complete_line(message: &str) -> Vec<u8> {
  let mut bytes = Vec::with_capacity(message.len() + 1);
  bytes.extend_from_slice(message.as_bytes());
  if bytes.last().is_some_and(|&last| last != b'\n') {
    bytes.push(b'\n');
  }
  bytes
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
  write_commits_with_notes(git_executor, repo_path, commits, None, &[], None)
}

/// `write_commits` that also attaches notes to the new commits, in one notes commit of the same `fast-import` run, and
/// writes `stores`, commits of stores by the current user, after them, each on a temporary ref of its own. Both are
/// committed by `committer` (`current_committer`), read only when there are any and the caller passes `None`.
/// The notes ref moves even if the caller later abandons the commits; a note on an unreachable commit is harmless.
#[instrument(skip(git_executor, commits, notes, stores, committer), fields(count = commits.len(), stores = stores.len()))]
pub fn write_commits_with_notes(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  commits: &[NewCommit<'_>],
  notes: Option<&NotesOfWritten<'_>>,
  stores: &[StoreCommit<'_>],
  committer: Option<&str>,
) -> Result<WrittenCommits> {
  let temp_ref = unique_temp_ref();
  let mut stream = Vec::with_capacity(commits.len() * 256);
  for (index, commit) in commits.iter().enumerate() {
    if commit.parent.is_none() {
      // without `from`, a commit continues the ref's previous commit; reset it to write a root commit
      stream.extend_from_slice(format!("reset {temp_ref}\n\n").as_bytes());
    }
    push_commit_header(&mut stream, &temp_ref, index + 1, (commit.author, commit.committer), commit.encoding, commit.message);
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
  let committer = match committer {
    _ if !has_notes && stores.is_empty() => None,
    Some(committer) => Some(committer.to_string()),
    None => Some(current_committer(git_executor, repo_path)?),
  };
  if let (Some(notes), Some(committer)) = (notes.filter(|notes| !notes.notes.is_empty()), &committer) {
    if let Some((index, _)) = notes.notes.iter().find(|(index, _)| *index >= commits.len()) {
      return Err(anyhow!("note for commit {index} of a batch of {}", commits.len()));
    }
    let targets = notes.notes.iter().map(|(index, content)| (format!(":{}", index + 1), content.as_str()));
    let from = ref_exists(git_executor, repo_path, notes.notes_ref).then(|| format!("{}^0", notes.notes_ref));
    push_notes_commit(&mut stream, notes.notes_ref, None, from.as_deref(), committer, targets);
  }
  let store_temp_refs: Vec<String> = (0..stores.len()).map(|index| format!("{temp_ref}-store-{index}")).collect();
  if let Some(committer) = &committer {
    for (index, (store, store_temp_ref)) in stores.iter().zip(&store_temp_refs).enumerate() {
      push_store_commit(&mut stream, store_temp_ref, commits.len() + 1 + index, committer, store, |index| index + 1);
    }
  }
  stream.extend_from_slice(b"done\n");

  let output = git_executor
    .cmd(&["fast-import", "--quiet", "--done", "--date-format=raw", "--signed-commits=strip"], repo_path)
    .stdin(&stream)
    .bytes()?;
  let mut ids: Vec<String> = String::from_utf8(output)?.lines().map(str::to_string).collect();
  let store_commits = ids.split_off(ids.len().saturating_sub(stores.len()));
  if ids.len() != commits.len() || store_commits.len() != stores.len() || store_commits.iter().any(String::is_empty) {
    return Err(anyhow!("fast-import wrote {} of {} commits", ids.len(), commits.len()));
  }
  Ok(WrittenCommits {
    ids,
    store_commits,
    temp_ref,
    store_temp_refs,
  })
}

/// Write the commits of `stores` with one `git fast-import` run and move their refs in one transaction, with `verify`
/// (`(full ref, expected id)`, e.g. the checked out branch at the HEAD the stores were computed for) in the same
/// transaction. Returns the new commits of the stores, in input order; on a failure nothing moves
#[instrument(skip(git_executor, stores), fields(stores = stores.len()))]
pub fn write_store_updates(git_executor: &GitCommandExecutor, repo_path: &str, stores: &[StoreUpdate<'_>], verify: Option<(&str, &str)>, operation: &str) -> Result<Vec<String>> {
  let commits: Vec<StoreCommit<'_>> = stores.iter().map(|store| store.commit.clone()).collect();
  let written = write_commits_with_notes(git_executor, repo_path, &[], None, &commits, None)?;
  let mut transaction = RefTransaction::new();
  if let Some((full_ref, expected)) = verify {
    transaction.verify(full_ref, expected);
  }
  for (store, new_commit) in stores.iter().zip(&written.store_commits) {
    store.add_to(&mut transaction, new_commit);
  }
  written.delete_temp_refs(&mut transaction);
  if let Err(e) = transaction.commit(git_executor, repo_path, operation) {
    written.discard(git_executor, repo_path);
    return Err(e);
  }
  Ok(written.store_commits)
}

/// Notes written by `write_store_commits` on a temporary ref; the notes ref itself moves only with the caller's
/// transaction, with compare-and-swap on `previous`
#[derive(Debug)]
pub struct StoreNotes<'a> {
  pub notes_ref: &'a str,
  /// Where `notes_ref` points now, which the notes commit continues; `None` when it does not exist
  pub previous: Option<&'a str>,
  /// `(commit id, note content)`
  pub notes: &'a [(String, String)],
}

impl StoreNotes<'_> {
  /// Move the notes ref to `new_commit` in `transaction`, only if it is still at `previous` (or does not exist)
  pub fn add_to(&self, transaction: &mut RefTransaction, new_commit: &str) {
    match self.previous {
      Some(previous) => transaction.update(self.notes_ref, new_commit, Some(previous)),
      None => transaction.create(self.notes_ref, new_commit),
    };
  }
}

/// The commits of `write_store_commits`, reachable from temporary refs until the caller's transaction deletes them
#[derive(Debug)]
pub struct WrittenStore {
  /// In input order
  pub ids: Vec<String>,
  pub temp_ref: String,
  /// The notes commit and its temporary ref
  pub notes: Option<(String, String)>,
}

impl WrittenStore {
  /// Delete the temporary refs in `transaction`, the one that moves the real refs, each with compare-and-swap on the
  /// commit written last on it (`temp_ref` exists only with commits)
  pub fn delete_temp_refs(&self, transaction: &mut RefTransaction) {
    if let Some(tip) = self.ids.last() {
      transaction.delete(&self.temp_ref, Some(tip));
    }
    if let Some((commit, notes_temp_ref)) = &self.notes {
      transaction.delete(notes_temp_ref, Some(commit));
    }
  }

  /// Delete the temporary refs of abandoned commits; the unreferenced objects are left to `git gc`
  pub fn discard(&self, git_executor: &GitCommandExecutor, repo_path: &str) {
    discard_temp_refs(git_executor, repo_path, &self.temp_ref, |transaction| self.delete_temp_refs(transaction));
  }
}

/// Write commits of a store with one `git fast-import` run, on a temporary ref (a `NewParent::Written` parent is an
/// index into `commits`), and `notes`, one notes commit on its own temporary ref, by `committer` (`Name <email> <seconds>
/// <tz>`; `None` for the current user). No real ref moves: the caller moves them in one transaction, which deletes the
/// temporary refs too
#[instrument(skip(git_executor, commits, notes), fields(count = commits.len()))]
pub fn write_store_commits(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  commits: &[StoreCommit<'_>],
  notes: Option<&StoreNotes<'_>>,
  committer: Option<&str>,
) -> Result<WrittenStore> {
  let committer = match committer {
    Some(committer) => committer.to_string(),
    None => current_committer(git_executor, repo_path)?,
  };
  let temp_ref = unique_temp_ref();
  let mut stream = Vec::with_capacity(commits.len() * 512);
  for (index, commit) in commits.iter().enumerate() {
    if let Some(NewParent::Written(parent_index)) = commit.parent
      && parent_index >= index
    {
      return Err(anyhow!("store commit {index} refers to a parent written after it ({parent_index})"));
    }
    push_store_commit(&mut stream, &temp_ref, index + 1, &committer, commit, |index| index + 1);
  }
  let notes_temp_ref = notes.map(|_| format!("{temp_ref}-notes"));
  if let (Some(notes), Some(notes_temp_ref)) = (notes, &notes_temp_ref) {
    let mark = commits.len() + 1;
    if notes.previous.is_none() {
      stream.extend_from_slice(format!("reset {notes_temp_ref}\n\n").as_bytes());
    }
    let targets = notes.notes.iter().map(|(target, content)| (target.as_str(), content.as_str()));
    push_notes_commit(&mut stream, notes_temp_ref, Some(mark), notes.previous, &committer, targets);
    stream.extend_from_slice(format!("get-mark :{mark}\n").as_bytes());
  }
  stream.extend_from_slice(b"done\n");

  let output = git_executor
    .cmd(&["fast-import", "--quiet", "--done", "--date-format=raw"], repo_path)
    .stdin(&stream)
    .bytes()?;
  let mut ids: Vec<String> = String::from_utf8(output)?.lines().map(str::to_string).collect();
  let notes_commit = if notes.is_some() { ids.pop() } else { None };
  if ids.len() != commits.len() || notes_commit.as_ref().is_none_or(String::is_empty) != notes.is_none() {
    return Err(anyhow!("fast-import wrote {} of {} store commits", ids.len(), commits.len()));
  }
  Ok(WrittenStore {
    ids,
    temp_ref,
    notes: notes_commit.zip(notes_temp_ref),
  })
}

/// A change of one path of the tree in `write_edited_commit`
#[derive(Debug, Clone, Copy)]
pub enum TreeEdit<'a> {
  /// The file gets `content`, stored as a new blob
  Content {
    path: &'a str,
    mode: &'a str,
    content: &'a [u8],
  },
  /// The file is an existing blob
  Blob {
    path: &'a str,
    mode: &'a str,
    id: &'a str,
  },
  Delete {
    path: &'a str,
  },
}

/// A commit whose tree is `tree` with `edits` applied
#[derive(Debug, Clone, Copy)]
pub struct EditedCommit<'a> {
  pub tree: &'a str,
  pub edits: &'a [TreeEdit<'a>],
  pub parent: &'a str,
  /// `Name <email> <seconds> <tz>`, as in `RawCommit`
  pub author: &'a str,
  pub committer: &'a str,
  pub encoding: Option<&'a str>,
  pub message: &'a [u8],
}

/// What `write_edited_commit` wrote; the commit is reachable from `temp_ref` until the caller deletes it
#[derive(Debug)]
pub struct WrittenEdit {
  pub commit: String,
  pub tree: String,
  /// The tree of the parent: the commit is empty when it equals `tree`
  pub parent_tree: String,
  pub temp_ref: String,
}

impl WrittenEdit {
  /// Delete `temp_ref` in `transaction`, with compare-and-swap on the commit
  pub fn delete_temp_ref(&self, transaction: &mut RefTransaction) {
    transaction.delete(&self.temp_ref, Some(&self.commit));
  }
}

/// Write `commit` with one `git fast-import` run: the blobs, the tree and the commit, and with `ls` the trees of the
/// commit and of its parent, so the caller sees an empty result without another process
#[instrument(skip(git_executor, commit), fields(edits = commit.edits.len()))]
pub fn write_edited_commit(git_executor: &GitCommandExecutor, repo_path: &str, commit: &EditedCommit<'_>) -> Result<WrittenEdit> {
  let temp_ref = unique_temp_ref();
  let mut stream = Vec::with_capacity(1024);
  push_commit_header(&mut stream, &temp_ref, 1, (commit.author, commit.committer), commit.encoding, commit.message);
  stream.extend_from_slice(format!("from {}\nM 040000 {} \"\"\n", commit.parent, commit.tree).as_bytes());
  for edit in commit.edits {
    match edit {
      TreeEdit::Content { path, mode, content } => {
        stream.extend_from_slice(format!("M {mode} inline {}\n", quote_path(path)).as_bytes());
        push_data(&mut stream, content);
      }
      TreeEdit::Blob { path, mode, id } => stream.extend_from_slice(format!("M {mode} {id} {}\n", quote_path(path)).as_bytes()),
      TreeEdit::Delete { path } => stream.extend_from_slice(format!("D {}\n", quote_path(path)).as_bytes()),
    }
  }
  stream.extend_from_slice(format!("\nls :1 \"\"\nls {} \"\"\nget-mark :1\ndone\n", commit.parent).as_bytes());

  let output = git_executor
    .cmd(&["fast-import", "--quiet", "--done", "--date-format=raw", "--signed-commits=strip"], repo_path)
    .stdin(&stream)
    .bytes()?;
  let output = String::from_utf8(output)?;
  // `040000 tree <id>\t` per `ls`, then the commit id
  let mut lines = output.lines();
  let mut tree_of = || lines.next().and_then(|line| line.split_whitespace().nth(2)).map(str::to_string);
  let (Some(tree), Some(parent_tree)) = (tree_of(), tree_of()) else {
    return Err(anyhow!("Unexpected fast-import output: {output}"));
  };
  let commit_id = output
    .lines()
    .nth(2)
    .filter(|id| !id.is_empty())
    .ok_or_else(|| anyhow!("fast-import wrote no commit: {output}"))?;
  Ok(WrittenEdit {
    commit: commit_id.to_string(),
    tree,
    parent_tree,
    temp_ref,
  })
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
    let from = ref_exists(git_executor, repo_path, notes_ref).then(|| format!("{notes_ref}^0"));
    push_notes_commit(
      &mut stream,
      notes_ref,
      None,
      from.as_deref(),
      &committer,
      notes.iter().map(|(target, content)| (target.as_str(), content.as_str())),
    );
  }
  stream.extend_from_slice(b"done\n");
  git_executor.cmd(&["fast-import", "--quiet", "--done"], repo_path).stdin(&stream).bytes()?;
  Ok(())
}

/// Append a notes commit on `notes_ref` that continues `from` and adds or overwrites a note per `(target, content)`; a
/// target is a commit id or a mark of the same stream. Like `git notes add`, each note is stored with a trailing newline.
fn push_notes_commit<'a, T: AsRef<str>>(
  stream: &mut Vec<u8>,
  notes_ref: &str,
  mark: Option<usize>,
  from: Option<&str>,
  committer: &str,
  notes: impl Iterator<Item = (T, &'a str)>,
) {
  stream.extend_from_slice(format!("commit {notes_ref}\n").as_bytes());
  if let Some(mark) = mark {
    stream.extend_from_slice(format!("mark :{mark}\n").as_bytes());
  }
  stream.extend_from_slice(format!("committer {committer}\n").as_bytes());
  push_data(stream, b"Notes added by Branch Deck\n");
  if let Some(from) = from {
    stream.extend_from_slice(format!("from {from}\n").as_bytes());
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

#[cfg(test)]
#[path = "commit_writer_test.rs"]
mod commit_writer_test;
