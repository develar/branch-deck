use super::cache::{CachedDetection, deserialize_from_cache};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::{BranchKind, strip_branch_namespace};
use std::collections::{HashMap, HashSet};
use sync_types::upstream_commit::UpstreamCommit;
use sync_utils::timestamp::utc_today;
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
  /// The branches checked out in a worktree: `update-ref` would delete them from under it, so they are neither
  /// archived nor cleaned up
  pub checked_out: HashSet<String>,
}

/// Get all virtual and archived branches with their notes
#[instrument(skip(git_executor))]
pub fn get_all_branch_data(git_executor: &GitCommandExecutor, repo_path: &str, branch_prefix: &str) -> Result<BranchData> {
  let branch_prefix = branch_prefix.trim_end_matches('/');
  // Get all branches with their tip commits, worktrees and subjects in a single command; `lstrip=2`, not `short`,
  // which is `heads/<name>` when a tag has the same name
  let lines = git_executor.execute_command_lines(
    &[
      "for-each-ref",
      "--format=%(refname:lstrip=2)%00%(objectname)%00%(worktreepath)%00%(contents:subject)",
      // Newest first: `archived_all` keeps this order and the UI uses it until integration dates are known
      "--sort=-committerdate",
      &format!("refs/heads/{branch_prefix}/"),
    ],
    repo_path,
  )?;

  let today = utc_today();

  // Pre-allocate with estimated capacity
  let line_count = lines.len();
  let mut virtual_commits = HashMap::with_capacity(line_count / 2);
  let mut archived_all = Vec::with_capacity(line_count / 2);
  let mut archived_today_names = HashSet::new();
  let mut all_branch_commits = HashMap::with_capacity(line_count);
  let mut commit_subjects = HashMap::with_capacity(line_count);
  let mut checked_out = HashSet::new();

  // First pass: collect branches and their commits
  for line in &lines {
    let mut fields = line.splitn(4, '\0');
    let (Some(branch), Some(commit), worktree) = (fields.next(), fields.next(), fields.next().unwrap_or_default()) else {
      continue;
    };
    debug!(branch = %branch, commit = %commit, "Found branch with commit");
    commit_subjects.insert(commit.to_string(), fields.next().unwrap_or_default().to_string());
    // Store ALL branch commits, not just virtual ones
    all_branch_commits.insert(branch.to_string(), commit.to_string());
    if !worktree.is_empty() {
      checked_out.insert(branch.to_string());
    }

    // by the namespace, not by a `/virtual/` anywhere: a group name may contain `virtual/` or `archived/`, and an
    // unapplied branch is neither
    if strip_branch_namespace(branch, branch_prefix, BranchKind::Virtual).is_some() {
      virtual_commits.insert(branch.to_string(), commit.to_string());
    } else if let Some(dated) = strip_branch_namespace(branch, branch_prefix, BranchKind::Archived) {
      archived_all.push(branch.to_string());

      // Extract today's archive names for conflict resolution
      if let Some(name) = dated.strip_prefix(today.as_str()).and_then(|rest| rest.strip_prefix('/')) {
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
    let args = ["log", "--no-walk", "--stdin", "--format=%H%x1f%N%x1e", &notes_arg];
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
      // every archive is detected again in full then, which is slow
      warn!("Failed to read the detection notes");
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
    checked_out,
  })
}
