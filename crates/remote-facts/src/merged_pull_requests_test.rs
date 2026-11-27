use crate::merged_pull_requests::{MERGED_PULL_REQUESTS_QUERY, list_merged_pull_requests};
use domain_types::pull_request::MergedPullRequest;
use github::fake_github::{FakeAnswer, start_fake_service};
use pretty_assertions::assert_eq;
use sync_core::github_repository::GitHubRepository;
use test_log::test;

/// The merged pull requests since the commits are one search of the slim fields, its owner and name as variables, since a
/// day before the oldest author time; a fork's and one without a merge commit are none; a refusal of GitHub is `None`
#[test(tokio::test)]
async fn the_merged_pull_requests_since_the_commits_are_one_search() {
  let (fake, github) = start_fake_service("t0ken").await;
  let node = |number: u32, merge_commit: serde_json::Value, cross: bool| {
    serde_json::json!({"number": number, "headRefName": "develar/virtual/auth", "headRefOid": "1".repeat(40), "isCrossRepository": cross,
      "headRepositoryOwner": {"login": "o"}, "mergedAt": "2026-10-15T09:30:00Z", "mergeCommit": merge_commit})
  };
  let commit = serde_json::json!({"oid": "2".repeat(40), "author": {"email": "1+develar@users.noreply.github.com"}});
  let nodes = vec![node(11, commit.clone(), false), node(12, serde_json::Value::Null, false), node(13, commit, true)];
  fake.answer(
    "POST",
    "/graphql",
    FakeAnswer::ok(serde_json::json!({"data": {"repository": {"viewerPermission": "WRITE"}, "search": {"nodes": nodes}}})),
  );
  let repository = GitHubRepository::new("o", "r");
  // 2026-10-15T00:00:00Z
  let merged = list_merged_pull_requests(&github, ".", &repository, 1_792_022_400).await;
  assert_eq!(
    merged,
    Some(vec![MergedPullRequest {
      number: 11,
      head_ref_name: "develar/virtual/auth".to_string(),
      head_ref_oid: "1".repeat(40),
      merge_commit: "2".repeat(40),
      merge_author_email: Some("1+develar@users.noreply.github.com".to_string()),
      merged_at: 1_792_056_600,
    }])
  );
  let requests = fake.requests_to("POST", "/graphql");
  assert_eq!(requests.len(), 1);
  assert_eq!(requests[0].json()["query"], MERGED_PULL_REQUESTS_QUERY);
  assert_eq!(
    requests[0].json()["variables"],
    serde_json::json!({"q": "repo:o/r is:pr is:merged author:@me merged:>=2026-10-14 sort:updated-desc", "owner": "o", "name": "r"})
  );

  fake.answer("POST", "/graphql", FakeAnswer::status(401, serde_json::json!({"message": "Bad credentials"})));
  assert_eq!(list_merged_pull_requests(&github, ".", &repository, 1_792_022_400).await, None);
}
