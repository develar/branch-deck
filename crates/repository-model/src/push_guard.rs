//! The checks of the app's push (`push_branch`, `push_branches` and the E2E server's routes of them, which run them in
//! `operations::push`) in Rust, by the rules of `bd push` over the Patronus view (`derive_patronus`): under the
//! repository lock, a branch the last sync has no row of is no branch to push, a parked branch and a branch built on one
//! stay local, a branch a merged Safe Push landed whole has nothing left to push, a branch a running Safe Push holds (its
//! own robot, one of a branch built on it, the whole stack on its stack branch) is not pushed (the push won't land, and a
//! push into the robot's own branch fails it at the end), and the stack branch of a whole stack that is not `built` is no
//! build of its commits. An unapplied branch is checked by nothing. The page checks the same before it pushes and says it
//! in its words; these refuse a push that raced with a start (the IDE, `bd`) or came by IPC. The robots are the ones Rust
//! read itself, never the page's: a robot the page canceled before Push and Restart is canceling and holds nothing.
//!
//! The words of the holds are `bd`'s too (`bd push`, `bd safe-push`): `parked_words`, `landed_words`,
//! `every_branch_landed`, `own_hold_words`, `span_hold_words`, `stack_hold_words`; the holds themselves are the rules'
//! (`domain_policy::holds`, decided into the view by `derive::decide_landing`).

use crate::branch_table::{BranchTable, RemoteFacts};
use crate::derive::LandingFacts;
use crate::facts::{PatronusOf, TableFacts, derive_patronus};
use crate::start_guard::{stack_conflict_words, stack_not_built_words};
use domain_policy::parked::parked_state;
use domain_types::branch_names::to_final_branch_name;
use domain_types::landing::Subject;
use domain_types::patronus::PatronusRobot;
use domain_types::reason::Reason;
use domain_types::repository_view::{Carried, HoldKind, HoldView, PatronusView, RobotOf};
use domain_types::session::Parked;
use domain_types::stack::StackBranchState;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::BTreeMap;
use std::sync::Arc;
use sync_core::patronus_start::{carried_ids, carried_words, join_names, push_mode_name};
use sync_core::push::StackPushBranch;
use tracing::instrument;

/// What the app's own robot hold offers instead of a push: its Push and Restart
const APP_OWN_HOLD_REMEDY: &str = "restart it on the current branch";

/// The facts the checks read (`guard_facts`): the table of the last sync, the robots Rust read, and the Patronus view of
/// both
pub struct GuardFacts {
  synced: Arc<BranchTable>,
  remote: RemoteFacts,
  pub robots: Vec<PatronusRobot>,
  pub view: PatronusView,
}

/// The facts the checks of `repository_path` read: `table`, the table of the last sync of its session with the memo of
/// its views, `robots` as Rust read them, and the Patronus view of both on the remote branches as the session knows them
/// (`derive_patronus`, the rule of the session's own view). Runs git for the robots the hashes and the memo don't
/// answer: blocking
#[instrument(skip_all, fields(repository_path = %repository_path, robots = robots.len()))]
pub fn guard_facts(git_executor: &GitCommandExecutor, repository_path: &str, table: &TableFacts, robots: Vec<PatronusRobot>) -> GuardFacts {
  let synced = Arc::clone(&table.table);
  // the restarts of the view are not the checks': no robot is known to need approvals
  let of = PatronusOf {
    table: &synced,
    remote: &table.remote,
    robots: &robots,
    needs_review: &BTreeMap::new(),
  };
  let view = derive_patronus(git_executor, repository_path, &of, &mut table.memo());
  GuardFacts {
    synced,
    remote: table.remote.clone(),
    robots,
    view,
  }
}

impl GuardFacts {
  pub fn table(&self) -> &BranchTable {
    &self.synced
  }

  pub fn robot(&self, id: &str) -> Option<&PatronusRobot> {
    self.robots.iter().find(|robot| robot.id == id)
  }

  /// What the rules read of these facts, for an intent of the caller's own (`derive::LandingFacts`)
  pub(crate) fn landing(&self) -> LandingFacts<'_> {
    LandingFacts::new(&self.synced, &self.remote, &self.view, &self.robots, None)
  }

  /// The hold of the push of `name` (`domain_policy::holds::branch_hold`)
  pub(crate) fn hold(&self, name: &str) -> Option<&HoldView> {
    self.view.branches.get(name)?.landing.hold.as_ref()
  }

  /// The merged robot that landed all of `name` (its hold is `Landed`), with what it ran on
  pub(crate) fn landed_whole(&self, name: &str) -> Option<(Subject, &PatronusRobot)> {
    let hold = self.hold(name).filter(|hold| hold.kind == HoldKind::Landed)?;
    Some((hold.by.subject(), self.robot(&hold.by.robot_id)?))
  }

  /// The hold of the push of the stack branch of `root` (`domain_policy::holds::stack_branch_hold`)
  pub(crate) fn stack_hold(&self, root: &str) -> Option<&Reason> {
    self.view.stacks.get(root)?.landing.hold.as_ref()
  }

  /// "every branch of the whole auth stack landed in master with the Safe Push of oauth": the robots of a
  /// `StackLanded` reason (`every_branch_landed`)
  pub(crate) fn stack_landed_words(&self, root: &str, by: &[RobotOf]) -> String {
    let landed: Vec<(Subject, &PatronusRobot)> = by.iter().filter_map(|of| Some((of.subject(), self.robot(&of.robot_id)?))).collect();
    every_branch_landed(&Subject::whole_stack(root).words(), &landed)
  }
}

/// "auth is parked: waits for IJPL-1", "oauth is built on parked auth": why `name` stays local
#[must_use]
pub fn parked_words(name: &str, parked: &Parked) -> String {
  let note = if parked.note.is_empty() { String::new() } else { format!(": {}", parked.note) };
  match &parked.with {
    None => format!("{name} is parked{note}"),
    Some(parent) => format!("{name} is built on parked {parent}{note}"),
  }
}

/// Why the push of `branches` is refused: "Nothing was pushed: …" with every refusal, `bd push`'s words; `Ok` when
/// nothing refuses. An unapplied branch is no branch of the table
pub fn check_push(facts: &GuardFacts, branches: &[StackPushBranch]) -> Result<(), String> {
  let refusals: Vec<String> = branches
    .iter()
    .filter(|branch| !branch.unapplied)
    .filter_map(|branch| {
      if branch.whole_stack {
        stack_refusal(facts, &branch.branch_name)
      } else {
        branch_refusal(facts, &branch.branch_name)
      }
    })
    .collect();
  if refusals.is_empty() {
    Ok(())
  } else {
    Err(format!("Nothing was pushed: {}", refusals.join("; ")))
  }
}

/// The refusals the push and the start of a branch share: no row of the last sync, parked or built on parked, landed
/// whole ("nothing of it is left to `left`": "push", "land")
pub(crate) fn row_refusal(facts: &GuardFacts, name: &str, left: &str) -> Option<String> {
  let table = facts.table();
  if table.branch(name).is_none() {
    return Some(format!("{name} is no branch of the last sync"));
  }
  if let Some(parked) = parked_state(&table.repository(), name) {
    return Some(format!("{}; it stays local until the user unparks it", parked_words(name, &parked)));
  }
  let (by, robot) = facts.landed_whole(name)?;
  Some(format!("{}: nothing of it is left to {left}", landed_words(name, &by, robot)))
}

/// No row of the last sync, else parked or built on parked, else landed whole (`row_refusal`), else held by a running
/// Safe Push
fn branch_refusal(facts: &GuardFacts, name: &str) -> Option<String> {
  if let Some(refusal) = row_refusal(facts, name, "push") {
    return Some(refusal);
  }
  let table = facts.table();
  let hold = facts.hold(name)?;
  let robot = facts.robot(&hold.by.robot_id)?;
  Some(match hold.kind {
    HoldKind::Landed => return None,
    HoldKind::Own => {
      let source = to_final_branch_name(&table.prefix, name).ok();
      own_hold_words(name, robot, source.as_deref(), APP_OWN_HOLD_REMEDY)
    }
    HoldKind::Span => span_hold_words(name, &hold.by.subject(), robot, hold.carried.as_ref()),
  })
}

/// The stack branch of the whole stack of `root`: it holds the commits of every member, so a parked member keeps it
/// local; every member landed whole leaves nothing to push; the robot of the whole stack runs on it; and one that is not
/// `built` is no build of its commits (`bd push` pushes only a built one), as the start of the whole stack refuses it
/// (`domain_policy::holds::stack_branch_hold`)
fn stack_refusal(facts: &GuardFacts, root: &str) -> Option<String> {
  let table = facts.table();
  let Some(stack) = table.stack(root) else {
    return Some(format!("{} has no stack branch of the last sync", Subject::whole_stack(root).words()));
  };
  let branch = &stack.branch_name;
  Some(match facts.stack_hold(root)? {
    Reason::StackParked { member, .. } => format!("{branch} holds parked {member}; land the stack in pieces"),
    Reason::StackLanded { by, .. } => format!("{branch}: {}: nothing of it is left to push", facts.stack_landed_words(root, by)),
    Reason::StackHeld { robot_id, .. } => format!("{branch}: {}", stack_hold_words(root, facts.robot(robot_id)?)),
    Reason::StackNotBuilt {
      state: StackBranchState::Conflict,
      ..
    } => format!("{branch}: {}", stack_conflict_words(&table.branches, stack, &|id| git_ops::revs::short_id(id).to_string())),
    _ => format!("{branch}: {}", stack_not_built_words(stack)),
  })
}

/// "auth landed in master with the Safe Push of oauth (<url>)", of its own robot "auth merged into master with its Safe
/// Push (<url>)": what a merged robot `by` ran on landed of `name`
pub fn landed_words(name: &str, by: &Subject, robot: &PatronusRobot) -> String {
  let target = &robot.target_branch;
  if by.is_branch(name) {
    format!("{name} merged into {target} with its Safe Push ({})", robot.url)
  } else {
    format!("{name} landed in {target} with the Safe Push of {} ({})", by.words(), robot.url)
  }
}

/// "every branch of the whole auth stack landed in master with the Safe Pushes of login and session": what merged Safe
/// Pushes landed of a whole stack every branch of which they landed whole (`landed`: what each robot ran on, one per
/// branch), each subject named once in the order they merged, as the app names them (`formatLandedWholeReason`); the
/// target of the first
pub fn every_branch_landed(what: &str, landed: &[(Subject, &PatronusRobot)]) -> String {
  let mut merged: Vec<&(Subject, &PatronusRobot)> = landed.iter().collect();
  merged.sort_by_key(|(_, robot)| robot.finish_time.unwrap_or(0));
  let mut by: Vec<String> = Vec::new();
  for subject in merged.iter().map(|(subject, _)| subject.words()) {
    if !by.contains(&subject) {
      by.push(subject);
    }
  }
  let pushes = if by.len() == 1 { "Safe Push" } else { "Safe Pushes" };
  let target = merged.first().map_or("the target", |(_, robot)| robot.target_branch.as_str());
  format!("every branch of {what} landed in {target} with the {pushes} of {}", join_names(&by))
}

/// "A Safe Push of auth is running on test/virtual/auth: a push moves the branch it took its commit from, …. Wait until
/// it merges, or <remedy> (<url>)": the robot runs on `name` itself; `source` is the branch it took its commit from
pub fn own_hold_words(name: &str, robot: &PatronusRobot, source: Option<&str>, remedy: &str) -> String {
  let mode = push_mode_name(robot.push_mode);
  let source = source.map_or_else(String::new, |branch| format!(" on {branch}"));
  format!(
    "A {mode} of {name} is running{source}: a push moves the branch it took its commit from, so it won't land and fails the {mode} at the end. Wait until it merges, or {remedy} ({})",
    robot.url
  )
}

/// "A Safe Push of oauth is running: it lands 2 commits of auth with it (…), a push won't land (<url>)": the robot of
/// `by`, a branch built on `name` or its whole stack, lands `carried` of it
pub fn span_hold_words(name: &str, by: &Subject, robot: &PatronusRobot, carried: Option<&Carried>) -> String {
  format!(
    "A {} of {} is running: it lands {} with it{}, a push won't land ({})",
    push_mode_name(robot.push_mode),
    by.words(),
    carried_words(name, carried),
    carried_ids(carried),
    robot.url
  )
}

/// "a Safe Push of the whole auth stack is running on it: …": why the stack branch of `root` is not pushed while the
/// robot of the whole stack runs on it, as the app says it (`formatStackPushHoldReason`)
pub fn stack_hold_words(root: &str, robot: &PatronusRobot) -> String {
  let mode = push_mode_name(robot.push_mode);
  format!(
    "a {mode} of {} is running on it: a push moves the branch it took its commit from and fails the {mode} at the end; wait until it merges ({})",
    Subject::WholeStack { root: root.to_string() }.words(),
    robot.url
  )
}

#[cfg(test)]
#[path = "push_guard_test.rs"]
pub(crate) mod push_guard_test;
