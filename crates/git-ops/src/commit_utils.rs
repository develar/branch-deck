use crate::commit_list::{self, Commit};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use tracing::{debug, instrument};

/// `git log` pretty format for one commit record, parsed by `commit_list::parse_single_commit`.
/// Fields are separated by `%x1f`, records are terminated by `%x1e`.
pub const COMMIT_RECORD_FORMAT: &str = "%H%x1f%B%x1f%an%x1f%ae%x1f%at%x1f%ct%x1f%P%x1f%T%x1f%N%x1e";

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
/// Duplicate ids are fine. An empty `ids` returns an empty map without running git; an unknown id is an error.
#[instrument(skip(git_executor))]
pub fn get_commits_info(git_executor: &GitCommandExecutor, repo_path: &str, ids: &[&str]) -> Result<HashMap<String, Commit>> {
  if ids.is_empty() {
    return Ok(HashMap::new());
  }

  let pretty_format = format!("--pretty=format:{COMMIT_RECORD_FORMAT}");
  let mut args = vec!["--no-pager", "log", "--no-walk=unsorted", pretty_format.as_str(), "--end-of-options"];
  args.extend_from_slice(ids);

  let output = git_executor.execute_command(&args, repo_path)?;
  commit_records(&output)
    .map(|record| commit_list::parse_single_commit(record).map(|commit| (commit.id.clone(), commit)))
    .collect()
}

/// Fetch metadata for a single commit (any revision that resolves to a commit)
#[instrument(skip(git_executor))]
pub fn get_commit_info(git_executor: &GitCommandExecutor, repo_path: &str, id: &str) -> Result<Commit> {
  get_commits_info(git_executor, repo_path, &[id])?
    .into_values()
    .next()
    .ok_or_else(|| anyhow!("commit {id} not found"))
}

/// Create a commit from a tree using metadata from an existing Commit object.
/// Allows overriding parent and message while preserving author/committer info.
#[instrument(skip(git_executor, commit))]
pub fn create_commit_with_metadata(git_executor: &GitCommandExecutor, repo_path: &str, tree_id: &str, parent_id: Option<&str>, commit: &Commit, message: &str) -> Result<String> {
  let mut args = vec!["commit-tree", tree_id];

  if let Some(parent) = parent_id.or(commit.parent_id.as_deref()) {
    args.push("-p");
    args.push(parent);
  }

  args.push("-m");
  args.push(message);

  let author_date = commit.author_timestamp.to_string();
  let committer_date = commit.committer_timestamp.to_string();

  let env_vars = vec![
    ("GIT_AUTHOR_NAME", commit.author_name.as_str()),
    ("GIT_AUTHOR_EMAIL", commit.author_email.as_str()),
    ("GIT_AUTHOR_DATE", &author_date),
    ("GIT_COMMITTER_NAME", commit.author_name.as_str()),
    ("GIT_COMMITTER_EMAIL", commit.author_email.as_str()),
    ("GIT_COMMITTER_DATE", &committer_date),
  ];

  let output = git_executor.execute_command_with_env(&args, repo_path, &env_vars)?;
  Ok(output.trim().to_string())
}
