//! The report of a conflicting cherry-pick (`MergeConflictInfo`): the merge with its markers, the conflicts counted in
//! each file, and the analysis of `conflict_analysis.rs`, four processes whatever the number of files and commits. The
//! views read the merges and the diffs from `file_diffs` (`get_file_merges`, `get_file_diffs`).

use crate::commit_list::Commit;
use crate::commit_utils::{find_commit, get_commits_info};
use crate::conflict_analysis::{ConflictAnalysis, find_divergence, get_files_content_at_revisions, list_missing_commits};
use crate::copy_commit::CopyCommitError;
use crate::merge_memo::CherryCommits;
use crate::merge_tree_output::parse_merge_tree_output;
use crate::model::{ConflictCommit, ConflictFile, MergeConflictInfo};
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

/// Build the full conflict report: commit metadata, the conflicts of each file and the conflict analysis, with four git
/// processes whatever the number of conflicting files and missing commits: one `rev-list` for the merge base and the
/// divergence (`merge-base` once more only when the boundary is not one commit), one `log` for the commits, one `log`
/// for the missing commits, one `cat-file` for the merged files, whose markers are counted.
#[instrument(skip_all, fields(conflict_files = ctx.conflict_files.len()))]
pub fn build_merge_conflict_info(ctx: ConflictContext<'_>) -> Result<MergeConflictInfo, CopyCommitError> {
  build_complete_conflict_info(ctx).map(|(info, _complete)| info)
}

/// `build_merge_conflict_info`, and whether every read of the analysis succeeded: a view with a fallback (no
/// divergence walk, no missing commits, no merged files) is not one to remember (`MergeMemo::remember_conflict`)
pub(crate) fn build_complete_conflict_info(ctx: ConflictContext<'_>) -> Result<(MergeConflictInfo, bool), CopyCommitError> {
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

  // one cat-file: the conflicting files in the merge tree, for the count of their markers; the text is not kept
  let contents = get_files_content_at_revisions(git_executor, repo_path, &[(merge_tree_oid, file_paths.as_slice())]).unwrap_or_else(|e| {
    debug!(error = %e, "failed to read file contents, counting no conflicts");
    complete = false;
    HashMap::new()
  });
  let merged = contents.get(merge_tree_oid);
  let conflicting_files = file_paths
    .into_iter()
    .map(|file| ConflictFile {
      conflict_count: merged.and_then(|merged| merged.get(&file)).map_or(0, |content| count_conflicts(content)),
      file,
    })
    .collect();
  let conflict_analysis = match (divergence, listed) {
    (Some(divergence), Some(listed)) => ConflictAnalysis {
      missing_commits: listed,
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
  Ok((info, complete))
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
  let output = git_executor
    .cmd(&args, repo_path)
    .accept(&[1])
    .bytes()
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
