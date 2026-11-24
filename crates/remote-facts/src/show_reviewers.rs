//! Whom a review after landing asks when the user names nobody (docs/design/landing.md, "Show: a review after landing";
//! openspec/specs/branch/shown-reviews/spec.md, "Reviewers"): the owner groups of the paths the branch changes, by the
//! `OWNERSHIP` files of the target (`git_ops::ownership`, the groups Patronus resolves too), their members and
//! reviewers from the `CodeOwners` service (`codeowners::client`, asked with the Space token of Patronus), one of each
//! group by `domain_policy::shown::show_candidates`, never the author. `brd safe-push --request-review` and the picker
//! of the app's Safe Push and Request Review… (`get_show_reviewers`) read them alike: two git processes (the changed
//! paths, the `OWNERSHIP` files) and one lookup per group.

use codeowners::client::{CodeOwnersClient, CodeOwnersError};
use domain_policy::shown::{ShowCandidates, show_candidates};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::ownership::read_file_owners;
use git_ops::paths::changed_paths_since_fork;
use std::collections::BTreeSet;
use std::sync::Arc;

/// The `CodeOwners` service of a host and the Space token it is asked with, the one Patronus takes
#[derive(Clone)]
pub struct CodeOwnersAccess {
  pub client: Arc<CodeOwnersClient>,
  pub token: String,
}

/// The owner groups of the paths the commits up to `tip` change against `target_ref` (`refs/remotes/origin/master`),
/// each once, in the order of their names; none where no `OWNERSHIP` file names an owner of them
///
/// # Errors
/// A failure of git
pub fn owner_groups(git: &GitCommandExecutor, repository_path: &str, target_ref: &str, tip: &str) -> anyhow::Result<Vec<String>> {
  let changed = changed_paths_since_fork(git, repository_path, target_ref, tip, |_| true)?;
  let paths: Vec<String> = changed.iter().map(|changed| String::from_utf8_lossy(changed.path()).into_owned()).collect();
  let owners: BTreeSet<String> = read_file_owners(git, repository_path, target_ref, &paths)?.into_iter().map(|owner| owner.owner).collect();
  Ok(owners.into_iter().collect())
}

/// One reviewer of each of `groups` for a review after landing by `author` (a GitHub login), of their members and
/// reviewers as the service answers them, one lookup per group in their order
///
/// # Errors
/// The first lookup that failed: a rejected token, a failure of the service or of the network
pub async fn lookup_candidates(client: &CodeOwnersClient, token: &str, groups: &[String], author: &str) -> Result<ShowCandidates, CodeOwnersError> {
  let mut owner_groups = Vec::with_capacity(groups.len());
  for name in groups {
    owner_groups.push(client.lookup_group(token, name).await?);
  }
  Ok(show_candidates(&owner_groups, author))
}
