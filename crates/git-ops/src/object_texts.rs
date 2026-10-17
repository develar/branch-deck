//! The bytes and texts of many objects, by id or by `<rev>:<path>`, from one `cat-file --batch` (`cat_file_batch`)

use crate::cat_file_batch::{BatchEntry, NO_LIMIT, read_objects};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::fmt::Write as _;
use tracing::instrument;

/// The bytes git checks for a NUL to call a file binary
const BINARY_CHECK_BYTES: usize = 8000;

/// Whether git calls `content` binary: a NUL in its first 8000 bytes
pub fn is_binary(content: &[u8]) -> bool {
  memchr::memchr(0, &content[..content.len().min(BINARY_CHECK_BYTES)]).is_some()
}

/// `bytes` as text, copied only when they are not UTF-8 (lossily then)
pub fn into_text(bytes: Vec<u8>) -> String {
  String::from_utf8(bytes).unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

/// The bytes of each of `names` (an object id, or `<rev>:<path>`), in this order, from one `cat-file --batch -Z`;
/// `None` for a missing object, one that is no blob, a binary blob (`is_binary`) and one larger than `max_bytes`, whose
/// bytes are skipped unread
#[instrument(skip(git, names), fields(names = names.len()))]
pub fn read_blob_bytes(git: &GitCommandExecutor, repository_path: &str, names: &[String], max_bytes: usize) -> Result<Vec<Option<Vec<u8>>>> {
  read_objects(git, repository_path, names, max_bytes, |entry| match entry {
    BatchEntry::Object { kind: "blob", content, .. } if !is_binary(&content) => Some(content),
    _ => None,
  })
}

/// `read_blob_bytes` as text, invalid UTF-8 replaced
pub fn read_texts(git: &GitCommandExecutor, repository_path: &str, names: &[String], max_bytes: usize) -> Result<Vec<Option<String>>> {
  Ok(
    read_blob_bytes(git, repository_path, names, max_bytes)?
      .into_iter()
      .map(|bytes| bytes.map(into_text))
      .collect(),
  )
}

/// One entry of a tree object
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
  /// Octal, as git stores it: `100644`, `100755`, `120000`, `40000`, `160000`
  pub mode: String,
  pub name: String,
  pub id: String,
}

/// The entries of each of `names` (a tree id, or `<rev>:<directory>`), in this order, from one `cat-file --batch -Z`;
/// `None` for a missing object and one that is no tree
#[instrument(skip(git, names), fields(names = names.len()))]
pub fn read_trees(git: &GitCommandExecutor, repository_path: &str, names: &[String]) -> Result<Vec<Option<Vec<TreeEntry>>>> {
  read_objects(git, repository_path, names, NO_LIMIT, |entry| match entry {
    BatchEntry::Object { id, kind: "tree", content } => Some(tree_entries(&content, id.len() / 2)),
    _ => None,
  })
}

/// The entries of a raw tree: `<mode> <name>\0<id bytes>`, the id `hash_len` bytes long
fn tree_entries(mut content: &[u8], hash_len: usize) -> Vec<TreeEntry> {
  let mut entries = Vec::new();
  while let Some(space) = memchr::memchr(b' ', content) {
    let Some(nul) = memchr::memchr(0, &content[space..]).map(|pos| space + pos) else {
      break;
    };
    let Some(id) = content.get(nul + 1..nul + 1 + hash_len) else { break };
    entries.push(TreeEntry {
      mode: String::from_utf8_lossy(&content[..space]).into_owned(),
      name: String::from_utf8_lossy(&content[space + 1..nul]).into_owned(),
      id: id.iter().fold(String::with_capacity(hash_len * 2), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
      }),
    });
    content = &content[nul + 1 + hash_len..];
  }
  entries
}
