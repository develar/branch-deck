//! The files that the git attributes mark generated: `linguist-generated` (the convention of GitHub, which folds them in
//! a pull request too) or `-diff` (which `binary` implies)

use crate::names::is_lock_file;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::attributes::read_attributes;
use std::collections::HashSet;
use tracing::instrument;

/// The paths that the attributes of the tree-ish `source` mark `linguist-generated` or `-diff`, from one
/// `check-attr --source=<source>` for all of them; none without paths
#[instrument(skip(git, paths))]
pub fn attribute_generated_paths<'a>(git: &GitCommandExecutor, repository: &str, source: &str, paths: impl IntoIterator<Item = &'a str>) -> Result<HashSet<String>> {
  let mut generated = HashSet::new();
  for attribute in read_attributes(git, repository, Some(source), &["linguist-generated", "diff"], paths)? {
    if matches!(
      (attribute.attribute.as_str(), attribute.value.as_str()),
      ("linguist-generated", "set" | "true") | ("diff", "unset")
    ) {
      generated.insert(attribute.path);
    }
  }
  Ok(generated)
}

/// The paths that are lock files, or that the attributes of `source` mark generated: what `bd commits` and
/// `bd status` leave out. The lock files need no process
#[instrument(skip(git, paths))]
pub fn generated_paths<'a>(git: &GitCommandExecutor, repository: &str, source: &str, paths: impl IntoIterator<Item = &'a str>) -> Result<HashSet<String>> {
  let (locks, others): (Vec<&str>, Vec<&str>) = paths.into_iter().partition(|path| is_lock_file(path));
  let mut generated = attribute_generated_paths(git, repository, source, others)?;
  generated.extend(locks.into_iter().map(str::to_string));
  Ok(generated)
}
