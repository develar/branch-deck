//! The branch prefix to suggest where none names the branches: the local part of the committer email of git, which the
//! refusal of a session without a prefix carries (`no_branch_prefix`) for the welcome card. Git itself and the identity
//! are no check of their own: a command that finds no git answers `git_not_found`, a write that git cannot sign with
//! the user's name `no_identity`.

use domain_types::branch_names::sanitize_branch_name;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::instrument;

/// The prefix the identity of the user suggests (`suggest_branch_prefix`): one `git var GIT_COMMITTER_IDENT` in the
/// repository, else in the global config; empty when git names no email or cannot form the identity
#[instrument(skip(git_executor))]
pub fn prefix_suggestion(git_executor: &GitCommandExecutor, repository_path: &str) -> String {
  // an inaccessible repository is the error of the session; the identity falls back to the global one
  let cwd = if !repository_path.is_empty() && std::path::Path::new(repository_path).is_dir() {
    repository_path
  } else {
    "."
  };
  git_executor
    .execute_command(&["var", "GIT_COMMITTER_IDENT"], cwd)
    .map_or_default(|ident| suggest_branch_prefix(&ident))
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
