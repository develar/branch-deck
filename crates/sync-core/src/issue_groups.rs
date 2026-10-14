//! Issue groups: the issue key -> branch name map that puts the commits of several issues into one branch without a
//! history rewrite (spec/branch/issue-groups.spec.md). The store is the ref `refs/branch-deck/issue-groups`: a commit
//! whose tree holds the one file `issue-groups`, sorted `KEY<TAB>name` lines. Every write is a new commit on the
//! previous one, so its history shows each change (git keeps no reflog outside `refs/heads`). Only a regroup writes it, under the repository lock,
//! in the transaction that moves HEAD (`regroup.rs`); a sync only reads it.

use crate::commit_grouper::GroupingInputs;
use crate::move_commits::validate_branch_name;
use crate::stack::stacks_enabled;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_writer::{WrittenCommits, write_file_commit};
use git_ops::ref_transaction::RefTransaction;
use std::collections::BTreeMap;
use std::fmt::Write;
use sync_utils::issue_pattern::find_issue_number;
use tracing::instrument;

pub const ISSUE_GROUPS_REF: &str = "refs/branch-deck/issue-groups";
/// The file of the store's tree
pub const ISSUE_GROUPS_FILE: &str = "issue-groups";

/// The issue groups as read from the store
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IssueGroups {
  /// The commit `ISSUE_GROUPS_REF` points at; `None` when the ref does not exist
  pub commit: Option<String>,
  /// Issue key -> branch name: the valid entries, the only ones the grouping uses
  pub entries: BTreeMap<String, String>,
  /// The lines that are not used
  pub problems: Vec<IssueGroupProblem>,
}

/// A line of the store that the grouping does not use
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueGroupProblem {
  pub line: String,
  pub message: String,
}

/// The ref of the store moved since it was read: another regroup or `git update-ref` changed it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueGroupsMoved {
  /// `None`: the ref did not exist
  pub expected: Option<String>,
  pub actual: Option<String>,
}

impl std::fmt::Display for IssueGroupsMoved {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let show = |id: &Option<String>| id.as_deref().map_or_else(|| "nothing".to_string(), |id| id.get(..8).unwrap_or(id).to_string());
    write!(
      f,
      "The issue groups ({ISSUE_GROUPS_REF}) moved: expected {}, found {}",
      show(&self.expected),
      show(&self.actual)
    )
  }
}

impl std::error::Error for IssueGroupsMoved {}

/// The stack setting and the issue groups: what decides the groups besides the subjects. Two git processes
#[instrument(skip(git_executor))]
pub fn read_grouping_inputs(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<GroupingInputs> {
  Ok(GroupingInputs {
    stacks: stacks_enabled(git_executor, repository_path)?,
    issue_groups: read_issue_groups(git_executor, repository_path)?,
  })
}

/// Read the store with one `cat-file --batch`: the commit of the ref and its file. A missing ref is an empty map
#[instrument(skip(git_executor))]
pub fn read_issue_groups(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<IssueGroups> {
  let input = format!("{ISSUE_GROUPS_REF}\n{ISSUE_GROUPS_REF}:{ISSUE_GROUPS_FILE}\n");
  let output = git_executor.execute_command_with_input_raw(&["cat-file", "--batch"], repository_path, &input)?;
  let mut rest = output.as_slice();
  let mut objects = Vec::with_capacity(2);
  for _ in 0..2 {
    let header_end = rest.iter().position(|&b| b == b'\n').ok_or_else(|| anyhow!("truncated cat-file output"))?;
    let header = std::str::from_utf8(&rest[..header_end])?;
    let mut fields = header.split(' ');
    if let (Some(id), Some(_), Some(size)) = (fields.next(), fields.next(), fields.next()) {
      let size: usize = size.parse()?;
      let content = rest.get(header_end + 1..header_end + 1 + size).ok_or_else(|| anyhow!("truncated cat-file output"))?;
      objects.push(Some((id.to_string(), content.to_vec())));
      rest = rest.get(header_end + 2 + size..).unwrap_or_default();
    } else {
      // `<name> missing`
      objects.push(None);
      rest = &rest[header_end + 1..];
    }
  }
  let commit = objects[0].as_ref().map(|(id, _)| id.clone());
  let Some(commit) = commit else {
    return Ok(IssueGroups::default());
  };
  let text = objects[1].as_ref().map_or_default(|(_, content)| String::from_utf8_lossy(content).into_owned());
  let (entries, problems) = parse_issue_groups(&text);
  Ok(IssueGroups {
    commit: Some(commit),
    entries,
    problems,
  })
}

/// The valid entries of the file and its other lines as problems: a key that is not an issue key, a name that is not a
/// branch name without `/`, a key listed twice (neither entry is used). Blank lines are skipped
pub fn parse_issue_groups(text: &str) -> (BTreeMap<String, String>, Vec<IssueGroupProblem>) {
  let mut parsed: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
  let mut problems = Vec::new();
  for line in text.lines().filter(|line| !line.trim().is_empty()) {
    let problem = |message: String| IssueGroupProblem { line: line.to_string(), message };
    let Some((key, name)) = line.split_once('\t') else {
      problems.push(problem("Not a `KEY<TAB>name` line".to_string()));
      continue;
    };
    if let Err(message) = validate_issue_key(key).and_then(|()| validate_issue_group_name(name)) {
      problems.push(problem(message));
      continue;
    }
    parsed.entry(key.to_string()).or_default().push((line.to_string(), name.to_string()));
  }
  let mut entries = BTreeMap::new();
  for (key, lines) in parsed {
    if let [(_, name)] = lines.as_slice() {
      entries.insert(key, name.clone());
    } else {
      problems.extend(lines.into_iter().map(|(line, _)| IssueGroupProblem {
        line,
        message: format!("{key} is listed more than once"),
      }));
    }
  }
  (entries, problems)
}

/// The file for `entries`: one `KEY<TAB>name` line each, sorted by key
pub fn format_issue_groups(entries: &BTreeMap<String, String>) -> String {
  let mut text = String::new();
  for (key, name) in entries {
    let _ = writeln!(text, "{key}\t{name}");
  }
  text
}

/// An issue key as the grouper reads one, e.g. `ABC-123`
pub fn validate_issue_key(key: &str) -> Result<(), String> {
  if find_issue_number(key) == Some(key) {
    Ok(())
  } else {
    Err(format!("{key:?} is not an issue key (ABC-123)"))
  }
}

/// The name of an issue group: a branch name without `/`, so that it is a leaf with stacked branches on and off
pub fn validate_issue_group_name(name: &str) -> Result<(), String> {
  validate_branch_name(name).map_err(|e| format!("{e:#}"))?;
  if name.contains('/') {
    return Err(format!("The issue group name {name:?} must be a branch name without `/`"));
  }
  Ok(())
}

/// A new commit of the store with `entries`, on `previous` (the commit read), written with one `fast-import`, and the
/// transaction that moves the ref to it with compare-and-swap on `previous` and deletes the temporary ref. The caller
/// commits the transaction, or adds it to the one that moves HEAD; on failure it discards `written`
pub fn prepare_issue_groups_write(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  previous: Option<&str>,
  entries: &BTreeMap<String, String>,
) -> Result<(WrittenCommits, RefTransaction)> {
  let written = write_file_commit(
    git_executor,
    repository_path,
    ISSUE_GROUPS_FILE,
    format_issue_groups(entries).as_bytes(),
    previous,
    "Issue groups of Branch Deck",
  )?;
  let new_commit = written.ids.first().ok_or_else(|| anyhow!("no issue groups commit was written"))?;
  let mut transaction = RefTransaction::new();
  match previous {
    Some(previous) => transaction.update(ISSUE_GROUPS_REF, new_commit, Some(previous)),
    None => transaction.create(ISSUE_GROUPS_REF, new_commit),
  };
  transaction.delete(&written.temp_ref, None);
  Ok((written, transaction))
}

/// The commit `ISSUE_GROUPS_REF` points at now; `None` when it does not exist
pub fn read_issue_groups_commit(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<String>> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", "--end-of-options", ISSUE_GROUPS_REF], repository_path)?;
  Ok((exit_code == 0).then(|| output.trim().to_string()).filter(|id| !id.is_empty()))
}

#[cfg(test)]
#[path = "issue_groups_test.rs"]
mod issue_groups_test;
