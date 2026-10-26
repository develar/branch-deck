use crate::robot_start::{CommentStart, Commented, RobotWait, RobotsSince, start_by_comment};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::robot;
use domain_types::patronus::{PatronusCommandSubject, PatronusPullRequestLink, PatronusPushMode, PatronusRobot, PatronusRobotStatus};
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

/// A Safe Push on #7 by a comment, no robot known before it
fn start_on_seven<'a>(github: &'a GitHubService, repository: &'a GitHubRepository, subject: PatronusCommandSubject, robot_wait: RobotWait) -> CommentStart<'a> {
  CommentStart {
    github,
    repository_path: "/repository",
    repository,
    number: 7,
    mode: PatronusPushMode::RebaseAutosquash,
    known: HashSet::new(),
    subject,
    robot_wait,
  }
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
    subject: PatronusCommandSubject::OwnBranch,
    robot_wait: RobotWait::UntilListed,
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

/// A comment the bot accepted whose robot Patronus lists not within the minute: the app looks for it every 15 s after
/// the minute while its start holds the subject, and the robot listed later is the start's; none within the five minutes
/// after the minute is a start all the same, its robot comes with a later list of the session
#[tokio::test]
async fn an_accepted_comment_looks_for_its_robot_after_the_minute_until_patronus_lists_it() {
  let listed: Arc<std::sync::Mutex<Vec<PatronusRobot>>> = Arc::default();
  let lists = Arc::new(std::sync::atomic::AtomicUsize::new(0));
  let run = |github: GitHubService, clock: TestClock| {
    let (listed, lists) = (Arc::clone(&listed), Arc::clone(&lists));
    tokio::spawn(async move {
      let repository = repository();
      let start = start_on_seven(&github, &repository, PatronusCommandSubject::OwnBranch, RobotWait::UntilListed);
      let since = move |_since: u32| -> Read<'_, Result<Vec<PatronusRobot>, ApiError>> {
        lists.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let robots = listed.lock().unwrap().clone();
        Box::pin(async move { Ok(robots) })
      };
      let since: RobotsSince<'_> = &since;
      start_by_comment(start, &clock, since).await
    })
  };
  let advance = async |clock: &TestClock, by: Duration| {
    clock.advance(by);
    for _ in 0..50 {
      tokio::task::yield_now().await;
    }
  };

  let (_, github) = github_with_comment(json!({"+1": 1, "-1": 0})).await;
  let clock = TestClock::new();
  let found = run(github, clock.clone());
  advance(&clock, Duration::from_mins(1)).await;
  assert!(!found.is_finished(), "the start holds its subject past the minute while the robot is looked for");
  listed.lock().unwrap().push(of_seven("late", PatronusPushMode::RebaseAutosquash, 200));
  while !found.is_finished() {
    advance(&clock, Duration::from_secs(15)).await;
  }
  let commented = found.await.unwrap().unwrap();
  assert_eq!((commented.accepted, commented.robot.map(|robot| robot.id)), (true, Some("late".to_owned())));

  listed.lock().unwrap().clear();
  let (_, github) = github_with_comment(json!({"+1": 1, "-1": 0})).await;
  let clock = TestClock::new();
  let missing = run(github, clock.clone());
  let before = lists.load(std::sync::atomic::Ordering::SeqCst);
  while !missing.is_finished() {
    advance(&clock, Duration::from_secs(15)).await;
  }
  let commented = missing.await.unwrap().unwrap();
  assert_eq!((commented.accepted, commented.robot), (true, None));
  // the ticks of the minute, then one each 15 s for five minutes
  let asked = lists.load(std::sync::atomic::Ordering::SeqCst) - before;
  assert!((20..=45).contains(&asked), "{asked} lists");
}

/// The Safe Push of a reviewed pull request of someone else comments a plain `/patronus merge` (its branch goes once
/// merged) and looks for its robot among the user's only for a while after the 👍: Patronus may count it as the
/// author's, and the start is the 👍 all the same
#[tokio::test]
async fn a_reviewed_pull_request_is_merged_plainly_and_its_robot_looked_for_only_a_while_after_the_reply() {
  let (fake, github) = github_with_comment(json!({"+1": 1, "-1": 0})).await;
  let lists = Arc::new(std::sync::atomic::AtomicUsize::new(0));
  let clock = TestClock::new();
  let run = tokio::spawn({
    let (clock, lists) = (clock.clone(), Arc::clone(&lists));
    async move {
      let repository = repository();
      let start = start_on_seven(&github, &repository, PatronusCommandSubject::ReviewedPullRequest, RobotWait::For(Duration::from_secs(10)));
      let since = move |_since: u32| -> Read<'_, Result<Vec<PatronusRobot>, ApiError>> {
        lists.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async move { Ok(Vec::new()) })
      };
      let since: RobotsSince<'_> = &since;
      start_by_comment(start, &clock, since).await
    }
  });
  while !run.is_finished() {
    clock.advance(Duration::from_secs(3));
    for _ in 0..50 {
      tokio::task::yield_now().await;
    }
  }
  let commented = run.await.unwrap().unwrap();
  assert_eq!((commented.accepted, commented.robot), (true, None));
  assert_eq!(fake.requests_to("POST", COMMENTS)[0].json(), json!({"body": "/patronus merge"}));
  // right after the reply, then each tick of the ten seconds after it
  assert_eq!(lists.load(std::sync::atomic::Ordering::SeqCst), 4);
}
