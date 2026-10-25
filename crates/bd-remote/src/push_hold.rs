//! What holds the push of a virtual branch and of a stack branch in `bd push`, as the rules decide it for the app too
//! (`domain_policy::holds`, through `repository_model::derive::LandingFacts`): a Safe Push that runs on the branch, or
//! one of a branch built on it or of the whole stack it is a branch of whose commit has commits of it. A push now won't
//! land (the robot merges the commit it took), and a push into the robot's own branch makes it fail at the end ("branch
//! was modified"). Before any of them, a merged Safe Push that landed all of the branch (`landed`): its commits are in
//! the target, a push sends nothing that lands. A stack branch is held by a parked member, by merged Safe Pushes that
//! landed every member, by its own robot and by a build the sync did not make (`stack_hold`). Best effort: a repository
//! not on Space, no Space token or no answer of Patronus holds nothing by a robot.

use crate::landed::{LandedBranch, landed_branches};
use crate::patronus_facts::{PatronusFacts, table_of};
use crate::robot_list::list_robots;
use crate::safe_push_command::patronus_service;
use bd_core::bd_command::shell_word;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::sync_command::SyncOutput;
use domain_types::branch_names::to_final_branch_name;
use domain_types::error::ErrorCode;
use domain_types::landing::Subject;
use domain_types::patronus::PatronusRobot;
use domain_types::reason::Reason;
use domain_types::repository_view::{Carried, HoldKind};
use domain_types::stack::StackBranchUpdate;
use patronus::service::PatronusService;
use repository_model::branch_table::BranchTable;
use repository_model::push_guard::{every_branch_landed, own_hold_words, span_hold_words, stack_hold_words};
use repository_model::start_guard::start_refusal_text;
use std::collections::HashMap;
use sync_core::patronus_start::push_mode_name;

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
    if self.is_own(name) {
      own_hold_words(
        name,
        &self.robot,
        self.source_branch.as_deref(),
        "once the user approved, --restart-running cancels it, pushes and restarts it",
      )
    } else {
      span_hold_words(name, &self.by, &self.robot, self.carried.as_ref())
    }
  }
}

/// `bd robot <branch>`, or `bd robot <root> --whole-stack`: what follows the robot that runs on `subject`, a read
pub(crate) fn robot_command(context: &RepoContext, subject: &Subject) -> String {
  match subject {
    Subject::Branch { name } => format!("{} robot {}", context.bd, shell_word(name)),
    Subject::WholeStack { root } => format!("{} robot {} --whole-stack", context.bd, shell_word(root)),
  }
}

/// What the robot that refuses the start of `own` runs on: `own` itself, the branch built on it or the whole stack that
/// lands it with more, or what lands a part of it
pub(crate) fn refusing_subject(refusal: &Reason, own: &Subject) -> Subject {
  match refusal {
    Reason::Descendant { by, .. } => by.subject(),
    Reason::Ancestor { landing, .. } => landing.subject(),
    _ => own.clone(),
  }
}

/// The holds of the branches, the table and the Patronus facts the holds of the stack branches are decided over, and
/// the Patronus service that cancels and restarts their robots; none of a push that checked them already
#[derive(Default)]
pub struct PushHolds {
  holds: HashMap<String, PushHold>,
  table: BranchTable,
  facts: PatronusFacts,
  service: Option<PatronusService>,
}

impl PushHolds {
  pub fn get(&self, name: &str) -> Option<&PushHold> {
    self.holds.get(name)
  }

  /// What holds the push of the stack branch of `stack` (`domain_policy::holds::stack_branch_hold`)
  pub fn stack_hold(&self, stack: &StackBranchUpdate) -> Option<Reason> {
    self.facts.landing(&self.table).stack_hold(stack)
  }

  /// Why the stack branch of `root` held by `hold` is not pushed, as `bd` says it: "it holds parked login; …", "every
  /// branch of the whole auth stack landed in master with …: nothing of it is left to push", "a Safe Push of the whole
  /// auth stack is running on it: …" (`stack_hold_words`)
  pub fn stack_hold_words(&self, root: &str, hold: &Reason) -> String {
    let what = Subject::whole_stack(root).words();
    match hold {
      Reason::StackParked { member, .. } => format!("it holds parked {member}; land the stack in pieces"),
      Reason::StackLanded { by, .. } => {
        let landed: Vec<(Subject, &PatronusRobot)> = by.iter().filter_map(|of| Some((of.subject(), self.facts.robot(&of.robot_id)?))).collect();
        format!("{}: nothing of it is left to push", every_branch_landed(&what, &landed))
      }
      Reason::StackHeld { robot_id, mode, .. } => match self.facts.robot(robot_id) {
        Some(robot) => stack_hold_words(root, robot),
        None => format!("a {} of {what} is running on it", push_mode_name(*mode)),
      },
      other => start_refusal_text(other, &what, &self.facts.robots),
    }
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
  let table = table_of(context, synced);
  match find_holds(context, environment, &table) {
    Ok(Some((facts, service))) => push_holds(context, table, facts, Some(service)),
    Ok(None) => push_holds(context, table, PatronusFacts::default(), None),
    Err(error) => {
      tracing::debug!(error = %error.message, "no push holds of robots: Patronus is not available");
      push_holds(context, table, PatronusFacts::default(), None)
    }
  }
}

fn find_holds(context: &RepoContext, environment: &Environment, table: &BranchTable) -> Result<Option<(PatronusFacts, PatronusService)>, CliError> {
  let Some(space) = context.space_repository()? else {
    return Ok(None);
  };
  let service = patronus_service(environment)?;
  let robots = list_robots(context, &service, &space)?;
  Ok(Some((PatronusFacts::read(context, table, robots), service)))
}

/// The holds of the view's branches, over `table` and `facts`
pub(crate) fn push_holds(context: &RepoContext, table: BranchTable, facts: PatronusFacts, service: Option<PatronusService>) -> PushHolds {
  let mut landed = landed_branches(&facts);
  let holds = facts
    .view
    .branches
    .iter()
    .filter_map(|(name, branch)| {
      let hold = branch.landing.hold.as_ref()?;
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
  PushHolds { holds, table, facts, service }
}
