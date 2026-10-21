//! What holds the push of a virtual branch in `bd push`, as the app decides it (`PushHold`, `src/lib/patronus.ts`): a
//! Safe Push that runs on the branch, or one of a branch built on it or of the whole stack it is a branch of whose
//! commit has commits of it (`robot_spans`: the content tells after a restack or an amend below the robot's commit; the
//! stack as it is now when the repository lacks the robot's commit). A push now won't land (the robot merges the commit
//! it took), and a push into the robot's own branch makes it fail at the end ("branch was modified"). Before any of
//! them, a merged Safe Push that landed all of the branch (`landed`): its commits are in the target, a push sends
//! nothing that lands. Best effort: a repository not on Space, no Space token or no answer of Patronus holds nothing.

use crate::landed::{LandedBranch, LandingTable, find_landed, robot_request};
use crate::robot_list::list_robots;
use crate::safe_push_command::patronus_service;
use bd_core::branch_view::stack_ancestors;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::{CliError, ErrorCode};
use bd_core::sync_command::SyncOutput;
use git_ops::branch_names::to_final_branch_name;
use patronus::service::PatronusService;
use std::collections::HashMap;
use sync_core::patronus_start::{
  CarriedCommits, LandingRobot, RobotSource, carried_ids, carried_words, find_branch_robot, find_landing_descendant, find_landing_stack, find_stack_robot, is_pushing_robot,
  is_robot_active, push_mode_name,
};
use sync_core::robot_landing::robot_spans_of_grouped;
use sync_core::sync::SyncReport;
use sync_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use sync_types::robot_landing::{BranchSpan, RobotLandingRequest};
use sync_types::stack::StackBranchUpdate;

/// The robot that holds the push of a branch
#[derive(Debug, Clone)]
pub struct PushHold {
  /// The branch of the robot: the held branch itself, a branch built on it, or the root of the whole stack
  pub branch: String,
  pub robot: PatronusRobot,
  /// The robot of the whole stack of the root `branch`, on its stack branch: it lands every branch of the stack
  pub whole_stack: bool,
  /// The robot merged and landed all of the branch: refused as `landed`, never restarted
  pub landed: Option<LandedBranch>,
  /// The commits of the held branch a robot of another branch lands, as its content told; None for its own robot, or
  /// when only the stack as it is now tells
  pub carried: Option<CarriedCommits>,
  /// The branch the robot took its commit from, which a push of the held branch moves: its own robot's virtual branch
  pub source_branch: Option<String>,
}

impl PushHold {
  /// The robot runs on the held branch itself: `--restart-running` cancels it, pushes and restarts it
  pub fn is_own(&self, name: &str) -> bool {
    self.branch == name && !self.whole_stack && self.landed.is_none()
  }

  /// "A Safe Push of auth is running: …", in the words of the app
  pub fn reason(&self, name: &str) -> String {
    if let Some(landed) = &self.landed {
      return format!("{}: nothing of it is left to push", landed.reason(name));
    }
    let mode = push_mode_name(self.robot.push_mode);
    if self.is_own(name) {
      let source = self.source_branch.as_deref().map_or_else(String::new, |branch| format!(" on {branch}"));
      format!(
        "A {mode} of {name} is running{source}: a push moves the branch it took its commit from, so it won't land and fails the {mode} at the end. Wait until it merges, or pass --restart-running to cancel it, push and restart it ({})",
        self.robot.url
      )
    } else {
      let of = if self.whole_stack {
        format!("the whole {} stack", self.branch)
      } else {
        self.branch.clone()
      };
      let carried = self.carried.as_ref();
      format!(
        "A {mode} of {of} is running: it lands {} with it{}, a push won't land ({})",
        carried_words(name, carried),
        carried_ids(carried),
        self.robot.url
      )
    }
  }
}

/// The holds of the branches, the running robots of the whole stacks, and the Patronus service that cancels and
/// restarts their robots
#[derive(Default)]
pub struct PushHolds {
  holds: HashMap<String, PushHold>,
  /// By root, the robot of the whole stack that runs on its stack branch: a push of the stack branch moves the branch it
  /// took its commit from
  stack_robots: HashMap<String, PatronusRobot>,
  service: Option<PatronusService>,
}

impl PushHolds {
  pub fn get(&self, name: &str) -> Option<&PushHold> {
    self.holds.get(name)
  }

  /// Why the stack branch of `root` is not pushed while the Safe Push of the whole stack runs on it, as the app says it
  /// (`formatStackPushHoldReason`): a push moves the branch the robot took its commit from
  pub fn stack_hold_reason(&self, root: &str) -> Option<String> {
    let robot = self.stack_robots.get(root)?;
    let mode = push_mode_name(robot.push_mode);
    Some(format!(
      "a {mode} of the whole {root} stack is running on it: a push moves the branch it took its commit from and fails the {mode} at the end; wait until it merges ({})",
      robot.url
    ))
  }

  /// Cancels the robot before the push that restarts it
  pub fn cancel(&self, context: &RepoContext, robot: &PatronusRobot) -> Result<(), CliError> {
    context
      .runtime()?
      .block_on(self.service()?.cancel(&context.prefix, &robot.id))
      .map_err(|message| CliError::new(ErrorCode::PatronusError, message))?;
    Ok(())
  }

  /// Starts the canceled robot again, on the pushed commit
  pub fn restart(&self, context: &RepoContext, robot: &PatronusRobot) -> Result<PatronusRobot, CliError> {
    context
      .runtime()?
      .block_on(self.service()?.restart(&context.prefix, &robot.id))
      .map_err(|message| CliError::new(ErrorCode::PatronusError, message))
  }

  fn service(&self) -> Result<&PatronusService, CliError> {
    self.service.as_ref().ok_or_else(|| CliError::new(ErrorCode::PatronusError, "Patronus is not available"))
  }
}

/// A robot that holds the pushes of the branches it lands: running (a canceling one won't merge) and pushing into the
/// target (a dry run lands nothing)
fn is_holding(robot: &PatronusRobot) -> bool {
  robot.status == PatronusRobotStatus::Running && robot.push_mode != PatronusPushMode::DryRun
}

/// The holds of every synced branch: a merged Safe Push that landed all of it (`find_landed`), else its own robot started
/// on it, else the robot of the first branch built on it whose commit has commits of it, else the robot of its whole
/// stack whose commit has. One robots request; nothing without Space, a token or an answer of Patronus.
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
  let Some(space) = context.space_repository()? else {
    return Ok(PushHolds::default());
  };
  let service = patronus_service(environment)?;
  let robots = list_robots(context, &service, &space)?;
  let report = &synced.report;
  let table = LandingTable {
    branches: &report.branches,
    stacks: &report.stack_branches,
    tips: &report.tips,
    baseline: &synced.baseline,
  };
  let landed = find_landed(context, &robots, &table)?;

  let stack = StackRobots::read(context, synced, robots, &table);
  let ancestors_of = |name: &str| stack.landed_ancestors_of(name);
  // only robots that hold: a canceling or a dry-run one lands nothing
  let holding_of = |name: &str| stack.robot_of(name).filter(|robot| is_holding(robot));
  let order = &stack.order;

  let stack_holding_of = |root: &str| stack.stack_robot_of(root).filter(|robot| is_holding(robot));

  let mut holds = HashMap::new();
  for name in order {
    // its commits are in the target for good: nothing a push sends lands
    if let Some(landed) = landed.get(name).filter(|landed| landed.is_whole()) {
      holds.insert(
        name.clone(),
        PushHold {
          branch: landed.by.clone(),
          robot: landed.robot.clone(),
          whole_stack: landed.whole_stack,
          landed: Some(landed.clone()),
          carried: None,
          source_branch: None,
        },
      );
      continue;
    }
    // a Safe Push of a temporary ref (the IDE, the CLI) is not on the branch: a push leaves it as it is
    let own = holding_of(name).filter(|robot| robot.branch_name.as_deref() == Some(name.as_str()));
    let hold = match own {
      Some(robot) => Some(PushHold {
        branch: name.clone(),
        robot: robot.clone(),
        whole_stack: false,
        landed: None,
        carried: None,
        source_branch: to_final_branch_name(&context.prefix, name).ok(),
      }),
      None => stack.find_landing(name, ancestors_of, holding_of, stack_holding_of).map(|landing| PushHold {
        branch: landing.branch,
        robot: landing.robot.clone(),
        whole_stack: landing.whole_stack,
        landed: None,
        carried: landing.carried,
        source_branch: None,
      }),
    };
    if let Some(hold) = hold {
      holds.insert(name.clone(), hold);
    }
  }
  // a robot of a temporary ref at the stack branch's tip is not on it: a push leaves it as it is
  let stack_robots = report
    .stack_branches
    .iter()
    .filter_map(|update| {
      let robot = stack_holding_of(&update.root).filter(|robot| robot.stack_root.as_deref() == Some(update.root.as_str()))?;
      Some((update.root.clone(), robot.clone()))
    })
    .collect();
  Ok(PushHolds {
    holds,
    stack_robots,
    service: Some(service),
  })
}

/// The stack of the synced branches and the user's robots, each matched to its branch as the app does
/// (`find_branch_robot`: started on the branch, or of a temporary ref at its local or pushed tip), and what the active
/// ones land of the branches as they are now (`read`, `read_robot_spans`). All from the sync's report and the
/// spans read before: no git process
pub(crate) struct StackRobots<'a> {
  /// Parents first: the stack order with stacked branches, else the sync order
  pub order: Vec<String>,
  /// The stack as the sync placed it, the tips it left and the remote tips it compared with
  report: &'a SyncReport,
  robots: Vec<PatronusRobot>,
  prefix: &'a str,
  /// By robot id, the branches its commit lands (`robot_spans`); None for a robot whose commit the repository can't
  /// tell of. A robot not here (no spans read) lands what the stack as it is now says
  spans: HashMap<String, Option<Vec<BranchSpan>>>,
}

impl<'a> StackRobots<'a> {
  pub(crate) fn new(context: &'a RepoContext, synced: &'a SyncOutput, robots: Vec<PatronusRobot>) -> Self {
    Self {
      order: synced.order().into_iter().map(str::to_string).collect(),
      report: &synced.report,
      robots,
      prefix: &context.prefix,
      spans: HashMap::new(),
    }
  }

  /// The stack and the robots with what the active ones land (`read_robot_spans`)
  pub(crate) fn read(context: &'a RepoContext, synced: &'a SyncOutput, robots: Vec<PatronusRobot>, table: &LandingTable<'_>) -> Self {
    let mut stack = Self::new(context, synced, robots);
    stack.spans = read_robot_spans(context, &stack, table);
    stack
  }

  /// The branches `name` is built on, root first
  pub(crate) fn ancestors_of(&self, name: &str) -> Vec<String> {
    stack_ancestors(&self.report.branches, name)
  }

  /// The branches the robot of `name` lands with it, root first: those its commit has commits of, as the content told
  /// (a parent a restack placed below `name` after the start is not one); else the branches `name` is built on now
  pub(crate) fn landed_ancestors_of(&self, name: &str) -> Vec<String> {
    match self.robot_of(name).and_then(|robot| self.span_of(robot)) {
      Some(branches) => branches.iter().filter(|span| span.branch != name).map(|span| span.branch.clone()).collect(),
      None => self.ancestors_of(name),
    }
  }

  /// The robot of a branch built on `name` (`find_landing_descendant` over `landed_ancestors_of`), else of the whole
  /// stack `name` is a branch of, whose commit has commits of `name`; with those commits, as the content told
  pub(crate) fn find_landing<'r>(
    &'r self,
    name: &str,
    ancestors_of: impl Fn(&str) -> Vec<String>,
    robot_of: impl Fn(&str) -> Option<&'r PatronusRobot>,
    stack_robot_of: impl Fn(&str) -> Option<&'r PatronusRobot>,
  ) -> Option<LandingRobot<'r>> {
    let landing = find_landing_descendant(name, self.order.iter().map(String::as_str), ancestors_of, robot_of).or_else(|| {
      find_landing_stack(self.stack_root_of(name), stack_robot_of)
        .filter(|landing| self.span_of(landing.robot).is_none_or(|branches| branches.iter().any(|span| span.branch == name)))
    })?;
    let carried = self
      .span_of(landing.robot)
      .and_then(|branches| branches.iter().find(|span| span.branch == name))
      .map(|span| CarriedCommits {
        commits: span.commits.clone(),
        amended: span.amended.clone(),
      });
    Some(LandingRobot { carried, ..landing })
  }

  /// The branches the robot's commit lands, as the content told; None when only the stack as it is now tells
  fn span_of(&self, robot: &PatronusRobot) -> Option<&[BranchSpan]> {
    self.spans.get(&robot.id).and_then(Option::as_deref)
  }

  /// The local tip of the virtual branch
  pub(crate) fn tip_of(&self, name: &str) -> Option<String> {
    self.tip_in(&self.report.tips, name)
  }

  pub(crate) fn robot_of(&self, name: &str) -> Option<&PatronusRobot> {
    let remote_tip = self.tip_in(&self.report.remote_tips, name);
    find_branch_robot(&self.robots, name, self.tip_of(name).as_deref(), remote_tip.as_deref())
  }

  /// The stack branch of the stack that forks `name` is a branch of, as the sync reported it
  pub(crate) fn stack_branch_of(&self, name: &str) -> Option<&'a StackBranchUpdate> {
    self.report.stack_branches.iter().find(|stack| stack.members.iter().any(|member| member == name))
  }

  /// The root of the stack that forks `name` is a branch of
  pub(crate) fn stack_root_of(&self, name: &str) -> Option<&'a str> {
    self.stack_branch_of(name).map(|stack| stack.root.as_str())
  }

  /// The robot of the whole stack of `root`: on its stack branch, or of a temporary ref at its tip or remote tip
  pub(crate) fn stack_robot_of(&self, root: &str) -> Option<&PatronusRobot> {
    let stack = self.report.stack_branches.iter().find(|stack| stack.root == root);
    let local_tip = stack.and_then(|stack| stack.tip.as_deref());
    let remote_tip = stack.and_then(|stack| self.report.remote_tips.get(&stack.branch_name)).map(String::as_str);
    find_stack_robot(&self.robots, root, local_tip, remote_tip)
  }

  fn tip_in(&self, tips: &HashMap<String, String>, name: &str) -> Option<String> {
    to_final_branch_name(self.prefix, name).ok().and_then(|full_name| tips.get(&full_name).cloned())
  }
}

/// What the active robots that push into the target land of the branches as they are now (`robot_spans_of_grouped`, by
/// robot id): the robot of each branch with ancestors (also of a temporary ref, matched to the branch by its tip) and of
/// each whole stack. One call; a robot at the tip the sync left needs no git, the others their walks. Best effort: on a
/// git error every robot lands what the stack as it is now says
fn read_robot_spans(context: &RepoContext, stack: &StackRobots<'_>, table: &LandingTable<'_>) -> HashMap<String, Option<Vec<BranchSpan>>> {
  let lands = |robot: &&PatronusRobot| is_robot_active(robot) && is_pushing_robot(robot);
  let mut requests: Vec<RobotLandingRequest> = Vec::new();
  let mut ask = |request: Option<RobotLandingRequest>| {
    if let Some(request) = request
      && !requests.iter().any(|known| known.robot_id == request.robot_id)
    {
      requests.push(request);
    }
  };
  for name in &stack.order {
    if let Some(robot) = stack.robot_of(name).filter(lands)
      && !stack.ancestors_of(name).is_empty()
    {
      ask(robot_request(context, robot, RobotSource::Branch(name), table));
    }
  }
  for update in table.stacks {
    if let Some(robot) = stack.stack_robot_of(&update.root).filter(lands) {
      ask(robot_request(context, robot, RobotSource::WholeStack(&update.root), table));
    }
  }
  match robot_spans_of_grouped(&context.git, &context.repository, &context.prefix, table.baseline, &requests, table.branches) {
    Ok(spans) => spans.into_iter().map(|span| (span.robot_id, span.branches)).collect(),
    Err(error) => {
      tracing::debug!(error = %format!("{error:#}"), "no robot spans: the stack as it is now tells");
      HashMap::new()
    }
  }
}
