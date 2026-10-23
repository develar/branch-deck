//! The line maps of Branch Deck: the issue groups (`issue_groups.rs`), the stack parents (`stack_parents.rs`) and the
//! parked branches (`parked.rs`), each a `LineStore`. Each is a ref of its own, to a commit whose tree holds one file of
//! sorted `key<TAB>value` lines, and every write is a new commit on the previous one. A ref of its own, because a store commit replaces its whole tree
//! (`deleteall`): a second file next to another map would be lost by every write of a version that does not know it.

use crate::repository_lock::{RepositoryLocked, lock_repository};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::{BatchEntry, NO_LIMIT, read_objects};
use git_ops::commit_writer::{StoreCommit, StoreUpdate, write_store_updates};
use git_ops::object_texts::into_text;
use git_ops::revs::short_id;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::time::Duration;
use tracing::{debug, info, instrument, warn};

/// A line map: the ref of its store, the file of the store's tree, the message of its commits and what its lines are
#[derive(Debug, Clone, Copy)]
pub struct LineStore {
  pub store_ref: &'static str,
  /// The one file of the store's tree
  pub file: &'static str,
  /// What the map is called in messages, e.g. `issue groups`
  pub name: &'static str,
  /// The message of a store commit of a regroup
  pub message: &'static str,
  /// The line the map expects, e.g. `KEY<TAB>name`: the problem of a line without a tab names it
  pub shape: &'static str,
  /// Whether the key and the value of a line make an entry; else the problem of the line
  pub validate: fn(&str, &str) -> Result<(), String>,
}

/// A line map as read from its store
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreLines {
  /// The commit the ref of the store points at; `None` when the ref does not exist
  pub commit: Option<String>,
  /// Key -> value: the valid entries, the only ones the grouping uses
  pub entries: BTreeMap<String, String>,
  /// The lines that are not used
  pub problems: Vec<StoreLineProblem>,
}

/// The ref of a store moved since it was read: another regroup, a sync or `git update-ref` changed it
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("The {name} ({store_ref}) moved: expected {}, found {}", short_or_nothing(.expected.as_deref()), short_or_nothing(.actual.as_deref()))]
pub struct StoreMoved {
  /// `LineStore::name`
  pub name: &'static str,
  pub store_ref: &'static str,
  /// `None`: the ref did not exist
  pub expected: Option<String>,
  pub actual: Option<String>,
}

impl LineStore {
  /// The valid entries of the file and its other lines as problems: a line without a tab, a pair `validate` refuses, a
  /// key listed twice (neither entry is used). Blank lines are skipped
  pub fn parse(&self, text: &str) -> (BTreeMap<String, String>, Vec<StoreLineProblem>) {
    parse_store_lines(text, self.shape, self.validate)
  }

  /// Read the store with one `cat-file --batch`: the commit of the ref and its file. A missing ref is an empty map
  pub fn read(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<StoreLines> {
    Ok(read_line_stores(git_executor, repository_path, &[self])?.pop().unwrap_or_default())
  }

  /// The map of a `(commit, text)` that `read_store_files` read, an empty one for a ref that does not exist
  fn lines_of(&self, file: Option<(String, String)>) -> StoreLines {
    let Some((commit, text)) = file else {
      return StoreLines::default();
    };
    let (entries, problems) = self.parse(&text);
    StoreLines {
      commit: Some(commit),
      entries,
      problems,
    }
  }

  /// The new commit of the store with the file `content` on `previous` (the commit read), for `write_store_updates` or
  /// the `fast-import` run of a reword: its ref moves with compare-and-swap on `previous`
  pub fn update<'a>(&self, previous: Option<&'a str>, content: &'a str) -> StoreUpdate<'a> {
    self.update_with(previous, content, self.message)
  }

  /// `update` with another commit message
  pub fn update_with<'a>(&self, previous: Option<&'a str>, content: &'a str, message: &'a str) -> StoreUpdate<'a> {
    StoreUpdate {
      store_ref: self.store_ref,
      commit: StoreCommit::file(self.file, content.as_bytes(), previous, message),
    }
  }

  /// The error of the ref of the store found at `actual` where a write expected `expected`
  pub const fn moved(&self, expected: Option<String>, actual: Option<String>) -> StoreMoved {
    StoreMoved {
      name: self.name,
      store_ref: self.store_ref,
      expected,
      actual,
    }
  }
}

/// The maps of `stores`, in their order, with one `cat-file --batch`
pub fn read_line_stores(git_executor: &GitCommandExecutor, repository_path: &str, stores: &[&LineStore]) -> Result<Vec<StoreLines>> {
  let names: Vec<(&str, &str)> = stores.iter().map(|store| (store.store_ref, store.file)).collect();
  let files = read_store_files(git_executor, repository_path, &names)?;
  Ok(stores.iter().zip(files).map(|(store, file)| store.lines_of(file)).collect())
}

/// A line of a map that is not used. A write keeps it as it is, so a line of a newer format survives, unless the write
/// sets or removes its key
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreLineProblem {
  pub line: String,
  /// The text before the tab, if the line has one
  pub key: Option<String>,
  pub message: String,
}

/// The commit and the file of each `(ref, file)` of `stores`, with one `cat-file --batch`; `None` for a ref that does not
/// exist, an empty text for a commit without the file
pub(crate) fn read_store_files(git_executor: &GitCommandExecutor, repository_path: &str, stores: &[(&str, &str)]) -> Result<Vec<Option<(String, String)>>> {
  let names: Vec<String> = stores
    .iter()
    .flat_map(|(store_ref, file)| [(*store_ref).to_string(), format!("{store_ref}:{file}")])
    .collect();
  let objects = read_objects(git_executor, repository_path, &names, NO_LIMIT, |entry| match entry {
    BatchEntry::Object { id, content, .. } => Some((id.to_string(), content)),
    BatchEntry::TooLarge { .. } | BatchEntry::Missing => None,
  })?;
  let mut objects = objects.into_iter();
  let mut files = Vec::with_capacity(stores.len());
  while let (Some(commit), Some(file)) = (objects.next(), objects.next()) {
    files.push(commit.map(|(commit, _)| (commit, file.map_or_default(|(_, content)| into_text(content)))));
  }
  Ok(files)
}

/// `LineStore::parse` of `shape` and `validate`
fn parse_store_lines(text: &str, shape: &str, validate: impl Fn(&str, &str) -> Result<(), String>) -> (BTreeMap<String, String>, Vec<StoreLineProblem>) {
  let mut parsed: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
  let mut problems = Vec::new();
  for line in text.lines().filter(|line| !line.trim().is_empty()) {
    let problem = |message: String| StoreLineProblem {
      line: line.to_string(),
      key: line.split_once('\t').map(|(key, _)| key.to_string()),
      message,
    };
    let Some((key, value)) = line.split_once('\t') else {
      problems.push(problem(format!("Not a `{shape}` line")));
      continue;
    };
    if let Err(message) = validate(key, value) {
      problems.push(problem(message));
      continue;
    }
    parsed.entry(key.to_string()).or_default().push((line.to_string(), value.to_string()));
  }
  let mut entries = BTreeMap::new();
  for (key, lines) in parsed {
    if let [(_, value)] = lines.as_slice() {
      entries.insert(key, value.clone());
    } else {
      problems.extend(lines.into_iter().map(|(line, _)| StoreLineProblem {
        line,
        key: Some(key.clone()),
        message: format!("{key} is listed more than once"),
      }));
    }
  }
  (entries, problems)
}

/// The file for `entries`, one `key<TAB>value` line each, and the `kept` lines as they are, all sorted
pub fn format_store_lines(entries: &BTreeMap<String, String>, kept: &[String]) -> String {
  let mut lines: Vec<String> = entries.iter().map(|(key, value)| format!("{key}\t{value}")).chain(kept.iter().cloned()).collect();
  lines.sort();
  let mut text = String::new();
  for line in lines {
    let _ = writeln!(text, "{line}");
  }
  text
}

/// The commit `store_ref` points at now and its parents, with one `for-each-ref`; `None` when it does not exist
pub fn read_store_head(git_executor: &GitCommandExecutor, repository_path: &str, store_ref: &str) -> Result<Option<(String, Vec<String>)>> {
  let output = git_executor.execute_command(&["for-each-ref", "--format=%(objectname) %(parent)", store_ref], repository_path)?;
  Ok(output.lines().next().and_then(|line| {
    let mut ids = line.split_whitespace().map(str::to_string);
    ids.next().map(|commit| (commit, ids.collect()))
  }))
}

/// The short form of an object id, or "nothing" for a ref that did not exist
fn short_or_nothing(id: Option<&str>) -> String {
  id.map_or("nothing", short_id).to_string()
}

/// The map of `store` that the undo of the write `(previous, new)` leaves when only drops of a sync (commits with
/// `drop_message`) followed it, up to `actual`: the map of `previous` without the keys that those drops removed. `None`
/// when `actual` is not such a chain of drops on `new`, or `new` is not a write on `previous`. One `log` and one
/// `cat-file --batch`
pub(crate) fn undo_over_drops(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  store: &LineStore,
  drop_message: &str,
  (previous, new): (Option<&str>, &str),
  actual: &str,
) -> Result<Option<String>> {
  // newest first: `<id> <parents>` and the subject, from `actual` down to the parent of `new`
  let output = git_executor.execute_command(
    &["log", "--first-parent", "--format=%H %P%x1f%s", "-n", "100", "--end-of-options", actual, "--"],
    repository_path,
  )?;
  let mut expected = actual;
  let mut chain_ends_on_new = false;
  for record in output.lines() {
    let Some((ids, subject)) = record.split_once('\u{1f}') else { return Ok(None) };
    let mut ids = ids.split(' ');
    let (Some(id), parent) = (ids.next(), ids.next()) else { return Ok(None) };
    if id != expected || ids.next().is_some() {
      return Ok(None);
    }
    if id == new {
      // the token names a write on `previous`
      if parent != previous {
        return Ok(None);
      }
      chain_ends_on_new = true;
      break;
    }
    match parent {
      Some(parent) if subject == drop_message => expected = parent,
      _ => return Ok(None),
    }
  }
  if !chain_ends_on_new {
    return Ok(None);
  }
  let mut stores = Vec::with_capacity(3);
  stores.extend(previous.map(|previous| (previous, store.file)));
  stores.extend([(new, store.file), (actual, store.file)]);
  let mut files = read_store_files(git_executor, repository_path, &stores)?
    .into_iter()
    .map(|file| file.map_or_default(|(_, text)| text));
  let previous_text = if previous.is_some() { files.next().unwrap_or_default() } else { String::new() };
  let (new_entries, _) = store.parse(&files.next().unwrap_or_default());
  let (actual_entries, _) = store.parse(&files.next().unwrap_or_default());
  let (mut entries, problems) = store.parse(&previous_text);
  for dropped in new_entries.keys().filter(|key| !actual_entries.contains_key(*key)) {
    entries.remove(dropped);
  }
  let kept: Vec<String> = problems.into_iter().map(|problem| problem.line).collect();
  Ok(Some(format_store_lines(&entries, &kept)))
}

/// Drop the lines of `keys` from `store` in a commit with `drop_message`, which an undo of a regroup passes over
/// (`undo_over_drops`). Reads the store again (one `cat-file --batch`) and writes only when a line goes, with
/// compare-and-swap; best effort, because the next sync tries again: a failure is only logged. Only while the
/// repository lock is free, which it takes without waiting: a regroup holds it from its read of the stores to its
/// transaction, and a drop between them would fail the regroup (`StacksMoved`, `ParkedMoved`); a held lock (another
/// operation of this process too) leaves the drop to the next sync
#[instrument(skip(git_executor, store, keys), fields(store = store.name, count = keys.len()))]
pub fn drop_store_lines(git_executor: &GitCommandExecutor, repository_path: &str, store: &LineStore, drop_message: &str, keys: &[&str]) {
  if keys.is_empty() {
    return;
  }
  let _lock = match lock_repository(git_executor, repository_path, Duration::ZERO) {
    Ok(lock) => lock,
    Err(e) if e.downcast_ref::<RepositoryLocked>().is_some() => {
      debug!(store = store.name, "the repository lock is held: the next sync drops the lines");
      return;
    }
    Err(e) => {
      warn!(store = store.name, error = %format!("{e:#}"), "Failed to drop the lines of integrated branches");
      return;
    }
  };
  let result = (|| -> Result<Vec<String>> {
    let read = store.read(git_executor, repository_path)?;
    let (Some(commit), mut entries) = (read.commit, read.entries) else {
      return Ok(Vec::new());
    };
    let dropped: Vec<String> = keys.iter().filter(|key| entries.remove(**key).is_some()).map(|key| (*key).to_string()).collect();
    if dropped.is_empty() {
      return Ok(dropped);
    }
    let kept: Vec<String> = read.problems.into_iter().map(|problem| problem.line).collect();
    let content = format_store_lines(&entries, &kept);
    write_store_updates(
      git_executor,
      repository_path,
      &[store.update_with(Some(&commit), &content, drop_message)],
      None,
      &format!("{} of integrated branches", store.name),
    )?;
    Ok(dropped)
  })();
  match result {
    Ok(dropped) if !dropped.is_empty() => info!(store = store.name, ?dropped, "Dropped the lines of integrated branches"),
    Ok(_) => {}
    Err(e) => warn!(store = store.name, error = %format!("{e:#}"), "Failed to drop the lines of integrated branches"),
  }
}
