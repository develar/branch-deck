//! The texts of many objects, by id or by `<rev>:<path>`, from one `cat-file --batch`

use crate::cat_file_batch::parse_batch_output;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::instrument;

/// The bytes git checks for a NUL to call a file binary
const BINARY_CHECK_BYTES: usize = 8000;

/// The text of each of `names` (an object id, or `<rev>:<path>`), in this order, from one `cat-file --batch -Z`;
/// `None` for a missing object, one that is no blob, a binary blob (a NUL in its first 8000 bytes) and one larger
/// than `max_bytes`
#[instrument(skip(git, names), fields(names = names.len()))]
pub fn read_texts(git: &GitCommandExecutor, repository_path: &str, names: &[String], max_bytes: usize) -> Result<Vec<Option<String>>> {
  if names.is_empty() {
    return Ok(Vec::new());
  }
  let input = names.iter().fold(String::new(), |mut input, name| {
    input.push_str(name);
    input.push('\0');
    input
  });
  let output = git.execute_command_with_input_raw(&["cat-file", "--batch", "--buffer", "-Z"], repository_path, &input)?;
  let objects = parse_batch_output(&output, names.len())?;
  Ok(
    objects
      .into_iter()
      .map(|object| {
        let object = object.filter(|object| object.kind == "blob" && object.content.len() <= max_bytes)?;
        let binary = memchr::memchr(0, &object.content[..object.content.len().min(BINARY_CHECK_BYTES)]).is_some();
        (!binary).then(|| String::from_utf8_lossy(object.content).into_owned())
      })
      .collect(),
  )
}
