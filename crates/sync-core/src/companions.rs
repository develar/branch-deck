//! Companion repositories (openspec/specs/branch/declared-needs/spec.md#companion-repositories): the repositories whose branches a
//! need names as `<companion>:<branch>`. A primary lists its companions (`branchdeck.companion.<name>.path`), each
//! companion names its primary (`branchdeck.primary`); a need of a companion names the primary by the name of its
//! directory, and the other companions of that primary by their names. How a branch of a companion stands is read of
//! what its last sync left, never by a sync of it: its config, one listing of its branches under the prefix and the
//! integration notes of their tips (`branch_integration::common`), three processes per companion.

use crate::config_snapshot::ConfigSnapshot;
use branch_integration::common::{BranchData, branch_data_of, list_branches};
use domain_types::branch_integration::BranchIntegrationStatus;
use domain_types::branch_names::{BranchKind, kept_branch_name, strip_branch_namespace};
use domain_types::landing::NeedState;
use domain_types::session::{LinkRole, RepositoryLink};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::BTreeMap;
use std::path::Path;
use tracing::{debug, instrument};

/// How a branch a need names in a companion stands
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionNeed {
  /// The newest archived branch of that name is integrated into the baseline of the companion: the need is met
  Integrated,
  /// It is not in master: the need refuses the start, in this state
  Unlanded(NeedState),
}

/// How each branch of a companion that a need of `needed` names stands (`<companion>:<branch>` as it was declared),
/// read for the repository at `repository_path` with the config `snapshot`; no process while no need names a companion
pub fn read_companion_needs(git: &GitCommandExecutor, repository_path: &str, snapshot: &ConfigSnapshot, branch_prefix: &str, needed: &[String]) -> BTreeMap<String, CompanionNeed> {
  if needed.is_empty() {
    return BTreeMap::new();
  }
  let repositories = need_repositories(git, repository_path, snapshot);
  companion_needs(git, &repositories, branch_prefix, needed.iter().map(String::as_str))
}

/// The repositories a need of the repository of `snapshot` can name, name -> path: its companions; of a companion, its
/// primary (by the name of its directory) and the other companions of the primary, read of the config of the primary.
/// A primary whose config can not be read lends no names but its own
pub fn need_repositories(git: &GitCommandExecutor, repository_path: &str, snapshot: &ConfigSnapshot) -> BTreeMap<String, String> {
  let mut repositories = snapshot.companions().clone();
  if let Some(primary) = snapshot.primary() {
    let own = Path::new(repository_path);
    match ConfigSnapshot::read(git, primary) {
      Ok(of_primary) => repositories.extend(
        of_primary
          .companions()
          .iter()
          .filter(|(_, path)| !same_path(Path::new(path), own))
          .map(|(name, path)| (name.clone(), path.clone())),
      ),
      Err(error) => debug!(primary, error = %format!("{error:#}"), "the companions of the primary are unknown"),
    }
    repositories.insert(directory_name(primary), primary.to_string());
  }
  repositories
}

/// The repositories linked to this one: its companions, or its primary followed by that primary's other companions
pub fn repository_links(git: &GitCommandExecutor, repository_path: &str, snapshot: &ConfigSnapshot) -> Vec<RepositoryLink> {
  let primary = snapshot.primary();
  let mut links: Vec<_> = need_repositories(git, repository_path, snapshot)
    .into_iter()
    .map(|(name, path)| {
      let role = if primary.is_some_and(|primary| same_path(Path::new(primary), Path::new(&path))) {
        LinkRole::Primary
      } else {
        LinkRole::Companion
      };
      let problem = repository_problem(git, &path);
      RepositoryLink { name, path, role, problem }
    })
    .collect();
  links.sort_by(|a, b| link_role_order(a.role).cmp(&link_role_order(b.role)).then_with(|| a.name.cmp(&b.name)));
  links
}

const fn link_role_order(role: LinkRole) -> u8 {
  match role {
    LinkRole::Primary => 0,
    LinkRole::Companion => 1,
  }
}

fn repository_problem(git: &GitCommandExecutor, path: &str) -> Option<String> {
  if !Path::new(path).is_dir() {
    return Some(format!("{path} is gone"));
  }
  match git.execute_command_with_status(&["config", "--local", "--list"], path) {
    Ok((_, 0)) => None,
    Ok(_) => Some(format!("{path} is no git repository")),
    Err(error) => Some(format!("{path} is no git repository: {error:#}")),
  }
}

/// The name a repository goes by in the needs of its companions: the name of its directory
pub fn directory_name(path: &str) -> String {
  Path::new(path).file_name().map_or_else(|| path.to_string(), |name| name.to_string_lossy().into_owned())
}

fn same_path(a: &Path, b: &Path) -> bool {
  a == b || std::fs::canonicalize(a).ok().zip(std::fs::canonicalize(b).ok()).is_some_and(|(a, b)| a == b)
}

/// The state of each branch of a companion that `needed` names (`<companion>:<branch>`, as it was declared), by the
/// branch prefix of the companion (`branchdeck.branchPrefix` of its own config, else `branch_prefix`). A name of no companion of
/// `repositories`, and a companion that can not be read (its path is gone, it is no git repository), are
/// `CompanionUnreadable` with the reason
#[instrument(skip(git, repositories, needed))]
pub fn companion_needs<'a>(
  git: &GitCommandExecutor,
  repositories: &BTreeMap<String, String>,
  branch_prefix: &str,
  needed: impl IntoIterator<Item = &'a str>,
) -> BTreeMap<String, CompanionNeed> {
  let mut by_companion: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
  for need in needed {
    if let Some((companion, branch)) = need.split_once(':') {
      by_companion.entry(companion).or_default().push((need, branch));
    }
  }
  let mut states = BTreeMap::new();
  for (companion, needs) in by_companion {
    let read = match repositories.get(companion) {
      None => Err(format!("no companion repository is named {companion}")),
      Some(path) => read_companion(git, path, branch_prefix),
    };
    for (need, branch) in needs {
      let state = match &read {
        Ok((prefix, data)) => state_of(data, prefix, branch),
        Err(reason) => CompanionNeed::Unlanded(NeedState::CompanionUnreadable {
          companion: companion.to_string(),
          reason: reason.clone(),
        }),
      };
      states.insert(need.to_string(), state);
    }
  }
  states
}

/// The branch prefix of the companion at `path` and its branches under it with the notes of their tips; the reason it
/// can not be read
fn read_companion(git: &GitCommandExecutor, path: &str, branch_prefix: &str) -> Result<(String, BranchData), String> {
  if !Path::new(path).is_dir() {
    return Err(format!("{path} is gone"));
  }
  let snapshot = ConfigSnapshot::read(git, path).map_err(|error| format!("{path} is no git repository: {error:#}"))?;
  let prefix = prefix_of(&snapshot, branch_prefix);
  let listed = list_branches(git, path, &prefix).map_err(|error| format!("{error:#}"))?;
  let data = branch_data_of(git, path, &prefix, &listed);
  Ok((prefix, data))
}

/// The branch prefix of the companion at `path`: of its own config, as the prefix of a repository wins over the app's,
/// else `branch_prefix`, this repository's; `branch_prefix` too when its config can not be read
pub fn companion_prefix(git: &GitCommandExecutor, path: &str, branch_prefix: &str) -> String {
  ConfigSnapshot::read(git, path).map_or_else(|_| branch_prefix.to_string(), |snapshot| prefix_of(&snapshot, branch_prefix))
}

fn prefix_of(snapshot: &ConfigSnapshot, branch_prefix: &str) -> String {
  let own = snapshot.branch_prefix();
  if own.repository_scoped && !own.prefix.is_empty() {
    own.prefix
  } else {
    branch_prefix.to_string()
  }
}

/// `branch` among the branches of a companion: grouped (its virtual branch), unapplied, its newest archived branch
/// integrated or not, or none of them
fn state_of(data: &BranchData, prefix: &str, branch: &str) -> CompanionNeed {
  let named = |full: &str, kind: BranchKind| strip_branch_namespace(full, prefix, kind) == Some(branch);
  if data.virtual_commits.keys().any(|full| named(full, BranchKind::Virtual)) {
    return CompanionNeed::Unlanded(NeedState::Active);
  }
  if data.all_branch_commits.keys().any(|full| named(full, BranchKind::Unapplied)) {
    return CompanionNeed::Unlanded(NeedState::Unapplied);
  }
  // `archived_all` is newest first
  let newest = data.archived_all.iter().find(|full| kept_branch_name(full, prefix) == Some(branch));
  let integrated = newest
    .and_then(|full| data.all_branch_commits.get(full))
    .and_then(|tip| data.branch_notes.get(tip))
    .is_some_and(|note| matches!(note.info.status, BranchIntegrationStatus::Integrated { .. }));
  if integrated {
    CompanionNeed::Integrated
  } else {
    CompanionNeed::Unlanded(NeedState::Missing)
  }
}

#[cfg(test)]
#[path = "companions_test.rs"]
mod companions_test;
