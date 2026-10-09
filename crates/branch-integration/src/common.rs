use super::cache::deserialize_from_cache;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use sync_types::branch_integration::BranchIntegrationInfo;
use tracing::{debug, instrument, warn};

/// Structure to hold all branch data from a single git query
pub struct BranchData {
  pub virtual_commits: HashMap<String, String>,             // virtual branch -> commit
  pub archived_all: Vec<String>,                            // all archived branches
  pub archived_today_names: HashSet<String>,                // just names (not full paths) for conflict check
  pub branch_notes: HashMap<String, BranchIntegrationInfo>, // commit -> parsed detection cache
  pub all_branch_commits: HashMap<String, String>,          // ALL branches -> commit (includes both virtual and archived)
}

/// Get all virtual and archived branches with their notes
#[instrument(skip(git_executor))]
pub fn get_all_branch_data(git_executor: &GitCommandExecutor, repo_path: &str, branch_prefix: &str) -> Result<BranchData> {
  // Get all branches with their commits in a single command
  let lines = git_executor.execute_command_lines(
    &[
      "for-each-ref",
      "--format=%(refname:short) %(objectname)",
      "--sort=-committerdate", // Minus sign means reverse order (newest first)
      &format!("refs/heads/{branch_prefix}/"),
    ],
    repo_path,
  )?;

  // Pre-compute today's archive prefix
  let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
  let today_archive_prefix = format!("{branch_prefix}/archived/{today}/");

  // Pre-allocate with estimated capacity
  let line_count = lines.len();
  let mut virtual_commits = HashMap::with_capacity(line_count / 2);
  let mut archived_all = Vec::with_capacity(line_count / 2);
  let mut archived_today_names = HashSet::new();
  let mut commit_set = HashSet::new();
  let mut all_branch_commits = HashMap::with_capacity(line_count);

  // First pass: collect branches and their commits
  for line in &lines {
    if let Some((branch, commit)) = line.split_once(' ') {
      debug!(
        branch = %branch,
        commit = %commit,
        "Found branch with commit"
      );
      commit_set.insert(commit.to_string());
      // Store ALL branch commits, not just virtual ones
      all_branch_commits.insert(branch.to_string(), commit.to_string());

      if branch.contains("/virtual/") {
        virtual_commits.insert(branch.to_string(), commit.to_string());
      } else if branch.contains("/archived/") {
        archived_all.push(branch.to_string());

        // Extract today's archive names for conflict resolution
        if let Some(name) = branch.strip_prefix(&today_archive_prefix) {
          archived_today_names.insert(name.to_string());
        }
      }
    }
  }

  // Second call: batch fetch all notes using git log --no-walk
  let mut branch_notes: HashMap<String, BranchIntegrationInfo> = HashMap::new();
  if !commit_set.is_empty() {
    // Build args for git log to fetch all notes in one call
    let notes_arg = format!("--notes={}", super::cache::NOTES_REF);
    let mut args = vec![
      "--no-pager",
      "log",
      "--no-walk",
      "--format=%H%x1f%N%x1e", // commit SHA, field separator, notes, record separator
      &notes_arg,
    ];

    // Add all commits as arguments directly from the set
    for commit in &commit_set {
      args.push(commit.as_str());
    }

    debug!(
      commits_to_fetch = commit_set.len(),
      commits = ?commit_set,
      "Fetching notes for commits"
    );

    // Execute the command to get all notes in one call
    if let Ok(output) = git_executor.execute_command(&args, repo_path) {
      debug!(output_length = output.len(), "Got notes output from git");
      // Parse the output: each record is "<commit>\x1f<note>\x1e"
      for record in output.split('\x1e') {
        let record = record.trim();
        if !record.is_empty()
          && let Some((commit, note)) = record.split_once('\x1f')
        {
          let note = note.trim();
          if note.is_empty() {
            debug!(commit = %commit, "Empty note for commit");
          } else {
            debug!(commit = %commit, "Found note for commit");
            // Parse the JSON note into DetectionCache immediately
            match deserialize_from_cache(note) {
              Ok(mut cache_info) => {
                // Find the branch name for this commit using all_branch_commits
                if let Some((branch_name, _)) = all_branch_commits.iter().find(|(_, c)| c == &commit) {
                  cache_info.name.clone_from(branch_name);
                }
                debug!(commit = %commit, "Successfully parsed detection cache");
                branch_notes.insert(commit.to_string(), cache_info);
              }
              Err(e) => {
                warn!(commit = %commit, note = %note, error = %e, "Failed to parse detection cache JSON");
              }
            }
          }
        } else if !record.is_empty() {
          debug!(record = %record, "Could not parse record");
        }
      }
    } else {
      debug!("Failed to fetch notes from git");
    }
  }

  debug!(
    virtual_count = virtual_commits.len(),
    archived_count = archived_all.len(),
    today_archived_count = archived_today_names.len(),
    unique_commits = commit_set.len(),
    notes_count = branch_notes.len(),
    "Fetched all branch data and notes in single call"
  );

  Ok(BranchData {
    virtual_commits,
    archived_all,
    archived_today_names,
    branch_notes,
    all_branch_commits,
  })
}
