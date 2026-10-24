//! The Patronus REST API (`/app/rest/v1`), as far as Branch Deck uses it. Every call is one HTTP request; a GET is
//! repeated at most once on a transient failure, a POST or PUT never.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use sync_utils::timestamp::format_utc_timestamp;
use tracing::{debug, info, instrument, warn};

pub const PATRONUS_URL: &str = "https://patronus.labs.jb.gg";

/// The connection is reused: polling is one small request on a kept-alive TLS connection
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Robot lists and statuses are database reads on the server
pub const LIST_TIMEOUT: Duration = Duration::from_secs(15);
/// A start checks the branch in Space first, which can take a while
pub const ACTION_TIMEOUT: Duration = Duration::from_secs(30);
/// The server gives up after 10 s (503)
pub const REVIEW_CHECK_TIMEOUT: Duration = Duration::from_secs(20);
/// Patronus asks Space live; nginx gives up after 60 s
pub const MERGE_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// The failed builds of an attempt and the message of a failed test: Patronus asks TeamCity for them
pub const CI_DETAILS_TIMEOUT: Duration = Duration::from_secs(30);
/// A transient failure of a GET is asked again once, after this pause
const GET_RETRY_DELAY: Duration = Duration::from_secs(1);
/// Patronus answers the progress of at most this many robots per request
pub const MAX_PROGRESS_ROBOTS: usize = 20;
/// A week of everyone's robots of a repository is a few megabytes
pub const HISTORY_LIST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatronusError {
  /// 401: the token is missing, expired or revoked. A 403 is an `Http` error: the user may not do that (e.g. start a
  /// robot in a repository without write access), the token is fine.
  Unauthorized,
  /// Another HTTP failure; `code` and `description` are the fields of the Patronus error body, when there is one
  Http {
    status: u16,
    code: Option<String>,
    description: Option<String>,
  },
  Timeout,
  /// No connection: offline, DNS, TLS
  Network(String),
  /// The answer is not what this version expects
  InvalidResponse(String),
}

impl PatronusError {
  /// Worth asking again later: the server or the network failed, not the request
  pub const fn is_transient(&self) -> bool {
    match self {
      Self::Http { status, .. } => *status >= 500,
      Self::Timeout | Self::Network(_) => true,
      Self::Unauthorized | Self::InvalidResponse(_) => false,
    }
  }

  /// For the user: the description Patronus gave, else what failed
  pub fn message(&self) -> String {
    match self {
      Self::Unauthorized => "Patronus rejected the Space token".to_string(),
      Self::Http { status, description, .. } => description.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| format!("Patronus answered HTTP {status}")),
      Self::Timeout => "Patronus did not answer in time".to_string(),
      Self::Network(message) => format!("Patronus is not reachable: {message}"),
      Self::InvalidResponse(message) => format!("Unexpected Patronus answer: {message}"),
    }
  }
}

/// `RobotListDto` of the list and `RobotDto` of `fetch-updates`: the fields both have
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotDto {
  pub id: String,
  pub name: String,
  pub status: String,
  pub push_mode: String,
  pub source_branch: Option<String>,
  pub source_revision: Option<String>,
  pub target_branch: String,
  pub start_date_time: String,
  pub finish_date_time: Option<String>,
  pub cancellation_reason: Option<String>,
  /// Only in `GET /robots/{id}` and `fetch-updates` (absent from a list): holds the merge profile
  #[serde(default)]
  pub options: Option<std::collections::HashMap<String, String>>,
  /// The pull request whose `/patronus` comment started the robot, `https://github.com/{owner}/{name}/pull/{number}`;
  /// a robot started otherwise has none (Patronus links a robot to a pull request only from its comment)
  #[serde(default)]
  pub github_pr_url: Option<String>,
  /// The commit TeamCity builds: the source revision rebased onto the target. Only in `GET /robots/{id}` and
  /// `fetch-updates`, and only once the robot rebased
  #[serde(default)]
  pub test_reference_revision: Option<String>,
  /// The commit of the target branch the robot rebased onto; as `test_reference_revision`
  #[serde(default)]
  pub target_revision: Option<String>,
  /// The merge profile of a list item, behind the `robot.list.merge.profile.enabled` flag of Patronus: null while the
  /// flag is off for the user and for a robot started without one
  #[serde(default)]
  pub merge_profile: Option<RobotMergeProfileDto>,
  /// Who started the robot; only of a list
  #[serde(default)]
  pub owner: Option<RobotOwnerDto>,
  /// The robot a restart restarted; only of a list
  #[serde(default)]
  pub source_robot_id: Option<String>,
  /// Who started it: `SAFE_PUSH`, `WEB_UI`, `SPACE_SAFE_MERGE`, `GITHUB_PR`, `CUSTOM`, `SPACE_REVIEW_CHERRY_PICK`, …;
  /// only of a list
  #[serde(default, rename = "type")]
  pub kind: Option<String>,
  /// Started with an emergency profile, which skips checks; only of a list
  #[serde(default)]
  pub is_emergency: bool,
}

/// The owner of a robot, and who asks (`me` of a list): the name and the email Patronus also sends are not read
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RobotOwnerDto {
  /// `USER` or `APPLICATION`
  #[serde(rename = "type")]
  pub kind: String,
  pub id: String,
}

/// `GET /robots/{id}/changes`: what the robot landed
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotChangesDto {
  /// `NOT_LOADED`, `PENDING` or `DELIVERED`
  #[serde(rename = "type")]
  pub kind: String,
  #[serde(default)]
  pub commits_number: Option<u32>,
}

/// `mergeProfile` of a robot of the list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotMergeProfileDto {
  pub id: String,
  pub name: String,
  /// The profile skipped the code-owner review approval check (`patronus.review.approval.check.skip`)
  #[serde(default)]
  pub skips_review: bool,
}

/// The option of a robot that names its merge profile
pub const MERGE_PROFILE_ID_OPTION: &str = "patronus.merge.profile.id";
pub const MERGE_PROFILE_NAME_OPTION: &str = "patronus.merge.profile.name";
/// The option of a robot whose profile skips the code-owner review approval check (an emergency profile too)
pub const REVIEW_APPROVAL_CHECK_SKIP_OPTION: &str = "patronus.review.approval.check.skip";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RobotStatusDto {
  robot_status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamCityCheckDto {
  /// The name of the check in the merge profile, e.g. "IntelliJ Aggregator" for the build configuration "Aggregator";
  /// what the robot page of the web UI shows
  #[serde(default)]
  pub name: Option<String>,
  /// e.g. `BazelCommunityBuild_Aggregator`
  #[serde(default)]
  pub build_configuration_id: Option<String>,
  pub build_configuration_name: String,
  /// `QUEUED`, `RUNNING`, `FAILURE`, `SUCCESS`, `CANCELED`, `SKIPPED`
  pub status: String,
  /// How many attempts the check may take (a failed build is retried until then)
  #[serde(default)]
  pub attempt_limit: Option<u32>,
  /// The builds of the check, one per attempt (a failed build may be retried)
  #[serde(default)]
  pub attempts: Vec<TeamCityCheckAttemptDto>,
}

impl TeamCityCheckDto {
  /// The name the user knows the check by: its name in the profile, else its build configuration
  pub fn label(&self) -> &str {
    self.name.as_deref().filter(|name| !name.is_empty()).unwrap_or(&self.build_configuration_name)
  }

  /// The build of the attempt that runs now: the running attempt with the highest number
  pub fn running_build_id(&self) -> Option<&str> {
    self
      .attempts
      .iter()
      .filter(|attempt| attempt.status == "RUNNING")
      .max_by_key(|attempt| attempt.number)
      .map(|attempt| attempt.build_id.as_str())
  }

  /// The attempt with the highest number
  pub fn last_attempt(&self) -> Option<&TeamCityCheckAttemptDto> {
    self.attempts.iter().max_by_key(|attempt| attempt.number)
  }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamCityCheckAttemptDto {
  /// The id of `get_attempt_details`
  #[serde(default)]
  pub id: Option<String>,
  /// From 0
  pub number: u32,
  pub build_id: String,
  #[serde(default)]
  pub build_url: Option<String>,
  /// `RUNNING`, `CANCELED`, `SUCCESS`, `FAILURE`
  pub status: String,
  /// The counts of the build chain, as far as it ran; absent before TeamCity reported
  #[serde(default)]
  pub failed_tests_number: Option<u32>,
  #[serde(default)]
  pub failed_builds_number: Option<u32>,
  #[serde(default)]
  pub failed_to_start_builds_number: Option<u32>,
}

/// The builds of an attempt, `GET /teamcity-checks/attempts/{id}`: by default only the failed ones, without the ignored
/// problems and test failures
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamCityCheckAttemptDetailsDto {
  #[serde(default)]
  pub builds: Vec<BuildDto>,
}

/// A build of the dependency chain of an attempt
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildDto {
  pub build_id: String,
  #[serde(default)]
  pub build_url: Option<String>,
  pub build_configuration_name: String,
  #[serde(default)]
  pub full_project_name: Option<String>,
  /// An aggregator of other builds: its problems are theirs
  #[serde(default)]
  pub is_composite: bool,
  #[serde(default)]
  pub is_failed_to_start: bool,
  /// Failed only because tests failed
  #[serde(default)]
  pub is_failed_tests_only_failure: bool,
  #[serde(default)]
  pub problems: Vec<BuildProblemDto>,
  #[serde(default)]
  pub failed_tests: Vec<FailedTestDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildProblemDto {
  #[serde(default)]
  pub details: String,
  /// e.g. `TC_COMPILATION_ERROR`, `TC_EXIT_CODE`, `TC_FAILED_TESTS`
  #[serde(default, rename = "type")]
  pub kind: Option<String>,
  #[serde(default)]
  pub problem_id: Option<String>,
  #[serde(default)]
  pub is_ignored_problem: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedTestDto {
  pub name: String,
  #[serde(default)]
  pub url: Option<String>,
  /// The three ids of `get_test_run_details`
  pub build_id: String,
  pub test_id: String,
  pub test_run_id: String,
  #[serde(default)]
  pub is_ignored_failure: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestRunDetailsDto {
  test_failure_message: String,
}

/// The estimate of the CI for a build (its `RUNNING_INFO`), as the robot page of the web UI shows it
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildProgressDto {
  pub queued: bool,
  #[serde(default)]
  pub percentage_complete: i64,
  #[serde(default)]
  pub elapsed_running_seconds: i64,
  pub estimated_total_running_seconds: Option<i64>,
  pub probably_hanging: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RobotsProgressDto {
  robots: Vec<RobotProgressDto>,
}

/// The progress of a running robot (`GET /robots/progress`): the CI estimates of its running checks, folded by Patronus
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotProgressDto {
  pub robot_id: String,
  /// RFC 3339: the latest finish over the running checks, in the past when every one ran past its estimate; null while
  /// no running check has an estimate
  pub estimated_finish_at: Option<String>,
  /// The check that finishes last; null with `estimated_finish_at`
  pub longest_check: Option<RobotProgressCheckDto>,
  /// In the Patronus queue, waiting for a TeamCity agent, or waiting for the build of the next attempt
  #[serde(default)]
  pub queued_checks: u32,
  #[serde(default)]
  pub running_checks: u32,
  #[serde(default)]
  pub finished_checks: u32,
  #[serde(default)]
  pub probably_hanging: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotProgressCheckDto {
  pub name: String,
  /// The completed fraction of the build, 0 to 1
  #[serde(default)]
  pub progress: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamCityChecksDto {
  team_city_checks: Vec<TeamCityCheckDto>,
}

/// The `uniqueId` of the warning Patronus registers once a required check failed while the robot still runs ("This Safe
/// Push will fail because required checks have already failed"); it is removed when the robot finishes
pub const ROBOT_IS_FAILING_PROBLEM: &str = "robot_is_failing";

#[derive(Debug, Clone, Deserialize)]
pub struct ProblemDto {
  /// Set for a problem Patronus registers once per robot, e.g. `ROBOT_IS_FAILING_PROBLEM`
  #[serde(default, rename = "uniqueId")]
  pub unique_id: Option<String>,
  pub title: String,
  /// `ERROR` or `WARNING`
  #[serde(default)]
  pub severity: Option<String>,
  pub subsystem: Option<String>,
  #[serde(default, rename = "detailsMarkdown")]
  pub details_markdown: Option<String>,
}

impl ProblemDto {
  /// What the user has to fix; a problem without a severity counts as one
  pub fn is_error(&self) -> bool {
    self.severity.as_deref().is_none_or(|severity| severity == "ERROR")
  }

  pub fn is_robot_failing(&self) -> bool {
    self.unique_id.as_deref() == Some(ROBOT_IS_FAILING_PROBLEM)
  }
}

#[derive(Deserialize)]
struct ProblemsDto {
  problems: Vec<ProblemDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeProfileDto {
  pub id: String,
  pub name: String,
  pub description: Option<String>,
  #[serde(default)]
  pub allowed_push_modes: Vec<String>,
  #[serde(default)]
  pub is_emergency: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartSupportedDto {
  pub supported: bool,
  #[serde(default)]
  pub profiles: Vec<MergeProfileDto>,
}

/// `POST /robots` of a robot on a pushed branch, as the web UI starts it: the branch is kept, and a restart takes its
/// newest commit
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchRobotStartRequest {
  /// `ROBOT_WEB_UI`
  #[serde(rename = "type")]
  pub kind: &'static str,
  /// The repository alias, e.g. `ultimate`
  pub repository: String,
  /// `refs/heads/{prefix}/virtual/{name}`
  pub source_branch: String,
  pub target_branch: String,
  /// `REBASE_AUTOSQUASH` or `DRY_RUN`
  pub mode: &'static str,
  /// `NEVER`: the virtual branch stays, the sync archives it once it is merged
  pub source_branch_delete_policy: &'static str,
  pub merge_profile_id: Option<String>,
  /// The merge request whose approvals the review check counts
  #[serde(skip_serializing_if = "Option::is_none")]
  pub space_review: Option<SpaceReviewDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceReviewDto {
  pub project_key: String,
  pub review_key: String,
}

/// A commit with its changed paths, as `pre-merge-approval-required-check` reads it
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreMergeCommitDto {
  pub hash: String,
  pub is_merge_commit: bool,
  pub message: String,
  pub author_name: String,
  pub author_email: String,
  // sic: the field is spelled so in the Patronus DTO
  pub commiter_name: String,
  pub commiter_email: String,
  /// UTC, `2026-10-14T09:30:00Z`
  pub author_date: String,
  pub committer_date: String,
  pub changed_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreMergeCheckRequest<'a> {
  pub space_project_key: &'a str,
  pub space_repository_name: &'a str,
  pub target_branch: &'a str,
  pub commits: &'a [PreMergeCommitDto],
  #[serde(skip_serializing_if = "Option::is_none")]
  pub space_review_key: Option<&'a str>,
  /// The pull request whose reviews count, as for the robot its comment starts; never with `space_review_key`.
  /// Patronus ignores it while its flag `review.precheck.github.pull.request.enabled` is off, and answers an unknown
  /// pull request as one without reviews.
  #[serde(rename = "githubPullRequest", skip_serializing_if = "Option::is_none")]
  pub github_pull_request: Option<GitHubPullRequestDto<'a>>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct GitHubPullRequestDto<'a> {
  /// `owner/name`: the GitHub repository linked to the Space repository
  pub repository: &'a str,
  pub number: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreMergeCheckResponse {
  pub approval_required: bool,
  #[serde(default)]
  pub reviewer_groups: Vec<ReviewerGroupDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewerGroupDto {
  pub group_name: String,
  #[serde(default)]
  pub reviewers: Vec<ReviewerDto>,
  /// The changed paths the group has to review; absent on a server without the feature
  #[serde(default)]
  pub paths: Option<Vec<String>>,
  /// Any change of the branch needs the group
  #[serde(default)]
  pub branch_wide: Option<bool>,
  /// The approvals an ownership rule needs; only with a pull request (and the flag of its evaluation on)
  #[serde(default)]
  pub required_approvals: Option<u32>,
  /// The approvals that count, a self-approval too; may exceed `required_approvals`; as `required_approvals`
  #[serde(default)]
  pub collected_approvals: Option<u32>,
  /// The group's approval is there; only with a pull request, also of a legacy rule (without the counts)
  #[serde(default)]
  pub approved: Option<bool>,
  /// The author's approval counts; as `required_approvals`
  #[serde(default)]
  pub self_approval_allowed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewerDto {
  /// Null only with a pull request, for a member without a Space username (a bot member)
  #[serde(default)]
  pub space_username: Option<String>,
  pub full_name: String,
  /// The GitHub login Patronus knows (`HiBob`), which counts the reviewer's approval of a pull request; absent while the
  /// flag `review.precheck.github.usernames.enabled` is off, and for a reviewer without one
  #[serde(default)]
  pub github_username: Option<String>,
  #[serde(default)]
  pub is_available: bool,
  #[serde(default)]
  pub is_suggested: bool,
  /// `APPROVED`, `AUTHOR_SELF_APPROVED`, `REVIEWING`, `NOT_ADDED`, `NOT_ELIGIBLE`; only with a pull request, where every
  /// member is listed, the not eligible ones too
  #[serde(default)]
  pub approval_state: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequestDto {
  pub project_key: String,
  pub review_key: String,
  pub url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeRequestsDto {
  merge_requests: Vec<MergeRequestDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RobotStartedDto {
  robot_id: String,
}

#[derive(Deserialize)]
struct RobotsDto {
  robots: Vec<RobotDto>,
  /// Who asks
  #[serde(default)]
  me: Option<RobotOwnerDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotUpdateRequest {
  pub id: String,
  /// `RUNNING`, `CANCELING`, …: Patronus answers only the robots whose status differs
  pub last_known_status: String,
}

#[derive(Serialize)]
struct FetchUpdatesRequest<'a> {
  robots: &'a [RobotUpdateRequest],
}

#[derive(Deserialize)]
struct ErrorDto {
  error: Option<String>,
  description: Option<String>,
}

#[derive(Clone)]
pub struct PatronusClient {
  http: reqwest::Client,
  base_url: String,
  /// The host of the web UI, the one of the REST API but for a fake (`with_web_url`)
  web_url: String,
  /// The wait before a GET is asked again after a transient failure (`GET_RETRY_DELAY`; a test of the failure waits
  /// none)
  retry_delay: Duration,
}

impl PatronusClient {
  /// `base_url` without the trailing slash, e.g. `PATRONUS_URL`
  pub fn new(base_url: impl Into<String>, user_agent: &str) -> Self {
    let http = reqwest::Client::builder()
      .connect_timeout(CONNECT_TIMEOUT)
      .user_agent(user_agent)
      .build()
      .expect("the HTTP client of Patronus");
    let base_url = base_url.into();
    Self {
      http,
      web_url: base_url.clone(),
      base_url,
      retry_delay: GET_RETRY_DELAY,
    }
  }

  /// The client with another wait before a GET is asked again after a transient failure: a test that fakes a failure
  /// of Patronus does not wait the second of the app
  #[must_use]
  pub const fn with_retry_delay(mut self, retry_delay: Duration) -> Self {
    self.retry_delay = retry_delay;
    self
  }

  /// The client with the robot pages on `web_url`: a fake serves only the REST API, while the pages are where Patronus
  /// has them
  #[must_use]
  pub fn with_web_url(mut self, web_url: impl Into<String>) -> Self {
    self.web_url = web_url.into();
    self
  }

  pub fn base_url(&self) -> &str {
    &self.base_url
  }

  /// The robot page in the web UI
  pub fn robot_url(&self, robot_id: &str) -> String {
    format!("{}/robot/{robot_id}", self.web_url)
  }

  /// The robots of the token's owner in the repository started since `start` (Unix seconds), newest first
  #[instrument(skip(self, token))]
  pub async fn list_robots(&self, token: &str, repository: &str, start: u32) -> Result<Vec<RobotDto>, PatronusError> {
    let start = format_utc_timestamp(start);
    let query = [("owner", "me"), ("repository", repository), ("start", start.as_str())];
    let url = reqwest::Url::parse_with_params(&format!("{}/app/rest/v1/robots", self.base_url), query).map_err(|e| PatronusError::InvalidResponse(e.to_string()))?;
    let started = Instant::now();
    let robots: RobotsDto = self.get_json(|| self.http.get(url.clone()).bearer_auth(token), LIST_TIMEOUT).await?;
    info!(count = robots.robots.len(), elapsed_ms = started.elapsed().as_millis(), "listed Patronus robots");
    Ok(robots.robots)
  }

  /// Everyone's robots into `target` of `repository` started in `[start, end)`, and who asks: no owner filter, a window
  /// of at most 70 days (Patronus refuses a longer one)
  #[instrument(skip(self, token))]
  pub async fn list_target_robots(&self, token: &str, repository: &str, target: &str, start: u32, end: u32) -> Result<(Option<RobotOwnerDto>, Vec<RobotDto>), PatronusError> {
    let (start, end) = (format_utc_timestamp(start), format_utc_timestamp(end));
    let query = [("repository", repository), ("targetBranch", target), ("start", start.as_str()), ("end", end.as_str())];
    let url = reqwest::Url::parse_with_params(&format!("{}/app/rest/v1/robots", self.base_url), query).map_err(|e| PatronusError::InvalidResponse(e.to_string()))?;
    let started = Instant::now();
    let robots: RobotsDto = self.get_json(|| self.http.get(url.clone()).bearer_auth(token), HISTORY_LIST_TIMEOUT).await?;
    info!(count = robots.robots.len(), elapsed_ms = started.elapsed().as_millis(), "listed everyone's Patronus robots");
    Ok((robots.me, robots.robots))
  }

  /// What a robot landed: the count of its commits once Patronus delivered them
  #[instrument(skip(self, token))]
  pub async fn get_robot_changes(&self, token: &str, robot_id: &str) -> Result<RobotChangesDto, PatronusError> {
    let url = self.robot_api_url(robot_id, &["changes"])?;
    self.get_json(|| self.http.get(url.clone()).bearer_auth(token), LIST_TIMEOUT).await
  }

  /// The robots whose status is not the known one; the others are not sent back
  #[instrument(skip(self, token, robots), fields(count = robots.len()))]
  pub async fn fetch_updates(&self, token: &str, robots: &[RobotUpdateRequest]) -> Result<Vec<RobotDto>, PatronusError> {
    let request = self
      .http
      .post(format!("{}/app/rest/v1/robots/fetch-updates", self.base_url))
      .json(&FetchUpdatesRequest { robots })
      .bearer_auth(token)
      .timeout(LIST_TIMEOUT);
    let started = Instant::now();
    let changed: RobotsDto = read_json(request.send().await).await?;
    info!(changed = changed.robots.len(), elapsed_ms = started.elapsed().as_millis(), "polled Patronus robots");
    Ok(changed.robots)
  }

  /// One robot with its options
  #[instrument(skip(self, token))]
  pub async fn get_robot(&self, token: &str, robot_id: &str) -> Result<RobotDto, PatronusError> {
    let url = self.robot_api_url(robot_id, &[])?;
    self.get_json(|| self.http.get(url.clone()).bearer_auth(token), LIST_TIMEOUT).await
  }

  /// The status of one robot, e.g. `RUNNING`: one row read on the server
  #[instrument(skip(self, token))]
  pub async fn get_robot_status(&self, token: &str, robot_id: &str) -> Result<String, PatronusError> {
    let url = self.robot_api_url(robot_id, &["status"])?;
    let status: RobotStatusDto = self.get_json(|| self.http.get(url.clone()).bearer_auth(token), LIST_TIMEOUT).await?;
    Ok(status.robot_status)
  }

  #[instrument(skip(self, token))]
  pub async fn get_teamcity_checks(&self, token: &str, robot_id: &str) -> Result<Vec<TeamCityCheckDto>, PatronusError> {
    let url = self.robot_api_url(robot_id, &["teamcity-checks"])?;
    let checks: TeamCityChecksDto = self.get_json(|| self.http.get(url.clone()).bearer_auth(token), LIST_TIMEOUT).await?;
    Ok(checks.team_city_checks)
  }

  /// The progress of the running robots among `robot_ids`, `MAX_PROGRESS_ROBOTS` per request: what took one request per
  /// running check, folded by Patronus (from the same cache of a minute); a finished or unknown robot is left out. 404
  /// while the `robot.progress.enabled` flag of Patronus is off for the user
  #[instrument(skip(self, token, robot_ids), fields(count = robot_ids.len()))]
  pub async fn get_robots_progress(&self, token: &str, robot_ids: &[&str]) -> Result<Vec<RobotProgressDto>, PatronusError> {
    let mut robots = Vec::new();
    for chunk in robot_ids.chunks(MAX_PROGRESS_ROBOTS) {
      let query: Vec<(&str, &str)> = chunk.iter().map(|id| ("robotId", *id)).collect();
      let url = reqwest::Url::parse_with_params(&format!("{}/app/rest/v1/robots/progress", self.base_url), query).map_err(|e| PatronusError::InvalidResponse(e.to_string()))?;
      let started = Instant::now();
      let answer: RobotsProgressDto = self.get_json(|| self.http.get(url.clone()).bearer_auth(token), LIST_TIMEOUT).await?;
      info!(count = answer.robots.len(), elapsed_ms = started.elapsed().as_millis(), "read the progress of the robots");
      robots.extend(answer.robots);
    }
    Ok(robots)
  }

  /// The estimate of the CI for a build of a check (`teamcity-proxy`, cached by Patronus for a minute)
  #[instrument(skip(self, token))]
  pub async fn get_build_progress(&self, token: &str, build_id: &str) -> Result<BuildProgressDto, PatronusError> {
    let url = self.api_url(&["teamcity-proxy", "build-progress", build_id])?;
    let started = Instant::now();
    let progress: BuildProgressDto = self.get_json(|| self.http.get(url.clone()).bearer_auth(token), LIST_TIMEOUT).await?;
    info!(elapsed_ms = started.elapsed().as_millis(), "read the build progress");
    Ok(progress)
  }

  /// The failed builds of an attempt of a check, with their problems and failed tests (Patronus asks TeamCity)
  #[instrument(skip(self, token))]
  pub async fn get_attempt_details(&self, token: &str, attempt_id: &str) -> Result<TeamCityCheckAttemptDetailsDto, PatronusError> {
    let url = self.api_url(&["teamcity-checks", "attempts", attempt_id])?;
    let started = Instant::now();
    let details: TeamCityCheckAttemptDetailsDto = self.get_json(|| self.http.get(url.clone()).bearer_auth(token), CI_DETAILS_TIMEOUT).await?;
    info!(
      builds = details.builds.len(),
      elapsed_ms = started.elapsed().as_millis(),
      "read the failed builds of an attempt"
    );
    Ok(details)
  }

  /// The failure message and stack trace of a failed test (`teamcity-proxy`, cached by Patronus for 30 minutes, at most
  /// 100 requests in 10 s)
  #[instrument(skip(self, token))]
  pub async fn get_test_run_details(&self, token: &str, build_id: &str, test_id: &str, test_run_id: &str) -> Result<String, PatronusError> {
    let url = self.api_url(&["teamcity-proxy", "test-run-details", build_id, test_id, test_run_id])?;
    let details: TestRunDetailsDto = self.get_json(|| self.http.get(url.clone()).bearer_auth(token), CI_DETAILS_TIMEOUT).await?;
    Ok(details.test_failure_message)
  }

  /// The problems of every severity: the errors, and the warnings such as `ROBOT_IS_FAILING_PROBLEM`
  #[instrument(skip(self, token))]
  pub async fn get_problems(&self, token: &str, robot_id: &str) -> Result<Vec<ProblemDto>, PatronusError> {
    let url = self.robot_api_url(robot_id, &["problems"])?;
    let problems: ProblemsDto = self.get_json(|| self.http.get(url.clone()).bearer_auth(token), LIST_TIMEOUT).await?;
    Ok(problems.problems)
  }

  /// Whether Patronus protects the branch, and the merge profiles the user may use for it
  #[instrument(skip(self, token))]
  pub async fn get_start_supported(&self, token: &str, project_key: &str, repository: &str, target_branch: &str) -> Result<StartSupportedDto, PatronusError> {
    let query = [("spaceProjectKey", project_key), ("spaceRepositoryName", repository), ("branchName", target_branch)];
    let url = reqwest::Url::parse_with_params(&format!("{}/app/rest/v1/robot-start-supported", self.base_url), query).map_err(|e| PatronusError::InvalidResponse(e.to_string()))?;
    self.get_json(|| self.http.get(url.clone()).bearer_auth(token), LIST_TIMEOUT).await
  }

  /// Starts a robot; never repeated by itself. The id of the new robot.
  #[instrument(skip(self, token))]
  pub async fn start_robot(&self, token: &str, request: &BranchRobotStartRequest) -> Result<String, PatronusError> {
    let response = self
      .http
      .post(format!("{}/app/rest/v1/robots", self.base_url))
      .json(request)
      .bearer_auth(token)
      .timeout(ACTION_TIMEOUT)
      .send()
      .await;
    let started: RobotStartedDto = read_json(response).await?;
    info!(robot_id = %started.robot_id, "started a Patronus robot");
    Ok(started.robot_id)
  }

  /// Starts the robot again on the newest commit of its branch; the id of the new robot
  #[instrument(skip(self, token))]
  pub async fn restart_robot(&self, token: &str, robot_id: &str) -> Result<String, PatronusError> {
    let response = self
      .http
      .post(self.robot_api_url(robot_id, &["restart"])?)
      .bearer_auth(token)
      .timeout(ACTION_TIMEOUT)
      .send()
      .await;
    let started: RobotStartedDto = read_json(response).await?;
    info!(robot_id = %started.robot_id, "restarted a Patronus robot");
    Ok(started.robot_id)
  }

  #[instrument(skip(self, token))]
  pub async fn cancel_robot(&self, token: &str, robot_id: &str) -> Result<(), PatronusError> {
    let response = self
      .http
      .put(self.robot_api_url(robot_id, &["cancel"])?)
      .bearer_auth(token)
      .timeout(ACTION_TIMEOUT)
      .send()
      .await;
    read_empty(response).await
  }

  /// Whether the code owners have to approve the commits; the heaviest call of the API (it decodes the ownership
  /// snapshots of the target branch), never repeated by itself
  #[instrument(skip(self, token, request), fields(commits = request.commits.len(), review = ?request.space_review_key, pull_request = ?request.github_pull_request.map(|pull_request| pull_request.number)))]
  pub async fn pre_merge_check(&self, token: &str, request: &PreMergeCheckRequest<'_>) -> Result<PreMergeCheckResponse, PatronusError> {
    let started = Instant::now();
    let response = self
      .http
      .post(format!("{}/app/rest/v1/pre-merge-approval-required-check", self.base_url))
      .json(request)
      .bearer_auth(token)
      .timeout(REVIEW_CHECK_TIMEOUT)
      .send()
      .await;
    let answer: PreMergeCheckResponse = read_json(response).await?;
    info!(
      required = answer.approval_required,
      elapsed_ms = started.elapsed().as_millis(),
      "checked the code-owner review"
    );
    Ok(answer)
  }

  /// The open merge requests from `source_branch` into `target_branch` (branch names); Patronus asks Space for them
  #[instrument(skip(self, token))]
  pub async fn find_merge_requests(&self, token: &str, repository: &str, source_branch: &str, target_branch: &str) -> Result<Vec<MergeRequestDto>, PatronusError> {
    let query = [("repository", repository), ("sourceBranch", source_branch), ("targetBranch", target_branch)];
    let url = reqwest::Url::parse_with_params(&format!("{}/app/rest/v1/space/merge-requests", self.base_url), query).map_err(|e| PatronusError::InvalidResponse(e.to_string()))?;
    let response = self.http.get(url).bearer_auth(token).timeout(MERGE_REQUEST_TIMEOUT).send().await;
    let found: MergeRequestsDto = read_json(response).await?;
    Ok(found.merge_requests)
  }

  /// `/app/rest/v1/robots/<robot_id>/<suffix…>`, the id escaped as one path segment: it comes from the page
  fn robot_api_url(&self, robot_id: &str, suffix: &[&str]) -> Result<reqwest::Url, PatronusError> {
    let mut url = self.api_url(&["robots", robot_id])?;
    url.path_segments_mut().map_err(|()| self.not_a_base_url())?.extend(suffix);
    Ok(url)
  }

  /// `/app/rest/v1/<segment…>`, each escaped as one path segment: the ids come from the page or from an answer
  fn api_url(&self, segments: &[&str]) -> Result<reqwest::Url, PatronusError> {
    let mut url = reqwest::Url::parse(&format!("{}/app/rest/v1", self.base_url)).map_err(|e| PatronusError::InvalidResponse(e.to_string()))?;
    url.path_segments_mut().map_err(|()| self.not_a_base_url())?.extend(segments);
    Ok(url)
  }

  fn not_a_base_url(&self) -> PatronusError {
    PatronusError::InvalidResponse(format!("{} is not a base URL", self.base_url))
  }

  /// A GET, asked once more after a transient failure
  async fn get_json<T: DeserializeOwned>(&self, request: impl Fn() -> reqwest::RequestBuilder, timeout: Duration) -> Result<T, PatronusError> {
    match read_json(request().timeout(timeout).send().await).await {
      Err(e) if e.is_transient() => {
        debug!(error = ?e, "asking Patronus again");
        tokio::time::sleep(self.retry_delay).await;
        read_json(request().timeout(timeout).send().await).await
      }
      result => result,
    }
  }
}

async fn read_json<T: DeserializeOwned>(response: reqwest::Result<reqwest::Response>) -> Result<T, PatronusError> {
  let body = read_body(response).await?;
  serde_json::from_slice(&body).map_err(|e| PatronusError::InvalidResponse(e.to_string()))
}

async fn read_empty(response: reqwest::Result<reqwest::Response>) -> Result<(), PatronusError> {
  read_body(response).await.map(|_| ())
}

/// The body of a successful answer; a failure classified
async fn read_body(response: reqwest::Result<reqwest::Response>) -> Result<Vec<u8>, PatronusError> {
  let response = response.map_err(|e| request_error(&e))?;
  let status = response.status();
  if status == reqwest::StatusCode::UNAUTHORIZED {
    return Err(PatronusError::Unauthorized);
  }
  let body = response.bytes().await.map_err(|e| request_error(&e))?;
  if !status.is_success() {
    // a 4xx body may be empty
    let error = serde_json::from_slice::<ErrorDto>(&body).ok();
    warn!(status = status.as_u16(), body = %String::from_utf8_lossy(&body), "Patronus request failed");
    return Err(PatronusError::Http {
      status: status.as_u16(),
      code: error.as_ref().and_then(|e| e.error.clone()),
      description: error.and_then(|e| e.description),
    });
  }
  Ok(body.to_vec())
}

fn request_error(e: &reqwest::Error) -> PatronusError {
  if e.is_timeout() {
    PatronusError::Timeout
  } else if e.is_decode() {
    PatronusError::InvalidResponse(e.to_string())
  } else {
    PatronusError::Network(e.to_string())
  }
}

#[cfg(test)]
mod tests {
  use super::{PatronusClient, ProblemsDto, RobotDto, TeamCityCheckAttemptDetailsDto, TeamCityChecksDto};
  use crate::fake_patronus::start_fake;

  #[test]
  fn a_robot_id_is_one_escaped_path_segment() {
    let client = PatronusClient::new("https://patronus.example", "test");
    assert_eq!(
      client.robot_api_url("r-1", &["status"]).unwrap().as_str(),
      "https://patronus.example/app/rest/v1/robots/r-1/status"
    );
    assert_eq!(
      client.robot_api_url("../x?y", &[]).unwrap().as_str(),
      "https://patronus.example/app/rest/v1/robots/..%2Fx%3Fy"
    );
  }

  /// Trimmed from the answers for a Safe Push whose required check failed at 08:21 while it ran until a cancel at 08:54
  #[test]
  fn a_failed_check_of_a_running_robot_reads_as_patronus_answers_it() {
    let checks: TeamCityChecksDto = serde_json::from_str(
      r#"{"robotId": "44d9", "teamCityChecks": [
        {"id": "c1", "name": "Bazel Community Build Aggregator", "buildConfigurationId": "BazelCommunityBuild_Aggregator",
         "buildConfigurationName": "Bazel Community Build Aggregator", "buildConfigurationProjectName": "Bazel / Community",
         "status": "FAILURE", "attemptLimit": 1, "problems": [],
         "attempts": [{"id": "a1", "number": 0, "buildId": "1095220100", "buildUrl": "https://ci/build/1095220100",
           "startedAt": "2026-10-16T08:17:55.539990Z", "finishedAt": "2026-10-16T08:21:12Z", "status": "FAILURE",
           "failedTestsNumber": 0, "failedBuildsNumber": 2, "failedToStartBuildsNumber": 0}]},
        {"id": "c2", "name": "IntelliJ Aggregator", "buildConfigurationId": "Idea_Tests_Aggregator", "buildConfigurationName": "Aggregator",
         "status": "RUNNING", "attemptLimit": 3, "problems": [],
         "attempts": [{"id": "a2", "number": 0, "buildId": "1095220200", "buildUrl": "https://ci/build/1095220200",
           "startedAt": "2026-10-16T08:18:02Z", "finishedAt": null, "status": "RUNNING"}]}
      ]}"#,
    )
    .unwrap();
    let labels: Vec<&str> = checks.team_city_checks.iter().map(super::TeamCityCheckDto::label).collect();
    assert_eq!(labels, ["Bazel Community Build Aggregator", "IntelliJ Aggregator"]);
    assert_eq!(checks.team_city_checks[1].running_build_id(), Some("1095220200"));
    let failed = &checks.team_city_checks[0];
    assert_eq!(
      (failed.build_configuration_id.as_deref(), failed.attempt_limit),
      (Some("BazelCommunityBuild_Aggregator"), Some(1))
    );
    let attempt = failed.last_attempt().unwrap();
    assert_eq!((attempt.id.as_deref(), attempt.build_url.as_deref()), (Some("a1"), Some("https://ci/build/1095220100")));
    assert_eq!(
      (attempt.failed_tests_number, attempt.failed_builds_number, attempt.failed_to_start_builds_number),
      (Some(0), Some(2), Some(0))
    );
    // a running attempt has no counts yet
    assert_eq!(checks.team_city_checks[1].last_attempt().unwrap().failed_builds_number, None);

    let problems: ProblemsDto = serde_json::from_str(
      r#"{"robotId": "44d9", "problems": [{"id": "p1", "uniqueId": "robot_is_failing",
        "title": "This Safe Push will fail because required checks have already failed", "detailsMarkdown": "Cancel it to save resources",
        "severity": "WARNING", "subsystem": "TEAMCITY_CHECKS"}]}"#,
    )
    .unwrap();
    assert!(problems.problems[0].is_robot_failing());
    assert!(!problems.problems[0].is_error());
    assert_eq!(problems.problems[0].details_markdown.as_deref(), Some("Cancel it to save resources"));
  }

  /// `GET /robots/{id}` once the robot rebased; a list answer lacks both revisions
  #[test]
  fn a_robot_has_the_revision_teamcity_built_and_the_target_revision() {
    let robot: RobotDto = serde_json::from_str(
      r#"{"name": "IJPL-1 fix", "id": "44d9", "repository": "ultimate", "sourceBranch": "refs/heads/me/virtual/IJPL-1",
        "sourceRevision": "aaa", "sourceBranchDeletePolicy": "NEVER", "testReferenceUpdateMode": null,
        "testReference": "refs/patronus/robots/44d9", "testReferenceRevision": "ccc", "targetBranch": "refs/heads/master",
        "targetRevision": "bbb", "startDateTime": "2026-10-16T08:17:00Z", "finishDateTime": null, "status": "RUNNING",
        "type": "WEB_UI", "pushMode": "REBASE_AUTOSQUASH", "cancellationReason": null,
        "owner": {"type": "USER", "id": "1", "name": "Me"}, "options": {}, "isEmergency": false}"#,
    )
    .unwrap();
    assert_eq!((robot.test_reference_revision.as_deref(), robot.target_revision.as_deref()), (Some("ccc"), Some("bbb")));
  }

  /// As Patronus answers `GET /teamcity-checks/attempts/{id}` (its `TeamCityCheckAttemptDetailsDto`), trimmed
  #[test]
  fn the_failed_builds_of_an_attempt_read_as_patronus_answers_them() {
    let details: TeamCityCheckAttemptDetailsDto = serde_json::from_str(
      r#"{"id": "a1", "number": 0, "buildId": "1095220100", "buildUrl": "https://ci/build/1095220100",
        "startedAt": "2026-10-16T08:17:55Z", "finishedAt": "2026-10-16T08:21:12Z", "status": "FAILURE",
        "failedTestsNumber": 1, "failedBuildsNumber": 2, "failedToStartBuildsNumber": 0, "failedTests": [],
        "cancellationIgnoreReason": null,
        "builds": [
          {"buildId": "1095220100", "buildUrl": "https://ci/build/1095220100", "buildConfigurationId": "Aggregator",
           "buildConfigurationUrl": "https://ci/buildConfiguration/Aggregator", "buildConfigurationName": "Aggregator",
           "fullProjectName": "IntelliJ / Tests", "status": "FAILURE", "isFailedToStart": false, "isComposite": true,
           "isFailedTestsOnlyFailure": false, "problems": [], "failedTests": []},
          {"buildId": "1095219213", "buildUrl": "https://ci/build/1095219213", "buildConfigurationId": "Compile",
           "buildConfigurationUrl": "https://ci/buildConfiguration/Compile", "buildConfigurationName": "Compile",
           "fullProjectName": "IntelliJ / Build", "status": "FAILURE", "isFailedToStart": false, "isComposite": false,
           "isFailedTestsOnlyFailure": true,
           "problems": [{"details": "1 test failed", "type": "TC_FAILED_TESTS", "isIgnoredProblem": false, "ignoreReason": null, "problemId": "p1"}],
           "failedTests": [{"name": "com.intellij.FooTest.testBar", "url": "https://ci/test/1", "isIgnoredFailure": false,
             "ignoreReason": null, "buildId": "1095219213", "testId": "-42", "testRunId": "2000000002", "oldUiUrl": null,
             "testHistoryUrl": null, "testNameMethodWithParameters": "testBar", "testNameClass": "FooTest",
             "testNamePackage": "com.intellij", "testNameSuite": null, "testNameWithoutSuite": "com.intellij.FooTest.testBar"}]}
        ]}"#,
    )
    .unwrap();
    assert!(details.builds[0].is_composite);
    let build = &details.builds[1];
    assert!(build.is_failed_tests_only_failure && !build.is_failed_to_start);
    assert_eq!(build.full_project_name.as_deref(), Some("IntelliJ / Build"));
    assert_eq!(
      (build.problems[0].kind.as_deref(), build.problems[0].details.as_str()),
      (Some("TC_FAILED_TESTS"), "1 test failed")
    );
    let test = &build.failed_tests[0];
    assert_eq!(
      (test.build_id.as_str(), test.test_id.as_str(), test.test_run_id.as_str()),
      ("1095219213", "-42", "2000000002")
    );
  }

  #[tokio::test]
  async fn asks_the_failed_builds_of_an_attempt_and_the_message_of_a_test() {
    let (fake, client) = start_fake().await;
    let details = client.get_attempt_details("t", "a1").await.unwrap();
    assert!(!details.builds.is_empty());
    let message = client.get_test_run_details("t", "301", "-7", "2000000001").await.unwrap();
    assert!(message.starts_with("expected:"), "{message}");
    assert_eq!(*fake.attempt_detail_calls.lock().unwrap(), ["a1"]);
    assert_eq!(*fake.test_run_calls.lock().unwrap(), ["301/-7/2000000001"]);
  }
}
