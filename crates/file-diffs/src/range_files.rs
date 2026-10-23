//! The file lists of ranges (`base..head`, the review of a pull request or of a branch), shared by every window and by
//! the diffs of the range (`FileDiffs::stream`): one `diff-tree` of the two commits (`read_diff_files`) for each, kept
//! in memory, which concurrent readers of one range run once. A range is never written to the object cache: a
//! `commit_files` entry is keyed by one commit.

use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, DiffSubject, read_diff_files};
use moka::policy::EvictionPolicy;
use moka::sync::Cache;
use std::sync::Arc;

/// The bytes of the file lists the memory cache holds: a range of a thousand files is about a hundred KB
const MEMORY_BYTES: u64 = 16 << 20;

/// What a file of a list costs besides its paths: the counts, the status, the allocations
const FILE_OVERHEAD_BYTES: usize = 64;

/// What a list is cached by: the repository path and the subject id (`DiffSubject::id`)
type RangeKey = (String, String);

/// The memory cache, `FileDiffs::range_files`; the app manages a clone of it. Cloning is cheap and shares it
#[derive(Clone)]
pub struct RangeFilesCache {
  files: Cache<RangeKey, Arc<CommitFiles>>,
}

impl Default for RangeFilesCache {
  fn default() -> Self {
    Self {
      files: Cache::builder()
        .max_capacity(MEMORY_BYTES)
        .weigher(|_key: &RangeKey, files: &Arc<CommitFiles>| u32::try_from(files_bytes(files)).unwrap_or(u32::MAX))
        .eviction_policy(EvictionPolicy::lru())
        .build(),
    }
  }
}

impl RangeFilesCache {
  /// The files of `subject` from memory, else from one `diff-tree`, which concurrent readers of one subject run once.
  /// Call from a blocking context
  pub fn read(&self, git: &GitCommandExecutor, repository_path: &str, subject: &DiffSubject) -> Result<Arc<CommitFiles>> {
    self
      .files
      .try_get_with((repository_path.to_string(), subject.id()), || read_diff_files(git, repository_path, subject).map(Arc::new))
      .map_err(|e| anyhow!("{e:#}"))
  }

  /// The bytes the lists hold now
  #[cfg(test)]
  pub(crate) fn bytes(&self) -> u64 {
    self.files.run_pending_tasks();
    self.files.weighted_size()
  }
}

/// The bytes a file list holds: its paths and an overhead per file
fn files_bytes(files: &CommitFiles) -> usize {
  files
    .files
    .iter()
    .map(|file| file.path.len() + file.old_path.as_ref().map_or(0, String::len) + FILE_OVERHEAD_BYTES)
    .sum()
}

#[cfg(test)]
#[path = "range_files_test.rs"]
mod range_files_test;
