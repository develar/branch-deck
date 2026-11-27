use crate::context::{Environment, RepoContext, RepoOptions};
use crate::merged_pull_requests::AskedMergedPullRequests;
use domain_types::pull_request::MergedPullRequests;
use github::fake_github::{FakeAnswer, start_fake_server};
use github::token::TokenSources;
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

/// A block on the runtime of the command inside another runtime panics: an ask from within one answers none and sends
/// nothing; outside one it is one search, and a later ask of no older time takes its answer
#[test]
fn the_merged_pull_requests_are_not_asked_inside_a_runtime() {
  // the fake serves on a runtime of its own, outside the one of the command
  let server = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
  let (fake, url) = server.block_on(start_fake_server());
  fake.answer(
    "POST",
    "/graphql",
    FakeAnswer::ok(serde_json::json!({"data": {"repository": {"viewerPermission": "WRITE"}, "search": {"nodes": []}}})),
  );
  let repo = TestRepo::new();
  repo.create_commit("Initial", "a.txt", "a\n");
  repo.set_config("branchdeck.githubRepository", "o/r").unwrap();
  let environment = Environment {
    github_api_url: url,
    github_tokens: TokenSources::fixed("t0ken"),
    ..Environment::from_process()
  };
  let options = RepoOptions {
    repo: Some(repo.path()),
    prefix: Some("user"),
    full_ids: true,
    no_cache: true,
  };
  let context = RepoContext::resolve(&environment, &options).unwrap();

  let inside = AskedMergedPullRequests::new(&context, &environment);
  assert_eq!(server.block_on(async { inside.merged_since(1_700_000_000) }), None);
  assert_eq!(fake.requests_to("POST", "/graphql").len(), 0);

  let outside = AskedMergedPullRequests::new(&context, &environment);
  let answer = outside.merged_since(1_700_000_000).expect("an answer of GitHub");
  assert!(answer.merged.is_empty());
  assert_eq!(outside.merged_since(1_700_000_500), Some(answer));
  assert_eq!(fake.requests_to("POST", "/graphql").len(), 1);
}
