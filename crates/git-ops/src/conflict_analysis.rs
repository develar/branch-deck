use crate::commit_list::Commit;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::PathBuf;
use tracing::{debug, instrument};

/// Represents a commit that exists in the source branch but is missing from the target branch.
/// These commits might be causing merge conflicts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct MissingCommit {
  pub hash: String,
  pub subject: String,
  pub message: String,
  pub author_time: u32,
  pub committer_time: u32,
  pub author: String,
  pub files_touched: Vec<String>,
  pub file_diffs: Vec<FileDiff>,
}

/// Represents the diff between two versions of a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
  pub old_file: FileInfo,
  pub new_file: FileInfo,
  /// The patch of the file: one `diff --git` section, or hunks that each repeat the `---`/`+++` header; the frontend
  /// joins them into one patch for the diff view
  pub hunks: Vec<String>,
}

/// Information about a file including its content and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
  pub file_name: String,
  pub file_lang: String,
  pub content: String,
}

/// Analysis results for a merge conflict, including missing commits and divergence information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictAnalysis {
  pub missing_commits: Vec<MissingCommit>,
  pub merge_base_hash: String,
  pub merge_base_subject: String,
  pub merge_base_message: String,
  pub merge_base_time: u32,
  pub merge_base_author: String,
  pub divergence_summary: DivergenceSummary,
}

/// Summary of how two branches have diverged from their common ancestor.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct DivergenceSummary {
  pub commits_ahead_in_source: u32,  // How many commits the source branch is ahead
  pub commits_ahead_in_target: u32,  // How many commits the target branch is ahead
  pub common_ancestor_distance: u32, // How far back is the common ancestor
}

/// Analyze why a cherry-pick conflict occurred and find missing commits.
/// `merge_base` is the merge base of `original_parent_hash` and `target_commit_hash`, already resolved by the caller.
#[instrument(skip(git_executor, merge_base, conflicting_files), fields(merge_base = %merge_base.id))]
pub fn analyze_conflict(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  original_parent_hash: &str,
  target_commit_hash: &str,
  merge_base: &Commit,
  conflicting_files: &[PathBuf],
) -> Result<ConflictAnalysis> {
  // Find missing commits that touch conflicting files
  let missing_commits = find_missing_commits_for_conflicts(git_executor, repo_path, original_parent_hash, target_commit_hash, conflicting_files)?;

  // Calculate divergence summary
  let divergence_summary = calculate_divergence(git_executor, repo_path, original_parent_hash, target_commit_hash)?;

  Ok(ConflictAnalysis {
    missing_commits,
    merge_base_hash: merge_base.id.clone(),
    merge_base_subject: merge_base.subject.clone(),
    merge_base_message: merge_base.message.clone(),
    merge_base_time: merge_base.committer_timestamp,
    merge_base_author: merge_base.author_name.clone(),
    divergence_summary,
  })
}

/// Find commits that are in the source history but not in target that touch specific files
#[instrument(skip(git_executor, conflicting_files))]
pub(crate) fn find_missing_commits_for_conflicts(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  original_parent_hash: &str,
  target_commit_hash: &str,
  conflicting_files: &[PathBuf],
) -> Result<Vec<MissingCommit>> {
  let mut missing_commits = Vec::new();

  // Convert conflicting files to strings for git command
  let file_paths: Vec<String> = conflicting_files.iter().map(|p| p.to_string_lossy().to_string()).collect();

  if file_paths.is_empty() {
    return Ok(missing_commits);
  }

  // Use a single git log command to get commits and their file changes
  // This combines rev-list and diff-tree functionality
  let exclude_target = format!("^{target_commit_hash}");
  let mut args = vec![
    "log",
    "--format=COMMIT:%H%x00%at%x00%ct%x00%an%x00%s%x00%B", // Use null bytes as delimiters for machine-readable parsing
    "--name-only",                                         // Show file names changed in each commit
    "--no-merges",
    original_parent_hash,
    &exclude_target,
    "--", // Separator for file paths
  ];

  // Add file paths to filter commits
  for file_path in &file_paths {
    args.push(file_path);
  }

  let output = git_executor.execute_command(&args, repo_path)?;
  if output.trim().is_empty() {
    return Ok(missing_commits);
  }

  let conflicting_files_set: HashSet<String> = file_paths.into_iter().collect();

  // Define type alias for commit data to reduce complexity
  type CommitData = (String, u32, u32, String, String, String);
  type CommitWithFiles = (String, u32, u32, String, String, String, Vec<String>);

  // Parse the combined output and collect commit data
  let mut current_commit: Option<CommitData> = None;
  let mut current_files: Vec<String> = Vec::new();
  let mut commits_to_process: Vec<CommitWithFiles> = Vec::new();

  for line in output.lines() {
    if let Some(commit_data) = line.strip_prefix("COMMIT:") {
      // Process previous commit if exists
      if let Some((hash, author_time, committer_time, author, subject, message)) = current_commit.take()
        && !current_files.is_empty()
      {
        commits_to_process.push((hash, author_time, committer_time, author, subject, message, current_files.clone()));
      }

      // Parse new commit line with null-delimited format
      // Format: hash\0author_time\0committer_time\0author\0subject\0message
      let parts: Vec<&str> = commit_data.split('\0').collect();
      if parts.len() >= 6 {
        let hash = parts[0].to_string();
        let author_time = parts[1].parse::<u32>().unwrap_or(0);
        let committer_time = parts[2].parse::<u32>().unwrap_or(0);
        let author = parts[3].to_string();
        let subject = parts[4].to_string();
        let message = parts[5].to_string();

        current_commit = Some((hash, author_time, committer_time, author, subject, message));
        current_files.clear();
      }
    } else if !line.is_empty() && !line.starts_with("commit ") {
      // This is a file name
      if conflicting_files_set.contains(line) {
        current_files.push(line.to_string());
      }
    }
  }

  // Process the last commit
  if let Some((hash, author_time, committer_time, author, subject, message)) = current_commit
    && !current_files.is_empty()
  {
    commits_to_process.push((hash, author_time, committer_time, author, subject, message, current_files));
  }

  // Batch get all file diffs
  if !commits_to_process.is_empty() {
    let commit_files_map: Vec<(String, Vec<String>)> = commits_to_process.iter().map(|(hash, _, _, _, _, _, files)| (hash.clone(), files.clone())).collect();

    let all_file_diffs = batch_get_file_diffs(git_executor, repo_path, &commit_files_map)?;

    // Build the final missing commits with their diffs
    for (hash, author_time, committer_time, author, subject, message, files_touched) in commits_to_process {
      let file_diffs = all_file_diffs.get(&hash).cloned().unwrap_or_default();

      debug!(commit_hash = %hash, ?files_touched, "found missing commit that touches conflicting files");

      missing_commits.push(MissingCommit {
        hash,
        subject,
        message,
        author_time,
        committer_time,
        author,
        files_touched,
        file_diffs,
      });
    }
  }

  Ok(missing_commits)
}

/// Find the merge base between two commits
#[instrument(skip(git_executor))]
pub(crate) fn find_merge_base(git_executor: &GitCommandExecutor, repo_path: &str, commit1: &str, commit2: &str) -> Result<String> {
  let args = vec!["merge-base", commit1, commit2];
  let output = git_executor.execute_command(&args, repo_path)?;
  Ok(output.trim().to_string())
}

/// Calculate how the branches have diverged using a single git command
#[instrument(skip(git_executor))]
fn calculate_divergence(git_executor: &GitCommandExecutor, repo_path: &str, source_commit: &str, target_commit: &str) -> Result<DivergenceSummary> {
  // Use git rev-list --left-right --count to get both counts in a single command
  // This shows commits that are reachable from source but not target (left)
  // and commits that are reachable from target but not source (right)
  let range_arg = format!("{source_commit}...{target_commit}");
  let args = vec!["rev-list", "--left-right", "--count", &range_arg];

  let output = git_executor.execute_command(&args, repo_path)?;

  // Parse output format: "<left>\t<right>"
  let parts: Vec<&str> = output.trim().split('\t').collect();
  if parts.len() != 2 {
    return Err(anyhow!("Unexpected rev-list output format: {output}"));
  }

  let source_count = parts[0].parse::<u32>().map_err(|e| anyhow!("Failed to parse source commit count: {e}"))?;
  let target_count = parts[1].parse::<u32>().map_err(|e| anyhow!("Failed to parse target commit count: {e}"))?;

  // The common ancestor distance is the minimum of the two
  let common_ancestor_distance = source_count.min(target_count);

  Ok(DivergenceSummary {
    commits_ahead_in_source: source_count,
    commits_ahead_in_target: target_count,
    common_ancestor_distance,
  })
}

/// Batch resolve references to commit hashes
#[instrument(skip(git_executor, refs))]
pub(crate) fn batch_rev_parse(git_executor: &GitCommandExecutor, repo_path: &str, refs: &[&str]) -> Result<HashMap<String, String>> {
  let mut result = HashMap::new();

  if refs.is_empty() {
    return Ok(result);
  }

  // Use git rev-parse with multiple refs at once
  let mut args = vec!["rev-parse"];
  args.extend(refs);

  let output = git_executor.execute_command(&args, repo_path)?;

  // Parse output - one hash per line, in the same order as input refs
  let hashes: Vec<&str> = output.lines().collect();

  if hashes.len() == refs.len() {
    // Success - map refs to their hashes
    for (i, &ref_str) in refs.iter().enumerate() {
      result.insert(ref_str.to_string(), hashes[i].to_string());
    }
  } else {
    // If some refs failed, fall back to individual resolution
    for &ref_str in refs {
      match git_executor.execute_command(&["rev-parse", ref_str], repo_path) {
        Ok(hash) => {
          result.insert(ref_str.to_string(), hash.trim().to_string());
        }
        Err(_) => {
          // For parent refs that don't exist (first commit), use empty tree
          if ref_str.ends_with('^') {
            result.insert(ref_str.to_string(), "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string());
          }
        }
      }
    }
  }

  Ok(result)
}

/// Batch get file diffs for multiple commits
/// This function optimizes getting diffs for multiple commits by batching operations
#[instrument(skip(git_executor, commit_files_map))]
pub(crate) fn batch_get_file_diffs(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  commit_files_map: &[(String, Vec<String>)], // (commit_hash, files)
) -> Result<HashMap<String, Vec<FileDiff>>> {
  let mut result = HashMap::new();

  if commit_files_map.is_empty() {
    return Ok(result);
  }

  // Collect all unique commit/file pairs we need to fetch
  let mut all_files_by_commit: HashMap<String, HashSet<String>> = HashMap::new();

  for (commit_hash, files) in commit_files_map {
    all_files_by_commit.entry(commit_hash.clone()).or_default().extend(files.iter().cloned());
  }

  // Batch resolve all parent commits at once
  let parent_refs: Vec<String> = commit_files_map.iter().map(|(commit_hash, _)| format!("{commit_hash}^")).collect();
  let parent_ref_strs: Vec<&str> = parent_refs.iter().map(std::string::String::as_str).collect();

  let resolved_parents = batch_rev_parse(git_executor, repo_path, &parent_ref_strs)?;

  let mut parent_commits: HashSet<String> = HashSet::new();
  for parent_ref in &parent_refs {
    if let Some(parent_hash) = resolved_parents.get(parent_ref) {
      parent_commits.insert(parent_hash.clone());
    }
  }

  // Fetch the contents at all commits and their parents (every file touched by any of the commits) at once
  let all_files: Vec<String> = all_files_by_commit
    .values()
    .flat_map(|files| files.iter().cloned())
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let commit_files: Vec<(String, Vec<String>)> = all_files_by_commit
    .iter()
    .map(|(commit_hash, files)| (commit_hash.clone(), files.iter().cloned().collect()))
    .collect();
  let mut requests: Vec<(&str, &[String])> = commit_files.iter().map(|(commit_hash, files)| (commit_hash.as_str(), files.as_slice())).collect();
  requests.extend(parent_commits.iter().map(|parent_hash| (parent_hash.as_str(), all_files.as_slice())));
  let all_contents = get_files_content_at_revisions(git_executor, repo_path, &requests)?;

  // Now generate diffs for each commit
  for (commit_hash, files) in commit_files_map {
    let parent_ref = format!("{commit_hash}^");
    let parent_hash = resolved_parents
      .get(&parent_ref)
      .cloned()
      .unwrap_or_else(|| "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string());

    // Get the unified diff for all files at once
    let mut show_args = vec!["-c", "core.quotePath=false", "--no-pager", "show", "--format="];
    show_args.extend(PATCH_ARGS);
    show_args.extend(["--end-of-options", commit_hash.as_str(), "--"]);
    show_args.extend(files.iter().map(std::string::String::as_str));

    // untrimmed: the last line of a patch may be a context line of a blank line, a single space
    let diff_output = git_executor.execute_command_raw(&show_args, repo_path)?;
    let file_to_diff: HashMap<String, String> = split_diff_by_file(&diff_output).into_iter().collect();

    // Build FileDiff objects
    let mut file_diffs = Vec::new();
    let empty_map = HashMap::new();
    let parent_contents = all_contents.get(&parent_hash).unwrap_or(&empty_map);
    let current_contents = all_contents.get(commit_hash).unwrap_or(&empty_map);

    for file in files {
      let ext = file.split('.').next_back().unwrap_or("").to_string();
      let old_content = parent_contents.get(file).cloned().unwrap_or_default();
      let new_content = current_contents.get(file).cloned().unwrap_or_default();
      let diff = file_to_diff.get(file).cloned().unwrap_or_default();
      let hunks = if diff.trim().is_empty() { vec![] } else { vec![diff] };

      file_diffs.push(FileDiff {
        old_file: FileInfo {
          file_name: file.clone(),
          file_lang: ext.clone(),
          content: old_content,
        },
        new_file: FileInfo {
          file_name: file.clone(),
          file_lang: ext,
          content: new_content,
        },
        hunks,
      });
    }

    result.insert(commit_hash.clone(), file_diffs);
  }

  Ok(result)
}

/// Options for patch output that `split_diff_by_file` can parse regardless of user configuration:
/// fixed `a/` and `b/` prefixes and no rename pairs, so both header paths are equal.
/// Run with `-c core.quotePath=false` so that only paths with control characters or quotes are C-quoted.
pub(crate) const PATCH_ARGS: [&str; 8] = [
  "-p",
  "--no-color",
  "--no-ext-diff",
  "--no-textconv",
  "--no-renames",
  "--unified=3",
  "--src-prefix=a/",
  "--dst-prefix=b/",
];

/// Split patch output (`git diff-tree -p`, `git show`) produced with `PATCH_ARGS` into per-file sections.
/// Each section is `(path, text)`, where the text starts with the section's `diff --git` line.
/// Sections whose header cannot be parsed are dropped.
pub(crate) fn split_diff_by_file(diff: &str) -> Vec<(String, String)> {
  let mut sections: Vec<(String, String)> = Vec::new();
  let mut in_section = false;
  for line in diff.split_inclusive('\n') {
    if let Some(header) = line.strip_prefix("diff --git ") {
      if let Some(path) = parse_diff_header_path(header.trim_end_matches(['\n', '\r'])) {
        sections.push((path, String::new()));
        in_section = true;
      } else {
        debug!(header, "cannot parse diff header, skipping file section");
        in_section = false;
      }
    }
    if in_section && let Some((_, text)) = sections.last_mut() {
      text.push_str(line);
    }
  }
  sections
}

/// Path from the part of a `diff --git` line after the command: `a/<path> b/<path>`, C-quoted when needed
fn parse_diff_header_path(header: &str) -> Option<String> {
  if header.starts_with('"') {
    let (path, _) = unquote_c_style(header)?;
    return path.strip_prefix("a/").map(str::to_string);
  }
  // Without renames both paths are equal, so the line is "a/" + path + " b/" + path
  let path_len = header.len().checked_sub(5)? / 2;
  let path = header.strip_prefix("a/")?.get(..path_len)?;
  (header.get(2 + path_len..)? == format!(" b/{path}")).then(|| path.to_string())
}

/// Decode a C-style quoted string as git writes it for unusual paths. Returns the value and the rest after the closing quote.
fn unquote_c_style(quoted: &str) -> Option<(String, &str)> {
  let bytes = quoted.as_bytes();
  if bytes.first() != Some(&b'"') {
    return None;
  }
  let mut value = Vec::with_capacity(bytes.len());
  let mut i = 1;
  while i < bytes.len() {
    match bytes[i] {
      b'"' => return Some((String::from_utf8_lossy(&value).into_owned(), &quoted[i + 1..])),
      b'\\' => {
        let escaped = *bytes.get(i + 1)?;
        i += 2;
        let decoded = match escaped {
          b'a' => 0x07,
          b'b' => 0x08,
          b'f' => 0x0c,
          b'n' => b'\n',
          b'r' => b'\r',
          b't' => b'\t',
          b'v' => 0x0b,
          b'0'..=b'7' => {
            // Three octal digits, the first one already consumed
            let octal = quoted.get(i - 1..i + 2)?;
            i += 2;
            u8::from_str_radix(octal, 8).ok()?
          }
          other => other,
        };
        value.push(decoded);
      }
      byte => {
        value.push(byte);
        i += 1;
      }
    }
  }
  None
}

/// Content of files at several revisions with one `git cat-file --batch` reading `<rev>:<path>` names, keyed by
/// revision and path. A file that is absent at a revision (or is not a blob) has empty content.
#[instrument(skip(git_executor, requests), fields(revisions = requests.len()))]
pub(crate) fn get_files_content_at_revisions(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  requests: &[(&str, &[String])],
) -> Result<HashMap<String, HashMap<String, String>>> {
  let mut result: HashMap<String, HashMap<String, String>> = HashMap::with_capacity(requests.len());
  let mut names = Vec::new();
  let mut input = String::new();
  for (rev, file_paths) in requests {
    let contents = result.entry((*rev).to_string()).or_default();
    for file_path in *file_paths {
      // absent files keep this empty content
      contents.insert(file_path.clone(), String::new());
      let _ = write!(input, "{rev}:{file_path}\0");
      names.push((*rev, file_path.as_str()));
    }
  }
  if names.is_empty() {
    return Ok(result);
  }

  let bytes = git_executor
    .execute_command_with_input_raw(&["cat-file", "--batch", "--buffer", "-Z"], repo_path, &input)
    .map_err(|e| anyhow!("Failed to cat-file batch: {e}"))?;

  // With -Z, every entry is `<oid> <type> <size>\0<content>\0`, or `<name> missing\0` for an absent file
  let mut i = 0;
  for (rev, file_path) in names {
    let end = memchr::memchr(0, &bytes[i..]).map(|pos| i + pos).ok_or_else(|| anyhow!("truncated cat-file output"))?;
    let header = String::from_utf8_lossy(&bytes[i..end]).into_owned();
    i = end + 1;
    let mut fields = header.rsplitn(3, ' ');
    let (Some(size), Some(kind)) = (fields.next(), fields.next()) else {
      return Err(anyhow!("Failed to parse git cat-file header: {header:?}"));
    };
    let Ok(size) = size.parse::<usize>() else {
      // `missing` or `ambiguous`
      debug!(header, "object missing");
      continue;
    };
    let content = bytes
      .get(i..i + size)
      .ok_or_else(|| anyhow!("Insufficient data for content: expected {size} bytes, but only {} remaining", bytes.len() - i))?;
    if kind == "blob"
      && let Some(contents) = result.get_mut(rev)
    {
      contents.insert(file_path.to_string(), String::from_utf8_lossy(content).into_owned());
    }
    // skip the content and its NUL terminator
    i += size + 1;
  }
  Ok(result)
}
