use crate::commit_list::{self, Commit};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use tracing::{debug, instrument};

/// `git log` pretty format for one commit record, parsed by `commit_list::parse_single_commit`.
/// Fields are separated by `%x1f`, records are terminated by `%x1e`.
pub const COMMIT_RECORD_FORMAT: &str = "%H%x1f%B%x1f%an%x1f%ae%x1f%at%x1f%ct%x1f%P%x1f%T%x1f%aI%x1f%cI%x1f%N%x1e";

/// Split `git log` output in `COMMIT_RECORD_FORMAT` into non-empty records
fn commit_records(output: &str) -> impl Iterator<Item = &str> {
  output.split('\u{1e}').map(str::trim).filter(|record| !record.is_empty())
}

/// Prefetch commit metadata for a range and return as a map keyed by commit hash.
/// Uses a delimiter-based pretty format and the shared parser for robustness.
#[instrument(skip(git_executor))]
pub fn prefetch_commit_infos_map(git_executor: &GitCommandExecutor, repo_path: &str, range: &str) -> Result<HashMap<String, Commit>> {
  let pretty_format = format!("--pretty=format:{COMMIT_RECORD_FORMAT}");
  let args = vec!["--no-pager", "log", "--first-parent", "--reverse", pretty_format.as_str(), range];

  let output = git_executor.execute_command(&args, repo_path)?;
  let mut map = HashMap::new();
  for record in commit_records(&output) {
    match commit_list::parse_single_commit(record) {
      Ok(commit) => {
        map.insert(commit.id.clone(), commit);
      }
      Err(e) => {
        debug!(error = %e, "failed to parse commit during prefetch");
      }
    }
  }
  Ok(map)
}

/// Fetch metadata for specific commits with one `git log --no-walk`, keyed by full commit hash.
/// The ids go on stdin, so their number is not bounded by the command line length (about 780 ids on Windows).
/// Duplicate ids are fine. An empty `ids` returns an empty map without running git; an unknown id is an error.
#[instrument(skip(git_executor, ids), fields(count = ids.len()))]
pub fn get_commits_info(git_executor: &GitCommandExecutor, repo_path: &str, ids: &[&str]) -> Result<HashMap<String, Commit>> {
  if ids.is_empty() {
    return Ok(HashMap::new());
  }

  let mut input = String::with_capacity(ids.len() * 41);
  for id in ids {
    // a line starting with `-` would be read as an option such as `--all`, one with `^` as an exclusion
    if id.is_empty() || id.starts_with(['-', '^']) || id.contains(char::is_whitespace) {
      return Err(anyhow!("invalid commit id '{id}'"));
    }
    input.push_str(id);
    input.push('\n');
  }

  let pretty_format = format!("--pretty=format:{COMMIT_RECORD_FORMAT}");
  let args = ["--no-pager", "log", "--no-walk=unsorted", "--stdin", pretty_format.as_str()];
  let output = git_executor.execute_command_with_input(&args, repo_path, &input)?;
  commit_records(&output)
    .map(|record| commit_list::parse_single_commit(record).map(|commit| (commit.id.clone(), commit)))
    .collect()
}

/// The commit of a `get_commits_info` result that `id` (a full or a short hash) names
pub fn find_commit<'a>(commits: &'a HashMap<String, Commit>, id: &str) -> Option<&'a Commit> {
  commits.get(id).or_else(|| commits.values().find(|commit| commit.id.starts_with(id)))
}

/// Fetch metadata for a single commit (any revision that resolves to a commit)
#[instrument(skip(git_executor))]
pub fn get_commit_info(git_executor: &GitCommandExecutor, repo_path: &str, id: &str) -> Result<Commit> {
  get_commits_info(git_executor, repo_path, &[id])?
    .into_values()
    .next()
    .ok_or_else(|| anyhow!("commit {id} not found"))
}

/// The empty tree of a SHA-1 repository
const EMPTY_TREE_SHA1: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
/// The empty tree of a SHA-256 repository (`git init --object-format=sha256`)
const EMPTY_TREE_SHA256: &str = "6ef19b41225c5369f1c104d45d8d85efa9b057b53b14b4b9b939dd74decc5321";

/// The id of the empty tree in the hash algorithm of `object_id`, a full id of the same repository, without a process:
/// git knows the empty tree in every repository, so the object need not be written
#[must_use]
pub const fn empty_tree(object_id: &str) -> &'static str {
  if object_id.len() == 64 { EMPTY_TREE_SHA256 } else { EMPTY_TREE_SHA1 }
}
