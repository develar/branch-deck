//! The check record of a worktree (`bd lease pick`, spec/cli/lease.spec.md): `bd-checks.jsonl` in the git dir of the
//! worktree, where a build or test tool appends one JSON line per green run with the tree that run saw (`bt` of the
//! IntelliJ monorepo writes it). The tree is `HEAD^{tree}` of a clean worktree, else the tree of the worktree with its
//! local changes, so a check before the commit counts for the commit of the same files. bd only reads the file.
//!
//! ```json
//! {"schema":1,"tree":"<id>","head":"<id>","tool":"bt","command":"build","patterns":["//plugins/air/..."],"exit":0,"at":1793108792000}
//! ```

use crate::lease_clean::read_parsed;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::paths::changed_path_names;
use git_ops::revs::head_and_tree;
use serde::Deserialize;
use std::path::Path;

/// The file of the records, in the git dir of a worktree (`git rev-parse --git-path bd-checks.jsonl`)
pub const CHECKS_FILE: &str = "bd-checks.jsonl";

/// The schema of a record this reader knows
const SCHEMA: u32 = 1;

/// A green run of a check tool on one tree
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CheckRecord {
  pub tree: String,
  /// HEAD when the run ended
  #[serde(default)]
  pub head: String,
  /// `bt`
  #[serde(default)]
  pub tool: String,
  /// `build` or `test`
  #[serde(default)]
  pub command: String,
  /// What the run built or tested: target patterns, or the arguments of a run on another machine
  #[serde(default)]
  pub patterns: Vec<String>,
  /// The machine of a run on another machine
  #[serde(default)]
  pub host: Option<String>,
  /// When the run ended, ms since the epoch
  #[serde(default)]
  pub at: u64,
}

impl CheckRecord {
  /// The command line of the run as a person types it: `bt build //plugins/air/...`, `bt test --lane fast --host studio`
  pub fn describe(&self) -> String {
    let mut words: Vec<&str> = [self.tool.as_str(), self.command.as_str()].into_iter().filter(|word| !word.is_empty()).collect();
    words.extend(self.patterns.iter().map(String::as_str));
    if let Some(host) = &self.host {
      words.extend(["--host", host.as_str()]);
    }
    words.join(" ")
  }
}

/// One line of the file, before the checks of [`read_check_records`]
#[derive(Deserialize)]
struct RecordLine {
  schema: u32,
  exit: i64,
  #[serde(flatten)]
  record: CheckRecord,
}

/// The green records of the text of a `bd-checks.jsonl`, oldest first: a line of another schema, of a run that did not
/// exit 0, or that does not parse is left out
pub fn parse_check_records(text: &str) -> Vec<CheckRecord> {
  text
    .lines()
    .filter_map(|line| serde_json::from_str::<RecordLine>(line).ok())
    .filter(|line| line.schema == SCHEMA && line.exit == 0)
    .map(|line| line.record)
    .collect()
}

/// The green records of the worktree whose git dir is `git_dir`; none without the file
pub fn read_check_records(git_dir: &Path) -> Result<Vec<CheckRecord>> {
  read_parsed(&git_dir.join(CHECKS_FILE), |text| Ok(parse_check_records(text))).map(Option::unwrap_or_default)
}

/// Whether a green run recorded the tree of HEAD
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
  /// A record names the tree
  Ok,
  /// Records exist, none names the tree
  Stale,
  /// No record
  Missing,
}

/// The check of the tree of a worktree's HEAD
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeCheck {
  pub head: String,
  pub tree: String,
  pub state: CheckState,
  /// `Ok`: the newest record of the tree; `Stale`: the newest record
  pub record: Option<CheckRecord>,
  /// `Stale`: the paths that differ between the tree of the record and the tree of HEAD, which no check saw as they are
  /// in HEAD; `None` when git knows the tree of the record no longer
  pub changed_paths: Option<Vec<String>>,
}

/// How many changed paths a refusal and an answer name; the rest is a count
pub const NAMED_PATHS: usize = 10;

/// The check of HEAD of the worktree at `top_level` against the records in its git dir `git_dir`: one `rev-parse`, and
/// for a stale tree one `diff-tree`
pub fn check_tree(git_executor: &GitCommandExecutor, top_level: &str, git_dir: &Path) -> Result<TreeCheck> {
  let (head, tree) = head_and_tree(git_executor, top_level)?;
  let records = read_check_records(git_dir)?;
  if let Some(record) = records.iter().rev().find(|record| record.tree == tree) {
    return Ok(TreeCheck {
      head,
      tree,
      state: CheckState::Ok,
      record: Some(record.clone()),
      changed_paths: None,
    });
  }
  let Some(last) = records.last() else {
    return Ok(TreeCheck {
      head,
      tree,
      state: CheckState::Missing,
      record: None,
      changed_paths: None,
    });
  };
  let changed_paths = changed_path_names(git_executor, top_level, &last.tree, &tree)?;
  Ok(TreeCheck {
    head,
    tree,
    state: CheckState::Stale,
    record: Some(last.clone()),
    changed_paths,
  })
}
