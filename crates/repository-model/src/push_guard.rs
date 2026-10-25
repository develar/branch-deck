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
//! now, best effort), then, in a blocking step, the lock, the checks and the commits of the push fixed
//! (`prepare_push_branches`), and after the lock the network push of those commits (`PreparedPush::send`). The words of
//! the holds are `bd`'s too (`bd push`, `bd safe-push`):
//! `landed_words`, `every_branch_landed`, `own_hold_words`, `span_hold_words`, `stack_hold_words`.
//!
//! The lock is held only while the push is decided: the pushes of the app run side by side, each sending the commits it
//! checked (`<commit>:refs/heads/<name>`, leased on the remote head the user saw), whatever moves the local branches
//! meanwhile. Only the pushes of this process that write one remote ref wait for each other (`PushingRefs`), without a
//! bound, as the one before will finish: the second then finds the remote at its commit, not a lease the first broke.

use crate::actions::RemoteActions;
use crate::branch_table::BranchTable;
use crate::facts::TableFacts;
use crate::patronus_view::{match_robots, patronus_view};
use crate::start_guard::{stack_conflict_words, stack_not_built_words};
use crate::table::RemoteSubject;
use anyhow::{Result, anyhow, bail};
use domain_types::branch_names::to_final_branch_name;
use domain_types::landing::Subject;
use domain_types::patronus::PatronusRobot;
use domain_types::repository_view::{Carried, HoldKind, PatronusView};
use domain_types::session::ViewAnswer;
use domain_types::stack::StackBranchState;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, RepositoryLocked, lock_repository_while_held};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use sync_core::parked::parked_state;
use sync_core::patronus_start::{carried_ids, carried_words, join_names, push_mode_name};
use sync_core::push::{NOT_SYNCED_YET, PushBranchesOutcome, PushBranchesParams, StackPushBranch, prepare_push_branches, push_ref_names};
use sync_core::transfer_progress::TransferProgress;
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};

/// What the app's own robot hold offers instead of a push: its Push and Restart
const APP_OWN_HOLD_REMEDY: &str = "restart it on the current branch";

/// The facts the checks read (`guard_facts`): the table of the last sync, the robots Rust read, and the Patronus view of
/// both
pub struct GuardFacts {
  synced: Arc<BranchTable>,
  pub robots: Vec<PatronusRobot>,
  pub view: PatronusView,
}

/// The facts the checks of `repository_path` read: `table`, the table of the last sync of its session with the memo of
/// its views, `robots` as Rust read them, matched as the rows match them, and the Patronus view of both on the table's
/// remote facts. Runs git for the robots the hashes and the memo don't answer: blocking
#[instrument(skip_all, fields(repository_path = %repository_path, robots = robots.len()))]
pub fn guard_facts(git_executor: &GitCommandExecutor, repository_path: &str, table: &TableFacts, robots: Vec<PatronusRobot>) -> GuardFacts {
  let synced = Arc::clone(&table.table);
  let matched = match_robots(&synced, &synced.remote, &robots);
  let view = patronus_view(git_executor, repository_path, &synced, &synced.remote, &robots, &matched, &mut table.memo());
  GuardFacts { synced, robots, view }
}

impl GuardFacts {
  pub fn table(&self) -> &BranchTable {
    &self.synced
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

/// The app's push of a stack: its refs claimed against the pushes of this process that write one of them
/// (`PushingRefs`), then, under the repository lock (`$GIT_COMMON_DIR/branch-deck.lock`, no git process), the checks on
/// `table`, the table of the last sync of the session, with `robots` (`check_push`) and the commits of the push fixed
/// (`prepare_push_branches`, which refuses
/// a branch the last sync did not build), then, after the lock, the network push of those commits. While another
/// process (or a rewrite of this one) holds the lock, the push waits as long as the holder lives, its progress naming
/// it ("Waiting for `bd pick` (pid 4242, for 12 s)", `TransferProgress::waiting_for`), until `canceled` says the user
/// stopped waiting (Stop Waiting: `LockWaitCanceled`, nothing pushed); a lock whose holder no record names refuses it
/// after `DEFAULT_LOCK_TIMEOUT`, and so does a repository not synced yet (no `table`). The branches are named under
/// `branch_prefix`
#[instrument(skip_all, fields(repository_path = %params.repository_path, branches = params.branches.len()))]
#[expect(clippy::too_many_arguments, reason = "the push, its checks, its progress and its cancel")]
pub fn push_guarded(
  git_executor: &GitCommandExecutor,
  git_common_dir: &Path,
  table: Option<TableFacts>,
  robots: Vec<PatronusRobot>,
  branch_prefix: &str,
  params: &PushBranchesParams,
  on_progress: &mut dyn FnMut(TransferProgress),
  canceled: &dyn Fn() -> bool,
) -> Result<PushBranchesOutcome> {
  let on_wait = || debug!(git_common_dir = %git_common_dir.display(), "the push waits for a push of its refs");
  push_guarded_within(
    DEFAULT_LOCK_TIMEOUT,
    &on_wait,
    canceled,
    git_executor,
    git_common_dir,
    table,
    robots,
    branch_prefix,
    params,
    on_progress,
  )
}

/// `push_guarded`, waiting as long as it takes for the pushes of this process that write one of its refs, then for a live
/// holder of the lock until `canceled`, and at most `unknown_timeout` for one no record names; `on_wait` is called when
/// the wait for the refs starts
#[expect(clippy::too_many_arguments, reason = "push_guarded and the waits its tests control")]
fn push_guarded_within(
  unknown_timeout: Duration,
  on_wait: &dyn Fn(),
  canceled: &dyn Fn() -> bool,
  git_executor: &GitCommandExecutor,
  git_common_dir: &Path,
  table: Option<TableFacts>,
  robots: Vec<PatronusRobot>,
  branch_prefix: &str,
  params: &PushBranchesParams,
  on_progress: &mut dyn FnMut(TransferProgress),
) -> Result<PushBranchesOutcome> {
  let names: Vec<String> = push_ref_names(branch_prefix, params)?.into_iter().map(|(name, _)| name).collect();
  let _claim = PUSHING_REFS.claim(git_common_dir, &names, on_wait);
  let prepared = {
    let mut on_holder = |holder: &_| on_progress(TransferProgress::waiting_for(holder));
    let _lock = lock_repository_while_held(git_common_dir, unknown_timeout, &mut on_holder, canceled).map_err(|error| match error.downcast_ref::<RepositoryLocked>() {
      Some(locked) => anyhow!(locked_refusal(locked)),
      None => error,
    })?;
    let table = table.ok_or_else(|| anyhow!(NOT_SYNCED_YET))?;
    let facts = guard_facts(git_executor, &params.repository_path, &table, robots);
    if let Err(refusal) = check_push(&facts, &params.branches) {
      bail!(refusal);
    }
    prepare_push_branches(git_executor, branch_prefix, params, &facts.table().not_built)?
  };
  prepared.send(git_executor, on_progress)
}

impl RemoteActions<'_> {
  /// The app's push of a stack in the session of its repository (`Sessions::push`, the key of the path and of the
  /// prefix the host resolved, so the outcome changes only that table): the user's robots (`robots_for_checks`) and the
  /// table of the session's last sync, then, in a blocking step, `push_guarded`, its checks under the repository lock of
  /// `git_common_dir` (`$GIT_COMMON_DIR` of
  /// the repository, which the host knows), its progress sent to `on_progress`, its wait for another holder of the lock
  /// ended by `canceled`. Answers the outcome with the version of the patch that has it
  #[instrument(skip_all, fields(repository_path = %params.repository_path, branches = params.branches.len()))]
  pub async fn push_branches(
    &self,
    git_common_dir: PathBuf,
    params: PushBranchesParams,
    mut on_progress: impl FnMut(TransferProgress) + Send + 'static,
    canceled: impl Fn() -> bool + Send + 'static,
  ) -> Result<ViewAnswer<PushBranchesOutcome>> {
    let key = self.key(&params.repository_path);
    let subjects = params.branches.iter().map(RemoteSubject::of).collect();
    let push = async move {
      let robots = self.robots_for_checks().await;
      // the table of the last sync: a sync takes no lock, so the one read under it would be no newer
      let table = self.table(&params.repository_path).await;
      let (git, branch_prefix) = (self.git_executor.clone(), self.branch_prefix.to_owned());
      tokio::task::spawn_blocking(in_current_span(move || {
        push_guarded(&git, &git_common_dir, table, robots, &branch_prefix, &params, &mut on_progress, &canceled)
      }))
      .await?
    };
    self.sessions.push(&key, subjects, push).await
  }
}

/// The refs the guarded pushes of this process are writing
static PUSHING_REFS: PushingRefs = PushingRefs::new();

/// The remote refs the pushes of a process are writing, by `$GIT_COMMON_DIR` and full name: a push of a ref another
/// push writes (the same branch twice, two whole stacks of one stack branch) waits until that one ended, since both
/// lease on the head the user saw and the second would find the lease broken by the first; a push of other refs runs
/// beside it. The lock file can't do it: it is held only while a push is decided, and its wait is bounded
struct PushingRefs {
  refs: Mutex<BTreeSet<(PathBuf, String)>>,
  released: Condvar,
}

/// The refs of a push (`PushingRefs::claim`), released when dropped
struct PushClaim<'a> {
  pushing: &'a PushingRefs,
  refs: Vec<(PathBuf, String)>,
}

impl PushingRefs {
  const fn new() -> Self {
    Self {
      refs: Mutex::new(BTreeSet::new()),
      released: Condvar::new(),
    }
  }

  /// The set changes in a few lines that don't panic, so a poisoned one is still in order
  fn lock(&self) -> MutexGuard<'_, BTreeSet<(PathBuf, String)>> {
    self.refs.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// Claims all of `names` of `git_common_dir` at once, waiting without a bound while another push writes one of them;
  /// `on_wait` is called once when it waits
  fn claim(&self, git_common_dir: &Path, names: &[String], on_wait: &dyn Fn()) -> PushClaim<'_> {
    let refs: Vec<(PathBuf, String)> = names.iter().map(|name| (git_common_dir.to_path_buf(), name.clone())).collect();
    let busy = |pushing: &BTreeSet<(PathBuf, String)>| refs.iter().any(|key| pushing.contains(key));
    let mut pushing = self.lock();
    if busy(&pushing) {
      drop(pushing);
      on_wait();
      pushing = self.released.wait_while(self.lock(), |pushing| busy(pushing)).unwrap_or_else(PoisonError::into_inner);
    }
    pushing.extend(refs.iter().cloned());
    drop(pushing);
    PushClaim { pushing: self, refs }
  }
}

impl Drop for PushClaim<'_> {
  fn drop(&mut self) {
    let mut pushing = self.pushing.lock();
    for key in &self.refs {
      pushing.remove(key);
    }
    drop(pushing);
    self.pushing.released.notify_all();
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
