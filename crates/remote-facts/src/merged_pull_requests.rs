//! The merged pull requests of the user that may hold commits authored since a time, for `brd update`
//! (openspec/specs/cli/update/spec.md, "brd update asks GitHub for the merged pull requests once, only after a
//! conflict"): one GraphQL search of the slim fields an update pairs the squash of a pull request by (`mergedAt`,
//! `mergeCommit`), asked with the token chain. The app reads them with its listing (`pull_requests`), the 50 most
//! recently updated pull requests of the user; this search reads those merged since the commits, so the two may find
//! other pull requests. A failure is `None`: the update pairs as it does without them.

use crate::pull_requests::{GhPullRequest, Nodes, merged_pull_request, of_owner, visible_data};
use domain_types::pull_request::MergedPullRequest;
use github::service::{GitHubService, RequestKind};
use serde::Deserialize;
use sync_core::github_repository::GitHubRepository;
use sync_utils::timestamp::format_utc_date;
use tracing::{instrument, warn};

/// The search of the merged pull requests (`merged_pull_requests_search`), the fields the pairing reads; the owner and the
/// name of the repository are variables, as GitHub tells a repository the token can't see by them
pub const MERGED_PULL_REQUESTS_QUERY: &str = "query($q: String!, $owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) { viewerPermission }
  search(query: $q, type: ISSUE, first: 100) { nodes { ... on PullRequest {
    number headRefName headRefOid isCrossRepository headRepositoryOwner { login } mergedAt mergeCommit { oid author { email } }
  } } }
}";

/// A day before the oldest author time of the commits: the clocks of the authors and of GitHub differ, and GitHub's
/// `merged:` takes a day
const SLACK_SECONDS: u32 = 24 * 60 * 60;

/// The search of the user's pull requests of `repository` merged since a day before `since` (Unix seconds)
#[must_use]
pub fn merged_pull_requests_search(repository: &GitHubRepository, since: u32) -> String {
  format!(
    "repo:{} is:pr is:merged author:@me merged:>={} sort:updated-desc",
    repository.full_name(),
    format_utc_date(i64::from(since.saturating_sub(SLACK_SECONDS)))
  )
}

#[derive(Debug, Deserialize)]
struct MergedData {
  /// Null when the token can't see the repository
  repository: Option<serde_json::Value>,
  search: Option<Nodes<GhPullRequest>>,
}

/// The merged pull requests of the user in `repository` that GitHub names a merge commit of, merged since a day before
/// `since`: one request; `None` when GitHub could not be asked or refused (signed out, offline, no access), with a warning
#[instrument(skip(github))]
pub async fn list_merged_pull_requests(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, since: u32) -> Option<Vec<MergedPullRequest>> {
  let full_name = repository.full_name();
  let search = serde_json::json!({"q": merged_pull_requests_search(repository, since), "owner": repository.owner, "name": repository.name});
  let answer = github
    .ask(repository_path, &full_name, RequestKind::Listing, |client, token| {
      let (search, full_name) = (&search, full_name.as_str());
      async move {
        visible_data(client.graphql::<MergedData>(&token, MERGED_PULL_REQUESTS_QUERY, search).await?, full_name, |data| {
          data.repository.is_some()
        })
      }
    })
    .await;
  match answer {
    Ok(data) => Some(
      Nodes::into_nodes(data.search)
        .iter()
        .filter(|gh| of_owner(gh, &repository.owner))
        .filter_map(merged_pull_request)
        .collect(),
    ),
    Err(failure) => {
      warn!(?failure, "can't list the merged pull requests: the update pairs without them");
      None
    }
  }
}

#[cfg(test)]
#[path = "merged_pull_requests_test.rs"]
mod merged_pull_requests_test;
