//! What holds the push of a virtual branch in `bd push`, as the repository view decides it for the app too (`hold` of
//! `repository_view::patronus_view`): a Safe Push that runs on the branch, or one of a branch built on it or of the
//! whole stack it is a branch of whose commit has commits of it. A push now won't land (the robot merges the commit
//! it took), and a push into the robot's own branch makes it fail at the end ("branch was modified"). Before any of
//! them, a merged Safe Push that landed all of the branch (`landed`): its commits are in the target, a push sends
//! nothing that lands. Best effort: a repository not on Space, no Space token or no answer of Patronus holds nothing.

use crate::landed::{LandedBranch, landed_branches};
use crate::patronus_facts::{PatronusFacts, remote_of, table_of};
use crate::robot_list::list_robots;
use crate::safe_push_command::patronus_service;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::sync_command::SyncOutput;
use domain_types::branch_names::to_final_branch_name;
use domain_types::error::ErrorCode;
use domain_types::landing::Subject;
use domain_types::patronus::PatronusRobot;
use domain_types::repository_view::{Carried, HoldKind};
use patronus::service::PatronusService;
use std::collections::HashMap;
use sync_core::patronus_start::{carried_ids, carried_words, push_mode_name};

/// The robot that holds the push of a branch
#[derive(Debug, Clone)]
pub struct PushHold {
  /// What the robot runs on: the held branch itself, a branch built on it, or the whole stack of a root (on its stack
  /// branch: it lands every branch of the stack)
  pub by: Subject,
  pub robot: PatronusRobot,
  /// The robot merged and landed all of the branch: refused as `landed`, never restarted
  pub landed: Option<LandedBranch>,
  /// The commits of the held branch a robot of another branch lands, as its content told; None for its own robot, or
  /// when only the stack as it is now tells
  pub carried: Option<Carried>,
  /// The branch the robot took its commit from, which a push of the held branch moves: its own robot's virtual branch
  pub source_branch: Option<String>,
}

impl PushHold {
  /// The robot runs on the held branch itself: `--restart-running` cancels it, pushes and restarts it
  pub fn is_own(&self, name: &str) -> bool {
    self.by.is_branch(name) && self.landed.is_none()
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
      let carried = self.carried.as_ref();
      format!(
        "A {mode} of {} is running: it lands {} with it{}, a push won't land ({})",
        self.by.words(),
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
      "a {mode} of {} is running on it: a push moves the branch it took its commit from and fails the {mode} at the end; wait until it merges ({})",
      Subject::WholeStack { root: root.to_string() }.words(),
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

/// The holds of every synced branch: a merged Safe Push that landed all of it (`landed_branches`), else its own robot started
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
  let facts = PatronusFacts::read(context, &table_of(context, synced), &remote_of(context, synced), robots);
  Ok(push_holds(context, &facts, Some(service)))
}

/// The holds of the view's branches and the robots of the whole stacks that run on their stack branches
pub(crate) fn push_holds(context: &RepoContext, facts: &PatronusFacts, service: Option<PatronusService>) -> PushHolds {
  let mut landed = landed_branches(facts);
  let holds = facts
    .view
    .branches
    .iter()
    .filter_map(|(name, branch)| {
      let hold = branch.hold.as_ref()?;
      let robot = facts.robot(&hold.by.robot_id)?.clone();
      Some((
        name.clone(),
        PushHold {
          by: hold.by.subject(),
          robot,
          landed: if hold.kind == HoldKind::Landed { landed.remove(name) } else { None },
          carried: hold.carried.clone(),
          source_branch: (hold.kind == HoldKind::Own).then(|| to_final_branch_name(&context.prefix, name).ok()).flatten(),
        },
      ))
    })
    .collect();
  let stack_robots = facts
    .view
    .stacks
    .iter()
    .filter_map(|(root, stack)| {
      let robot = facts.robot(stack.hold_robot_id.as_deref()?)?;
      Some((root.clone(), robot.clone()))
    })
    .collect();
  PushHolds { holds, stack_robots, service }
}
