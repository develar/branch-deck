//! Line counts per file: of the diffs of tree pairs from one `git diff-tree --stdin --numstat -z`, or of a cached commit
//! diff.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::commit_diff::FileChange;
use std::collections::HashMap;
use tracing::instrument;

/// One `--numstat -z` entry
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumstatEntry {
  /// The path after the change
  pub path: String,
  /// The path before a rename
  pub old_path: Option<String>,
  /// `None` for a binary file (`-` in `--numstat`)
  pub additions: Option<u32>,
  pub deletions: Option<u32>,
}

impl From<&FileChange> for NumstatEntry {
  fn from(change: &FileChange) -> Self {
    Self {
      path: change.path.clone(),
      old_path: change.old_path.clone(),
      additions: (!change.binary).then_some(change.additions),
      deletions: (!change.binary).then_some(change.deletions),
    }
  }
}

/// Diff options that make the counts independent of the user's diff config
const NUMSTAT_OPTIONS: [&str; 6] = ["--numstat", "-z", "-M", "--no-ext-diff", "--no-textconv", "--no-relative"];

/// The entries of the diff from `base` to each `tip`, by tip, in one process. A pair without changes has no entry
#[instrument(skip(git, tips), fields(tips = tips.len()))]
pub fn read_tree_numstats(git: &GitCommandExecutor, repository: &str, base: &str, tips: &[&str]) -> Result<HashMap<String, Vec<NumstatEntry>>> {
  if tips.is_empty() {
    return Ok(HashMap::new());
  }
  // `<commit> <parent>` compares the commit with that parent
  let input = lines(tips.iter().map(|tip| format!("{tip} {base}")));
  let mut args = vec!["diff-tree", "--stdin", "-r"];
  args.extend(NUMSTAT_OPTIONS);
  let output = git.execute_command_with_input_raw(&args, repository, &input)?;
  Ok(parse_numstats(&output))
}

fn lines<T: AsRef<str>>(items: impl Iterator<Item = T>) -> String {
  items.fold(String::new(), |mut input, item| {
    input.push_str(item.as_ref());
    input.push('\n');
    input
  })
}

/// Parse NUL-separated fields: a commit header (`\x1e<id>` from `log --format=%x1e%H`, or a bare id from `diff-tree`),
/// then entries `<added>\t<deleted>\t<path>`, or `<added>\t<deleted>\t` NUL `<old>` NUL `<new>` for a rename. A header
/// has no tab; an entry always has two
pub fn parse_numstats(output: &[u8]) -> HashMap<String, Vec<NumstatEntry>> {
  let mut result: HashMap<String, Vec<NumstatEntry>> = HashMap::new();
  let mut fields = output.split(|&b| b == 0).map(|field| String::from_utf8_lossy(field).trim_start_matches('\n').to_string());
  let mut current: Option<String> = None;
  while let Some(field) = fields.next() {
    let mut parts = field.splitn(3, '\t');
    let (Some(added), Some(deleted), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
      let id = field.trim_start_matches('\x1e').trim();
      if !id.is_empty() {
        current = Some(id.to_string());
        result.entry(id.to_string()).or_default();
      }
      continue;
    };
    let (path, old_path) = if path.is_empty() {
      let old_path = fields.next().unwrap_or_default();
      (fields.next().unwrap_or_default(), Some(old_path))
    } else {
      (path.to_string(), None)
    };
    if let Some(id) = &current {
      result.entry(id.clone()).or_default().push(NumstatEntry {
        path,
        old_path,
        additions: added.parse().ok(),
        deletions: deleted.parse().ok(),
      });
    }
  }
  result
}
