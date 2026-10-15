use super::cache::{CachedDetection, deserialize_from_cache};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use sync_types::upstream_commit::UpstreamCommit;
use tracing::{debug, instrument, warn};

/// The fields of an `UpstreamCommit` in `log`/`rev-list` output: id, author, committer and commit time, NUL-separated
pub const UPSTREAM_COMMIT_FORMAT: &str = "%H%x00%an%x00%cn%x00%ct";

/// Parse one line of `UPSTREAM_COMMIT_FORMAT`
pub fn parse_upstream_commit(line: &str) -> Option<UpstreamCommit> {
  let mut fields = line.split('\0');
  let (id, author, committer, time) = (fields.next()?, fields.next()?, fields.next()?, fields.next()?);
  if id.is_empty() {
    return None;
  }
  Some(UpstreamCommit {
    id: id.to_string(),
    author: author.to_string(),
    committer: committer.to_string(),
    time: time.trim().parse().ok()?,
  })
}

/// The oldest commit that `git log <log_args>` lists (`log` lists newest first), `None` without one or on an error
pub fn oldest_logged_commit(git_executor: &GitCommandExecutor, repo_path: &str, log_args: &[&str]) -> Option<UpstreamCommit> {
  let format = format!("--format={UPSTREAM_COMMIT_FORMAT}");
  let mut args = vec!["log", format.as_str()];
  args.extend_from_slice(log_args);
  let lines = git_executor.execute_command_lines(&args, repo_path).ok()?;
  lines.last().and_then(|line| parse_upstream_commit(line))
}

/// Structure to hold all branch data from a single git query
pub struct BranchData {
  pub virtual_commits: HashMap<String, String>,       // virtual branch -> commit
  pub archived_all: Vec<String>,                      // all archived branches
  pub archived_today_names: HashSet<String>,          // just names (not full paths) for conflict check
  pub branch_notes: HashMap<String, CachedDetection>, // commit -> parsed detection cache
  pub all_branch_commits: HashMap<String, String>,    // ALL branches -> commit (includes both virtual and archived)
  pub commit_subjects: HashMap<String, String>,       // branch tip commit -> subject
}

/// Get all virtual and archived branches with their notes
#[instrument(skip(git_executor))]
pub fn get_all_branch_data(git_executor: &GitCommandExecutor, repo_path: &str, branch_prefix: &str) -> Result<BranchData> {
  // Get all branches with their tip commits and subjects in a single command
  let lines = git_executor.execute_command_lines(
    &[
      "for-each-ref",
      "--format=%(refname:short) %(objectname) %(contents:subject)",
      // Newest first: `archived_all` keeps this order and the UI uses it until integration dates are known
      "--sort=-committerdate",
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
  let mut all_branch_commits = HashMap::with_capacity(line_count);
  let mut commit_subjects = HashMap::with_capacity(line_count);

  // First pass: collect branches and their commits
  for line in &lines {
    let mut fields = line.splitn(3, ' ');
    let (Some(branch), Some(commit)) = (fields.next(), fields.next()) else {
      continue;
    };
    debug!(branch = %branch, commit = %commit, "Found branch with commit");
    commit_subjects.insert(commit.to_string(), fields.next().unwrap_or_default().to_string());
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

  // Second call: batch fetch all notes, passing the commits on stdin (no argv length limit)
  let mut branch_notes: HashMap<String, CachedDetection> = HashMap::new();
  if !commit_subjects.is_empty() {
    let notes_arg = format!("--notes={}", super::cache::NOTES_REF);
    let mut input = String::with_capacity(commit_subjects.len() * 41);
    for commit in commit_subjects.keys() {
      input.push_str(commit);
      input.push('\n');
    }
    debug!(commits_to_fetch = commit_subjects.len(), "Fetching notes for commits");

    // commit SHA, field separator, notes, record separator
    let args = ["--no-pager", "log", "--no-walk", "--stdin", "--format=%H%x1f%N%x1e", &notes_arg];
    if let Ok(output) = git_executor.execute_command_with_input(&args, repo_path, &input) {
      debug!(output_length = output.len(), "Got notes output from git");
      let commit_to_branch: HashMap<&str, &str> = all_branch_commits.iter().map(|(branch, commit)| (commit.as_str(), branch.as_str())).collect();
      // Parse the output: each record is "<commit>\x1f<note>\x1e"
      for record in output.split('\x1e') {
        let record = record.trim();
        if record.is_empty() {
          continue;
        }
        let Some((commit, note)) = record.split_once('\x1f') else {
          debug!(record = %record, "Could not parse record");
          continue;
        };
        let note = note.trim();
        if note.is_empty() {
          continue;
        }
        match deserialize_from_cache(note) {
          Ok(mut cache_info) => {
            if let Some(branch_name) = commit_to_branch.get(commit) {
              (*branch_name).clone_into(&mut cache_info.info.name);
            }
            branch_notes.insert(commit.to_string(), cache_info);
          }
          Err(e) => {
            warn!(commit = %commit, note = %note, error = %e, "Failed to parse detection cache JSON");
          }
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
    unique_commits = commit_subjects.len(),
    notes_count = branch_notes.len(),
    "Fetched all branch data and notes"
  );

  Ok(BranchData {
    virtual_commits,
    archived_all,
    archived_today_names,
    branch_notes,
    all_branch_commits,
    commit_subjects,
  })
}
