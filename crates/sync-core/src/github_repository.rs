//! The GitHub repository the virtual branches end up in, without HTTP: `branchdeck.githubRepository`, else the push URL
//! of `origin` when it is on github.com, else the GitHub mirror of the committed Space settings. The pull requests
//! (`pull_requests`), the review fetch (`review_fetch`) and Settings → Repository (`repository_config`) read it.

use crate::config_snapshot::ConfigSnapshot;
use crate::remote_url::{origin_push_url, parse_remote_url, split_owner_and_name};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};

/// `owner/name` of the GitHub repository of the branches, for a remote that mirrors pushed branches to GitHub without
/// saying so in its settings; it wins over everything else
pub const GITHUB_REPOSITORY_CONFIG: &str = "branchdeck.githubRepository";

/// A repository on github.com
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepository {
  pub owner: String,
  pub name: String,
}

impl GitHubRepository {
  pub fn new(owner: &str, name: &str) -> Self {
    Self {
      owner: owner.to_string(),
      name: name.to_string(),
    }
  }

  /// `owner/name`
  pub fn full_name(&self) -> String {
    format!("{}/{}", self.owner, self.name)
  }
}

/// The file of a Space repository's settings, committed in the repository; its `mirror.url` names the GitHub mirror
pub const SPACE_SETTINGS_FILE: &str = ".space/repository-settings.json";

/// Where `resolve_github_repository` found the GitHub repository
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum GitHubRepositorySource {
  /// `branchdeck.githubRepository`
  Configured,
  /// The push URL of `origin`
  Origin,
  /// `mirror.url` of the committed Space settings: `origin` is the Space repository
  SpaceMirror,
}

/// The GitHub repository the virtual branches end up in: `branchdeck.githubRepository` (`owner/name`), else the
/// repository `origin` pushes to (the virtual branches are pushed to origin) when it is on github.com, else the GitHub mirror the
/// committed Space settings name (`.space/repository-settings.json`, read from `HEAD`). None: the pull requests of the
/// branches are not on GitHub.
pub fn resolve_github_repository(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<GitHubRepository>> {
  Ok(resolve_github_repository_source(git_executor, repository_path)?.map(|(repository, _)| repository))
}

/// The repository of an `owner/name` the frontend passes back from an earlier answer
pub fn parse_known_github_repository(known: Option<&str>) -> Option<GitHubRepository> {
  let (owner, name) = split_owner_and_name(known?)?;
  Some(GitHubRepository::new(owner, name))
}

/// `resolve_github_repository` that also says where the repository came from
pub fn resolve_github_repository_source(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<(GitHubRepository, GitHubRepositorySource)>> {
  if let Some(configured) = read_configured_repository(git_executor, repository_path)? {
    return Ok(Some((configured, GitHubRepositorySource::Configured)));
  }
  detect_github_repository_source(git_executor, repository_path)
}

/// The GitHub repository without `branchdeck.githubRepository`: the one of `origin`, else the Space mirror (one or two
/// git processes); what Settings → Repository names as the detected one
pub fn detect_github_repository_source(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<(GitHubRepository, GitHubRepositorySource)>> {
  match origin_push_url(git_executor, repository_path)? {
    Some(url) => detect_github_repository_of(git_executor, repository_path, &url),
    None => Ok(None),
  }
}

/// `resolve_github_repository` from what a caller read: the configured repository (`ConfigSnapshot::github_repository`)
/// and the push URL of `origin` (`origin_push_url`); only the Space mirror costs a git process, when neither names one
pub fn resolve_github_repository_with(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  configured: Option<GitHubRepository>,
  origin_url: Option<&str>,
) -> Result<Option<GitHubRepository>> {
  if configured.is_some() {
    return Ok(configured);
  }
  let Some(url) = origin_url else {
    return Ok(None);
  };
  Ok(detect_github_repository_of(git_executor, repository_path, url)?.map(|(repository, _)| repository))
}

/// The GitHub repository of the push URL of `origin`, else the Space mirror the committed Space settings name (one
/// `cat-file`)
fn detect_github_repository_of(git_executor: &GitCommandExecutor, repository_path: &str, origin_url: &str) -> Result<Option<(GitHubRepository, GitHubRepositorySource)>> {
  if let Some(repository) = parse_github_repository(origin_url) {
    return Ok(Some((repository, GitHubRepositorySource::Origin)));
  }
  let (settings, exit_code) = git_executor.execute_command_with_status(&["cat-file", "blob", &format!("HEAD:{SPACE_SETTINGS_FILE}")], repository_path)?;
  if exit_code != 0 {
    return Ok(None);
  }
  Ok(parse_space_mirror(&settings).map(|repository| (repository, GitHubRepositorySource::SpaceMirror)))
}

/// The GitHub repository of `mirror.url` in Space repository settings; None without a mirror on github.com
pub fn parse_space_mirror(settings: &str) -> Option<GitHubRepository> {
  let settings: SpaceSettings = serde_json::from_str(settings).ok()?;
  parse_github_repository(&settings.mirror?.url?)
}

#[derive(Deserialize)]
struct SpaceSettings {
  mirror: Option<SpaceMirror>,
}

#[derive(Deserialize)]
struct SpaceMirror {
  url: Option<String>,
}

/// `branchdeck.githubRepository`, one `ConfigSnapshot`
fn read_configured_repository(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<GitHubRepository>> {
  Ok(ConfigSnapshot::read(git_executor, repository_path)?.github_repository())
}

/// The github.com repository of a remote URL: `git@github.com:o/r.git`, `ssh://git@github.com/o/r.git`,
/// `https://github.com/o/r(.git)`; None for another host or a local path.
pub fn parse_github_repository(url: &str) -> Option<GitHubRepository> {
  let remote = parse_remote_url(url)?;
  // `ssh.github.com` is SSH over port 443
  if !remote.host.eq_ignore_ascii_case("github.com") && !remote.host.eq_ignore_ascii_case("ssh.github.com") {
    return None;
  }
  let (owner, name) = split_owner_and_name(remote.path)?;
  Some(GitHubRepository::new(owner, name))
}
