//! Commits by id from one `log --no-walk --stdin` (`get_commits_info`; `find_commit` takes an abbreviated id too), and
//! the ids git computes without a process: the empty tree and the id of a blob.

use crate::commit_list::{self, Commit};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::collections::HashMap;
use std::fmt::Write;
use tracing::instrument;

/// `git log` pretty format for one commit record, parsed by `commit_list::parse_single_commit`.
/// Fields are separated by `%x1f`, records are terminated by `%x1e`.
pub const COMMIT_RECORD_FORMAT: &str = "%H%x1f%B%x1f%an%x1f%ae%x1f%at%x1f%ct%x1f%P%x1f%T%x1f%aI%x1f%cI%x1f%N%x1e";

/// Split `git log` output in `COMMIT_RECORD_FORMAT` into non-empty records
fn commit_records(output: &str) -> impl Iterator<Item = &str> {
  output.split('\u{1e}').map(str::trim).filter(|record| !record.is_empty())
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
  let args = ["log", "--no-walk=unsorted", "--stdin", pretty_format.as_str()];
  let output = git_executor.cmd(&args, repo_path).stdin(input.as_bytes()).text()?;
  commit_records(&output)
    .map(|record| commit_list::parse_single_commit(record).map(|commit| (commit.id.clone(), commit)))
    .collect()
}

/// The commit of a `get_commits_info` result that `id` (a full or a short hash) names
pub fn find_commit<'a>(commits: &'a HashMap<String, Commit>, id: &str) -> Option<&'a Commit> {
  commits.get(id).or_else(|| commits.values().find(|commit| commit.id.starts_with(id)))
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

/// The id of a blob of `content` in the hash algorithm of `object_id`, a full id of the same repository, as
/// `hash-object --stdin` computes it (no filters), without a process: the hash of `blob <length>\0<content>`
#[must_use]
pub fn blob_id(object_id: &str, content: &[u8]) -> String {
  object_id_of(object_id, "blob", content)
}

/// The id of an object of `kind` (`blob`, `tree`) with `content` in the hash algorithm of `object_id`, a full id of the
/// same repository: the hash of `<kind> <length>\0<content>`
#[must_use]
pub fn object_id_of(object_id: &str, kind: &str, content: &[u8]) -> String {
  fn hex<D: Digest>(header: &[u8], content: &[u8]) -> String {
    let digest = D::new().chain_update(header).chain_update(content).finalize();
    digest.iter().fold(String::with_capacity(digest.len() * 2), |mut hex, byte| {
      let _ = write!(hex, "{byte:02x}");
      hex
    })
  }
  let header = format!("{kind} {}\0", content.len());
  if object_id.len() == 64 {
    hex::<Sha256>(header.as_bytes(), content)
  } else {
    hex::<Sha1>(header.as_bytes(), content)
  }
}
