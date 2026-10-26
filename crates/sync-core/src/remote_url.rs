//! The host and the repository path of a git remote URL, for the services that live on the host (GitHub, Space).

use git_executor::git_command_executor::GitCommandExecutor;

/// The URL `origin` pushes to (the virtual branches are pushed to origin), `None` without that remote: one
/// `remote get-url --push`, which applies `pushurl` and `pushInsteadOf`. Read it once per operation: the Space
/// repository and the GitHub repository are both found by it
pub fn origin_push_url(git_executor: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<Option<String>> {
  let (url, exit_code) = git_executor.execute_command_with_status(&["remote", "get-url", "--push", "origin"], repository_path)?;
  Ok((exit_code == 0).then_some(url))
}

/// A remote URL split into the host and the repository path
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUrl<'a> {
  /// Without the user and the port
  pub host: &'a str,
  /// Without the surrounding slashes and the `.git` suffix, e.g. `ij/ultimate`
  pub path: &'a str,
}

/// `git@host:o/r.git`, `ssh://git@host[:port]/o/r.git`, `https://[user[:token]@]host/o/r(.git)`, `git://host/o/r`;
/// None for a local path.
pub fn parse_remote_url(url: &str) -> Option<RemoteUrl<'_>> {
  let url = url.trim();
  let (host, path) = if let Some((_, rest)) = url.split_once("://") {
    // scheme://[user@]host[:port]/path
    let (authority, path) = rest.split_once('/')?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    (host.split_once(':').map_or(host, |(host, _)| host), path)
  } else {
    // scp-like [user@]host:path; a slash before the colon makes it a local path
    let (authority, path) = url.split_once(':')?;
    if authority.contains('/') {
      return None;
    }
    (authority.rsplit_once('@').map_or(authority, |(_, host)| host), path)
  };
  let path = path.trim_matches('/');
  let path = path.strip_suffix(".git").unwrap_or(path);
  (!host.is_empty()).then_some(RemoteUrl { host, path })
}

/// The `owner/name` of a repository path; None for a path with fewer or more segments
pub fn split_owner_and_name(path: &str) -> Option<(&str, &str)> {
  let (owner, name) = path.split_once('/')?;
  (!owner.is_empty() && !name.is_empty() && !name.contains('/')).then_some((owner, name))
}
