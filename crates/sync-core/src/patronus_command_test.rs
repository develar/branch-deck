use crate::patronus_command::{GhComment, patronus_command_body, post_patronus_command, read_patronus_command_reply, rejection_reason};
use crate::pull_requests::GitHubRepository;
use github::fake_github::{FakeAnswer, start_fake_service};
use pretty_assertions::assert_eq;
use serde_json::json;
use sync_types::patronus::{PatronusCommandComment, PatronusCommandReply, PatronusCommandSubject, PatronusPushMode};
use test_log::test;

pub(crate) fn repository() -> GitHubRepository {
  GitHubRepository {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  }
}

#[test]
fn a_merge_keeps_the_source_branch_and_a_dry_run_is_plain() {
  let own = PatronusCommandSubject::OwnBranch;
  assert_eq!(
    patronus_command_body(PatronusPushMode::RebaseAutosquash, own).unwrap(),
    "/patronus merge --keep-source-branch"
  );
  assert_eq!(patronus_command_body(PatronusPushMode::DryRun, own).unwrap(), "/patronus dry-run");
  patronus_command_body(PatronusPushMode::SquashAll, own).unwrap_err();
}

#[test]
fn a_merge_of_a_reviewed_pull_request_deletes_its_branch() {
  let reviewed = PatronusCommandSubject::ReviewedPullRequest;
  assert_eq!(patronus_command_body(PatronusPushMode::RebaseAutosquash, reviewed).unwrap(), "/patronus merge");
  assert_eq!(patronus_command_body(PatronusPushMode::DryRun, reviewed).unwrap(), "/patronus dry-run");
}

#[test(tokio::test)]
async fn posts_the_command_as_a_regular_comment_of_the_pull_request() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer(
    "POST",
    "/repos/acme/monorepo/issues/7/comments",
    FakeAnswer::status(
      201,
      json!({"id": 4402154133_u64, "html_url": "https://github.com/acme/monorepo/pull/7#issuecomment-4402154133"}),
    ),
  );
  let comment = post_patronus_command(&github, ".", &repository(), 7, PatronusPushMode::RebaseAutosquash, PatronusCommandSubject::OwnBranch)
    .await
    .unwrap();
  assert_eq!(
    comment,
    PatronusCommandComment {
      id: "4402154133".to_string(),
      url: "https://github.com/acme/monorepo/pull/7#issuecomment-4402154133".to_string(),
    }
  );
  assert_eq!(
    fake.requests_to("POST", "/repos/acme/monorepo/issues/7/comments")[0].json(),
    json!({"body": "/patronus merge --keep-source-branch"})
  );
}

#[test(tokio::test)]
async fn a_comment_without_reactions_is_pending_and_a_thumbs_up_is_accepted() {
  let (fake, github) = start_fake_service("t").await;
  let path = "/repos/acme/monorepo/issues/comments/10";
  fake.answer(
    "GET",
    path,
    FakeAnswer::ok(json!({"id": 10, "created_at": "2026-10-15T10:00:00Z", "reactions": {"+1": 0, "-1": 0}})),
  );
  let reply = read_patronus_command_reply(&github, ".", &repository(), 7, "10").await.unwrap();
  assert_eq!(reply, PatronusCommandReply::Pending);

  fake.answer(
    "GET",
    path,
    FakeAnswer::ok(json!({"id": 10, "created_at": "2026-10-15T10:00:00Z", "reactions": {"+1": 1, "-1": 0}})),
  );
  let reply = read_patronus_command_reply(&github, ".", &repository(), 7, "10").await.unwrap();
  assert_eq!(reply, PatronusCommandReply::Accepted);
}

#[test(tokio::test)]
async fn a_thumbs_down_reads_the_reason_from_the_bot_comment() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer(
    "GET",
    "/repos/acme/monorepo/issues/comments/10",
    FakeAnswer::ok(json!({"id": 10, "created_at": "2026-10-15T10:00:00Z", "reactions": {"+1": 0, "-1": 1}})),
  );
  fake.answer(
    "GET",
    "/repos/acme/monorepo/issues/7/comments",
    FakeAnswer::ok(json!([
      {"id": 10, "body": "/patronus merge", "user": {"type": "User"}},
      {"id": 11, "body": "⚠️ Pull request #7 is a draft and cannot be merged.", "user": {"type": "Bot"}}
    ])),
  );
  let reply = read_patronus_command_reply(&github, ".", &repository(), 7, "10").await.unwrap();
  assert_eq!(
    reply,
    PatronusCommandReply::Rejected {
      reason: Some("Pull request #7 is a draft and cannot be merged.".to_string())
    }
  );
  let comments = fake.requests_to("GET", "/repos/acme/monorepo/issues/7/comments");
  assert_eq!(comments[0].query.as_deref(), Some("since=2026-10-15T10%3A00%3A00Z&per_page=100"));
}

#[test]
fn the_reason_is_a_later_bot_comment_and_a_user_warning_is_not_one() {
  let comments: Vec<GhComment> = serde_json::from_str(
    r#"[
      {"id": 9, "body": "⚠️ an older answer", "user": {"type": "Bot"}},
      {"id": 11, "body": "⚠️ a user quoting the bot", "user": {"type": "User"}},
      {"id": 12, "body": "🚀 started", "user": {"type": "Bot"}}
    ]"#,
  )
  .unwrap();
  assert_eq!(rejection_reason(&comments, 10), None);
}

#[test(tokio::test)]
async fn a_comment_id_that_is_not_a_number_asks_nothing() {
  let (fake, github) = start_fake_service("t").await;
  for id in ["", "10/../../user", "10?x=1"] {
    let error = read_patronus_command_reply(&github, ".", &repository(), 7, id).await.unwrap_err().to_string();
    assert!(error.contains("is not the id of a GitHub comment"), "{error}");
  }
  assert!(fake.requests().is_empty());
}
