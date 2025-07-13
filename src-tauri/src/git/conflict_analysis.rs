use crate::git::git_command::GitCommandExecutor;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tracing::{debug, instrument};

/// Represents a commit that exists in the source branch but is missing from the target branch.
/// These commits might be causing merge conflicts.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MissingCommit {
  pub hash: String,
  pub message: String,
  pub time: u32,
  pub author: String,
  pub files_touched: Vec<String>,
  pub file_diffs: Vec<FileDiff>,
}

/// Represents the diff between two versions of a file.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
  pub old_file: FileInfo,
  pub new_file: FileInfo,
  pub hunks: Vec<String>, // Array of unified diff hunks for git-diff-view
}

/// Information about a file including its content and metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
  pub file_name: String,
  pub file_lang: String,
  pub content: String,
}

/// Analysis results for a merge conflict, including missing commits and divergence information.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictAnalysis {
  pub missing_commits: Vec<MissingCommit>,
  pub merge_base_hash: String,
  pub merge_base_message: String,
  pub merge_base_time: u32,
  pub merge_base_author: String,
  pub divergence_summary: DivergenceSummary,
}

/// Summary of how two branches have diverged from their common ancestor.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DivergenceSummary {
  pub commits_ahead_in_source: u32,  // How many commits the source branch is ahead
  pub commits_ahead_in_target: u32,  // How many commits the target branch is ahead
  pub common_ancestor_distance: u32, // How far back is the common ancestor
}

/// Analyze why a cherry-pick conflict occurred and find missing commits
#[instrument(skip(git_executor, conflicting_files))]
pub fn analyze_conflict(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  original_parent_hash: &str,
  target_commit_hash: &str,
  conflicting_files: &[PathBuf],
) -> Result<ConflictAnalysis> {
  // Find merge base
  let merge_base = find_merge_base(git_executor, repo_path, original_parent_hash, target_commit_hash)?;

  // Get merge base details
  let merge_base_info = get_commit_info(git_executor, repo_path, &merge_base)?;

  // Find missing commits that touch conflicting files
  let missing_commits = find_missing_commits_for_conflicts(git_executor, repo_path, original_parent_hash, target_commit_hash, conflicting_files)?;

  // Calculate divergence summary
  let divergence_summary = calculate_divergence(git_executor, repo_path, &merge_base, original_parent_hash, target_commit_hash)?;

  Ok(ConflictAnalysis {
    missing_commits,
    merge_base_hash: merge_base.clone(),
    merge_base_message: merge_base_info.1,
    merge_base_time: merge_base_info.2,
    merge_base_author: merge_base_info.3,
    divergence_summary,
  })
}

/// Find commits that are in the source history but not in target that touch specific files
#[instrument(skip(git_executor, conflicting_files))]
fn find_missing_commits_for_conflicts(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  original_parent_hash: &str,
  target_commit_hash: &str,
  conflicting_files: &[PathBuf],
) -> Result<Vec<MissingCommit>> {
  let mut missing_commits = Vec::new();

  // Get commits that are in original parent but not in target
  // git rev-list original_parent ^target_commit
  let exclude_arg = format!("^{target_commit_hash}");
  let args = vec![
    "rev-list",
    "-z",                     // Use null-terminated output for safer parsing
    "--format=%H %ct %an %s", // hash, commit time, author name, subject
    "--no-commit-header",
    original_parent_hash,
    &exclude_arg,
  ];

  let output = git_executor.execute_command(&args, repo_path).map_err(|e| anyhow!(e))?;
  if output.trim().is_empty() {
    return Ok(missing_commits);
  }

  let conflicting_files_set: HashSet<String> = conflicting_files.iter().map(|p| p.to_string_lossy().to_string()).collect();

  // Process each commit (null-terminated)
  for line in output.split('\0').filter(|s| !s.is_empty()) {
    // Split into: hash, time, and the rest (author name + message)
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() < 3 {
      continue;
    }

    let hash = parts[0];
    let time = parts[1].parse::<u32>().unwrap_or(0);

    // The rest contains "Author Name commit message"
    // We need to find where the author name ends and message begins
    let author_and_message = parts[2];

    // Author name can contain spaces, so we need a heuristic
    // Common patterns: messages often start with lowercase, brackets, or special chars
    let (author, message) = extract_author_and_message(author_and_message);

    // Check which conflicting files this commit touches
    // git diff-tree --no-commit-id --name-only -r -z <commit>
    let diff_args = vec!["diff-tree", "--no-commit-id", "--name-only", "-r", "-z", hash];

    let files_output = git_executor.execute_command(&diff_args, repo_path).map_err(|e| anyhow!(e))?;
    let mut files_touched = Vec::new();

    // Parse null-terminated output
    for file in files_output.split('\0').filter(|s| !s.is_empty()) {
      if conflicting_files_set.contains(file) {
        files_touched.push(file.to_string());
      }
    }

    if !files_touched.is_empty() {
      debug!(commit_hash = %hash, ?files_touched, "found missing commit that touches conflicting files");

      // Get diffs for each touched file
      let file_diffs = get_file_diffs(git_executor, repo_path, hash, &files_touched)?;

      missing_commits.push(MissingCommit {
        hash: hash.to_string(),
        message: message.to_string(),
        time,
        author: author.to_string(),
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
  let output = git_executor.execute_command(&args, repo_path).map_err(|e| anyhow!(e))?;
  Ok(output.trim().to_string())
}

/// Get commit information
#[instrument(skip(git_executor))]
fn get_commit_info(git_executor: &GitCommandExecutor, repo_path: &str, commit_hash: &str) -> Result<(String, String, u32, String)> {
  let args = vec!["show", "--no-patch", "--format=%H%x00%s%x00%ct%x00%an", commit_hash];

  let output = git_executor.execute_command(&args, repo_path).map_err(|e| anyhow!(e))?;
  let parts: Vec<&str> = output.trim().split('\0').collect();

  if parts.len() < 4 {
    return Err(anyhow!("Failed to parse commit info"));
  }

  Ok((parts[0].to_string(), parts[1].to_string(), parts[2].parse::<u32>().unwrap_or(0), parts[3].to_string()))
}

/// Calculate how the branches have diverged
#[instrument(skip(git_executor))]
fn calculate_divergence(git_executor: &GitCommandExecutor, repo_path: &str, merge_base: &str, source_commit: &str, target_commit: &str) -> Result<DivergenceSummary> {
  // Count commits from merge base to source
  let source_count = count_commits(git_executor, repo_path, merge_base, source_commit)?;

  // Count commits from merge base to target
  let target_count = count_commits(git_executor, repo_path, merge_base, target_commit)?;

  // The common ancestor distance is the minimum of the two
  let common_ancestor_distance = source_count.min(target_count);

  Ok(DivergenceSummary {
    commits_ahead_in_source: source_count,
    commits_ahead_in_target: target_count,
    common_ancestor_distance,
  })
}

/// Count commits between two commits
#[instrument(skip(git_executor))]
pub(crate) fn count_commits(git_executor: &GitCommandExecutor, repo_path: &str, from_commit: &str, to_commit: &str) -> Result<u32> {
  let range_arg = format!("{from_commit}..{to_commit}");
  let args = vec!["rev-list", "--count", &range_arg];

  let output = git_executor.execute_command(&args, repo_path).map_err(|e| anyhow!(e))?;
  output.trim().parse::<u32>().map_err(|e| anyhow!("Failed to parse commit count: {}", e))
}

/// Extract author name and commit message from a combined string
#[instrument]
pub(crate) fn extract_author_and_message(author_and_message: &str) -> (String, String) {
  // Common patterns for commit messages:
  // - Start with lowercase letter (unless it's a proper name)
  // - Start with brackets like [, (, {
  // - Start with special prefixes like "fix:", "feat:", etc.

  let words: Vec<&str> = author_and_message.split_whitespace().collect();
  if words.is_empty() {
    return (String::new(), String::new());
  }

  // Try to find where the message starts
  let mut message_start_idx = 0;
  for (i, word) in words.iter().enumerate() {
    // Check if this word looks like the start of a commit message
    if word.starts_with('[') || word.starts_with('(') || word.starts_with('{') || word.contains(':') || (i > 0 && word.chars().next().is_some_and(|c| c.is_lowercase())) {
      message_start_idx = i;
      break;
    }
  }

  // If we didn't find a clear message start, assume first 2 words are the name
  if message_start_idx == 0 {
    message_start_idx = words.len().min(2);
  }

  let author = words[..message_start_idx].join(" ");
  let message = words[message_start_idx..].join(" ");

  (author, message)
}

/// Get file contents for specific files in a commit
#[instrument(skip(git_executor, files))]
pub(crate) fn get_file_diffs(git_executor: &GitCommandExecutor, repo_path: &str, commit_hash: &str, files: &[String]) -> Result<Vec<FileDiff>> {
  if files.is_empty() {
    return Ok(vec![]);
  }

  let mut file_diffs = Vec::new();

  // Get the parent commit to fetch the "before" state
  let parent_hash = {
    let parent_ref = format!("{commit_hash}^");
    let args = vec!["rev-parse", &parent_ref];
    git_executor.execute_command(&args, repo_path).map(|output| output.trim().to_string()).unwrap_or_else(|_| {
      // If there's no parent (first commit), use empty tree
      "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string()
    })
  };

  // Batch retrieve all file contents for both commits
  let parent_contents = get_files_content_at_commit(git_executor, repo_path, &parent_hash, files)?;
  let current_contents = get_files_content_at_commit(git_executor, repo_path, commit_hash, files)?;

  // Get the unified diff for all files at once
  // Use zdiff3 style for consistency
  let mut show_args = vec!["-c", "merge.conflictStyle=zdiff3", "show", "--no-color", "--format=", "--unified=3", commit_hash, "--"];
  show_args.extend(files.iter().map(|s| s.as_str()));

  let diff_output = git_executor.execute_command(&show_args, repo_path).map_err(|e| anyhow!(e))?;

  // Parse the diff output - it contains all files' diffs
  let mut current_file_diff = String::new();
  let mut current_file: Option<&str> = None;
  let mut file_to_diff: HashMap<String, String> = HashMap::new();

  for line in diff_output.lines() {
    if line.starts_with("diff --git") {
      // Save previous file's diff if any
      if let Some(file) = current_file {
        file_to_diff.insert(file.to_string(), current_file_diff.clone());
      }
      // Extract filename from diff line
      if let Some(file) = files.iter().find(|f| line.contains(f.as_str())) {
        current_file = Some(file);
        current_file_diff.clear();
      }
    }
    if current_file.is_some() {
      current_file_diff.push_str(line);
      current_file_diff.push('\n');
    }
  }

  // Save the last file's diff
  if let Some(file) = current_file {
    file_to_diff.insert(file.to_string(), current_file_diff);
  }

  // Build FileDiff objects
  for file in files {
    let ext = file.split('.').next_back().unwrap_or("").to_string();

    // Get contents from the batch results
    let old_content = parent_contents.get(file).cloned().unwrap_or_default();
    let new_content = current_contents.get(file).cloned().unwrap_or_default();

    // Get the diff from git show output
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

  debug!(file_diffs_count = file_diffs.len(), "retrieved file diffs");
  Ok(file_diffs)
}

/// Get content of multiple files at a specific commit using batch operation
#[instrument(skip(git_executor, file_paths))]
pub(crate) fn get_files_content_at_commit(git_executor: &GitCommandExecutor, repo_path: &str, commit_hash: &str, file_paths: &[String]) -> Result<HashMap<String, String>> {
  let mut contents = HashMap::new();

  if file_paths.is_empty() {
    return Ok(contents);
  }

  // Use git ls-tree with -z for null-terminated output to list files in the commit
  // Then use git cat-file --batch to efficiently retrieve all file contents

  // First, get the object IDs for all files
  let mut args = vec!["ls-tree", "-z", "-r", commit_hash, "--"];
  let file_refs: Vec<&str> = file_paths.iter().map(|s| s.as_str()).collect();
  args.extend(&file_refs);

  let ls_tree_output = git_executor
    .execute_command(&args, repo_path)
    .map_err(|e| anyhow!("Failed to ls-tree at commit {}: {}", commit_hash, e))?;

  debug!(output_length = ls_tree_output.len(), "ls-tree completed");

  // Parse ls-tree output to get object IDs
  let mut oid_to_file = HashMap::new();
  let mut object_requests = Vec::new();

  for entry in ls_tree_output.split('\0').filter(|s| !s.is_empty()) {
    // Format: <mode> <type> <object>	<file>
    // Example: "100644 blob 1234567890abcdef	path/to/file.txt"
    if let Some(tab_pos) = entry.find('\t') {
      let (metadata, file_path) = entry.split_at(tab_pos);
      let file_path = &file_path[1..]; // Skip the tab character

      // Parse metadata: "<mode> <type> <object>"
      let parts: Vec<&str> = metadata.split_whitespace().collect();
      if parts.len() >= 3 && parts[1] == "blob" {
        let oid = parts[2];
        oid_to_file.insert(oid.to_string(), file_path.to_string());
        object_requests.push(oid);
      }
    }
  }

  // Use git cat-file --batch to get all contents at once
  debug!(objects_to_fetch = object_requests.len(), "found objects to fetch");

  if !object_requests.is_empty() {
    let fetched_contents = execute_batch_cat_file(git_executor, repo_path, &object_requests, None)?;

    for (oid, content) in fetched_contents {
      if let Some(file_path) = oid_to_file.get(&oid) {
        contents.insert(file_path.clone(), content);
      }
    }
  }

  // Files that don't exist at this commit will have empty content
  // This is normal for files that were added in later commits
  for file in file_paths {
    if !contents.contains_key(file) {
      contents.insert(file.clone(), String::new());
    }
  }

  Ok(contents)
}

/// Parse git cat-file --batch header line
#[instrument]
pub(crate) fn parse_cat_file_header(line: &str) -> Option<(String, usize)> {
  // Format: <sha> <type> <size>
  let parts: Vec<&str> = line.split_whitespace().collect();
  if parts.len() >= 3 {
    if let Ok(size) = parts[2].parse::<usize>() {
      return Some((parts[0].to_string(), size));
    }
  }
  None
}

#[instrument(skip(git_executor, oids))]
fn execute_batch_cat_file(git_executor: &GitCommandExecutor, repo_path: &str, oids: &[&str], labeled_oids: Option<&[(&str, &str)]>) -> Result<HashMap<String, String>> {
  let mut contents = HashMap::new();

  if oids.is_empty() {
    return Ok(contents);
  }

  // Build batch input - just the OIDs
  let batch_input = oids.join("\0") + "\0";

  debug!(batch_objects = oids.len(), "executing cat-file batch");

  let args = vec!["cat-file", "--batch", "--buffer", "-Z"];
  match git_executor.execute_command_with_input(&args, repo_path, &batch_input) {
    Ok(batch_output) => {
      // Parse batch output
      // With -Z flag: <oid> <type> <size>\0<content>\0
      let entries = batch_output.split('\0').filter(|s| !s.is_empty());
      let mut entries_iter = entries.peekable();
      let mut oid_index = 0;

      while let Some(header) = entries_iter.next() {
        if let Some((oid, size)) = parse_cat_file_header(header) {
          // Next entry should be the content
          if let Some(content) = entries_iter.next() {
            if oid_index < oids.len() {
              let expected_oid = oids[oid_index];
              let label = labeled_oids.and_then(|lo| lo.get(oid_index).map(|(_, label)| label)).unwrap_or(&"");
              if oid == expected_oid {
                debug!(label = %label, oid = %oid, size = size, "storing content");
                contents.insert(oid.to_string(), content.to_string());
              } else {
                return Err(anyhow!("OID mismatch: expected {}, got {}", expected_oid, oid));
              }
              oid_index += 1;
            }
          } else {
            return Err(anyhow!("Missing content for OID {} (expected {} bytes)", oid, size));
          }
        } else if header.contains("missing") {
          // Object doesn't exist - skip it
          oid_index += 1;
        } else {
          return Err(anyhow!("Failed to parse git cat-file header: {:?}", header));
        }
      }
    }
    Err(e) => {
      return Err(anyhow!("Failed to cat-file batch: {}", e));
    }
  }

  Ok(contents)
}
