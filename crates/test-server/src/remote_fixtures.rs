//! The fixtures of the remote services: what a test sets in the shapes the page knows (`PatronusRobot`,
//! `PatronusRobotProgress`), written once into the wire form of the fake that the real service then reads, so the
//! answer the page gets went through the real client and service. `remote_fixtures_test.rs` keeps each form faithful:
//! the real parser of the wire form gives back the fixture.

use crate::remote_host::RemoteHost;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_names::{to_final_branch_name, to_stack_branch_name};
use patronus::client::{MERGE_PROFILE_ID_OPTION, MERGE_PROFILE_NAME_OPTION, REVIEW_APPROVAL_CHECK_SKIP_OPTION};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use sync_core::pull_requests::GITHUB_REPOSITORY_CONFIG;
use sync_types::patronus::{PatronusAvailability, PatronusPushMode, PatronusRobot, PatronusRobotProgress, PatronusRobotStatus, PatronusRobotsResult};
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
  let resolved = git
    .execute_command(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{branch}{revision}")], repository_path)
    .ok();
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

type FixtureResult = Result<StatusCode, (StatusCode, String)>;

fn unknown_repository() -> (StatusCode, String) {
  (StatusCode::UNPROCESSABLE_ENTITY, "no such test repository".to_string())
}

/// The remote host of the repository `id` and its path
fn host_of(state: &AppState, id: &str) -> Result<(Arc<RemoteHost>, String), (StatusCode, String)> {
  let repo = state.repositories.get(id).ok_or_else(unknown_repository)?;
  Ok((repo.remote(&state.git_executor), repo.path.clone()))
}

/// Sets what Patronus answers from now on: the robots of the list, of a poll (those whose status changed) and of the
/// details; a `githubRepository` is written as the repository's `branchdeck.githubRepository`; not on Space for `notPatronus`, the token rejected for `notLoggedIn`, a failure of Patronus for `error`. The
/// source branch of a robot is the full ref under the repository's `branchdeck.branchPrefix`; an empty `sourceRevision`
/// is the tip of that branch when Patronus answers, `~N` its Nth first parent
pub async fn set_patronus(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(result): Json<PatronusRobotsResult>) -> FixtureResult {
  let (host, path) = host_of(&state, &id)?;
  let git = state.git_executor.clone();
  let robots_of = result.robots;
  let github_repository = result.github_repository;
  let robots = tokio::task::spawn_blocking(in_current_span(move || -> anyhow::Result<Vec<serde_json::Value>> {
    // the GitHub mirror is the repository's, as the app reads it (`resolve_github_repository`)
    if let Some(github) = github_repository {
      git.execute_command(&["config", "--local", GITHUB_REPOSITORY_CONFIG, &github], &path)?;
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

/// Sets the TeamCity checks and the problems Patronus answers for robots (`PatronusDetailsFixture`)
pub async fn set_patronus_details(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<PatronusDetailsFixture>) -> FixtureResult {
  let (host, _) = host_of(&state, &id)?;
  let patronus = host.patronus();
  *patronus.checks.lock().expect("patronus checks lock") = fixture.checks;
  *patronus.problems.lock().expect("patronus problems lock") = fixture.problems;
  *patronus.attempt_builds.lock().expect("patronus attempt builds lock") = fixture.attempt_builds;
  Ok(StatusCode::OK)
}

/// What the page asked (`listCalls`, `pollCalls`, `detailCalls`: its commands, which the caches of the service may
/// answer) and what reached the fake Patronus (`requests`)
pub async fn get_patronus_fixture(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
  let repo = state.repositories.get(&id).ok_or_else(unknown_repository)?;
  let host = repo.remote(&state.git_executor);
  let fake = host.patronus();
  let ordering = Ordering::SeqCst;
  Ok(Json(serde_json::json!({
    "listCalls": repo.patronus_list_calls.load(ordering),
    "pollCalls": repo.patronus_poll_calls.load(ordering),
    "detailCalls": repo.patronus_detail_calls.load(ordering),
    "requests": {
      "list": fake.list_calls.load(ordering),
      "updates": fake.update_calls.load(ordering),
      "details": fake.detail_calls.load(ordering),
      "problems": fake.problem_calls.lock().expect("patronus problem calls lock").clone(),
      "progress": fake.progress_calls.lock().expect("patronus progress calls lock").clone(),
      "startOptions": fake.start_supported_calls.load(ordering),
    },
  })))
}
