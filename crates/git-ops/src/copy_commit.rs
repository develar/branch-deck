use crate::cache::TreeIdCache;
use crate::commit_list::Commit;
use crate::model::{BranchError, BranchSyncStatus, CommitSyncStatus};
use crate::notes::{MappingNote, NoteSource};
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use tracing::{debug, instrument, warn};

/// Custom error type for copy commit operations
#[derive(Debug)]
pub enum CopyCommitError {
  BranchError(BranchError),
  Other(anyhow::Error),
}

impl From<anyhow::Error> for CopyCommitError {
  fn from(err: anyhow::Error) -> Self {
    Self::Other(err)
  }
}

impl std::fmt::Display for CopyCommitError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::BranchError(BranchError::Generic(message)) => f.write_str(message),
      Self::BranchError(e) => write!(f, "Branch error: {e:?}"),
      Self::Other(e) => write!(f, "{e}"),
    }
  }
}

impl std::error::Error for CopyCommitError {}

// Progress information for logging and user feedback
#[derive(Debug)]
pub struct ProgressInfo<'a> {
  pub branch_name: &'a str,
  pub current_commit_idx: usize,
  pub total_commits_in_branch: usize,
  pub current_branch_idx: usize,
  pub total_branches: usize,
}

/// A commit of an existing virtual branch, read with the branch so that a mapping note can be checked against it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualCommit {
  /// The first parent
  pub parent: String,
  pub tree: String,
  pub author_name: String,
  pub author_email: String,
  pub author_timestamp: u32,
  /// The raw message (`%B`)
  pub message: String,
}

/// The commits of a virtual branch from `tip` down to `base`, by id, in one `git log`
#[instrument(skip(git_executor))]
pub fn read_virtual_commits(git_executor: &GitCommandExecutor, repo_path: &str, tip: &str, base: &str) -> anyhow::Result<HashMap<String, VirtualCommit>> {
  let exclude = format!("^{base}");
  let output = git_executor.execute_command_raw(
    &["log", "--format=%H%x1f%P%x1f%T%x1f%an%x1f%ae%x1f%at%x1f%B%x1e", "--end-of-options", tip, &exclude],
    repo_path,
  )?;
  let mut commits = HashMap::new();
  for record in output.split('\u{1e}') {
    let record = record.trim_start_matches('\n');
    if record.is_empty() {
      continue;
    }
    let mut fields = record.splitn(7, '\u{1f}');
    let (Some(id), Some(parents), Some(tree), Some(author_name), Some(author_email), Some(author_timestamp), Some(message)) =
      (fields.next(), fields.next(), fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
    else {
      return Err(anyhow!("Unexpected record of virtual commit: {record:?}"));
    };
    let Some(parent) = parents.split_whitespace().next() else { continue };
    commits.insert(
      id.to_string(),
      VirtualCommit {
        parent: parent.to_string(),
        tree: tree.to_string(),
        author_name: author_name.to_string(),
        author_email: author_email.to_string(),
        author_timestamp: author_timestamp.parse().map_err(|e| anyhow!("Bad author date of {id}: {e}"))?,
        message: message.to_string(),
      },
    );
  }
  Ok(commits)
}

// Parameters for creating or updating a commit
#[derive(Clone, Copy)]
pub struct CreateCommitParams<'a> {
  pub commit: &'a Commit,
  pub new_parent_oid: &'a str,
  pub reuse_if_possible: bool,
  pub repo_path: &'a str,
  /// Receives intermediate branch statuses (e.g. `AnalyzingConflict`)
  pub on_status: &'a dyn Fn(BranchSyncStatus),
  pub progress_info: &'a ProgressInfo<'a>,
  pub git_executor: &'a GitCommandExecutor,
  pub tree_id_cache: &'a TreeIdCache,
  /// Commits of the existing virtual branch by id; `None` copies the commit
  pub existing_virtual_commits: Option<&'a HashMap<String, VirtualCommit>>,
  /// Check the virtual commit of every mapping note by its tree, as for a note without a source (`--no-cache`)
  pub verify_notes: bool,
  /// Committer date (unix seconds) of a new commit: the same input then writes the same commit
  pub committer_timestamp: u32,
}

/// Result of copying one commit onto the virtual branch
#[derive(Debug)]
pub struct CopiedCommit {
  pub hash: String,
  pub status: CommitSyncStatus,
  /// The mapping note to record: for a created commit, and for a reused one whose note had no source yet
  pub note: Option<MappingNote>,
}

fn git_date(timestamp: u32, timezone: &str) -> String {
  if timezone.is_empty() {
    timestamp.to_string()
  } else {
    format!("{timestamp} {timezone}")
  }
}

/// The message of the copy: the subject without the grouping prefix, then the body
fn virtual_message(commit: &Commit) -> String {
  match commit.message.find('\n') {
    Some(body_start) => format!("{}{}", commit.stripped_subject, &commit.message[body_start..]),
    None => commit.stripped_subject.clone(),
  }
}

/// Whether a virtual commit has the message and the author a copy of `commit` would get
fn same_metadata(virtual_commit: &VirtualCommit, commit: &Commit, message: &str) -> bool {
  virtual_commit.message.trim_end() == message.trim_end()
    && virtual_commit.author_name == commit.author_name
    && virtual_commit.author_email == commit.author_email
    && virtual_commit.author_timestamp == commit.author_timestamp
}

// Create or update a commit based on an original commit
#[instrument(skip(params), fields(commit_id = %params.commit.id, branch = %params.progress_info.branch_name))]
pub fn create_or_update_commit(params: CreateCommitParams<'_>) -> Result<CopiedCommit, CopyCommitError> {
  use crate::cherry_pick::perform_fast_cherry_pick_with_context;
  let CreateCommitParams {
    commit,
    new_parent_oid,
    reuse_if_possible,
    repo_path,
    on_status,
    progress_info: _, // only read by the span
    git_executor,
    tree_id_cache,
    existing_virtual_commits,
    verify_notes,
    committer_timestamp,
  } = params;

  // the commit list read the tree with the commit
  let original_tree_id = if commit.tree_id.is_empty() {
    tree_id_cache.get_tree_id(git_executor, repo_path, &commit.id)?
  } else {
    tree_id_cache.insert(&commit.id, &commit.tree_id);
    commit.tree_id.clone()
  };
  let original_parent_tree_id = if let Some(parent_id) = &commit.parent_id {
    tree_id_cache.get_tree_id(git_executor, repo_path, parent_id)?
  } else {
    // No parent means this is the initial commit, use empty tree
    String::new()
  };
  let source = NoteSource {
    parent_tree: original_parent_tree_id.clone(),
    tree: original_tree_id.clone(),
  };
  let commit_message = virtual_message(commit);

  // the tree of the copy: the original's tree on the same parent tree, else a 3-way merge with conflict handling
  let copied_tree = || -> Result<String, CopyCommitError> {
    let new_parent_tree_id = tree_id_cache.get_tree_id(git_executor, repo_path, new_parent_oid)?;
    if original_parent_tree_id == new_parent_tree_id {
      debug!(commit_id = %commit.id, "parent tree matches, reusing original commit tree");
      return Ok(original_tree_id.clone());
    }
    debug!(commit_id = %commit.id, "parent tree differs, performing merge");
    let parent_id = commit
      .parent_id
      .as_deref()
      .ok_or_else(|| CopyCommitError::Other(anyhow!("Cannot cherry-pick root commit {}: it has no parent", commit.id)))?;
    perform_fast_cherry_pick_with_context(git_executor, repo_path, &commit.id, parent_id, new_parent_oid, on_status, tree_id_cache)
  };

  // the commit the note maps to is reused while it is still in the virtual branch, on the same parent, and is still a
  // copy of this commit: a note copied onto another commit (`git notes copy`, `notes.rewriteRef`) names another source
  let mut tree_id = None;
  if reuse_if_possible
    && let Some(mapped_id) = &commit.mapped_commit_id
    && let Some(virtual_commit) = existing_virtual_commits.and_then(|commits| commits.get(mapped_id))
    && virtual_commit.parent == new_parent_oid
  {
    let reused = |note: Option<MappingNote>| {
      // the next commit of the branch is copied onto this one
      tree_id_cache.insert(mapped_id, &virtual_commit.tree);
      CopiedCommit {
        hash: mapped_id.clone(),
        status: CommitSyncStatus::Unchanged,
        note,
      }
    };
    match &commit.mapped_source {
      _ if !same_metadata(virtual_commit, commit, &commit_message) => {
        warn!(original_id = %commit.id, mapped_id = %mapped_id, "mapping note is stale: the message or the author changed since the commit was copied");
      }
      Some(recorded) if *recorded != source => {
        warn!(original_id = %commit.id, mapped_id = %mapped_id, "mapping note is stale: the commit changed since it was copied");
      }
      Some(_) if !verify_notes => {
        debug!(original_id = %commit.id, mapped_id = %mapped_id, "reusing existing commit from git note");
        return Ok(reused(None));
      }
      recorded => {
        // a note without a source, or one not trusted: the mapped commit must have the tree a copy would get
        let tree = copied_tree()?;
        if tree == virtual_commit.tree {
          debug!(original_id = %commit.id, mapped_id = %mapped_id, "the mapped commit is a copy of this commit");
          let note = (recorded.as_ref() != Some(&source)).then(|| MappingNote {
            original: commit.id.clone(),
            mapped: mapped_id.clone(),
            source: source.clone(),
          });
          return Ok(reused(note));
        }
        warn!(original_id = %commit.id, mapped_id = %mapped_id, "mapping note is stale: the mapped commit has another tree");
        tree_id = Some(tree);
      }
    }
  } else if reuse_if_possible && let Some(mapped_id) = &commit.mapped_commit_id {
    debug!(original_id = %commit.id, mapped_id = %mapped_id, "mapped commit is gone or has another parent, will re-copy");
  }

  let tree_id = match tree_id {
    Some(tree_id) => tree_id,
    None => copied_tree()?,
  };

  // Create new commit using git commit-tree
  let commit_args = vec!["commit-tree", &tree_id, "-p", new_parent_oid, "-m", &commit_message];

  // `<unix seconds> <offset>` keeps the original timezone; without the offset git takes the local one
  let author_date = git_date(commit.author_timestamp, &commit.author_timezone);
  let committer_date = git_date(committer_timestamp, &commit.committer_timezone);

  let env_vars = vec![
    ("GIT_AUTHOR_NAME", commit.author_name.as_str()),
    ("GIT_AUTHOR_EMAIL", commit.author_email.as_str()),
    ("GIT_AUTHOR_DATE", &author_date),
    ("GIT_COMMITTER_NAME", "branch-deck"),
    ("GIT_COMMITTER_EMAIL", commit.author_email.as_str()),
    ("GIT_COMMITTER_DATE", &committer_date),
  ];

  let output = git_executor
    .execute_command_with_env(&commit_args, repo_path, &env_vars)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to create commit: {e}")))?;

  let new_commit_hash = output.trim().to_string();
  // the next commit of the branch is copied onto this one; its tree is known, so don't ask git for it
  tree_id_cache.insert(&new_commit_hash, &tree_id);

  // a re-copy that writes the commit the note maps to (the same input gives the same commit) keeps the branch as it is
  let status = if commit.mapped_commit_id.as_deref() == Some(new_commit_hash.as_str())
    && existing_virtual_commits.is_some_and(|commits| commits.get(&new_commit_hash).is_some_and(|existing| existing.parent == new_parent_oid))
  {
    CommitSyncStatus::Unchanged
  } else {
    CommitSyncStatus::Created
  };
  let note = (status == CommitSyncStatus::Created || commit.mapped_source.as_ref() != Some(&source)).then(|| MappingNote {
    original: commit.id.clone(),
    mapped: new_commit_hash.clone(),
    source,
  });

  Ok(CopiedCommit {
    hash: new_commit_hash,
    status,
    note,
  })
}
