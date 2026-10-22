//! The checks of the app's push (`push_branch`, `push_branches` and the E2E server's routes of them) in Rust, by the rules
//! of `bd push` over the repository view (`patronus_view`): under the repository lock, a branch the last sync has no row
//! of is no branch to push, a parked branch and a branch built on one stay local, a branch a merged Safe Push landed
//! whole has nothing left to push, a branch a running Safe Push holds (its own robot, one of a branch built on it, the
//! whole stack on its stack branch) is not pushed (the push won't land, and a push into the robot's own branch fails it
//! at the end), and the stack branch of a whole stack that is not `built` is no build of its commits. An unapplied
//! branch takes only the lock. The page checks the same before it pushes and says it in its words; these refuse a push
//! that raced with a start (the IDE, `bd`) or came by IPC. The robots are the ones Rust read itself, never the page's: a
//! robot the page canceled before Push and Restart is canceling and holds nothing.
//!
//! `RemoteActions::push_branches` (`actions`) is the whole sequence of both hosts: the user's robots (one full list asked
//! now, best effort), then, in a blocking step, the lock, the checks and the core push. The words of the holds are
//! `bd`'s too (`bd push`, `bd safe-push`):
//! `landed_words`, `every_branch_landed`, `own_hold_words`, `span_hold_words`, `stack_hold_words`.

use crate::actions::RemoteActions;
use crate::branch_table::BranchTable;
use crate::start_guard::{stack_conflict_words, stack_not_built_words};
use crate::views::{RepositoryViews, SyncedTable};
use anyhow::{Result, anyhow, bail};
use domain_types::branch_names::to_final_branch_name;
use domain_types::landing::Subject;
use domain_types::patronus::PatronusRobot;
use domain_types::repository_view::{Carried, HoldKind, PatronusView};
use domain_types::stack::StackBranchState;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, RepositoryLocked, lock_repository_in};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use sync_core::parked::parked_state;
use sync_core::patronus_start::{carried_ids, carried_words, join_names, push_mode_name};
use sync_core::push::{NOT_SYNCED_YET, PushBranchParams, PushBranchesOutcome, PushBranchesParams, PushOutcome, StackPushBranch, push_branches};
use sync_core::transfer_progress::TransferProgress;
use sync_utils::span::in_current_span;
use tracing::instrument;

/// What the app's own robot hold offers instead of a push: its Push and Restart
const APP_OWN_HOLD_REMEDY: &str = "restart it on the current branch";

/// The facts the checks read (`RepositoryViews::guard_facts`): the table of the last sync, the robots Rust read, and the
/// Patronus view of both
pub struct GuardFacts {
  synced: Arc<SyncedTable>,
  pub robots: Vec<PatronusRobot>,
  pub view: PatronusView,
}

impl GuardFacts {
  pub(crate) const fn new(synced: Arc<SyncedTable>, robots: Vec<PatronusRobot>, view: PatronusView) -> Self {
    Self { synced, robots, view }
  }

  pub fn table(&self) -> &BranchTable {
    &self.synced.table
  }

  pub fn robot(&self, id: &str) -> Option<&PatronusRobot> {
    self.robots.iter().find(|robot| robot.id == id)
  }

  /// The merged robot that landed all of `name` (its hold is `Landed`), with what it ran on
  pub(crate) fn landed_whole(&self, name: &str) -> Option<(Subject, &PatronusRobot)> {
    let hold = self.view.branches.get(name)?.hold.as_ref().filter(|hold| hold.kind == HoldKind::Landed)?;
    Some((hold.by.subject(), self.robot(&hold.by.robot_id)?))
  }

  /// "every branch of the whole auth stack landed in master with the Safe Push of oauth": merged robots landed all of
  /// every member of the stack of `root` (`every_branch_landed`); `None` while one has work left
  pub(crate) fn stack_landed_whole(&self, root: &str) -> Option<String> {
    let stack = self.table().stack(root)?;
    let landed: Vec<(Subject, &PatronusRobot)> = stack.members.iter().map(|member| self.landed_whole(member)).collect::<Option<_>>()?;
    let what = Subject::WholeStack { root: root.to_string() }.words();
    (!landed.is_empty()).then(|| every_branch_landed(&what, &landed))
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
  if let Some(parked) = parked_state(&table.branches, &table.parked, name) {
    return Some(format!("{}; it stays local until the user unparks it", parked.reason(name)));
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
  let hold = facts.view.branches.get(name)?.hold.as_ref()?;
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
fn stack_refusal(facts: &GuardFacts, root: &str) -> Option<String> {
  let table = facts.table();
  let Some(stack) = table.stack(root) else {
    return Some(format!("{} has no stack branch of the last sync", Subject::whole_stack(root).words()));
  };
  let branch = &stack.branch_name;
  if let Some(parked) = stack.members.iter().find(|member| table.parked.contains_key(*member)) {
    return Some(format!("{branch} holds parked {parked}; land the stack in pieces"));
  }
  if let Some(landed) = facts.stack_landed_whole(root) {
    return Some(format!("{branch}: {landed}: nothing of it is left to push"));
  }
  let hold = facts.view.stacks.get(root).and_then(|stack| stack.hold_robot_id.as_deref()).and_then(|id| facts.robot(id));
  if let Some(robot) = hold {
    return Some(format!("{branch}: {}", stack_hold_words(root, robot)));
  }
  match stack.state {
    StackBranchState::Built if stack.tip.is_some() => None,
    StackBranchState::Conflict => Some(format!(
      "{branch}: {}",
      stack_conflict_words(&table.branches, stack, &|id| git_ops::revs::short_id(id).to_string())
    )),
    StackBranchState::Built | StackBranchState::Blocked | StackBranchState::Stale => Some(format!("{branch}: {}", stack_not_built_words(stack))),
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

/// The app's push of a stack: the repository lock (`$GIT_COMMON_DIR/branch-deck.lock`, no git process), then the checks
/// on the kept table with `robots` (`check_push`), then the core push (`push_branches`, which refuses a branch the last
/// sync did not build). A held lock refuses the push; so does a repository not synced yet
#[instrument(skip_all, fields(repository_path = %params.repository_path, branches = params.branches.len()))]
pub fn push_guarded(
  git_executor: &GitCommandExecutor,
  git_common_dir: &Path,
  views: &RepositoryViews,
  robots: Vec<PatronusRobot>,
  params: &PushBranchesParams,
  on_progress: &mut dyn FnMut(TransferProgress),
) -> Result<PushBranchesOutcome> {
  push_guarded_within(DEFAULT_LOCK_TIMEOUT, git_executor, git_common_dir, views, robots, params, on_progress)
}

/// `push_guarded`, waiting at most `lock_timeout` for another holder of the lock
fn push_guarded_within(
  lock_timeout: Duration,
  git_executor: &GitCommandExecutor,
  git_common_dir: &Path,
  views: &RepositoryViews,
  robots: Vec<PatronusRobot>,
  params: &PushBranchesParams,
  on_progress: &mut dyn FnMut(TransferProgress),
) -> Result<PushBranchesOutcome> {
  let _lock = lock_repository_in(git_common_dir, lock_timeout, &|| {}).map_err(|error| match error.downcast_ref::<RepositoryLocked>() {
    Some(locked) => anyhow!(locked_refusal(locked)),
    None => error,
  })?;
  let facts = views
    .guard_facts(git_executor, &params.repository_path, &params.branch_prefix, robots)
    .ok_or_else(|| anyhow!(NOT_SYNCED_YET))?;
  if let Err(refusal) = check_push(&facts, &params.branches) {
    bail!(refusal);
  }
  push_branches(git_executor, params, &facts.table().not_built, on_progress)
}

impl RemoteActions<'_> {
  /// The app's push of a stack: the user's robots (`robots_for_checks`), then, in a blocking step, `push_guarded` under
  /// the repository lock of `git_common_dir` (`$GIT_COMMON_DIR` of the repository, which the host knows), its progress
  /// sent to `on_progress`
  #[instrument(skip_all, fields(repository_path = %params.repository_path, branches = params.branches.len()))]
  pub async fn push_branches(
    &self,
    git_common_dir: PathBuf,
    params: PushBranchesParams,
    mut on_progress: impl FnMut(TransferProgress) + Send + 'static,
  ) -> Result<PushBranchesOutcome> {
    let robots = self.robots_for_checks(&params.branch_prefix).await;
    let (git, views) = (self.git_executor.clone(), self.views.clone());
    tokio::task::spawn_blocking(in_current_span(move || push_guarded(&git, &git_common_dir, &views, robots, &params, &mut on_progress))).await?
  }

  /// The app's push of one branch, as `push_branches` pushes the stack of it alone
  pub async fn push_branch(&self, git_common_dir: PathBuf, params: &PushBranchParams, on_progress: impl FnMut(TransferProgress) + Send + 'static) -> Result<PushOutcome> {
    PushOutcome::of_stack(self.push_branches(git_common_dir, params.as_stack(), on_progress).await?)
  }
}

/// "Nothing was pushed: `bd sync` (pid 4242, for 3 s) holds the repository; try again when it is done"
fn locked_refusal(locked: &RepositoryLocked) -> String {
  let holder = locked.holder.as_ref().map_or_else(|| "another Branch Deck operation".to_string(), ToString::to_string);
  format!("Nothing was pushed: {holder} holds the repository; try again when it is done")
}

#[cfg(test)]
#[path = "push_guard_test.rs"]
pub(crate) mod push_guard_test;
