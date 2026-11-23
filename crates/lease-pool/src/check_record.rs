//! The check record of a worktree (`brd lease pick` and `brd pick --from`, openspec/specs/cli/lease/spec.md, openspec/specs/cli/pick/spec.md):
//! `bd-checks.jsonl` in the git dir of the worktree, where a build or test tool appends one JSON line per green run with
//! the tree that run saw (`bt` of the IntelliJ monorepo writes it). The tree is `HEAD^{tree}` of a clean worktree, else
//! the tree of the worktree with its local changes, so a check before the commit counts for the commit of the same
//! files. brd only reads the file. A record of an older tree carries over a clean update onto the target branch
//! (`git_ops::carried_check`): HEAD is the checked tree merged onto a newer base, and the answer counts the commits of
//! the target that no check saw.
//!
//! ```json
//! {"schema":1,"tree":"<id>","head":"<id>","tool":"bt","command":"build","patterns":["//plugins/air/..."],"exit":0,"at":1793108792000}
//! ```

use crate::lease_clean::read_parsed;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::carried_check::carry_checked_tree;
use git_ops::paths::changed_path_names;
use git_ops::revs::{WorktreeHead, is_object_id, worktree_head};
use serde::Deserialize;
use std::path::Path;
use tracing::debug;

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
  /// No record names the tree, but the newest one carries over a clean update: its tree, merged onto the base of HEAD
  /// on the target branch, is the tree of HEAD
  CarriedOver,
  /// Records exist, none names the tree, and the newest does not carry over
  Stale,
  /// No record
  Missing,
}

impl CheckState {
  /// Whether the state passes the gate of `pick_requires_check`: a record names the tree or carries over onto it
  pub const fn passes(self) -> bool {
    matches!(self, Self::Ok | Self::CarriedOver)
  }
}

/// The check of the tree of a worktree's HEAD
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeCheck {
  pub head: String,
  pub tree: String,
  pub state: CheckState,
  /// `Ok`: the newest record of the tree; `CarriedOver`, `Stale`: the newest record
  pub record: Option<CheckRecord>,
  /// `Stale`: the paths that differ between the tree of the record and the tree of HEAD, which no check saw as they are
  /// in HEAD; `None` when git knows the tree of the record no longer
  pub changed_paths: Option<Vec<String>>,
  /// `CarriedOver`: the commits of the target branch between the base the record was built on and the base of HEAD,
  /// which no check saw
  pub untested_commits: Option<u64>,
}

/// How many changed paths a refusal and an answer name; the rest is a count
pub const NAMED_PATHS: usize = 10;

/// The check of HEAD of the worktree of `dir` against the records in its git dir: one `rev-parse` (the git dir, HEAD
/// and its tree). For a stale tree, with a target branch from `carry_onto` (a ref the worktree reads, `None` to carry
/// nothing, asked only then), whether the newest record carries over (`carried_over`: two `merge-base`, one
/// `merge-tree`, one `rev-list --count`); else one `diff-tree` of the paths changed since. A record whose HEAD or tree
/// git does not know carries nothing
pub fn check_tree(git_executor: &GitCommandExecutor, dir: &str, carry_onto: impl FnOnce() -> Result<Option<String>>) -> Result<TreeCheck> {
  let WorktreeHead { git_dir, head, tree } = worktree_head(git_executor, dir)?;
  let records = read_check_records(Path::new(&git_dir))?;
  let check = |state, record: Option<&CheckRecord>| TreeCheck {
    head: head.clone(),
    tree: tree.clone(),
    state,
    record: record.cloned(),
    changed_paths: None,
    untested_commits: None,
  };
  if let Some(record) = records.iter().rev().find(|record| record.tree == tree) {
    return Ok(check(CheckState::Ok, Some(record)));
  }
  let Some(last) = records.last() else {
    return Ok(check(CheckState::Missing, None));
  };
  if let Some(untested_commits) = carried_over(git_executor, dir, last, &head, &tree, carry_onto) {
    return Ok(TreeCheck {
      untested_commits: Some(untested_commits),
      ..check(CheckState::CarriedOver, Some(last))
    });
  }
  Ok(TreeCheck {
    changed_paths: changed_path_names(git_executor, dir, &last.tree, &tree)?,
    ..check(CheckState::Stale, Some(last))
  })
}

/// The commits of the target that `record` carries over, `None` when it carries nothing: no target, a record without
/// its HEAD, or a failure of git, which leaves the record stale
fn carried_over(git_executor: &GitCommandExecutor, dir: &str, record: &CheckRecord, head: &str, tree: &str, carry_onto: impl FnOnce() -> Result<Option<String>>) -> Option<u64> {
  if !is_object_id(&record.head) || !is_object_id(&record.tree) {
    return None;
  }
  let carried = carry_onto().and_then(|target| match target {
    Some(target) => carry_checked_tree(git_executor, dir, &record.head, &record.tree, head, tree, &target),
    None => Ok(None),
  });
  match carried {
    Ok(carried) => carried.map(|carried| carried.untested_commits),
    Err(error) => {
      debug!(error = format!("{error:#}"), "the record does not carry over");
      None
    }
  }
}
