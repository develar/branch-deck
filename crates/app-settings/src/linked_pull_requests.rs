//! The pull requests the user added to Reviews by their links (spec/repository/reviews.spec.md), kept by GitHub
//! repository in `linkedPullRequests` of `repository-state.json`, so every worktree of a repository has them: Rust reads
//! them when the session lists Reviews and writes them when the user adds or removes one, or GitHub answers one merged,
//! closed or missing. The newest link first, at most `MAX_LINKS` per repository.

use crate::file::LINKED_PULL_REQUESTS_KEY;
use crate::service::{SettingsError, SettingsService};
use domain_types::review_request::LinkedPullRequest;
use domain_types::settings::{SettingsChange, SettingsPatch};
use serde_json::{Map, Value};

/// The most links a repository keeps; an older one goes when a newer one comes
pub const MAX_LINKS: usize = 50;

/// The origin of the changes of the links
const ORIGIN: &str = "rust";

/// The links of `repository` (`owner/name`), the newest first; a value that doesn't read is none
#[must_use]
pub fn linked(service: &SettingsService, repository: &str) -> Vec<LinkedPullRequest> {
  let value = service.value(LINKED_PULL_REQUESTS_KEY);
  let Some(links) = value.get(repository).and_then(Value::as_array) else {
    return Vec::new();
  };
  links.iter().filter_map(|link| serde_json::from_value(link.clone()).ok()).collect()
}

/// `link` joins the links of `repository` as the newest one (a link it has moves to the front, with the new time)
///
/// # Errors
/// The service refused the change
pub fn link(service: &SettingsService, repository: &str, link: LinkedPullRequest) -> Result<SettingsChange, SettingsError> {
  let mut links = linked(service, repository);
  links.retain(|known| known.number != link.number);
  links.insert(0, link);
  links.truncate(MAX_LINKS);
  write(service, repository, &links)
}

/// The links of `numbers` leave `repository`; nothing is written when it has none of them
///
/// # Errors
/// The service refused the change
pub fn forget(service: &SettingsService, repository: &str, numbers: &[u32]) -> Result<Option<SettingsChange>, SettingsError> {
  let mut links = linked(service, repository);
  let before = links.len();
  links.retain(|link| !numbers.contains(&link.number));
  if links.len() == before {
    return Ok(None);
  }
  write(service, repository, &links).map(Some)
}

fn write(service: &SettingsService, repository: &str, links: &[LinkedPullRequest]) -> Result<SettingsChange, SettingsError> {
  let value = if links.is_empty() {
    Value::Null
  } else {
    serde_json::to_value(links).unwrap_or(Value::Null)
  };
  let mut fields = Map::new();
  fields.insert(repository.to_owned(), value);
  service.set(
    SettingsPatch::Fields {
      key: LINKED_PULL_REQUESTS_KEY.to_owned(),
      fields,
    },
    ORIGIN,
  )
}
