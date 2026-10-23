//! The prefix a command names the branches with, the specific one first: the repository's own git prefix, the app's,
//! the global git one; `--prefix` of `bd` over all of them.

use domain_types::settings::{GitBranchPrefix, PrefixSource, ResolvedBranchPrefix};

/// The repository's own git prefix (local or worktree config, `git -c`), else the app's prefix
/// (`globalUserBranchPrefix`), else the global or system git prefix; each normalized, and one that nothing is left of
/// counts as unset. No prefix (`PrefixSource::None`) if none is set
#[must_use]
pub fn resolve_branch_prefix(git: &GitBranchPrefix, app: Option<&str>) -> ResolvedBranchPrefix {
  let git_source = if git.repository_scoped {
    PrefixSource::RepositoryConfig
  } else {
    PrefixSource::GlobalConfig
  };
  let git_prefix = ResolvedBranchPrefix::new(&git.prefix, git_source);
  if git_prefix.source == PrefixSource::RepositoryConfig {
    return git_prefix;
  }
  let app_prefix = ResolvedBranchPrefix::new(app.unwrap_or_default(), PrefixSource::AppSetting);
  if app_prefix.source == PrefixSource::None { git_prefix } else { app_prefix }
}

/// The prefix `--prefix` names, over every other one; no prefix when it is blank
#[must_use]
pub fn cli_branch_prefix(prefix: &str) -> ResolvedBranchPrefix {
  ResolvedBranchPrefix::new(prefix, PrefixSource::Cli)
}
