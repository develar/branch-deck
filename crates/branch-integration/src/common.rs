use super::cache::{CachedDetection, deserialize_from_cache};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_names::{BranchKind, strip_branch_namespace};
use git_ops::notes::read_notes;
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

/// A branch under the prefix, as one `for-each-ref` lists it for the archive step and the sync
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedBranch {
  /// Without `refs/heads/`
  pub name: String,
  pub tip: String,
  /// Checked out in a worktree: `update-ref` would delete it from under it
  pub checked_out: bool,
  /// The subject of the tip
  pub subject: String,
}

/// The atoms of a `ListedBranch` after the full ref name, NUL-separated: tip, worktree, subject
pub const LISTED_BRANCH_ATOMS: &str = "%(objectname)%00%(worktreepath)%00%(contents:subject)";

impl ListedBranch {
  /// One line of `%(refname)%00` + `LISTED_BRANCH_ATOMS`, if the ref is a local branch
  pub fn parse(line: &str) -> Option<Self> {
    let mut fields = line.splitn(4, '\0');
    let (name, tip, worktree, subject) = (
      fields.next()?.strip_prefix("refs/heads/")?,
      fields.next()?,
      fields.next()?,
      fields.next().unwrap_or_default(),
    );
    Some(Self {
      name: name.to_string(),
      tip: tip.to_string(),
      checked_out: !worktree.is_empty(),
      subject: subject.to_string(),
    })
  }
}

/// The branches under `{branch_prefix}/`, newest commit first, with one `for-each-ref`
#[instrument(skip(git_executor))]
pub fn list_branches(git_executor: &GitCommandExecutor, repo_path: &str, branch_prefix: &str) -> Result<Vec<ListedBranch>> {
  let branch_prefix = branch_prefix.trim_end_matches('/');
  let lines = git_executor.execute_command_lines(
    &[
      "for-each-ref",
      &format!("--format=%(refname)%00{LISTED_BRANCH_ATOMS}"),
      // Newest first: `archived_all` keeps this order and the UI uses it until integration dates are known
      "--sort=-committerdate",
      &format!("refs/heads/{branch_prefix}/"),
    ],
    repo_path,
  )?;
  Ok(lines.iter().filter_map(|line| ListedBranch::parse(line)).collect())
}

/// Get all virtual and archived branches with their notes: `list_branches`, then `branch_data_of`
#[instrument(skip(git_executor))]
pub fn get_all_branch_data(git_executor: &GitCommandExecutor, repo_path: &str, branch_prefix: &str) -> Result<BranchData> {
  let branches = list_branches(git_executor, repo_path, branch_prefix)?;
  Ok(branch_data_of(git_executor, repo_path, branch_prefix, &branches))
}

/// The virtual and archived branches among `branches` (newest first, as `list_branches` lists them), with the detection
/// notes of their tips: one `log` for the notes
#[instrument(skip(git_executor, branches), fields(count = branches.len()))]
pub fn branch_data_of(git_executor: &GitCommandExecutor, repo_path: &str, branch_prefix: &str, branches: &[ListedBranch]) -> BranchData {
  let branch_prefix = branch_prefix.trim_end_matches('/');
  let today = utc_today();

  // Pre-allocate with estimated capacity
  let line_count = branches.len();
  let mut virtual_commits = HashMap::with_capacity(line_count / 2);
  let mut archived_all = Vec::with_capacity(line_count / 2);
  let mut archived_today_names = HashSet::new();
  let mut all_branch_commits = HashMap::with_capacity(line_count);
  let mut commit_subjects = HashMap::with_capacity(line_count);
  let mut checked_out = HashSet::new();

  // First pass: collect branches and their commits
  for ListedBranch {
    name: branch,
    tip: commit,
    checked_out: is_checked_out,
    subject,
  } in branches
  {
    let (branch, commit) = (branch.as_str(), commit.as_str());
    debug!(branch = %branch, commit = %commit, "Found branch with commit");
    commit_subjects.insert(commit.to_string(), subject.clone());
    // Store ALL branch commits, not just virtual ones
    all_branch_commits.insert(branch.to_string(), commit.to_string());
    if *is_checked_out {
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
  let commits: Vec<&str> = commit_subjects.keys().map(String::as_str).collect();
  debug!(commits_to_fetch = commits.len(), "Fetching notes for commits");
  match read_notes(git_executor, repo_path, super::cache::NOTES_REF, &commits) {
    Ok(notes) => {
      let commit_to_branch: HashMap<&str, &str> = all_branch_commits.iter().map(|(branch, commit)| (commit.as_str(), branch.as_str())).collect();
      for (commit, note) in notes {
        match deserialize_from_cache(&note) {
          Ok(mut cache_info) => {
            if let Some(branch_name) = commit_to_branch.get(commit.as_str()) {
              (*branch_name).clone_into(&mut cache_info.info.name);
            }
            branch_notes.insert(commit, cache_info);
          }
          Err(e) => {
            warn!(commit = %commit, note = %note, error = %e, "Failed to parse detection cache JSON");
          }
        }
      }
    }
    // every archive is detected again in full then, which is slow
    Err(e) => warn!(error = %e, "Failed to read the detection notes"),
  }

  debug!(
    virtual_count = virtual_commits.len(),
    archived_count = archived_all.len(),
    today_archived_count = archived_today_names.len(),
    unique_commits = commit_subjects.len(),
    notes_count = branch_notes.len(),
    "Fetched all branch data and notes"
  );

  BranchData {
    virtual_commits,
    archived_all,
    archived_today_names,
    branch_notes,
    all_branch_commits,
    commit_subjects,
    checked_out,
  }
}
