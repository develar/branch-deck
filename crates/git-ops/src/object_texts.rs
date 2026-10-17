//! The bytes and texts of many objects, by id or by `<rev>:<path>`, from one `cat-file --batch`

use crate::cat_file_batch::{BatchObject, parse_batch_output};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::fmt::Write as _;
use tracing::instrument;

/// The bytes git checks for a NUL to call a file binary
const BINARY_CHECK_BYTES: usize = 8000;

/// The bytes of each of `names` (an object id, or `<rev>:<path>`), in this order, from one `cat-file --batch -Z`;
/// `None` for a missing object, one that is no blob, a binary blob (a NUL in its first 8000 bytes) and one larger
/// than `max_bytes`
#[instrument(skip(git, names), fields(names = names.len()))]
pub fn read_blob_bytes(git: &GitCommandExecutor, repository_path: &str, names: &[String], max_bytes: usize) -> Result<Vec<Option<Vec<u8>>>> {
  read_objects(git, repository_path, names, |object| {
    if object.kind != "blob" || object.content.len() > max_bytes {
      return None;
    }
    let binary = memchr::memchr(0, &object.content[..object.content.len().min(BINARY_CHECK_BYTES)]).is_some();
    (!binary).then(|| object.content.to_vec())
  })
}

/// What `keep` makes of each of `names`, in this order, from one `cat-file --batch -Z`; `None` for a missing object
fn read_objects<T>(git: &GitCommandExecutor, repository_path: &str, names: &[String], keep: impl Fn(BatchObject<'_>) -> Option<T>) -> Result<Vec<Option<T>>> {
  if names.is_empty() {
    return Ok(Vec::new());
  }
  let input = names.iter().fold(String::new(), |mut input, name| {
    input.push_str(name);
    input.push('\0');
    input
  });
  let output = git.execute_command_with_input_raw(&["cat-file", "--batch", "--buffer", "-Z"], repository_path, &input)?;
  Ok(parse_batch_output(&output, names.len())?.into_iter().map(|object| object.and_then(&keep)).collect())
}

/// `read_blob_bytes` as text, invalid UTF-8 replaced
pub fn read_texts(git: &GitCommandExecutor, repository_path: &str, names: &[String], max_bytes: usize) -> Result<Vec<Option<String>>> {
  Ok(
    read_blob_bytes(git, repository_path, names, max_bytes)?
      .into_iter()
      .map(|bytes| bytes.map(|bytes| String::from_utf8(bytes).unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())))
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
  read_objects(git, repository_path, names, |object| {
    (object.kind == "tree").then(|| tree_entries(object.content, object.id.len() / 2))
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
