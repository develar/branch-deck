//! `bd robot-stats`: the Safe Push statistics of the repository (`spec/repository/safe-push-statistics.spec.md`), the
//! numbers of the app's Landing view from the same core (`remote_facts::robot_stats`): the user's robots into the target
//! of the last 70 days against everyone else's, where the time went, why they failed and the hours they ran fastest.
//! Read only: no lock, no sync; what Patronus answered is kept in the object cache (none with --no-cache).

use crate::safe_push_command::{patronus_error, patronus_service};
use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::calendar::Calendar;
use domain_types::error::ErrorCode;
use domain_types::patronus::PatronusAvailability;
use domain_types::robot_stats::RobotStats;
use history_rewrite::baseline::detect_baseline_branch;
use remote_facts::robot_stats::{RobotStatsSource, robot_stats_core};
use schemars::JsonSchema;
use serde::Serialize;
use sync_core::patronus::required_target_branch;
use sync_utils::timestamp::unix_now;

#[derive(Debug, Default, clap::Args)]
pub struct RobotStatsArgs {
  /// List the weeks that have not settled again, whatever the age of their list (the app's Refresh)
  #[arg(long)]
  pub refresh: bool,
}

/// `bd robot-stats`
#[derive(Debug, Serialize, JsonSchema)]
pub struct RobotStatsOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  /// Patronus failed: the numbers are of what was kept, `warning` says why
  pub stale: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub warning: Option<String>,
  /// Unix seconds: when the newest week was listed
  pub fetched_at: Option<u32>,
  #[serde(flatten)]
  pub stats: RobotStats,
}

pub fn robot_stats(context: &RepoContext, args: &RobotStatsArgs, environment: &Environment) -> Result<RobotStatsOutput, CliError> {
  let service = patronus_service(environment)?;
  let space = context
    .space_repository()?
    .ok_or_else(|| CliError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus has no robots of it"))?;
  let baseline = detect_baseline_branch(&context.git, &context.repository)?;
  let target = required_target_branch(&context.git, &context.repository, &baseline).map_err(|e| CliError::new(ErrorCode::NoBaseline, format!("{e:#}")))?;
  let cache = context.cache();
  let source = RobotStatsSource {
    service: &service,
    cache: &cache,
    repository: &space.name,
    target: &target,
    branch_prefix: &context.prefix,
  };
  let calendar = Calendar::from_zone(jiff::tz::TimeZone::system());
  let result = context.runtime()?.block_on(robot_stats_core(&source, &calendar, unix_now(), args.refresh, &|_| {}));
  let warning = match &result.availability {
    PatronusAvailability::Available => None,
    PatronusAvailability::Error { message } => Some(message.clone()),
    PatronusAvailability::NotLoggedIn => Some("Patronus rejected the Space token: run ./safePush.cmd once to log in again".to_string()),
    PatronusAvailability::NotPatronus => Some("The repository is not on Space".to_string()),
  };
  let Some(stats) = result.stats else {
    return Err(match result.availability {
      PatronusAvailability::NotLoggedIn => CliError::new(ErrorCode::NotLoggedIn, warning.unwrap_or_default()),
      _ => patronus_error(warning.unwrap_or_else(|| "Patronus listed no robot".to_string())),
    });
  };
  Ok(RobotStatsOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    stale: result.stale,
    warning,
    fetched_at: result.fetched_at,
    stats,
  })
}
