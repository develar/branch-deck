//! The git of the rule of `generated_files`: the attributes that mark generated files (one `check-attr` for all the
//! paths), why each file of a round is generated, the generated files an update brought together, which only their
//! generator merges (docs/design/git-operations.md, "Update project", "Resolve session"), and the generated files the
//! words merged in the copies of a sync that the tips still hold ("Resolve by words").

use crate::attributes::read_attributes;
use crate::cat_file_batch::{BatchEntry, for_each_object_with_heads, object_ids_and_types};
use crate::commit_files::MAX_BLOB_BYTES;
use crate::conflict_analysis::list_missing_commits;
use crate::paths::changed_paths;
use anyhow::Result;
use domain_types::conflict::{GeneratedFile, MissingCommit};
use generated_files::attributes::{GENERATED_ATTRIBUTES, marks_generated};
use generated_files::generated_by::{GeneratedBy, generated_by};
use generated_files::header::HEADER_BYTES;
use generated_files::names::{generated_name, is_lock_file};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use tracing::instrument;

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

/// The generated files that an update brought together with the upstream's, and that `new_head`, the update onto
/// `upstream_head` (their merge base `base`), keeps: changed by the upstream since `base`, and by the local commits as
/// they replayed (`upstream_head..new_head`: a commit dropped because the upstream proves it has it changes nothing
/// there) or taken from `onto` in a round that kept its commit (`taken_onto`, whose change of the file the round left
/// out). Whatever wrote such a file in `new_head` (git's merge, the words, a side taken, an edit), its text is no run of
/// its generator on the merged inputs, so the generator runs again. In the order of the paths. Two `diff-tree`; with
/// files changed on both sides one `check-attr --source=<new_head>` for those their name does not call generated and one
/// `cat-file --batch` of them in `new_head`, for their header (the first `HEADER_BYTES` of a file over `MAX_BLOB_BYTES`);
/// none for a fast-forward
#[instrument(skip(git, taken_onto))]
pub fn merged_generated_files(git: &GitCommandExecutor, repository: &str, base: &str, upstream_head: &str, new_head: &str, taken_onto: &[String]) -> Result<Vec<GeneratedFile>> {
  if base == upstream_head {
    return Ok(Vec::new());
  }
  let mut local: BTreeSet<String> = taken_onto.iter().cloned().collect();
  if new_head != upstream_head {
    local.extend(
      changed_paths(git, repository, upstream_head, new_head)?
        .into_iter()
        .filter_map(|change| String::from_utf8(change.path).ok()),
    );
  }
  if local.is_empty() {
    return Ok(Vec::new());
  }
  let upstream: HashSet<Vec<u8>> = changed_paths(git, repository, base, upstream_head)?.into_iter().map(|change| change.path).collect();
  let both: Vec<String> = local.into_iter().filter(|path| upstream.contains(path.as_bytes())).collect();
  if both.is_empty() {
    return Ok(Vec::new());
  }
  let marked = marked_paths(
    git,
    repository,
    Some(new_head),
    both.iter().map(String::as_str).filter(|path| generated_name(path).is_none()),
  )?;
  // the file in `new_head`: whether it keeps it as a file, and its header
  let names: Vec<String> = both.iter().map(|path| format!("{new_head}:{path}")).collect();
  let mut texts: Vec<Option<Vec<u8>>> = vec![None; names.len()];
  for_each_object_with_heads(git, repository, &names, MAX_BLOB_BYTES, HEADER_BYTES, |index, entry| {
    match entry {
      BatchEntry::Object { kind: "blob", content, .. } => texts[index] = Some(content),
      BatchEntry::TooLarge { kind: "blob", head, .. } => texts[index] = Some(head),
      _ => {}
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

/// A generated file that the words merged in `copy`, the copy of the original `commit` (`WordResolutionSummary::generated`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyMerge {
  pub commit: String,
  pub copy: String,
  pub file: GeneratedFile,
}

impl CopyMerge {
  /// The one text form of a list for a record that `postcard` keeps (the build of a stack branch), which cannot read back
  /// the tagged `GeneratedBy`: compact JSON, empty for none
  pub fn encode(merges: &[Self]) -> String {
    if merges.is_empty() {
      String::new()
    } else {
      serde_json::to_string(merges).unwrap_or_default()
    }
  }

  /// The list `encode` wrote; empty for a text it did not write
  pub fn decode(text: &str) -> Vec<Self> {
    if text.is_empty() { Vec::new() } else { serde_json::from_str(text).unwrap_or_default() }
  }
}

/// Of each of `tips` (a tip and the merges of the copies of its history, the oldest copy first), the merges that the tip
/// still holds as the words wrote them, in the order of the paths: of each path the last copy that merged it, whose blob
/// the tip still has. A later commit that writes the file (a fixup) leaves it out, and so does one that deletes it. A
/// merge in the tip's own copy is held without a read; the others are one `cat-file --batch-check` for all the tips,
/// `<copy>:<path>` beside `<tip>:<path>`; none when every merge is in its tip
#[instrument(skip_all, fields(tips = tips.len()))]
pub fn held_word_merges(git: &GitCommandExecutor, repository: &str, tips: &[(String, Vec<CopyMerge>)]) -> Result<Vec<Vec<CopyMerge>>> {
  let latest: Vec<Vec<&CopyMerge>> = tips
    .iter()
    .map(|(_, merges)| {
      let mut by_path: HashMap<&str, &CopyMerge> = HashMap::new();
      for merge in merges {
        by_path.insert(&merge.file.path, merge);
      }
      let mut latest: Vec<&CopyMerge> = by_path.into_values().collect();
      latest.sort_by(|a, b| a.file.path.cmp(&b.file.path));
      latest
    })
    .collect();
  let mut names: Vec<String> = Vec::new();
  let mut read: Vec<(usize, usize)> = Vec::new();
  for (tip_index, ((tip, _), merges)) in tips.iter().zip(&latest).enumerate() {
    for (merge_index, merge) in merges.iter().enumerate() {
      if merge.copy != *tip {
        names.push(format!("{}:{}", merge.copy, merge.file.path));
        names.push(format!("{tip}:{}", merge.file.path));
        read.push((tip_index, merge_index));
      }
    }
  }
  let ids = object_ids_and_types(git, repository, &names)?;
  let mut dropped: HashSet<(usize, usize)> = HashSet::new();
  for (pair, place) in ids.chunks(2).zip(read) {
    let held = matches!(pair, [Some((copy, _)), Some((tip, _))] if copy == tip);
    if !held {
      dropped.insert(place);
    }
  }
  Ok(
    latest
      .into_iter()
      .enumerate()
      .map(|(tip_index, merges)| {
        merges
          .into_iter()
          .enumerate()
          .filter(|(merge_index, _)| !dropped.contains(&(tip_index, *merge_index)))
          .map(|(_, merge)| merge.clone())
          .collect()
      })
      .collect(),
  )
}

/// The commits of `head` that `baseline` lacks and that change one of `paths`, newest first, each with the paths of
/// `paths` it changes: one `log -z --name-only`, none without paths. Of a generated file the words merged, the commit
/// below its copy that the copy lacks (`RegenerateFile::lacks`)
#[instrument(skip(git, paths))]
pub fn commits_changing(git: &GitCommandExecutor, repository: &str, head: &str, baseline: &str, paths: &[String]) -> Result<Vec<MissingCommit>> {
  list_missing_commits(git, repository, head, baseline, paths)
}

#[cfg(test)]
#[path = "generated_test.rs"]
mod generated_test;
