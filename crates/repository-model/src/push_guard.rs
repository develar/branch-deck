//! The facts the run of a push and of a robot action is checked on (`guard_facts`): the table of the last sync, the
//! robots Rust read itself (never the page's: a robot the page canceled is canceling and holds nothing) and the Patronus
//! view of both, on which the plan decides again what holds the push (`operations::push`, the holds of
//! `domain_policy::holds` decided into the view by `derive::decide_landing`).
//!
//! The words of the holds are `bd`'s too (`bd push`, `bd safe-push`, the refusals of the engine): `parked_words`,
//! `landed_words`, `every_branch_landed`, `own_hold_words`, `span_hold_words`, `stack_hold_words`.

use crate::branch_table::BranchTable;
use crate::facts::{PatronusOf, TableFacts, derive_patronus};
use domain_types::landing::Subject;
use domain_types::patronus::PatronusRobot;
use domain_types::repository_view::{Carried, PatronusView};
use domain_types::session::Parked;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::BTreeMap;
use std::sync::Arc;
use sync_core::patronus_start::{carried_ids, carried_words, join_names, push_mode_name};
use tracing::instrument;

/// The facts the checks read (`guard_facts`): the table of the last sync, the robots Rust read, and the Patronus view of
/// both
pub struct GuardFacts {
  synced: Arc<BranchTable>,
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
    failing: &[],
  };
  let view = derive_patronus(git_executor, repository_path, &of, &mut table.memo());
  GuardFacts { synced, robots, view }
}

impl GuardFacts {
  pub fn table(&self) -> &BranchTable {
    &self.synced
  }

  pub fn robot(&self, id: &str) -> Option<&PatronusRobot> {
    self.robots.iter().find(|robot| robot.id == id)
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
