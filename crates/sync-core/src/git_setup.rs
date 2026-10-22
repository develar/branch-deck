//! What the first run needs from git, before anything else runs: git itself, the identity Branch Deck writes notes and
//! rewritten commits with (one `git var`), and a branch prefix to suggest.

use domain_types::branch_names::sanitize_branch_name;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// What the first run needs from git, checked before anything else runs: git itself, the identity that Branch Deck
/// writes its notes and rewritten commits with, and a branch prefix to suggest
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct GitSetup {
  /// Git is missing or too old: a summary line, then how to install or upgrade it
  pub git_error: Option<String>,
  /// `git var GIT_COMMITTER_IDENT` failed: user.name or user.email is not set and git cannot guess them
  pub identity_error: Option<String>,
  /// The local part of the committer email as a branch name segment, empty if there is none
  pub suggested_branch_prefix: String,
}

/// One `git var` for the identity of the repository, or of the global config when no repository is selected
#[instrument(skip(git_executor))]
pub fn check_git_setup(git_executor: &GitCommandExecutor, repository_path: &str) -> GitSetup {
  if let Err(e) = git_executor.get_info() {
    return GitSetup {
      git_error: Some(format!("{e:#}")),
      identity_error: None,
      suggested_branch_prefix: String::new(),
    };
  }

  // an inaccessible repository is reported by the branch prefix load; the identity falls back to the global one
  let cwd = if !repository_path.is_empty() && std::path::Path::new(repository_path).is_dir() {
    repository_path
  } else {
    "."
  };
  match git_executor.execute_command(&["var", "GIT_COMMITTER_IDENT"], cwd) {
    Ok(ident) => GitSetup {
      git_error: None,
      identity_error: None,
      suggested_branch_prefix: suggest_branch_prefix(&ident),
    },
    Err(e) => GitSetup {
      git_error: None,
      identity_error: Some(format!("{e:#}")),
      suggested_branch_prefix: String::new(),
    },
  }
}

/// `Jane Doe <jane.doe@example.com> 1700000000 +0100` -> `jane.doe`
pub fn suggest_branch_prefix(ident: &str) -> String {
  let Some(email) = ident.split_once('<').and_then(|(_, rest)| rest.split_once('>')).map(|(email, _)| email) else {
    return String::new();
  };
  let local_part = email.split_once('@').map_or(email, |(local, _)| local);
  // `+tag` of a mail alias is not part of the name
  let local_part = local_part.split_once('+').map_or(local_part, |(name, _)| name);
  sanitize_branch_name(local_part).replace('/', "-")
}
