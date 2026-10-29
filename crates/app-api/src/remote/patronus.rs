//! The robots of Patronus as the windows read them beside the facts of the session: the Safe Push statistics of the
//! repository (spec/repository/safe-push-statistics.spec.md), why a robot failed for the prompt for an agent
//! (spec/branch/safe-push-failure-prompt.spec.md) and which commits a running robot covers
//! (spec/branch/patronus.spec.md), over the Patronus of the session (`ServiceHost`)

use crate::api_error::patronus_error;
use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::transport::{Call, Sink};
use domain_types::error::ApiError;
use domain_types::patronus::PatronusAvailability;
use domain_types::robot_failure::{GetRobotCommitMapParams, GetRobotFailureParams, RobotCommitMap, RobotFailureReport};
use domain_types::robot_stats::{GetRobotStatsParams, RobotStatsEvent, RobotStatsResult};
use remote_facts::robot_stats::{RobotStatsSource, robot_stats_core};
use repository_model::session::SessionKey;
use sync_core::robot_commits::{RobotSubject, robot_failure_report, robot_subject_commit_map};
use tracing::instrument;

/// Everyone's robots into the target of the period (`robot_stats_core`), kept in the object cache of the repository, at
/// the time and in the calendar of the app (`AppHost`); off Space `notPatronus`
pub async fn get_robot_stats(ctx: &AppContext, _call: &Call, key: SessionKey, params: GetRobotStatsParams, events: Sink<RobotStatsEvent>) -> Result<RobotStatsResult, ApiError> {
  let Some(repository) = ctx.services.space_repository(&key).await? else {
    return Ok(RobotStatsResult {
      availability: PatronusAvailability::NotPatronus,
      stale: false,
      fetched_at: None,
      stats: None,
    });
  };
  let target = ctx.services.target(&key, &params.baseline_branch).await?;
  let service = ctx.services.patronus(&key)?;
  let cache = ctx.services.object_cache(&key).await;
  let source = RobotStatsSource {
    service: &service,
    cache: &cache,
    repository: &repository.name,
    target: &target,
    branch_prefix: &key.prefix,
    reads: Some(&ctx.remote.robot_stats),
  };
  let send = |event: RobotStatsEvent| {
    if events.send(event).is_err() {
      tracing::debug!("the statistics window is gone");
    }
  };
  Ok(robot_stats_core(&source, &ctx.host.calendar(), ctx.host.now(), params.period, params.refresh, &send).await)
}

/// The failure of the robot from Patronus, fast from its details or full with the logs of the `teamcity` CLI of the
/// app (`AppHost::teamcity`), with the commit map of its branch (`robot_failure_report`, one blocking step)
pub async fn get_robot_failure(ctx: &AppContext, _call: &Call, key: SessionKey, params: GetRobotFailureParams) -> Result<RobotFailureReport, ApiError> {
  let service = ctx.services.patronus(&key)?;
  let failure = if params.full {
    let teamcity = ctx.host.teamcity().await;
    service.robot_failure_full(&key.prefix, &params.robot_id, teamcity.as_ref()).await
  } else {
    service.robot_failure(&key.prefix, &params.robot_id).await
  }
  .map_err(patronus_error)?;
  let git = ctx.git.clone();
  run_blocking(move || robot_failure_report(&git, &key.worktree, &key.prefix, &params.ancestors, &params.baseline_branch, failure)).await
}

/// The commits of the subject of a running robot by the hashes of the checked-out branch (`robot_subject_commit_map`)
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = ?params.branch_name, stack_root = ?params.stack_root, ancestors = params.ancestors.len()))]
pub async fn get_robot_commit_map(ctx: &AppContext, params: GetRobotCommitMapParams) -> Result<RobotCommitMap, ApiError> {
  let inputs = ctx.prefix_inputs();
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let subject = RobotSubject {
      branch_name: params.branch_name.as_deref(),
      stack_root: params.stack_root.as_deref(),
    };
    robot_subject_commit_map(
      &inputs.git,
      &params.repository_path,
      &branch_prefix,
      subject,
      &params.source_revision,
      &params.baseline_branch,
      &params.ancestors,
    )
    .map(Option::unwrap_or_default)
  })
  .await
}
