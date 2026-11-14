//! The git of the rule of `generated_files`: the attributes that mark generated files (one `check-attr` for all the
//! paths), why each file of a round is generated, and the generated files an update brought together, which only their
//! generator merges (docs/design/git-operations.md, "Update project", "Resolve session").

use crate::attributes::read_attributes;
use crate::cat_file_batch::{BatchEntry, for_each_object};
use crate::commit_files::MAX_BLOB_BYTES;
use crate::paths::{ChangedPath, changed_paths};
use anyhow::Result;
use generated_files::attributes::{GENERATED_ATTRIBUTES, marks_generated};
use generated_files::generated_by::{GeneratedBy, generated_by};
use generated_files::names::{generated_name, is_lock_file};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use tracing::instrument;

/// The mode of a submodule: no file to generate
const GITLINK_MODE: &str = "160000";

/// The paths that the attributes of the tree-ish `source` mark `linguist-generated` or `-diff`, from one
/// `check-attr --source=<source>` for all of them; none without paths
#[instrument(skip(git, paths))]
pub fn attribute_generated_paths<'a>(git: &GitCommandExecutor, repository: &str, source: &str, paths: impl IntoIterator<Item = &'a str>) -> Result<HashSet<String>> {
  marked_paths(git, repository, Some(source), paths)
}

/// The paths that are lock files, or that the attributes of `source` mark generated: what `brd commits` and
/// `brd status` leave out. The lock files need no process
#[instrument(skip(git, paths))]
pub fn generated_paths<'a>(git: &GitCommandExecutor, repository: &str, source: &str, paths: impl IntoIterator<Item = &'a str>) -> Result<HashSet<String>> {
  let (locks, others): (Vec<&str>, Vec<&str>) = paths.into_iter().partition(|path| is_lock_file(path));
  let mut generated = attribute_generated_paths(git, repository, source, others)?;
  generated.extend(locks.into_iter().map(str::to_string));
  Ok(generated)
}

/// The paths of `paths` whose attributes mark them generated, as the tree-ish `source` sets them; without `source`, as
/// git finds them for a merge (`read_attributes`). One `check-attr`, none without paths
fn marked_paths<'a>(git: &GitCommandExecutor, repository: &str, source: Option<&str>, paths: impl IntoIterator<Item = &'a str>) -> Result<HashSet<String>> {
  Ok(
    read_attributes(git, repository, source, &GENERATED_ATTRIBUTES, paths)?
      .into_iter()
      .filter(|attribute| marks_generated(&attribute.attribute, &attribute.value))
      .map(|attribute| attribute.path)
      .collect(),
  )
}

/// Why each of `files` (a path and the versions of it that the caller read, as stored) is generated, by place
/// (`generated_by::generated_by`): its name, else its attributes as git finds them for a merge, else the header of one
/// of its versions. One `check-attr` for the paths their name does not call generated, none without such paths
#[instrument(skip_all, fields(files = files.len()))]
pub fn generated_of(git: &GitCommandExecutor, repository: &str, files: &[(&str, Vec<&[u8]>)]) -> Result<Vec<Option<GeneratedBy>>> {
  let marked = marked_paths(git, repository, None, files.iter().map(|(path, _)| *path).filter(|path| generated_name(path).is_none()))?;
  Ok(
    files
      .iter()
      .map(|(path, texts)| generated_by(path, marked.contains(*path), texts.iter().copied()))
      .collect(),
  )
}

/// A generated file of an update, and why it is generated
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
  pub path: String,
  pub generated_by: GeneratedBy,
}

/// The generated files that the local commits (`base..head`) and the upstream (`base..upstream_head`) both changed, each to
/// another blob, and that `new_head`, the update of `head` onto `upstream_head`, keeps: whatever wrote the file in
/// `new_head` (git's merge, the words, a side taken, an edit), its text is no run of its generator on the merged inputs,
/// so the generator runs again. In the order of the paths. Two `diff-tree`; with files changed on both sides one
/// `check-attr --source=<new_head>` for those their name does not call generated and one `cat-file --batch` of them in
/// `new_head`, for their header; none for a fast-forward (`base` is `head`)
#[instrument(skip(git))]
pub fn merged_generated_files(git: &GitCommandExecutor, repository: &str, base: &str, head: &str, upstream_head: &str, new_head: &str) -> Result<Vec<GeneratedFile>> {
  if base == head {
    return Ok(Vec::new());
  }
  let local = changed_paths(git, repository, base, head)?;
  if local.is_empty() {
    return Ok(Vec::new());
  }
  let upstream: HashMap<Vec<u8>, ChangedPath> = changed_paths(git, repository, base, upstream_head)?
    .into_iter()
    .map(|change| (change.path.clone(), change))
    .collect();
  // both sides keep the file, each another blob
  let kept = |id: &str| id.bytes().any(|byte| byte != b'0');
  let both: Vec<String> = local
    .into_iter()
    .filter(|change| {
      upstream
        .get(&change.path)
        .is_some_and(|theirs| kept(&change.new_id) && kept(&theirs.new_id) && change.new_id != theirs.new_id && change.old_mode != GITLINK_MODE)
    })
    .filter_map(|change| String::from_utf8(change.path).ok())
    .collect();
  if both.is_empty() {
    return Ok(Vec::new());
  }
  let marked = marked_paths(
    git,
    repository,
    Some(new_head),
    both.iter().map(String::as_str).filter(|path| generated_name(path).is_none()),
  )?;
  // the file in `new_head`: whether it keeps it, and its header
  let names: Vec<String> = both.iter().map(|path| format!("{new_head}:{path}")).collect();
  let mut texts: Vec<Option<Vec<u8>>> = vec![None; names.len()];
  for_each_object(git, repository, &names, MAX_BLOB_BYTES, |index, entry| {
    match entry {
      BatchEntry::Object { content, .. } => texts[index] = Some(content),
      // over the limit: kept, its header unread
      BatchEntry::TooLarge { .. } => texts[index] = Some(Vec::new()),
      BatchEntry::Missing => {}
    }
    Ok(())
  })?;
  Ok(
    both
      .into_iter()
      .zip(texts)
      .filter_map(|(path, text)| {
        let text = text?;
        let generated_by = generated_by(&path, marked.contains(&path), [text.as_slice()])?;
        Some(GeneratedFile { path, generated_by })
      })
      .collect(),
  )
}

#[cfg(test)]
#[path = "generated_test.rs"]
mod generated_test;
