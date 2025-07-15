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
  pub author_time: u32,
  pub committer_time: u32,
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
    "--format=COMMIT:%H%x00%at%x00%ct%x00%an%x00%s", // Use null bytes as delimiters for machine-readable parsing
    "--name-only",                                   // Show file names changed in each commit
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

  // Parse the combined output and collect commit data
  let mut current_commit: Option<(String, u32, u32, String, String)> = None;
  let mut current_files: Vec<String> = Vec::new();
  let mut commits_to_process: Vec<(String, u32, u32, String, String, Vec<String>)> = Vec::new();

  for line in output.lines() {
    if let Some(commit_data) = line.strip_prefix("COMMIT:") {
      // Process previous commit if exists
      if let Some((hash, author_time, committer_time, author, message)) = current_commit.take() {
        if !current_files.is_empty() {
          commits_to_process.push((hash, author_time, committer_time, author, message, current_files.clone()));
        }
      }

      // Parse new commit line with null-delimited format
      // Format: hash\0author_time\0committer_time\0author\0message
      let parts: Vec<&str> = commit_data.split('\0').collect();
      if parts.len() >= 5 {
        let hash = parts[0].to_string();
        let author_time = parts[1].parse::<u32>().unwrap_or(0);
        let committer_time = parts[2].parse::<u32>().unwrap_or(0);
        let author = parts[3].to_string();
        let message = parts[4].to_string();

        current_commit = Some((hash, author_time, committer_time, author, message));
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
  if let Some((hash, author_time, committer_time, author, message)) = current_commit {
    if !current_files.is_empty() {
      commits_to_process.push((hash, author_time, committer_time, author, message, current_files));
    }
  }

  // Batch get all file diffs
  if !commits_to_process.is_empty() {
    let commit_files_map: Vec<(String, Vec<String>)> = commits_to_process.iter().map(|(hash, _, _, _, _, files)| (hash.clone(), files.clone())).collect();

    let all_file_diffs = batch_get_file_diffs(git_executor, repo_path, &commit_files_map)?;

    // Build the final missing commits with their diffs
    for (hash, author_time, committer_time, author, message, files_touched) in commits_to_process {
      let file_diffs = all_file_diffs.get(&hash).cloned().unwrap_or_default();

      debug!(commit_hash = %hash, ?files_touched, "found missing commit that touches conflicting files");

      missing_commits.push(MissingCommit {
        hash,
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

/// Get commit information
#[instrument(skip(git_executor))]
fn get_commit_info(git_executor: &GitCommandExecutor, repo_path: &str, commit_hash: &str) -> Result<(String, String, u32, String)> {
  let args = vec!["show", "--no-patch", "--format=%H%x00%s%x00%ct%x00%an", commit_hash];

  let output = git_executor.execute_command(&args, repo_path)?;
  let parts: Vec<&str> = output.trim().split('\0').collect();

  if parts.len() < 4 {
    return Err(anyhow!("Failed to parse commit info"));
  }

  Ok((parts[0].to_string(), parts[1].to_string(), parts[2].parse::<u32>().unwrap_or(0), parts[3].to_string()))
}

/// Calculate how the branches have diverged using a single git command
#[instrument(skip(git_executor))]
fn calculate_divergence(git_executor: &GitCommandExecutor, repo_path: &str, merge_base: &str, source_commit: &str, target_commit: &str) -> Result<DivergenceSummary> {
  // Use git rev-list --left-right --count to get both counts in a single command
  // This shows commits that are reachable from source but not target (left)
  // and commits that are reachable from target but not source (right)
  let range_arg = format!("{source_commit}...{target_commit}");
  let args = vec!["rev-list", "--left-right", "--count", &range_arg];

  let output = git_executor.execute_command(&args, repo_path)?;

  // Parse output format: "<left>\t<right>"
  let parts: Vec<&str> = output.trim().split('\t').collect();
  if parts.len() != 2 {
    return Err(anyhow!("Unexpected rev-list output format: {}", output));
  }

  let source_count = parts[0].parse::<u32>().map_err(|e| anyhow!("Failed to parse source commit count: {}", e))?;
  let target_count = parts[1].parse::<u32>().map_err(|e| anyhow!("Failed to parse target commit count: {}", e))?;

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

  let output = git_executor.execute_command(&args, repo_path).map_err(|e| anyhow!("Failed to batch rev-parse: {}", e))?;

  // Parse output - one hash per line, in the same order as input refs
  let hashes: Vec<&str> = output.lines().collect();

  if hashes.len() != refs.len() {
    // If some refs failed, fall back to individual resolution
    for &ref_str in refs {
      match git_executor.execute_command(&["rev-parse", ref_str], repo_path) {
        Ok(hash) => {
          result.insert(ref_str.to_string(), hash.trim().to_string());
        }
        Err(_) => {
          // For parent refs that don't exist (first commit), use empty tree
          if ref_str.ends_with("^") {
            result.insert(ref_str.to_string(), "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string());
          }
        }
      }
    }
  } else {
    // Success - map refs to their hashes
    for (i, &ref_str) in refs.iter().enumerate() {
      result.insert(ref_str.to_string(), hashes[i].to_string());
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
  let parent_ref_strs: Vec<&str> = parent_refs.iter().map(|s| s.as_str()).collect();

  let resolved_parents = batch_rev_parse(git_executor, repo_path, &parent_ref_strs)?;

  let mut parent_commits: HashSet<String> = HashSet::new();
  for parent_ref in &parent_refs {
    if let Some(parent_hash) = resolved_parents.get(parent_ref) {
      parent_commits.insert(parent_hash.clone());
    }
  }

  // Batch fetch all file contents for all commits at once
  let mut all_contents: HashMap<String, HashMap<String, String>> = HashMap::new();

  for (commit_hash, files) in &all_files_by_commit {
    let files_vec: Vec<String> = files.iter().cloned().collect();
    let contents = get_files_content_at_commit(git_executor, repo_path, commit_hash, &files_vec)?;
    all_contents.insert(commit_hash.clone(), contents);
  }

  // Also fetch parent contents
  for parent_hash in &parent_commits {
    let all_files: Vec<String> = all_files_by_commit
      .values()
      .flat_map(|files| files.iter().cloned())
      .collect::<HashSet<_>>()
      .into_iter()
      .collect();
    let contents = get_files_content_at_commit(git_executor, repo_path, parent_hash, &all_files)?;
    all_contents.insert(parent_hash.clone(), contents);
  }

  // Now generate diffs for each commit
  for (commit_hash, files) in commit_files_map {
    let parent_ref = format!("{commit_hash}^");
    let parent_hash = resolved_parents
      .get(&parent_ref)
      .cloned()
      .unwrap_or_else(|| "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string());

    // Get the unified diff for all files at once
    let mut show_args = vec!["-c", "merge.conflictStyle=zdiff3", "show", "--no-color", "--format=", "--unified=3", commit_hash, "--"];
    show_args.extend(files.iter().map(|s| s.as_str()));

    let diff_output = git_executor.execute_command(&show_args, repo_path)?;

    // Parse the diff output
    let mut file_to_diff: HashMap<String, String> = HashMap::new();
    let mut current_file_diff = String::new();
    let mut current_file: Option<&str> = None;

    for line in diff_output.lines() {
      if line.starts_with("diff --git") {
        if let Some(file) = current_file {
          file_to_diff.insert(file.to_string(), current_file_diff.clone());
        }
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

    if let Some(file) = current_file {
      file_to_diff.insert(file.to_string(), current_file_diff);
    }

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
  let mut oid_to_files: HashMap<String, Vec<String>> = HashMap::new();
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
        oid_to_files.entry(oid.to_string()).or_default().push(file_path.to_string());
        object_requests.push(oid);
      }
    }
  }

  // Use git cat-file --batch to get all contents at once
  debug!(objects_to_fetch = object_requests.len(), "found objects to fetch");

  if !object_requests.is_empty() {
    // Deduplicate OIDs while keeping track of which files need which OID
    let mut unique_oids = Vec::new();
    let mut seen_oids = HashSet::new();

    for oid in &object_requests {
      if seen_oids.insert(oid.to_string()) {
        unique_oids.push(*oid);
      }
    }

    debug!(unique_objects = unique_oids.len(), "deduped objects to fetch");

    let fetched_contents = execute_batch_cat_file(git_executor, repo_path, &unique_oids, None)?;

    // Map content back to all files that use each OID
    for (oid, content) in fetched_contents {
      // Find all files that use this OID
      if let Some(file_paths) = oid_to_files.get(&oid) {
        for file_path in file_paths {
          contents.insert(file_path.clone(), content.clone());
        }
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
pub(crate) fn execute_batch_cat_file(git_executor: &GitCommandExecutor, repo_path: &str, oids: &[&str], labeled_oids: Option<&[(&str, &str)]>) -> Result<HashMap<String, String>> {
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
      debug!("Cat-file batch output length: {}, raw: {:?}", batch_output.len(), batch_output);

      // Parse batch output
      // With -Z flag: <oid> <type> <size>\0<content>\0
      // The output is a series of: header\0content\0
      let mut i = 0;
      let mut oid_index = 0;
      let bytes = batch_output.as_bytes();

      while i < bytes.len() && oid_index < oids.len() {
        // Find next null byte for the header
        let header_end = bytes[i..].iter().position(|&b| b == 0).map(|pos| i + pos);
        if let Some(end) = header_end {
          let header = std::str::from_utf8(&bytes[i..end]).map_err(|e| anyhow!("Invalid UTF-8 in header: {}", e))?;
          debug!("Processing header at position {}: {:?}", i, header);

          if let Some((oid, size)) = parse_cat_file_header(header) {
            debug!("Parsed header: oid={}, size={}", oid, size);

            // Move past the header and its null terminator
            i = end + 1;

            // Read the content (size bytes)
            if i + size <= bytes.len() {
              let content_bytes = &bytes[i..i + size];
              let content = std::str::from_utf8(content_bytes).map_err(|e| anyhow!("Invalid UTF-8 in content: {}", e))?;
              debug!("Found content for oid {}: length={}", oid, content.len());

              let expected_oid = oids[oid_index];
              let label = labeled_oids.and_then(|lo| lo.get(oid_index).map(|(_, label)| label)).unwrap_or(&"");
              if oid == expected_oid {
                debug!(label = %label, oid = %oid, size = size, "storing content");
                contents.insert(oid.to_string(), content.to_string());
              } else {
                return Err(anyhow!("OID mismatch: expected {}, got {}", expected_oid, oid));
              }

              // Move past the content and its null terminator
              i += size;
              if i < bytes.len() && bytes[i] == 0 {
                i += 1;
              }

              oid_index += 1;
            } else {
              return Err(anyhow!("Insufficient data for content: expected {} bytes, but only {} remaining", size, bytes.len() - i));
            }
          } else if header.contains("missing") {
            debug!("Object missing: {}", header);
            i = end + 1;
            oid_index += 1;
          } else {
            return Err(anyhow!("Failed to parse git cat-file header: {:?}", header));
          }
        } else {
          // No more null terminators found
          break;
        }
      }
    }
    Err(e) => {
      return Err(anyhow!("Failed to cat-file batch: {}", e));
    }
  }

  debug!("Returning {} contents from batch cat-file", contents.len());
  Ok(contents)
}
