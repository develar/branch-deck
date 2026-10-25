//! The fixtures of the remote services: what a test sets in the shapes the page knows (`PatronusRobot`,
//! `PatronusRobotProgress`), written once into the wire form of the fake that the real service then reads, so the
//! answer the page gets went through the real client and service. `remote_fixtures_test.rs` keeps each form faithful:
//! the real parser of the wire form gives back the fixture.

use crate::remote_host::{GITHUB_TOKEN, GITHUB_VIEWER, HeldAnswer, PAGE_CLOCK_START, RemoteHost};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::patronus::{PatronusAvailability, PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotProgress, PatronusRobotStatus, PatronusRobotsResult};
use domain_types::pull_request::{GitHubUser, PullRequest, PullRequestAvailability, PullRequestsResult, TokenSource};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::resolve_commit;
use github::fake_github::FakeAnswer;
use github::fake_repository::{FakePull, RepositoryState, search_data};
use patronus::client::{MERGE_PROFILE_ID_OPTION, MERGE_PROFILE_NAME_OPTION, REVIEW_APPROVAL_CHECK_SKIP_OPTION};
use remote_facts::pull_requests::parse_pull_request_maps;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use sync_core::github_repository::{GITHUB_REPOSITORY_CONFIG, GitHubRepository, parse_known_github_repository};
use sync_utils::span::in_current_span;
use sync_utils::timestamp::{format_utc_timestamp, unix_now};

/// `robot` as Patronus lists it, its source branch the full ref of its virtual branch or of the stack branch of its
/// whole stack under `branch_prefix`. Its profile is in the options, where a poll and the details have it; its URL is
/// the one the client derives from its id (the pages of `PATRONUS_URL`). An empty or `~N` `sourceRevision` stays for the hook of the host, which
/// resolves it when Patronus answers (`resolve_source_revision`)
pub fn fixture_robot(robot: &PatronusRobot, branch_prefix: &str) -> anyhow::Result<serde_json::Value> {
  let source_branch = match (robot.stack_root.as_deref(), robot.branch_name.as_deref()) {
    (Some(root), _) => Some(format!("refs/heads/{}", to_stack_branch_name(branch_prefix, root)?)),
    (None, Some(branch_name)) => Some(format!("refs/heads/{}", to_final_branch_name(branch_prefix, branch_name)?)),
    (None, None) => None,
  };
  let mut wire = serde_json::json!({
    "id": robot.id,
    "name": robot.name,
    "repository": crate::remote_host::SPACE_REPOSITORY,
    "sourceBranch": source_branch,
    "sourceRevision": robot.source_revision,
    "targetBranch": robot.target_branch,
    "startDateTime": format_utc_timestamp(robot.start_time),
    "finishDateTime": robot.finish_time.map(format_utc_timestamp),
    "status": status_name(robot.status),
    "pushMode": push_mode_name(robot.push_mode),
    "cancellationReason": robot.cancellation_reason,
    "githubPrUrl": robot.pull_request.as_ref().map(|link| link.url.clone()),
    "owner": {"type": "USER", "id": "1", "name": "Me"},
  });
  if let Some(profile) = &robot.profile {
    let mut options = serde_json::Map::new();
    let mut option = |key: &str, value: Option<String>| {
      if let Some(value) = value {
        options.insert(key.to_string(), serde_json::Value::String(value));
      }
    };
    option(MERGE_PROFILE_ID_OPTION, profile.id.clone());
    option(MERGE_PROFILE_NAME_OPTION, profile.name.clone());
    option(REVIEW_APPROVAL_CHECK_SKIP_OPTION, profile.skips_review.map(|skips| skips.to_string()));
    wire["options"] = serde_json::Value::Object(options);
  }
  Ok(wire)
}

/// What `robot-start-supported` answers for these merge profiles (`PatronusStartOptions` names the regular and the
/// emergency ones apart, as the service splits them): start supported, each mode by its wire name
pub fn fixture_start_options(fixture: &PatronusProfilesFixture) -> serde_json::Value {
  let profile = |profile: &PatronusProfile, emergency: bool| {
    serde_json::json!({
      "id": profile.id,
      "name": profile.name,
      "description": profile.description,
      "allowedPushModes": profile.allowed_push_modes.iter().map(|mode| push_mode_name(*mode)).collect::<Vec<_>>(),
      "isEmergency": emergency,
    })
  };
  let regular = fixture.profiles.iter().map(|it| profile(it, false));
  let emergency = fixture.emergency_profiles.iter().map(|it| profile(it, true));
  serde_json::json!({"supported": true, "profiles": regular.chain(emergency).collect::<Vec<_>>()})
}

/// The merge profiles Patronus offers for a start, as `PatronusStartOptions` names them. The default profile is not
/// set here: the real service takes the profile of the branch's last robot, which a fixture robot names (`profile`)
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatronusProfilesFixture {
  #[serde(default)]
  pub profiles: Vec<PatronusProfile>,
  #[serde(default)]
  pub emergency_profiles: Vec<PatronusProfile>,
}

/// What `robots/progress` folds for the robot `robot_id` that `progress` shows at `now` (Unix seconds): one running
/// check, the one that ends last, and the queued ones
pub fn fixture_progress(robot_id: &str, progress: &PatronusRobotProgress, now: u32) -> serde_json::Value {
  let finish = match (progress.remaining_seconds, progress.overtime_seconds) {
    (Some(remaining), _) => Some(now.saturating_add(remaining)),
    (None, Some(overtime)) => Some(now.saturating_sub(overtime)),
    (None, None) => None,
  };
  #[expect(clippy::cast_precision_loss, reason = "a percent of 0 to 100")]
  let fraction = progress.check_percent.map_or(0.0, |percent| percent as f32 / 100.0);
  serde_json::json!({
    "robotId": robot_id,
    "estimatedFinishAt": finish.map(format_utc_timestamp),
    "longestCheck": progress.check_name.as_ref().map(|name| serde_json::json!({"checkId": "1", "name": name, "progress": fraction})),
    "queuedChecks": progress.queued,
    "runningChecks": 1,
    "finishedChecks": 0,
    "probablyHanging": progress.probably_hanging,
  })
}

/// An empty `sourceRevision` of `robot` (wire form) is the commit its `sourceBranch` points to in the test repository
/// now, `~N` the Nth first parent of it; any other revision stays. Not resolved (no such branch): none
pub fn resolve_source_revision(git: &GitCommandExecutor, repository_path: &str, robot: &mut serde_json::Value) {
  let (Some(revision), Some(branch)) = (robot["sourceRevision"].as_str(), robot["sourceBranch"].as_str()) else {
    return;
  };
  if !(revision.is_empty() || revision.starts_with('~')) {
    return;
  }
  let resolved = resolve_commit(git, repository_path, &format!("{branch}{revision}")).ok().flatten();
  robot["sourceRevision"] = resolved.into();
}

const fn status_name(status: PatronusRobotStatus) -> &'static str {
  match status {
    PatronusRobotStatus::Running => "RUNNING",
    PatronusRobotStatus::Canceling => "CANCELING",
    PatronusRobotStatus::Canceled => "CANCELED",
    PatronusRobotStatus::Failure => "FAILURE",
    PatronusRobotStatus::Success => "SUCCESS",
    PatronusRobotStatus::Unknown => "UNKNOWN",
  }
}

const fn push_mode_name(mode: PatronusPushMode) -> &'static str {
  match mode {
    PatronusPushMode::DryRun => "DRY_RUN",
    PatronusPushMode::Rebase => "REBASE",
    PatronusPushMode::RebaseAutosquash => "REBASE_AUTOSQUASH",
    PatronusPushMode::SquashAll => "SQUASH_ALL",
    PatronusPushMode::Merge => "MERGE",
    PatronusPushMode::Unknown => "UNKNOWN",
  }
}

/// The TeamCity checks and the problems of robots, by robot id, and the failed builds of check attempts, by attempt id,
/// as Patronus answers them (`teamCityChecks`, `problems`, `builds`): what the details, the failure and the doomed robots
/// of the real service are read from. A robot or an attempt without them has none
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatronusDetailsFixture {
  #[serde(default)]
  checks: HashMap<String, serde_json::Value>,
  #[serde(default)]
  problems: HashMap<String, serde_json::Value>,
  #[serde(default)]
  attempt_builds: HashMap<String, serde_json::Value>,
}

pub(crate) type FixtureResult = Result<StatusCode, (StatusCode, String)>;

pub(crate) fn unknown_repository() -> (StatusCode, String) {
  (StatusCode::UNPROCESSABLE_ENTITY, "no such test repository".to_string())
}

/// The remote host of the repository `id` and its path
pub(crate) fn host_of(state: &AppState, id: &str) -> Result<(Arc<RemoteHost>, String), (StatusCode, String)> {
  let repo = state.repositories.get(id).ok_or_else(unknown_repository)?;
  Ok((repo.remote(&state.git_executor), repo.path.clone()))
}

/// Sets what Patronus answers from now on: the robots of the list, of a poll (those whose status changed) and of the
/// details; a `githubRepository` is written as the repository's `branchdeck.githubRepository` and is the repository of
/// the fake GitHub (`set_pull_requests` sets it too); not on Space for `notPatronus`, the token rejected for `notLoggedIn`, a failure of Patronus for `error`. The
/// source branch of a robot is the full ref under the repository's `branchdeck.branchPrefix`; an empty `sourceRevision`
/// is the tip of that branch when Patronus answers, `~N` its Nth first parent
pub async fn set_patronus(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(result): Json<PatronusRobotsResult>) -> FixtureResult {
  let (host, path) = host_of(&state, &id)?;
  let git = state.git_executor.clone();
  let robots_of = result.robots;
  let github_repository = match result.github_repository.as_deref() {
    Some(known) => Some(parse_known_github_repository(Some(known)).ok_or_else(|| (StatusCode::UNPROCESSABLE_ENTITY, format!("{known} is no owner/name")))?),
    None => None,
  };
  if let Some(github) = &github_repository {
    // the one fact of both fixtures: the repository the config names is the one the fake GitHub has
    host.github_repository().update(|state| {
      state.owner.clone_from(&github.owner);
      state.name.clone_from(&github.name);
    });
  }
  let robots = tokio::task::spawn_blocking(in_current_span(move || -> anyhow::Result<Vec<serde_json::Value>> {
    // the GitHub mirror is the repository's, as the app reads it (`resolve_github_repository`)
    if let Some(github) = &github_repository {
      write_github_repository(&git, &path, Some(github))?;
    }
    let prefix = crate::tauri_command_bridge::read_local_branch_prefix(&git, &path)?.prefix;
    robots_of.iter().map(|robot| fixture_robot(robot, &prefix)).collect()
  }))
  .await
  .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
  .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, format!("{e:#}")))?;
  let (on_space, status) = match result.availability {
    PatronusAvailability::NotPatronus => (false, 200),
    PatronusAvailability::NotLoggedIn => (true, 401),
    PatronusAvailability::Error { .. } => (true, 500),
    PatronusAvailability::Available => (true, 200),
  };
  host.set_patronus(on_space, status, robots);
  Ok(StatusCode::OK)
}

/// Sets what Patronus folds as the progress of running robots (`robots/progress`, the new API), by robot id, as of now
pub async fn set_patronus_progress(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(progress): Json<HashMap<String, PatronusRobotProgress>>) -> FixtureResult {
  let (host, _) = host_of(&state, &id)?;
  let now = unix_now();
  *host.patronus().progress.lock().expect("patronus progress lock") = progress
    .iter()
    .map(|(robot_id, progress)| (robot_id.clone(), fixture_progress(robot_id, progress, now)))
    .collect();
  Ok(StatusCode::OK)
}

/// Sets the merge profiles Patronus offers for a start (`PatronusProfilesFixture`); none until set
pub async fn set_patronus_profiles(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<PatronusProfilesFixture>) -> FixtureResult {
  let (host, _) = host_of(&state, &id)?;
  host.set_patronus_start_options(fixture_start_options(&fixture));
  Ok(StatusCode::OK)
}

/// How Patronus answers a start from now on: `description` refuses every start with it (400), logged as any start;
/// `null` starts again
#[derive(Deserialize)]
pub struct PatronusStartRefusalFixture {
  pub description: Option<String>,
}

/// Sets whether Patronus refuses the starts (`PatronusStartRefusalFixture`); a start goes through until set
pub async fn set_patronus_start_refusal(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<PatronusStartRefusalFixture>) -> FixtureResult {
  let (host, _) = host_of(&state, &id)?;
  *host.patronus().start_refusal.lock().expect("patronus start refusal lock") = fixture.description;
  Ok(StatusCode::OK)
}

/// The answers of the fakes a test holds back (`POST /repositories/{id}/remote-gates`): `hold` from now on, `fail` (an
/// answer the bridge gives) from now on, `release` the held and the failing ones; a deleted repository releases them all
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteGatesFixture {
  #[serde(default)]
  pub hold: Vec<HeldAnswer>,
  #[serde(default)]
  pub fail: Vec<HeldAnswer>,
  #[serde(default)]
  pub release: Vec<HeldAnswer>,
}

/// Holds or releases answers of the fakes (`RemoteGatesFixture`): a held request reaches its fake and waits there
pub async fn set_remote_gates(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<RemoteGatesFixture>) -> FixtureResult {
  let (host, _) = host_of(&state, &id)?;
  host.set_failing(&fixture.fail, &fixture.release);
  host.release_answers(&fixture.release);
  host.hold_answers(&fixture.hold);
  Ok(StatusCode::OK)
}

/// The held answers a request reached and waits at (`arrived`): the page asked, the answer is still to come
pub async fn get_remote_gates(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
  let (host, _) = host_of(&state, &id)?;
  Ok(Json(serde_json::json!({ "arrived": host.arrived_answers() })))
}

/// Sets the TeamCity checks and the problems Patronus answers for robots (`PatronusDetailsFixture`)
pub async fn set_patronus_details(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<PatronusDetailsFixture>) -> FixtureResult {
  let (host, _) = host_of(&state, &id)?;
  let patronus = host.patronus();
  *patronus.checks.lock().expect("patronus checks lock") = fixture.checks;
  *patronus.problems.lock().expect("patronus problems lock") = fixture.problems;
  *patronus.attempt_builds.lock().expect("patronus attempt builds lock") = fixture.attempt_builds;
  Ok(StatusCode::OK)
}

/// More robots of the list, after those of `set_patronus` (`POST /repositories/{id}/patronus-history`): the history
/// the statistics count, the user's (owner `1`) and other people's
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatronusHistoryFixture {
  pub robots: Vec<HistoryRobotFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRobotFixture {
  pub id: String,
  /// `1` is the user
  pub owner: String,
  pub status: PatronusRobotStatus,
  /// Unix seconds
  pub start_time: u32,
  pub finish_time: Option<u32>,
  /// The virtual branch it ran on, of the user's
  #[serde(default)]
  pub branch: Option<String>,
  #[serde(default)]
  pub dry_run: bool,
  #[serde(default)]
  pub source_robot_id: Option<String>,
}

/// `robot` as Patronus lists it to the statistics, its branch under `branch_prefix`
pub fn fixture_history_robot(robot: &HistoryRobotFixture, branch_prefix: &str) -> anyhow::Result<serde_json::Value> {
  let source_branch = robot
    .branch
    .as_deref()
    .map(|branch| to_final_branch_name(branch_prefix, branch).map(|name| format!("refs/heads/{name}")))
    .transpose()?;
  Ok(serde_json::json!({
    "id": robot.id,
    "name": robot.id,
    "repository": crate::remote_host::SPACE_REPOSITORY,
    "sourceBranch": source_branch,
    "sourceRevision": "",
    "targetBranch": "master",
    "startDateTime": format_utc_timestamp(robot.start_time),
    "finishDateTime": robot.finish_time.map(format_utc_timestamp),
    "status": status_name(robot.status),
    "pushMode": if robot.dry_run { "DRY_RUN" } else { "REBASE_AUTOSQUASH" },
    "type": "WEB_UI",
    "isEmergency": false,
    "sourceRobotId": robot.source_robot_id,
    "owner": {"type": "USER", "id": robot.owner, "name": format!("User {}", robot.owner)},
  }))
}

/// Adds the robots of `PatronusHistoryFixture` to the list of the fake Patronus
pub async fn set_patronus_history(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<PatronusHistoryFixture>) -> FixtureResult {
  let (host, path) = host_of(&state, &id)?;
  let git = state.git_executor.clone();
  let prefix = tokio::task::spawn_blocking(in_current_span(move || {
    crate::tauri_command_bridge::read_local_branch_prefix(&git, &path).map(|prefix| prefix.prefix)
  }))
  .await
  .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
  .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, format!("{e:#}")))?;
  let listed = fixture
    .robots
    .iter()
    .map(|robot| fixture_history_robot(robot, &prefix))
    .collect::<anyhow::Result<Vec<_>>>()
    .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, format!("{e:#}")))?;
  let patronus = host.patronus();
  let mut robots = patronus.robots.lock().expect("patronus robots lock");
  robots.get_or_insert_with(Vec::new).extend(listed);
  Ok(StatusCode::OK)
}

/// What the app asked (`listCalls`, `pollCalls`, `detailCalls`: the lists, the polls and the reads of the details of the
/// session, which the caches of the service may answer), the robots the lists and polls answered by id (`robots`), what reached
/// the fake Patronus (`requests`), and the actions it took, in order: the bodies of the start requests (`starts`), the
/// robots restarted (`restarts`), canceled (`cancels`) and whose status was read (`statusReads`)
pub async fn get_patronus_fixture(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
  let repo = state.repositories.get(&id).ok_or_else(unknown_repository)?;
  let host = repo.remote(&state.git_executor);
  let fake = host.patronus();
  let ordering = Ordering::SeqCst;
  Ok(Json(serde_json::json!({
    "listCalls": repo.patronus_list_calls.load(ordering),
    "pollCalls": repo.patronus_poll_calls.load(ordering),
    "robots": repo.patronus_answered.lock().expect("patronus answered lock").clone(),
    "detailCalls": repo.patronus_detail_calls.load(ordering),
    "requests": {
      "list": fake.list_calls.load(ordering),
      "updates": fake.update_calls.load(ordering),
      "details": fake.detail_calls.load(ordering),
      "problems": fake.problem_calls.lock().expect("patronus problem calls lock").clone(),
      "progress": fake.progress_calls.lock().expect("patronus progress calls lock").clone(),
      "startOptions": fake.start_supported_calls.load(ordering),
      "listQueries": fake.list_queries.lock().expect("patronus list queries lock").clone(),
      "checks": fake.checks_calls.lock().expect("patronus checks calls lock").clone(),
    },
    "starts": fake.starts.lock().expect("patronus starts lock").clone(),
    "restarts": fake.restarts.lock().expect("patronus restarts lock").clone(),
    "cancels": fake.canceled.lock().expect("patronus cancels lock").clone(),
    "statusReads": fake.status_calls.lock().expect("patronus status calls lock").clone(),
  })))
}

/// What the GitHub of a test repository holds (`POST /repositories/{id}/pull-requests`): the listing of the session's
/// pull requests (`PullRequestsResult`), and what only the writes and the other reads see
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubFixture {
  /// The availability says what GitHub answers: `available` names the repository, `notGitHub` names none (no
  /// `branchdeck.githubRepository`), `signedOut` has no token file, the others answer every request with their failure
  #[serde(flatten)]
  pub result: PullRequestsResult,
  /// `owner/name` of the repository of an availability that doesn't name it
  #[serde(default)]
  pub repository: Option<String>,
  /// Who can be assigned and asked to review
  #[serde(default)]
  pub users: Vec<GitHubUser>,
  /// The rules of every branch dismiss the approvals of a pull request on a push
  #[serde(default)]
  pub dismiss_stale_reviews_on_push: bool,
  /// The number of the next pull request created; one more than the highest of the fixture without it
  #[serde(default)]
  pub next_number: Option<u32>,
  /// Branches GitHub doesn't have yet although the test repository does: a mirror has not copied them
  #[serde(default)]
  pub missing_branches: std::collections::BTreeSet<String>,
}

/// The GitHub repository a fixture names, and how GitHub answers: signed in, and the failure of every request
pub struct GitHubAnswers {
  pub repository: Option<GitHubRepository>,
  pub signed_in: bool,
  pub refusal: Option<FakeAnswer>,
}

/// How the fake answers `fixture`'s availability. The token is the one of the app's sign-in, so a failure of a token
/// names that source (`branchDeck`)
pub fn github_answers(fixture: &GitHubFixture) -> anyhow::Result<GitHubAnswers> {
  let repository = match &fixture.result.availability {
    PullRequestAvailability::Available { owner, name } => Some(GitHubRepository {
      owner: owner.clone(),
      name: name.clone(),
    }),
    PullRequestAvailability::NotGitHub => None,
    _ => Some(parse_known_github_repository(fixture.repository.as_deref()).ok_or_else(|| anyhow::anyhow!("the fixture names no repository (`repository`)"))?),
  };
  let only_sign_in = |source: TokenSource| {
    anyhow::ensure!(
      source == TokenSource::BranchDeck,
      "the token of the test server is the one of the app's sign-in (`branchDeck`), not {source:?}"
    );
    Ok(())
  };
  let message = |message: &str| serde_json::json!({"message": message});
  let refusal = match &fixture.result.availability {
    PullRequestAvailability::Available { .. } | PullRequestAvailability::NotGitHub | PullRequestAvailability::SignedOut => None,
    PullRequestAvailability::NotAuthenticated { source } => {
      only_sign_in(*source)?;
      Some(FakeAnswer::status(401, message("Bad credentials")))
    }
    PullRequestAvailability::NoAccess { source, message: text } => {
      only_sign_in(*source)?;
      Some(FakeAnswer::status(404, message(text)))
    }
    PullRequestAvailability::RateLimited { retry_at } => Some(
      FakeAnswer::status(403, message("API rate limit exceeded"))
        .header("x-ratelimit-remaining", "0")
        .header("x-ratelimit-reset", &retry_at.to_string()),
    ),
    PullRequestAvailability::SsoRequired { url } => Some(
      FakeAnswer::status(403, message("Resource protected by organization SAML enforcement"))
        .header("x-github-sso", &url.as_ref().map_or_else(|| "required".to_string(), |url| format!("required; url={url}"))),
    ),
    PullRequestAvailability::Error { message: text } => Some(FakeAnswer::status(400, message(text))),
  };
  Ok(GitHubAnswers {
    repository,
    signed_in: fixture.result.availability != PullRequestAvailability::SignedOut,
    refusal,
  })
}

/// The pull requests of `result` as the fake holds them, after a check that the real parser reads the search of them
/// back into `result` (`parse_pull_request_maps`): a fixture GitHub could not answer fails, e.g. one of another author
/// or a requested reviewer listed before one who reviewed
pub fn fixture_pulls(result: &PullRequestsResult, owner: &str, branch_prefix: &str) -> anyhow::Result<Vec<FakePull>> {
  let pull_requests: Vec<PullRequest> = result.pull_requests.values().chain(result.stack_pull_requests.values()).cloned().collect();
  let data = serde_json::to_vec(&search_data(owner, GITHUB_VIEWER, pull_requests.clone()))?;
  let (branches, stacks) = parse_pull_request_maps(&data, branch_prefix, owner)?;
  anyhow::ensure!(
    branches == result.pull_requests && stacks == result.stack_pull_requests,
    "GitHub can't answer these pull requests: the real parser reads {branches:?} and the stacks {stacks:?}"
  );
  Ok(
    pull_requests
      .into_iter()
      .map(|pull_request| FakePull {
        pull_request,
        body: String::new(),
        comments: Vec::new(),
        reviews: Vec::new(),
        assignees: Vec::new(),
      })
      .collect(),
  )
}

/// `branchdeck.githubRepository` of the test repository, the GitHub repository the app resolves (its `origin` is a
/// local path): `owner/name`, or unset for none
pub fn write_github_repository(git: &GitCommandExecutor, repository_path: &str, repository: Option<&GitHubRepository>) -> anyhow::Result<()> {
  if let Some(repository) = repository {
    git.execute_command(&["config", "--local", GITHUB_REPOSITORY_CONFIG, &repository.full_name()], repository_path)?;
  } else {
    // 5: it was not set
    let (output, code) = git.execute_command_with_status(&["config", "--local", "--unset-all", GITHUB_REPOSITORY_CONFIG], repository_path)?;
    anyhow::ensure!(code == 0 || code == 5, "Cannot unset {GITHUB_REPOSITORY_CONFIG}: {output}");
  }
  Ok(())
}

/// Sets what the GitHub of the repository holds and answers from now on (`GitHubFixture`), read by the real service; a
/// fixture the real parser would not read back as it is fails (`fixture_pulls`)
pub async fn set_pull_requests(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<GitHubFixture>) -> FixtureResult {
  let (host, path) = host_of(&state, &id)?;
  let answers = github_answers(&fixture).map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, format!("{e:#}")))?;
  let (git, repository, result) = (state.git_executor.clone(), answers.repository.clone(), fixture.result.clone());
  let pulls = tokio::task::spawn_blocking(in_current_span(move || -> anyhow::Result<Vec<FakePull>> {
    write_github_repository(&git, &path, repository.as_ref())?;
    let prefix = crate::tauri_command_bridge::read_local_branch_prefix(&git, &path)?.prefix;
    match &repository {
      Some(repository) => fixture_pulls(&result, &repository.owner, &prefix),
      None if result.pull_requests.is_empty() && result.stack_pull_requests.is_empty() => Ok(Vec::new()),
      None => anyhow::bail!("pull requests of a repository that is not on GitHub"),
    }
  }))
  .await
  .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
  .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, format!("{e:#}")))?;
  let next_number = fixture
    .next_number
    .unwrap_or_else(|| pulls.iter().map(|pull| pull.pull_request.number).max().unwrap_or(0) + 1);
  let (owner, name) = answers.repository.map_or_default(|repository| (repository.owner, repository.name));
  let github = RepositoryState {
    owner,
    name,
    token: GITHUB_TOKEN.to_string(),
    viewer: GITHUB_VIEWER.to_string(),
    refusal: answers.refusal,
    pulls,
    next_number,
    users: fixture.users,
    missing_branches: fixture.missing_branches,
    dismiss_stale_reviews_on_push: fixture.dismiss_stale_reviews_on_push,
    clock: PAGE_CLOCK_START,
    // the host keeps the replies of the fixture before
    patronus_replies: std::collections::BTreeMap::new(),
  };
  host.set_github(github, answers.signed_in);
  Ok(StatusCode::OK)
}

/// What the app asked of GitHub (`calls`: the session's lists of the pull requests) and every request that reached the fake GitHub, in
/// order: method, path and JSON body (null for none)
pub async fn get_github_log(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
  let repo = state.repositories.get(&id).ok_or_else(unknown_repository)?;
  let requests: Vec<serde_json::Value> = repo
    .remote(&state.git_executor)
    .github()
    .requests()
    .iter()
    .map(|request| serde_json::json!({"method": request.method, "path": request.path, "body": request.json()}))
    .collect();
  Ok(Json(serde_json::json!({"calls": repo.pull_request_calls.load(Ordering::SeqCst), "requests": requests})))
}

/// The app's GitHub sign-in from now on; a field left out changes nothing
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitHubSignInFixture {
  /// The OAuth app of the build: the test server's build has none, as one without `BRANCH_DECK_GITHUB_CLIENT_ID`
  #[serde(default)]
  pub client_id: Option<String>,
  /// The user entered the codes issued so far on GitHub: the wait of a sign-in gets the token of the host's user
  #[serde(default)]
  pub authorize: bool,
  /// The token file holds the sign-in of the host's user (signed in elsewhere, by `bd` or another window), or none
  #[serde(default)]
  pub signed_in: Option<bool>,
  /// `gh` is signed in with this token (letters, digits, `_` and `-`)
  #[serde(default)]
  pub gh_token: Option<String>,
}

/// Sets the app's GitHub sign-in (`GitHubSignInFixture`), which the real session and token chain read
pub async fn set_github_sign_in(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<GitHubSignInFixture>) -> FixtureResult {
  let (host, _) = host_of(&state, &id)?;
  if let Some(token) = &fixture.gh_token {
    if token.is_empty() || !token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
      return Err((StatusCode::UNPROCESSABLE_ENTITY, format!("the gh token {token:?} is not letters, digits, `_` and `-`")));
    }
    host.set_gh_token(token);
  }
  if let Some(client_id) = fixture.client_id {
    host.set_github_client_id(client_id);
  }
  if let Some(signed_in) = fixture.signed_in {
    host.set_github_signed_in(signed_in);
  }
  if fixture.authorize {
    host.github().authorize_device_codes(GITHUB_TOKEN);
  }
  Ok(StatusCode::OK)
}

/// The sign-in as the session holds it (`pending`: a sign-in waits for its code) and what reached the device flow of
/// the fake GitHub: the codes asked for and the polls for the token
pub async fn get_github_sign_in(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
  let (host, _) = host_of(&state, &id)?;
  let count = |path: &str| host.github().requests_to("POST", path).len();
  Ok(Json(serde_json::json!({
    "pending": host.github_sign_in().is_pending(),
    "codes": count("/login/device/code"),
    "polls": count("/login/oauth/access_token"),
  })))
}
