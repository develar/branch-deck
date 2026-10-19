//! The git attributes of paths, read-only: one `check-attr --stdin -z` for all of them.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::instrument;

/// One attribute of one path, as `check-attr` prints it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathAttribute {
  pub path: String,
  pub attribute: String,
  /// `set`, `unset`, `unspecified`, or the value it is set to
  pub value: String,
}

/// The values of `attributes` of each of `paths`, as the attribute files of the tree-ish `source` (`HEAD`) set them, not
/// those of the working tree; without `source`, as git finds them for a merge (the working tree and the index, or
/// `attr.tree`). One `git check-attr [--source=<source>] --stdin -z`; none without paths
#[instrument(skip(git, attributes, paths))]
pub fn read_attributes<'a>(
  git: &GitCommandExecutor,
  repository_path: &str,
  source: Option<&str>,
  attributes: &[&str],
  paths: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<PathAttribute>> {
  let input = paths.into_iter().fold(String::new(), |mut input, path| {
    input.push_str(path);
    input.push('\0');
    input
  });
  if input.is_empty() {
    return Ok(Vec::new());
  }
  let source = source.map(|source| format!("--source={source}"));
  let mut args = vec!["check-attr"];
  args.extend(source.as_deref());
  args.extend_from_slice(&["--stdin", "-z"]);
  args.extend_from_slice(attributes);
  let output = git.cmd(&args, repository_path).stdin(input.as_bytes()).bytes()?;
  // `<path>` NUL `<attribute>` NUL `<value>` NUL
  let fields: Vec<&[u8]> = output.split(|&b| b == 0).collect();
  Ok(
    fields
      .as_chunks::<3>()
      .0
      .iter()
      .map(|[path, attribute, value]| PathAttribute {
        path: String::from_utf8_lossy(path).into_owned(),
        attribute: String::from_utf8_lossy(attribute).into_owned(),
        value: String::from_utf8_lossy(value).into_owned(),
      })
      .collect(),
  )
}

#[cfg(test)]
#[path = "attributes_test.rs"]
mod attributes_test;
