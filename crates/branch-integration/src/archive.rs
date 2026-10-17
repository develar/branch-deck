use crate::detector::read_stack_bases;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, parse_single_commit};
use git_ops::commit_utils::COMMIT_RECORD_FORMAT;
use git_ops::model::{BranchKind, strip_branch_namespace};
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::{merge_base, resolve_commit};
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, PoisonError};
use sync_utils::timestamp::utc_today;
use tracing::{debug, info, instrument, warn};

/// Days after its archive date (in its name) that a fully integrated archived branch is kept; the first sync after
/// them deletes it, while an archived branch that is not fully integrated stays
pub const ARCHIVE_RETENTION_DAYS: u64 = 7;

/// Process-wide lock serializing branch renames into the archive and unapplied namespaces,
/// so that picking a free name and creating the ref happen atomically.
pub static REF_RENAME_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Branch names checked against git's directory/file rule: refs `a` and `a/b` can't both exist
#[derive(Debug, Default)]
pub struct RefNames {
  names: HashSet<String>,
  /// Every `/`-prefix of a name
  directories: HashSet<String>,
}

impl RefNames {
  pub fn insert(&mut self, name: &str) {
    for (index, _) in name.match_indices('/') {
      self.directories.insert(name[..index].to_string());
    }
    self.names.insert(name.to_string());
  }

  /// Whether a ref `name` can be created: no ref has that name, lies below it or is a directory level above it
  pub fn is_free(&self, name: &str) -> bool {
    !self.names.contains(name) && !self.directories.contains(name) && !name.match_indices('/').any(|(index, _)| self.names.contains(&name[..index]))
  }

  /// `name` if it is free, else the first free name with a `-1`, `-2`, … suffix; `None` after `max_suffix` tries.
  /// Below a taken name (`a` for `a/b`) the suffix goes on that level (`a-1/b`), where it resolves the conflict.
  pub fn unique(&self, name: &str, max_suffix: u32) -> Option<String> {
    if self.is_free(name) {
      return Some(name.to_string());
    }
    let level_end = name
      .match_indices('/')
      .map(|(index, _)| index)
      .find(|&index| self.names.contains(&name[..index]))
      .unwrap_or(name.len());
    let (level, rest) = name.split_at(level_end);
    (1..=max_suffix).map(|suffix| format!("{level}-{suffix}{rest}")).find(|candidate| self.is_free(candidate))
  }
}

/// First free branch name of `to_base`, `to_base-1`, `to_base-2`, ... read with one `for-each-ref`; a name is also
/// taken by a ref below it (`to_base/x`) or above it (`to_base` is `a/b` and `a` exists), see `RefNames::unique`.
/// Hold `REF_RENAME_MUTEX` until the ref is created, so that another rename can't pick the same name.
pub fn unique_branch_name(git: &GitCommandExecutor, repo: &str, to_base: &str) -> Result<String> {
  // every ref a candidate can conflict with is below the first level of `to_base` or of its suffixed variant
  let top = to_base.split('/').next().unwrap_or(to_base);
  let below_top = format!("refs/heads/{top}");
  let suffixed_top = format!("refs/heads/{top}-*");
  let below_suffixed_top = format!("refs/heads/{top}-*/**");
  let mut taken = RefNames::default();
  for name in git.execute_command_lines(&["for-each-ref", "--format=%(refname:lstrip=2)", &below_top, &suffixed_top, &below_suffixed_top], repo)? {
    taken.insert(&name);
  }
  taken
    .unique(to_base, 1000)
    .ok_or_else(|| anyhow::anyhow!("Too many existing branches named like {to_base}"))
}

/// Archive inactive branches using pre-fetched data
/// Returns a map of archived branch name -> commit SHA
#[instrument(skip(git_executor, branch_commits, existing_today_names), fields(branch_count = inactive_branches.len()))]
pub fn batch_archive_inactive_branches(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  inactive_branches: &[String],
  branch_commits: &HashMap<String, String>,
  existing_today_names: &HashSet<String>,
) -> Result<HashMap<String, String>> {
  if inactive_branches.is_empty() {
    return Ok(HashMap::new());
  }

  let date = utc_today();
  let archive_prefix = format!("{branch_prefix}/archived/{date}");

  // Lock to prevent race conditions
  let _guard = REF_RENAME_MUTEX.lock().unwrap_or_else(PoisonError::into_inner);

  // Build mappings with conflict resolution against existing archives and the current batch
  let mut used_names = RefNames::default();
  for name in existing_today_names {
    used_names.insert(name);
  }
  let mut archive_mappings = Vec::with_capacity(inactive_branches.len());

  for branch_name in inactive_branches {
    let simple_name = strip_branch_namespace(branch_name, branch_prefix, BranchKind::Virtual).unwrap_or(branch_name);

    let target_name = used_names.unique(simple_name, 100).unwrap_or_else(|| {
      warn!("Excessive naming conflicts for {}", simple_name);
      simple_name.to_string()
    });

    used_names.insert(&target_name);
    let archived_name = format!("{archive_prefix}/{target_name}");
    archive_mappings.push((branch_name.clone(), archived_name));
  }

  let mut transaction = RefTransaction::new();
  let mut newly_archived = HashMap::with_capacity(archive_mappings.len());

  for (original_branch, archived_branch) in &archive_mappings {
    if let Some(commit_hash) = branch_commits.get(original_branch) {
      debug!(
        original_branch = %original_branch,
        archived_branch = %archived_branch,
        commit_hash = %commit_hash,
        "Archiving branch with commit mapping"
      );
      // the delete only if the tip is still the one the sync read
      transaction
        .create(&format!("refs/heads/{archived_branch}"), commit_hash)
        .delete(&format!("refs/heads/{original_branch}"), Some(commit_hash));
      newly_archived.insert(archived_branch.clone(), commit_hash.clone());
    } else {
      warn!(branch = %original_branch, "Branch not found in commit map, skipping archive");
    }
  }

  // all of them or none
  transaction.commit(git_executor, repo_path, "archive")?;

  info!(
    archived_count = newly_archived.len(),
    archive_prefix = %archive_prefix,
    "Successfully archived branches in batch"
  );

  Ok(newly_archived)
}

/// Get commits for an archived branch (integrated or orphaned)
/// This function retrieves all commits from a virtual branch, regardless of whether
/// it's integrated or orphaned. For orphaned branches, the commits still exist in the
/// virtual branch even though the original source commits are gone from HEAD.
/// A stacked branch lists only its own commits, those after its stack base, as its detection counts them.
#[instrument(skip(git_executor))]
pub fn get_archived_branch_commits(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str, branch_name: &str, baseline_branch: &str) -> Result<Vec<Commit>> {
  // The branch_name is the full branch path (e.g., "user/archived/2025-08-11/feature-auth")
  if strip_branch_namespace(branch_name, branch_prefix, BranchKind::Archived).is_none() {
    anyhow::bail!("Not an archived branch under the configured branch prefix: {branch_name}");
  }
  let branch_ref = format!("refs/heads/{branch_name}");
  let tip = resolve_commit(git_executor, repository_path, &branch_ref)?.ok_or_else(|| anyhow::anyhow!("Archived branch {branch_name} does not exist"))?;
  let tip = tip.as_str();

  // Where the branch's own commits start: its stack base, else where it diverged from the baseline; a branch without
  // common history with the baseline lists all its commits
  let start = match read_stack_bases(git_executor, repository_path, &[tip]).remove(tip) {
    Some(base) => Some(base),
    None => merge_base(git_executor, repository_path, baseline_branch, tip)?,
  };

  let range = start.as_ref().map_or_else(|| tip.to_string(), |start| format!("{start}..{tip}"));
  debug!(branch = %branch_name, start = ?start, range = %range, "Getting all commits on branch since divergence");
  let pretty_format = format!("--pretty=format:{COMMIT_RECORD_FORMAT}");
  let output = git_executor.execute_command(&["log", "--reverse", "--no-merges", &pretty_format, "--end-of-options", &range, "--"], repository_path)?;

  // Parse commits
  let mut commits = Vec::new();
  for record in output.split('\x1e') {
    let record = record.trim();
    if record.is_empty() {
      continue;
    }
    match parse_single_commit(record) {
      Ok(commit) => commits.push(commit),
      Err(e) => warn!(branch = %branch_name, error = %e, "Cannot parse a commit of an archived branch"),
    }
  }

  debug!(branch = %branch_name, commit_count = commits.len(), "Retrieved commits from archived branch");
  Ok(commits)
}

/// Delete archived branches, `(name, tip)`, in one atomic `update-ref --stdin` transaction (no argv length limit), each
/// only if it still points at the tip read. Returns the number of branches deleted.
#[instrument(skip(git_executor, branches), fields(repo = %repo_path, branch_count = branches.len()))]
pub fn batch_delete_archived_branches(git_executor: &GitCommandExecutor, repo_path: &str, branches: &[(String, String)]) -> Result<usize> {
  if branches.is_empty() {
    return Ok(0);
  }

  // Lock to prevent race conditions with concurrent archive renames
  let _guard = REF_RENAME_MUTEX.lock().unwrap_or_else(PoisonError::into_inner);

  let mut transaction = RefTransaction::new();
  for (name, tip) in branches {
    transaction.delete(&format!("refs/heads/{name}"), Some(tip));
  }
  transaction.commit(git_executor, repo_path, "clean up archived branches")?;
  Ok(branches.len())
}

#[cfg(test)]
#[path = "archive_test.rs"]
mod archive_test;
