//! The line maps of Branch Deck: the issue groups (`issue_groups.rs`) and the stack parents (`stack_parents.rs`). Each
//! is a ref of its own, to a commit whose tree holds one file of sorted `key<TAB>value` lines, and every write is a new
//! commit on the previous one. A ref of its own, because a store commit replaces its whole tree (`deleteall`): a second
//! file next to another map would be lost by every write of a version that does not know it.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::{BatchEntry, NO_LIMIT, read_objects};
use git_ops::object_texts::into_text;
use std::collections::BTreeMap;
use std::fmt::Write;

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

/// The valid entries of a map file and its other lines as problems: a line without a tab (`shape` names the expected
/// one, e.g. `KEY<TAB>name`), a pair `validate` refuses, a key listed twice (neither entry is used). Blank lines are
/// skipped
pub fn parse_store_lines(text: &str, shape: &str, validate: impl Fn(&str, &str) -> Result<(), String>) -> (BTreeMap<String, String>, Vec<StoreLineProblem>) {
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

/// The first 8 characters of an object id, or "nothing" for a ref that did not exist
pub(crate) fn short_or_nothing(id: Option<&str>) -> String {
  id.map_or_else(|| "nothing".to_string(), |id| id.get(..8).unwrap_or(id).to_string())
}
