//! The start of the robot of a Safe Push or a Dry Run, the last effect of a start (`PlannedEffect::StartRobot`,
//! `land_execute`): through the API of Patronus with the merge profile of the plan, or as a `/patronus` comment on the
//! open pull request the approvals count in (`StartRoute::Comment`). The bot answers a comment with a reaction: the
//! reply is read on the clock of the session (`domain_policy::refresh::PATRONUS_COMMENT_REPLY_INTERVAL`, never a timer
//! of the page) until `PATRONUS_COMMENT_REPLY_TIMEOUT` after the comment; once it accepted, the robot it started is the
//! newest robot of that pull request in the mode of the start that nobody knew before the comment (`comment_robot`),
//! looked for in the robots started since shortly before the comment, each tick until the same deadline, and for the app
//! less often for `PATRONUS_COMMENT_ROBOT_WAIT` more, while its start holds the subject. A comment the bot accepted whose
//! robot Patronus doesn't list by then is a start all the same: its robot comes with a later list of the session.

use domain_policy::refresh::{
  PATRONUS_COMMENT_CLOCK_SLACK, PATRONUS_COMMENT_REPLY_INTERVAL, PATRONUS_COMMENT_REPLY_TIMEOUT, PATRONUS_COMMENT_ROBOT_INTERVAL, PATRONUS_COMMENT_ROBOT_WAIT,
};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::patronus::{PatronusCommandReply, PatronusCommandSubject, PatronusMergeRequest, PatronusPushMode, PatronusRobot};
use github::client::GitHubError;
use github::service::{GitHubFailure, GitHubService};
use patronus::service::PatronusService;
use remote_facts::fact_sources::Read;
use remote_facts::patronus_command::{post_patronus_command, read_patronus_command_reply};
use remote_facts::pull_requests::{GitHubWriteError, failure_message};
use repository_model::runtime::sleep_until;
use std::collections::HashSet;
use sync_core::github_repository::GitHubRepository;
use sync_utils::clock::Clock;
use sync_utils::timestamp::unix_now;
use tracing::{debug, instrument};

/// What the API is asked to start
pub struct ApiStart<'a> {
  /// The Space repository the robot runs in
  pub space: &'a str,
  pub prefix: &'a str,
  /// `refs/heads/{prefix}/virtual/<name>`, or `refs/heads/{prefix}/stack/<root>` for a whole stack
  pub source_ref: &'a str,
  pub target: &'a str,
  pub mode: PatronusPushMode,
  pub profile: Option<String>,
  /// The merge request the pre-check of the review named, whose approvals count
  pub merge_request: Option<PatronusMergeRequest>,
}

/// Starts the robot of `start` through the API of Patronus: the robot it started
///
/// # Errors
/// What Patronus answered (`patronus_failure`)
#[instrument(skip_all, fields(source = start.source_ref, mode = ?start.mode))]
pub async fn start_through_api(service: &PatronusService, start: ApiStart<'_>) -> Result<PatronusRobot, ApiError> {
  service
    .start_robot(start.space, start.prefix, start.source_ref, start.target, start.mode, start.profile, start.merge_request)
    .await
    .map_err(patronus_failure)
}

/// A failed request of Patronus: the expired or missing token of the Safe Push CLI is `not_logged_in`, anything else
/// `patronus_error`, in its words
#[must_use]
pub fn patronus_failure(message: String) -> ApiError {
  if message.contains("safePush.cmd") {
    return ApiError::new(ErrorCode::NotLoggedIn, message);
  }
  ApiError::new(ErrorCode::PatronusError, message)
}

/// A failed write of GitHub for a start, in its words: a write GitHub refused is `patronus_refused` (the start can't
/// go), no token GitHub accepts `not_logged_in`, any other failure `network_error`
#[must_use]
pub fn github_failure(error: GitHubWriteError) -> ApiError {
  match error {
    GitHubWriteError::GitHub(failure) => github_request_failure(&failure),
    GitHubWriteError::Refused(message) => ApiError::new(ErrorCode::PatronusRefused, message),
  }
}

/// A failed request of GitHub: `not_logged_in` without a token GitHub accepts, else `network_error`
#[must_use]
pub fn github_request_failure(failure: &GitHubFailure) -> ApiError {
  let code = match failure {
    GitHubFailure::SignedOut
    | GitHubFailure::Failed {
      error: GitHubError::Unauthorized, ..
    } => ErrorCode::NotLoggedIn,
    GitHubFailure::Failed { .. } => ErrorCode::NetworkError,
  };
  ApiError::new(code, failure_message(failure))
}

/// Where a comment goes and how its robot is told from the others
pub struct CommentStart<'a> {
  pub github: &'a GitHubService,
  pub repository_path: &'a str,
  pub repository: &'a GitHubRepository,
  /// The open pull request of the subject
  pub number: u32,
  pub mode: PatronusPushMode,
  /// The robots known before the comment (those that had finished when listed before it, and the one the start
  /// canceled): never its robot
  pub known: HashSet<String>,
  /// The pull request is the user's own branch (its source branch stays once merged) or a reviewed one of someone else
  pub subject: PatronusCommandSubject,
  /// How long the robot is looked for once the bot accepted
  pub robot_wait: RobotWait,
}

/// How long the robot of an accepted comment is looked for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RobotWait {
  /// Once, right after the bot accepted (`bd`, which says the robot isn't listed yet)
  Once,
  /// Each tick for this long after the bot accepted, within the minute of the reply (a reviewed pull request of someone
  /// else, whose robot may be the author's: `PATRONUS_REVIEWED_ROBOT_WAIT`)
  For(std::time::Duration),
  /// Each tick until it is listed, `PATRONUS_COMMENT_ROBOT_WAIT` past the minute of the reply at most (the app, whose
  /// start holds its subject meanwhile and whose session shows the robot at once)
  UntilListed,
}

/// A comment the bot accepted, or one it didn't answer within the wait
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commented {
  pub number: u32,
  /// The comment on the pull request
  pub comment_url: String,
  /// The bot accepted it; `false`: it didn't react within `PATRONUS_COMMENT_REPLY_TIMEOUT`, see the pull request
  pub accepted: bool,
  /// The robot it started, once Patronus listed it within the wait
  pub robot: Option<PatronusRobot>,
}

/// The robots of the user started since a time (Unix seconds), asked now: where the robot of a comment is looked for
pub type RobotsSince<'a> = &'a (dyn Fn(u32) -> Read<'a, Result<Vec<PatronusRobot>, ApiError>> + Send + Sync);

/// Posts `/patronus merge --keep-source-branch` (or `dry-run`; `/patronus merge` on a reviewed pull request of someone
/// else) on the pull request, reads the reply of the bot on `clock` until it accepted or
/// `PATRONUS_COMMENT_REPLY_TIMEOUT` passed, and looks for the robot it started in what `robots_since` lists, as
/// `robot_wait` says: once, each tick for a while after the reply, or each tick until the same deadline and every
/// `PATRONUS_COMMENT_ROBOT_INTERVAL` after it for `PATRONUS_COMMENT_ROBOT_WAIT` more
///
/// # Errors
/// A comment GitHub refused, a reply it failed to answer, a comment the bot rejected (`patronus_refused`, with its
/// reason), a list of the robots that failed
#[instrument(skip_all, fields(number = start.number, mode = ?start.mode))]
pub async fn start_by_comment(start: CommentStart<'_>, clock: &dyn Clock, robots_since: RobotsSince<'_>) -> Result<Commented, ApiError> {
  let comment = post_patronus_command(start.github, start.repository_path, start.repository, start.number, start.mode, start.subject)
    .await
    .map_err(github_failure)?;
  let posted_at = unix_now();
  let deadline = clock.now() + PATRONUS_COMMENT_REPLY_TIMEOUT;
  // a wait of a while after the reply ends at the reply plus the wait, within the minute
  let mut accepted_until = None;
  let mut commented = Commented {
    number: start.number,
    comment_url: comment.url,
    accepted: false,
    robot: None,
  };
  // the bot reacts first and comments its reason right after: a rejection without one is read once more
  let mut rejected_once = false;
  loop {
    if !commented.accepted {
      match read_patronus_command_reply(start.github, start.repository_path, start.repository, start.number, &comment.id)
        .await
        .map_err(github_failure)?
      {
        PatronusCommandReply::Accepted => {
          commented.accepted = true;
          if let RobotWait::For(wait) = start.robot_wait {
            accepted_until = Some((clock.now() + wait).min(deadline));
          }
        }
        PatronusCommandReply::Rejected { reason } if reason.is_some() || rejected_once => return Err(rejected(start.number, reason)),
        PatronusCommandReply::Rejected { .. } => rejected_once = true,
        PatronusCommandReply::Pending => {}
      }
    }
    if commented.accepted {
      let robots = robots_since(posted_at.saturating_sub(PATRONUS_COMMENT_CLOCK_SLACK)).await?;
      commented.robot = comment_robot(robots, start.number, start.mode, &start.known);
      if commented.robot.is_some() || start.robot_wait == RobotWait::Once {
        return Ok(commented);
      }
    }
    let now = clock.now();
    let next = now + PATRONUS_COMMENT_REPLY_INTERVAL;
    let next = if next <= accepted_until.unwrap_or(deadline) {
      next
    } else if commented.accepted && start.robot_wait == RobotWait::UntilListed && now + PATRONUS_COMMENT_ROBOT_INTERVAL <= deadline + PATRONUS_COMMENT_ROBOT_WAIT {
      // accepted, the robot not listed within the minute: looked for less often, while the start holds its subject
      now + PATRONUS_COMMENT_ROBOT_INTERVAL
    } else {
      debug!(accepted = commented.accepted, "the wait for the reply and the robot of the comment ended");
      return Ok(commented);
    };
    sleep_until(clock, next).await;
  }
}

/// "Patronus rejected the comment on pull request #812: <its reason>"
fn rejected(number: u32, reason: Option<String>) -> ApiError {
  ApiError::new(
    ErrorCode::PatronusRefused,
    format!(
      "Patronus rejected the comment on pull request #{number}: {}",
      reason.unwrap_or_else(|| "no reason given yet; see the pull request".to_string())
    ),
  )
}

/// The robot a `/patronus` comment on pull request `number` started in `mode`: of that pull request, a dry run for a
/// dry run and a Safe Push for a Safe Push, and none of `known` (a canceled Dry Run or the replaced Safe Push of the same
/// pull request a minute before is never the new robot); the newest of them
#[must_use]
pub fn comment_robot(robots: Vec<PatronusRobot>, number: u32, mode: PatronusPushMode, known: &HashSet<String>) -> Option<PatronusRobot> {
  let dry_run = mode == PatronusPushMode::DryRun;
  robots
    .into_iter()
    .filter(|robot| robot.pull_request.as_ref().is_some_and(|link| link.number == number))
    .filter(|robot| (robot.push_mode == PatronusPushMode::DryRun) == dry_run && !known.contains(&robot.id))
    .max_by_key(|robot| robot.start_time)
}

#[cfg(test)]
#[path = "robot_start_test.rs"]
mod robot_start_test;
