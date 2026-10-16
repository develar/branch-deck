//! What holds the push of a virtual branch in `bd push`, as the app decides it (`PushHold`, `src/lib/patronus.ts`): a
//! Safe Push that runs on the branch, or on a branch built on it, which lands it. A push now won't land (the robot merges
//! the commit it took), and a push into the robot's own branch makes it fail at the end ("branch was modified"). Best
//! effort: a repository not on Space, no Space token or no answer of Patronus holds nothing.

use crate::context::{Environment, RepoContext};
use crate::error::{CliError, ErrorCode};
use crate::safe_push_command::{patronus_runtime, patronus_service, robots};
use crate::status::{read_grouping, stack_ancestors};
use crate::sync_command::SyncOutput;
use git_ops::model::to_final_branch_name;
use patronus::service::PatronusService;
use std::collections::HashMap;
use sync_core::grouping::BranchGrouping;
use sync_core::grouping::branch_tips;
use sync_core::patronus::find_space_repository;
use sync_core::patronus_start::{find_branch_robot, find_landing_descendant, push_mode_name};
use sync_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use tokio::runtime::Runtime;

/// The robot that holds the push of a branch
#[derive(Debug, Clone)]
pub struct PushHold {
  /// The branch of the robot: the held branch itself, or a branch built on it
  pub branch: String,
  pub robot: PatronusRobot,
}

impl PushHold {
  /// The robot runs on the held branch itself: `--restart-running` cancels it, pushes and restarts it
  pub fn is_own(&self, name: &str) -> bool {
    self.branch == name
  }

  /// "A Safe Push of auth is running: …", in the words of the app
  pub fn reason(&self, name: &str) -> String {
    let mode = push_mode_name(self.robot.push_mode);
    if self.is_own(name) {
      format!(
        "A {mode} of {name} is running: a push won't land, and it fails the {mode} at the end. Wait until it merges, or pass --restart-running to cancel it, push and restart it ({})",
        self.robot.url
      )
    } else {
      format!("A {mode} of {} is running: it lands {name} with it, a push won't land ({})", self.branch, self.robot.url)
    }
  }
}

/// The holds of the branches, and the Patronus service that cancels and restarts their robots
#[derive(Default)]
pub struct PushHolds {
  holds: HashMap<String, PushHold>,
  service: Option<(PatronusService, Runtime, String)>,
}

impl PushHolds {
  pub fn get(&self, name: &str) -> Option<&PushHold> {
    self.holds.get(name)
  }

  /// Cancels the robot before the push that restarts it
  pub fn cancel(&self, robot: &PatronusRobot) -> Result<(), CliError> {
    let (service, runtime, prefix) = self.service()?;
    runtime
      .block_on(service.cancel(prefix, &robot.id))
      .map_err(|message| CliError::new(ErrorCode::PatronusError, message))?;
    Ok(())
  }

  /// Starts the canceled robot again, on the pushed commit
  pub fn restart(&self, robot: &PatronusRobot) -> Result<PatronusRobot, CliError> {
    let (service, runtime, prefix) = self.service()?;
    runtime
      .block_on(service.restart(prefix, &robot.id))
      .map_err(|message| CliError::new(ErrorCode::PatronusError, message))
  }

  fn service(&self) -> Result<&(PatronusService, Runtime, String), CliError> {
    self.service.as_ref().ok_or_else(|| CliError::new(ErrorCode::PatronusError, "Patronus is not available"))
  }
}

/// A robot that holds the pushes of the branches it lands: running (a canceling one won't merge) and pushing into the
/// target (a dry run lands nothing)
fn is_holding(robot: &PatronusRobot) -> bool {
  robot.status == PatronusRobotStatus::Running && robot.push_mode != PatronusPushMode::DryRun
}

/// The holds of every synced branch: its own robot started on it, else the robot of the first branch built on it that
/// lands it. One robots request; nothing without Space, a token or an answer of Patronus.
pub fn read_push_holds(context: &RepoContext, environment: &Environment, synced: &SyncOutput) -> PushHolds {
  match find_holds(context, environment, synced) {
    Ok(holds) => holds,
    Err(error) => {
      tracing::debug!(error = %error.message, "no push holds: Patronus is not available");
      PushHolds::default()
    }
  }
}

fn find_holds(context: &RepoContext, environment: &Environment, synced: &SyncOutput) -> Result<PushHolds, CliError> {
  let Some(space) = find_space_repository(&context.git, &context.repository)? else {
    return Ok(PushHolds::default());
  };
  let service = patronus_service(environment)?;
  let runtime = patronus_runtime()?;
  let robots = robots(&runtime, &service, &space, &context.prefix)?;

  let stack = StackRobots::read(context, synced, robots)?;
  let ancestors_of = |name: &str| stack.ancestors_of(name);
  // only robots that hold: a canceling or a dry-run one lands nothing
  let holding_of = |name: &str| stack.robot_of(name).filter(|robot| is_holding(robot));
  let order = &stack.order;

  let mut holds = HashMap::new();
  for name in order {
    // a Safe Push of a temporary ref (the IDE, the CLI) is not on the branch: a push leaves it as it is
    let own = holding_of(name).filter(|robot| robot.branch_name.as_deref() == Some(name.as_str()));
    let hold = match own {
      Some(robot) => Some(PushHold {
        branch: name.clone(),
        robot: robot.clone(),
      }),
      None => find_landing_descendant(name, order.iter().map(String::as_str), ancestors_of, holding_of).map(|landing| PushHold {
        branch: landing.branch,
        robot: landing.robot.clone(),
      }),
    };
    if let Some(hold) = hold {
      holds.insert(name.clone(), hold);
    }
  }
  Ok(PushHolds {
    holds,
    service: Some((service, runtime, context.prefix.clone())),
  })
}

/// The stack of the synced branches and the user's robots, each matched to its branch as the app does
/// (`find_branch_robot`: started on the branch, or of a temporary ref at its local or pushed tip)
pub(crate) struct StackRobots {
  /// Parents first: the stack order with stacked branches, else the sync order
  pub order: Vec<String>,
  /// The stack as the sync places it (`stack_ancestors`)
  grouping: BranchGrouping,
  /// The local tips by full branch name
  tips: HashMap<String, String>,
  /// The remote heads the sync compared with, by branch
  remote_tips: HashMap<String, String>,
  robots: Vec<PatronusRobot>,
  prefix: String,
}

impl StackRobots {
  pub(crate) fn read(context: &RepoContext, synced: &SyncOutput, robots: Vec<PatronusRobot>) -> Result<Self, CliError> {
    let grouping = read_grouping(context, None)?;
    let order = grouping
      .forest
      .as_ref()
      .map_or_else(|| synced.branches.iter().map(|branch| branch.name.clone()).collect(), |forest| forest.order.clone());
    let remote_tips = synced
      .branches
      .iter()
      .filter_map(|branch| branch.remote.as_ref().and_then(|remote| remote.head_id.clone()).map(|head| (branch.name.clone(), head)))
      .collect();
    Ok(Self {
      order,
      grouping,
      tips: branch_tips(&context.git, &context.repository, &context.prefix)?,
      remote_tips,
      robots,
      prefix: context.prefix.clone(),
    })
  }

  /// The branches `name` is built on, root first
  pub(crate) fn ancestors_of(&self, name: &str) -> Vec<String> {
    stack_ancestors(&self.grouping, name)
  }

  /// The local tip of the virtual branch
  pub(crate) fn tip_of(&self, name: &str) -> Option<String> {
    to_final_branch_name(&self.prefix, name).ok().and_then(|full_name| self.tips.get(&full_name).cloned())
  }

  pub(crate) fn robot_of(&self, name: &str) -> Option<&PatronusRobot> {
    find_branch_robot(&self.robots, name, self.tip_of(name).as_deref(), self.remote_tips.get(name).map(String::as_str))
  }
}
