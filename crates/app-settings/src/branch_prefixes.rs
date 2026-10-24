//! The prefix every command of the app names the branches of a repository with, resolved in Rust per command
//! (`resolve_branch_prefix`): the app's prefix from the settings the command reads, the git prefix from a memo by
//! repository, so a command pays no git process for it. Each read of the git config refreshes the memo: the page's
//! `resolve_branch_prefix` (on open, on focus, after a change of a prefix), the config snapshot of every sync, a write of
//! `branchdeck.branchPrefix` in Settings → Repository.

use crate::resolve::resolve_branch_prefix;
use domain_types::settings::{AppSettings, GitBranchPrefix, ResolvedBranchPrefix};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The git prefix of each repository the app read it for, by repository path. Cloning shares it
#[derive(Clone, Default)]
pub struct BranchPrefixes {
  git: Arc<Mutex<HashMap<String, GitBranchPrefix>>>,
}

impl BranchPrefixes {
  /// The prefix of `repository_path` under `settings`, by the git prefix the memo has, else by the one `read_git` reads
  /// (kept for the next command). The memo is not held while git runs; an error of `read_git` keeps nothing
  pub fn resolve<E>(&self, settings: &AppSettings, repository_path: &str, read_git: impl FnOnce() -> Result<GitBranchPrefix, E>) -> Result<ResolvedBranchPrefix, E> {
    let known = self.lock().get(repository_path).cloned();
    let git = if let Some(git) = known {
      git
    } else {
      let git = read_git()?;
      self.lock().insert(repository_path.to_owned(), git.clone());
      git
    };
    Ok(resolve_branch_prefix(&git, settings.global_user_branch_prefix.as_deref()))
  }

  /// Keeps `git` as the git prefix of `repository_path`, as a read of its git config found it, and answers the prefix
  /// under `settings`
  pub fn refresh(&self, settings: &AppSettings, repository_path: &str, git: GitBranchPrefix) -> ResolvedBranchPrefix {
    let resolved = resolve_branch_prefix(&git, settings.global_user_branch_prefix.as_deref());
    self.lock().insert(repository_path.to_owned(), git);
    resolved
  }

  /// Forgets every git prefix: a write of the global git config changes the prefix of each repository that sets none
  /// of its own
  pub fn clear(&self) {
    self.lock().clear();
  }

  /// The memo; a poisoned lock keeps what it had, every entry is a whole value
  fn lock(&self) -> MutexGuard<'_, HashMap<String, GitBranchPrefix>> {
    self.git.lock().unwrap_or_else(PoisonError::into_inner)
  }
}
