use crate::commit_list::Commit;
use crate::commit_utils::{find_commit, get_commits_info};
use crate::conflict_analysis::{
  CommitChange, ConflictAnalysis, FileDiff, FileInfo, content_requests, file_diffs_of, find_divergence, get_files_content_at_revisions, list_missing_commits, read_conflict_patches,
};
use crate::copy_commit::CopyCommitError;
use crate::merge_memo::CherryCommits;
use crate::merge_tree_output::parse_merge_tree_output;
use crate::model::{ConflictCommit, ConflictContents, ConflictDetail, ConflictFile, MergeConflictInfo};
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, instrument};

/// The commits and trees of a conflicted merge, as needed to describe the conflict to the user
#[derive(Clone, Copy)]
pub struct ConflictContext<'a> {
  pub git_executor: &'a GitCommandExecutor,
  pub repo_path: &'a str,
  /// Conflicted file paths, without duplicates
  pub conflict_files: &'a [PathBuf],
  /// Tree-ish holding the conflicted files with conflict markers
  pub merge_tree_oid: &'a str,
  /// Original parent of the commit being applied
  pub parent_commit_id: &'a str,
  /// Commit the change is applied onto (ours)
  pub target_commit_id: &'a str,
  /// Commit being applied (theirs)
  pub cherry_commit_id: &'a str,
}

/// Build the full conflict report: commit metadata and the conflict analysis, and the contents of the view (per-file
/// contents and diffs), with five git processes whatever the number of conflicting files and missing commits: one
/// `rev-list` for the merge base and the divergence (`merge-base` once more only when the boundary is not one commit),
/// one `log` for the commits, one `log` for the missing commits, one `cat-file` for every content and one `diff-tree`
/// for every patch.
#[instrument(skip_all, fields(conflict_files = ctx.conflict_files.len()))]
pub fn build_merge_conflict_info(ctx: ConflictContext<'_>) -> Result<(MergeConflictInfo, ConflictContents), CopyCommitError> {
  build_complete_conflict_info(ctx).map(|(info, contents, _complete)| (info, contents))
}

/// `build_merge_conflict_info`, and whether every read of the analysis succeeded: a view with a fallback (no
/// divergence walk, no missing commits, no contents) is not one to remember (`MergeMemo::remember_conflict`)
pub(crate) fn build_complete_conflict_info(ctx: ConflictContext<'_>) -> Result<(MergeConflictInfo, ConflictContents, bool), CopyCommitError> {
  let ConflictContext {
    git_executor,
    repo_path,
    conflict_files,
    merge_tree_oid,
    parent_commit_id,
    target_commit_id,
    cherry_commit_id,
  } = ctx;

  let mut complete = true;
  // The merge base of parent and target shows the actual divergence point
  let divergence = find_divergence(git_executor, repo_path, parent_commit_id, target_commit_id).unwrap_or_else(|e| {
    debug!(error = %e, "the divergence walk failed, falling back to the parent commit");
    complete = false;
    None
  });
  let merge_base_id = divergence.as_ref().map_or(parent_commit_id, |divergence| divergence.merge_base.as_str());

  let commits = get_commits_info(git_executor, repo_path, &[parent_commit_id, target_commit_id, cherry_commit_id, merge_base_id])
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to get commit info for conflict: {e}")))?;
  let cherry = commit_of(&commits, cherry_commit_id)?;
  let parent = commit_of(&commits, parent_commit_id)?;
  let target = commit_of(&commits, target_commit_id)?;
  let merge_base = commit_of(&commits, merge_base_id)?;

  let file_paths: Vec<String> = conflict_files.iter().map(|p| p.display().to_string()).collect();
  // the commits the target lacks, only with a merge base; without them there is no analysis
  let listed = divergence.as_ref().and_then(|_| {
    list_missing_commits(git_executor, repo_path, parent_commit_id, target_commit_id, &file_paths)
      .inspect_err(|e| {
        debug!(error = %e, "failed to analyze conflict");
        complete = false;
      })
      .ok()
  });
  let changes = listed.as_deref().map_or_else(Vec::new, CommitChange::of);

  // one cat-file: the conflicting files at the merge base, the commit and the merge tree (in the amend case the commit is
  // the merge tree: read once), and the files of the missing commits at them and at their parents
  let mut revisions = vec![merge_base_id, cherry_commit_id];
  if merge_tree_oid != cherry_commit_id {
    revisions.push(merge_tree_oid);
  }
  let missing_requests = content_requests(&changes);
  let requests: Vec<(&str, &[String])> = revisions
    .iter()
    .map(|rev| (*rev, file_paths.as_slice()))
    .chain(missing_requests.iter().map(|(rev, files)| (*rev, files.as_slice())))
    .collect();
  let contents = get_files_content_at_revisions(git_executor, repo_path, &requests).unwrap_or_else(|e| {
    debug!(error = %e, "failed to read file contents, using empty content");
    complete = false;
    HashMap::new()
  });

  // one diff-tree: the change of the commit from the merge base, and the change of each missing commit
  let mut diffs: Vec<(&str, Option<&str>)> = Vec::with_capacity(changes.len() + 1);
  if merge_base.id != cherry.id {
    diffs.push((&cherry.id, Some(&merge_base.id)));
  }
  diffs.extend(changes.iter().map(|change| (change.id, None)));
  let mut patches = read_conflict_patches(git_executor, repo_path, &diffs, &file_paths).map_err(|e| CopyCommitError::Other(anyhow!("git diff-tree failed: {e}")))?;
  let commit_patches = patches.remove(&cherry.id).unwrap_or_default();

  let details = conflict_details(file_paths, &contents, &commit_patches, [merge_base_id, cherry_commit_id, merge_tree_oid]);
  let conflicting_files = details
    .iter()
    .map(|detail| ConflictFile {
      file: detail.file.clone(),
      conflict_count: count_conflicts(&detail.merged.content),
    })
    .collect();
  let missing_commit_diffs = file_diffs_of(&changes, &contents, &patches).into_iter().collect();
  let conflict_analysis = match (divergence, listed) {
    (Some(divergence), Some(listed)) => ConflictAnalysis {
      missing_commits: listed.into_iter().map(|(commit, _)| commit).collect(),
      merge_base_hash: merge_base.id.clone(),
      merge_base_subject: merge_base.subject.clone(),
      merge_base_message: merge_base.message.clone(),
      merge_base_time: merge_base.committer_timestamp,
      merge_base_author: merge_base.author_name.clone(),
      divergence_summary: divergence.summary,
    },
    _ => ConflictAnalysis::default(),
  };

  let info = MergeConflictInfo {
    commit: ConflictCommit::from(cherry),
    original_parent: ConflictCommit::from(parent),
    target_branch: ConflictCommit::from(target),
    conflicting_files,
    conflict_analysis,
  };
  let contents = ConflictContents {
    conflicting_files: details,
    missing_commit_diffs,
  };
  Ok((info, contents, complete))
}

/// The conflict of the cherry-pick of `commits` again, when its view is not remembered: the merge with the commits (for
/// the markers) and the report, six processes (`merge_with_markers`, `build_merge_conflict_info`). The report, its
/// contents and whether it is complete (one to remember); `None` when the commits merge cleanly
pub fn rebuild_merge_conflict(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  commits: CherryCommits<'_>,
) -> Result<Option<(MergeConflictInfo, ConflictContents, bool)>, CopyCommitError> {
  let (tree, conflict_files) = merge_with_markers(git_executor, repo_path, commits)?;
  if conflict_files.is_empty() {
    return Ok(None);
  }
  build_complete_conflict_info(ConflictContext {
    git_executor,
    repo_path,
    conflict_files: &conflict_files,
    merge_tree_oid: &tree,
    parent_commit_id: commits.parent,
    target_commit_id: commits.target,
    cherry_commit_id: commits.commit,
  })
  .map(Some)
}

/// The cherry-pick of `commits` by one `merge-tree --write-tree` with the commits, its markers in the `zdiff3` style
/// (the base included): the merged tree and the unique conflicted paths, none for a clean merge
pub(crate) fn merge_with_markers(git_executor: &GitCommandExecutor, repo_path: &str, commits: CherryCommits<'_>) -> Result<(String, Vec<PathBuf>), CopyCommitError> {
  let args = [
    "-c",
    "merge.conflictStyle=zdiff3",
    "merge-tree",
    "--write-tree",
    // NUL separates the entries
    "-z",
    "--merge-base",
    commits.parent,
    commits.target,
    commits.commit,
  ];
  // Exit code 1 means the merge has conflicts; the output still holds the tree and the conflicted entries
  let (output, _) = git_executor
    .execute_command_raw_accepting(&args, repo_path, &[1])
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to execute git merge-tree: {e}")))?;
  debug!(output_length = output.len(), "git merge-tree completed");
  let merge = parse_merge_tree_output(&output).map_err(CopyCommitError::Other)?;
  let conflict_files = merge
    .conflicted_paths()
    .into_iter()
    .map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
    .collect();
  Ok((merge.tree, conflict_files))
}

/// How many conflicts a merged file has: its `<<<<<<<` marker lines, exactly 7 characters, alone or before a space
pub fn count_conflicts(content: &str) -> u32 {
  let count = content
    .lines()
    .filter(|line| line.strip_prefix("<<<<<<<").is_some_and(|rest| rest.is_empty() || rest.starts_with(' ')))
    .count();
  u32::try_from(count).unwrap_or(u32::MAX)
}

/// The commit of `commits` that `id` names, by full hash or by the abbreviated id the caller passed
fn commit_of<'a>(commits: &'a HashMap<String, Commit>, id: &str) -> Result<&'a Commit, CopyCommitError> {
  find_commit(commits, id).ok_or_else(|| CopyCommitError::Other(anyhow!("Commit {id} not found")))
}

/// Parse one file's patch into hunks, each prefixed with its file header (`---`/`+++`); the frontend joins them into one patch.
/// Only the lines before the first hunk are metadata; inside a hunk every line is content.
fn parse_diff_hunks(file_patch: &str, file_path: &str) -> Vec<String> {
  let mut hunks = Vec::new();
  let mut old_side = format!("a/{file_path}");
  let mut new_side = format!("b/{file_path}");
  let mut current_hunk: Option<String> = None;

  for line in file_patch.lines() {
    if line.starts_with("@@") {
      if let Some(hunk) = current_hunk.take() {
        hunks.push(hunk);
      }
      current_hunk = Some(format!("--- {old_side}\n+++ {new_side}\n{line}"));
    } else if let Some(hunk) = current_hunk.as_mut() {
      hunk.push('\n');
      hunk.push_str(line);
    } else if line.starts_with("new file mode") {
      "/dev/null".clone_into(&mut old_side);
    } else if line.starts_with("deleted file mode") {
      "/dev/null".clone_into(&mut new_side);
    }
  }
  hunks.extend(current_hunk);
  hunks
}

/// A single hunk showing `content` unchanged; empty content has no hunks
fn context_hunks(file_path: &str, content: &str) -> Vec<String> {
  let line_count = content.lines().count();
  if line_count == 0 {
    return vec![];
  }
  let mut hunk = format!("--- a/{file_path}\n+++ b/{file_path}\n@@ -1,{line_count} +1,{line_count} @@");
  for line in content.lines() {
    hunk.push_str("\n ");
    hunk.push_str(line);
  }
  vec![hunk]
}

/// A single hunk replacing all of `from` with all of `to`, for changes git reports without hunks (e.g. binary files)
fn replacement_hunk(file_path: &str, from: &str, to: &str) -> String {
  let mut hunk = format!("--- a/{file_path}\n+++ b/{file_path}\n@@ -1,{} +1,{} @@", from.lines().count(), to.lines().count());
  for (prefix, content) in [("\n-", from), ("\n+", to)] {
    for line in content.lines() {
      hunk.push_str(prefix);
      hunk.push_str(line);
    }
  }
  hunk
}

/// Diff of one file between two versions, with both contents. Identical versions show the whole file as context.
fn version_diff(file_path: &str, file_lang: &str, from: String, to: String, file_patch: Option<&String>) -> FileDiff {
  let hunks = if from == to {
    context_hunks(file_path, &from)
  } else {
    let hunks = file_patch.map_or_else(Vec::new, |patch| parse_diff_hunks(patch, file_path));
    if hunks.is_empty() { vec![replacement_hunk(file_path, &from, &to)] } else { hunks }
  };
  FileDiff {
    old_file: file_info(file_path, file_lang, from),
    new_file: file_info(file_path, file_lang, to),
    hunks,
  }
}

fn file_info(file_path: &str, file_lang: &str, content: String) -> FileInfo {
  FileInfo {
    file_name: file_path.to_string(),
    file_lang: file_lang.to_string(),
    content,
  }
}

/// Per-file conflict details: the merged content with conflict markers and the change of the commit (the merge base's
/// version to its own, with both contents), from the `contents` at `[merge base, commit, merge tree]` and the patches of
/// the change of the commit by path
fn conflict_details(
  file_paths: Vec<String>,
  contents: &HashMap<String, HashMap<String, String>>,
  commit_patches: &HashMap<String, String>,
  [merge_base_id, cherry_commit_id, merge_tree_oid]: [&str; 3],
) -> Vec<ConflictDetail> {
  let empty = HashMap::new();
  let contents_at = |rev: &str| contents.get(rev).unwrap_or(&empty);
  let base_contents = contents_at(merge_base_id);
  let cherry_contents = contents_at(cherry_commit_id);
  let conflict_contents = contents_at(merge_tree_oid);
  let content_of = |contents: &HashMap<String, String>, file_path: &str| contents.get(file_path).cloned().unwrap_or_default();

  let mut conflict_details = Vec::with_capacity(file_paths.len());
  for file_path in file_paths {
    let file_lang = file_path.split('.').next_back().unwrap_or("txt").to_string();
    let commit_diff = version_diff(
      &file_path,
      &file_lang,
      content_of(base_contents, &file_path),
      content_of(cherry_contents, &file_path),
      commit_patches.get(&file_path),
    );
    conflict_details.push(ConflictDetail {
      merged: file_info(&file_path, &file_lang, content_of(conflict_contents, &file_path)),
      commit_diff,
      file: file_path,
    });
  }
  conflict_details
}
