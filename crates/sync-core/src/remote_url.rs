//! The host and the repository path of a git remote URL, for the services that live on the host (GitHub, Space).

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
