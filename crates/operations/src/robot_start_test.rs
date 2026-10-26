use crate::robot_start::{CommentStart, Commented, RobotsSince, start_by_comment};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::robot;
use domain_types::patronus::{PatronusPullRequestLink, PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use github::fake_github::{FakeAnswer, FakeGitHub, start_fake_service};
use github::service::GitHubService;
use pretty_assertions::assert_eq;
use remote_facts::fact_sources::Read;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use sync_core::github_repository::GitHubRepository;
use sync_utils::clock::TestClock;

const COMMENTS: &str = "/repos/acme/monorepo/issues/7/comments";
const COMMENT: &str = "/repos/acme/monorepo/issues/comments/4402";

fn repository() -> GitHubRepository {
  GitHubRepository {
    owner: "acme".to_owned(),
    name: "monorepo".to_owned(),
  }
}

/// The fake GitHub where pull request #7 takes the comment 4402, which has `reactions`
async fn github_with_comment(reactions: serde_json::Value) -> (Arc<FakeGitHub>, GitHubService) {
  let (fake, service) = start_fake_service("token").await;
  let comment = json!({"id": 4402, "html_url": "https://github.com/acme/monorepo/pull/7#issuecomment-4402", "created_at": "2026-10-14T09:30:00Z"});
  fake.answer("POST", COMMENTS, FakeAnswer::status(201, comment.clone()));
  let mut answered = comment;
  answered["reactions"] = reactions;
  fake.answer("GET", COMMENT, FakeAnswer::ok(answered));
  (fake, service)
}

/// A robot of pull request #7 in `mode`
fn of_seven(id: &str, mode: PatronusPushMode, start_time: u32) -> PatronusRobot {
  PatronusRobot {
    push_mode: mode,
    pull_request: Some(PatronusPullRequestLink {
      number: 7,
      url: "https://github.com/acme/monorepo/pull/7".to_owned(),
    }),
    ..robot(id, Some("auth"), PatronusRobotStatus::Running, start_time)
  }
}

/// The comment of a Safe Push on #7, `known` the robots known before it, `robots` what Patronus lists after it
async fn comment(github: &GitHubService, clock: &TestClock, known: &[&str], robots: Vec<PatronusRobot>) -> Result<Commented, ApiError> {
  let repository = repository();
  let start = CommentStart {
    github,
    repository_path: "/repository",
    repository: &repository,
    number: 7,
    mode: PatronusPushMode::RebaseAutosquash,
    known: known.iter().map(ToString::to_string).collect::<HashSet<_>>(),
    until_listed: true,
  };
  let listed = move |_since: u32| -> Read<'_, Result<Vec<PatronusRobot>, ApiError>> {
    let robots = robots.clone();
    Box::pin(async move { Ok(robots) })
  };
  let listed: RobotsSince<'_> = &listed;
  start_by_comment(start, clock, listed).await
}

/// The bot accepts the comment: its robot is the newest robot of the pull request in the mode of the start that nobody
/// knew before the comment, never the Safe Push the start replaced nor a Dry Run of the same pull request
#[tokio::test]
async fn a_comment_the_bot_accepts_starts_the_newest_robot_of_its_pull_request_nobody_knew() {
  let (fake, github) = github_with_comment(json!({"+1": 1, "-1": 0})).await;
  let robots = vec![
    of_seven("replaced", PatronusPushMode::RebaseAutosquash, 300),
    of_seven("dry-run", PatronusPushMode::DryRun, 400),
    of_seven("older", PatronusPushMode::RebaseAutosquash, 100),
    of_seven("started", PatronusPushMode::RebaseAutosquash, 200),
  ];
  let commented = comment(&github, &TestClock::new(), &["replaced"], robots).await.unwrap();
  assert_eq!(
    (commented.accepted, commented.robot.map(|robot| robot.id), commented.comment_url.as_str()),
    (true, Some("started".to_owned()), "https://github.com/acme/monorepo/pull/7#issuecomment-4402")
  );
  assert_eq!(fake.requests_to("POST", COMMENTS)[0].json(), json!({"body": "/patronus merge --keep-source-branch"}));
}

/// A rejection with a reason fails the start with it; a comment the bot doesn't answer is read on the clock of the
/// session until the minute after it ended, and is no start the run knows of then (pending)
#[tokio::test]
async fn a_comment_the_bot_rejects_fails_with_its_reason_and_one_it_never_answers_is_pending_after_the_minute() {
  let (fake, github) = github_with_comment(json!({"+1": 0, "-1": 1})).await;
  let reason = json!([{"id": 4403, "body": "⚠️ The branch has no approvals", "user": {"login": "patronus[bot]", "type": "Bot"}}]);
  fake.answer("GET", COMMENTS, FakeAnswer::ok(reason));
  let refused = comment(&github, &TestClock::new(), &[], Vec::new()).await.unwrap_err();
  assert_eq!(
    (refused.code, refused.message.as_str()),
    (ErrorCode::PatronusRefused, "Patronus rejected the comment on pull request #7: The branch has no approvals")
  );

  let (fake, github) = github_with_comment(json!({"+1": 0, "-1": 0})).await;
  let clock = TestClock::new();
  let pending = tokio::spawn({
    let clock = clock.clone();
    async move { comment(&github, &clock, &[], Vec::new()).await }
  });
  while !pending.is_finished() {
    clock.advance(Duration::from_secs(3));
    tokio::task::yield_now().await;
  }
  let commented = pending.await.unwrap().unwrap();
  assert_eq!((commented.accepted, commented.robot), (false, None));
  // read right after the comment, then on the ticks of the clock until the minute ended
  assert!(!fake.requests_to("GET", COMMENT).is_empty());
}
