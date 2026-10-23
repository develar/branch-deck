//! The checks of the app's start, restart and cancel of a robot (`start_patronus_robot`, `restart_patronus_robot`,
//! `cancel_patronus_robot` and the E2E server's routes of them) in Rust, by the rules of `bd safe-push` over the
//! repository view (`patronus_view`) and the robots Rust read itself, never the page's: a parked subject (a branch, a
//! branch built on parked, a whole stack with a parked member) stays local, a subject merged Safe Pushes landed whole
//! has nothing left to land, a subject the last sync did not build (a stack branch that is not `built`) is no build of
//! its commits, and a robot that runs on its last commit or lands a part of it refuses a second one (the `start` and
//! `dry_run` refusals of the view). The page checks the same before it asks and says it in its words; these refuse a
//! start that raced with another one (the IDE, `bd`) or came by IPC.
//!
//! A restart is a start of the robot's subject that the robot itself does not refuse; a robot of a temporary ref (the
//! IDE, the CLI) is not restarted: it would test its old commit. A cancel needs a running robot of the user; the cancel
//! of the robot a start or a push replaces answers one that finished meanwhile as it is (`CancelCheck::AlreadyFinished`).
//!
//! `RemoteActions::start_robot`, `restart_robot` and `cancel_robot` (`actions`) are the whole sequence of both hosts: one
//! list of the robots asked now (a failed list fails the action), the checks, then the landing gate and the target
//! (`prepare_robot_start`) and the call of Patronus; `check_landing` is the landing check of the popover on the same
//! robots. The words of the robot refusals are `bd`'s too (`start_refusal_text`).

use crate::actions::RemoteActions;
use crate::patronus_view::{branch_start_refusal, landed_commits, stack_start_refusal};
use crate::push_guard::{GuardFacts, row_refusal};
use anyhow::{Result, anyhow, bail};
use domain_types::GroupedBranchInfo;
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::landing::Subject;
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotParams, StartPatronusRobotParams};
use domain_types::repository_view::StartRefusal;
use domain_types::stack::{BlockReason, StackBranchState, StackBranchUpdate};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use std::collections::HashSet;
use sync_core::parked::parked_state;
use sync_core::patronus::required_target_branch;
use sync_core::patronus_start::{carried_ids, carried_words, is_robot_active, join_names, push_mode_name};
use sync_core::push::not_synced_refusal;
use sync_utils::span::in_current_span;
use tracing::instrument;
use trial_check::landing_gate::{LandingCheck, LandingGateParams, ensure_landing_starts, landing_check};

/// The branches of a start Patronus is asked for
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotStartBranches {
  /// `refs/heads/{prefix}/virtual/<name>`, or `refs/heads/{prefix}/stack/<root>` for the whole stack
  pub source_branch: String,
  /// The branch of origin the baseline tracks
  pub target_branch: String,
}

/// What the app checks before it asks Patronus to start `params`, after the checks of the repository view
/// (`check_start`): the landing gate, as `bd safe-push` refuses it (`ensure_landing_starts`), with `landed_commits`, the
/// commits (original ids) merged Safe Pushes landed as the view names them, whose names the target has; then the branch
/// the robot pushes into. The full ref of the pushed virtual branch, or of the stack branch with `stack`
pub fn prepare_robot_start(git_executor: &GitCommandExecutor, cache: &ObjectCache, params: &StartPatronusRobotParams, landed_commits: Vec<String>) -> Result<RobotStartBranches> {
  let gate = LandingGateParams {
    repository_path: params.repository_path.clone(),
    branch_prefix: params.branch_prefix.clone(),
    branch_name: params.branch_name.clone(),
    stack: params.stack,
    allow: params.allow,
  };
  ensure_landing_starts(git_executor, cache, &gate, &landed_commits.into_iter().collect())?;
  let target_branch = required_target_branch(git_executor, &params.repository_path, &params.baseline_branch)?;
  let source = if params.stack {
    to_stack_branch_name(&params.branch_prefix, &params.branch_name)?
  } else {
    to_final_branch_name(&params.branch_prefix, &params.branch_name)?
  };
  Ok(RobotStartBranches {
    source_branch: format!("refs/heads/{source}"),
    target_branch,
  })
}

/// Why a start of `subject` ("auth", "the whole auth stack") is not possible, in the words of the app, for the robot part
/// of the decision (`BranchPatronus::start`, `StackPatronus::start`): the commits a descendant lands named by their ids
pub fn start_refusal_text(refusal: &StartRefusal, subject: &str, robots: &[PatronusRobot]) -> String {
  let mode_of = |robot_id: &str| robots.iter().find(|robot| robot.id == robot_id).map_or("Robot", |robot| push_mode_name(robot.push_mode));
  match refusal {
    StartRefusal::LastCommit { robot_id } => format!("A {} of the last commit is running", mode_of(robot_id)),
    StartRefusal::Descendant { span } => {
      let carried = span.carried.as_ref();
      format!(
        "A {} of {} is running: it lands {} with it{}",
        mode_of(&span.by.robot_id),
        span.by.subject().words(),
        carried_words(subject, carried),
        carried_ids(carried)
      )
    }
    StartRefusal::Ancestor { landing } => format!("A {} of {} is running: wait until it merges", mode_of(&landing.robot_id), landing.subject().words()),
  }
}

/// Why a start of `subject` in `mode` is refused: "Nothing was started: …"; `Ok` when nothing refuses
pub fn check_start(facts: &GuardFacts, subject: &Subject, mode: PatronusPushMode) -> Result<(), String> {
  match start_refusal(facts, subject, mode == PatronusPushMode::DryRun, None) {
    Some(refusal) => Err(format!("Nothing was started: {refusal}")),
    None => Ok(()),
  }
}

/// Why the restart of `robot_id` is refused: "Nothing was restarted: …". A robot the list lacks or of a temporary ref is
/// refused; a restart is a start of the robot's subject in its mode, which the robot itself does not refuse. A subject
/// without a row of the table (archived, renamed) has nothing of the table to check: Patronus restarts the robot on its
/// remote branch
pub fn check_restart(facts: &GuardFacts, robot_id: &str) -> Result<(), String> {
  let refused = |why: String| Err(format!("Nothing was restarted: {why}"));
  let Some(robot) = facts.robot(robot_id) else {
    return refused(format!("Patronus lists no robot {robot_id} of the user"));
  };
  let subject = match (&robot.stack_root, &robot.branch_name) {
    (Some(root), _) => Subject::whole_stack(root),
    (None, Some(name)) => Subject::branch(name),
    (None, None) => {
      return refused(format!(
        "the {} is of a temporary ref (the IDE, the CLI): a restart would test its old commit ({})",
        push_mode_name(robot.push_mode),
        robot.url
      ));
    }
  };
  let has_row = match &subject {
    Subject::Branch { name } => facts.table().branch(name).is_some(),
    Subject::WholeStack { root } => facts.table().stack(root).is_some(),
  };
  match start_refusal(facts, &subject, robot.push_mode == PatronusPushMode::DryRun, Some(robot_id)).filter(|_| has_row) {
    Some(refusal) => refused(refusal),
    None => Ok(()),
  }
}

/// What the cancel of a robot does after its check: asks Patronus, or answers the robot a start or a push replaces as it
/// is, finished already: nothing is left to cancel, and the replacing start or push goes on
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelCheck {
  Cancel,
  AlreadyFinished(Box<PatronusRobot>),
}

/// Why the cancel of `robot_id` is refused: "Nothing was canceled: …" for a robot the list of the user lacks or one that
/// finished; a canceling robot is canceled again. For the robot a start or a push replaces or restarts (`replaced`), one
/// that finished meanwhile is no refusal but `AlreadyFinished`: an explicit Cancel still says "not running"
pub fn check_cancel(robots: &[PatronusRobot], robot_id: &str, replaced: bool) -> Result<CancelCheck, String> {
  match robots.iter().find(|robot| robot.id == robot_id) {
    None => Err(format!("Nothing was canceled: Patronus lists no robot {robot_id} of the user")),
    Some(robot) if !is_robot_active(robot) && replaced => Ok(CancelCheck::AlreadyFinished(Box::new(robot.clone()))),
    Some(robot) if !is_robot_active(robot) => Err(format!("Nothing was canceled: the {} is not running ({})", push_mode_name(robot.push_mode), robot.url)),
    Some(_) => Ok(CancelCheck::Cancel),
  }
}

/// The first reason a start of `subject` is refused: parked, landed whole, not built, then the robot part of the view;
/// `excluded` is the robot a restart replaces, which refuses nothing as the robot of the last commit
fn start_refusal(facts: &GuardFacts, subject: &Subject, dry_run: bool, excluded: Option<&str>) -> Option<String> {
  match subject {
    Subject::Branch { name } => branch_refusal(facts, name, dry_run, excluded),
    Subject::WholeStack { root } => stack_refusal(facts, root, dry_run, excluded),
  }
}

/// The robot of the last commit, unless it is `excluded`
fn without(last_commit: Option<&StartRefusal>, excluded: Option<&str>) -> Option<StartRefusal> {
  last_commit
    .filter(|refusal| !matches!(refusal, StartRefusal::LastCommit { robot_id } if Some(robot_id.as_str()) == excluded))
    .cloned()
}

fn branch_refusal(facts: &GuardFacts, name: &str, dry_run: bool, excluded: Option<&str>) -> Option<String> {
  if let Some(refusal) = row_refusal(facts, name, "land") {
    return Some(refusal);
  }
  let table = facts.table();
  if let Some(reason) = table.not_built.get(name) {
    return Some(not_synced_refusal(name, *reason));
  }
  let patronus = facts.view.branches.get(name)?;
  let last_commit = without(patronus.dry_run.as_ref(), excluded);
  let refusal = if dry_run {
    last_commit
  } else {
    branch_start_refusal(last_commit, patronus.span.as_ref(), patronus.landing_ancestor.as_ref())
  }?;
  Some(start_refusal_text(&refusal, name, &facts.robots))
}

fn stack_refusal(facts: &GuardFacts, root: &str, dry_run: bool, excluded: Option<&str>) -> Option<String> {
  let table = facts.table();
  let what = Subject::whole_stack(root).words();
  let Some(stack) = table.stack(root) else {
    return Some(format!("{what} has no stack branch: the stack doesn't fork"));
  };
  let parked = stack
    .members
    .iter()
    .find_map(|member| parked_state(&table.branches, &table.parked, member).map(|parked| parked.reason(member)));
  if let Some(reason) = parked {
    return Some(format!("{what} holds what is parked: {reason}; land the stack in pieces"));
  }
  if let Some(landed) = facts.stack_landed_whole(root) {
    return Some(format!("{landed}: nothing of it is left to land"));
  }
  match stack.state {
    StackBranchState::Built => {}
    StackBranchState::Conflict => {
      return Some(format!(
        "{}. Land it in pieces: the Safe Push of a branch lands it with what it is built on; or regroup so that the commits it needs come before it",
        stack_conflict_words(&table.branches, stack, &|id| git_ops::revs::short_id(id).to_string())
      ));
    }
    StackBranchState::Blocked | StackBranchState::Stale => return Some(format!("{}; fix it, then sync", stack_not_built_words(stack))),
  }
  let patronus = facts.view.stacks.get(root)?;
  let last_commit = without(patronus.dry_run.as_ref(), excluded);
  let refusal = if dry_run {
    last_commit
  } else {
    stack_start_refusal(last_commit, patronus.landing_member.as_ref())
  }?;
  Some(start_refusal_text(&refusal, &what, &facts.robots))
}

/// "the whole auth stack doesn't build as one branch: 1a2b3c4d (Add login, of login) conflicts in a.rs with the commits
/// of the stack before it, though each branch builds": why the conflicted stack branch of `update` lands nothing, the
/// commit by `short` of its id with its subject and branch as `branches` (the table of the sync) have it. `bd` adds its
/// commands, the app its words
pub fn stack_conflict_words(branches: &[GroupedBranchInfo], update: &StackBranchUpdate, short: &dyn Fn(&str) -> String) -> String {
  let root = &update.root;
  let Some(conflict) = &update.conflict else {
    return format!("the stack branch of {root} conflicts");
  };
  let found = branches.iter().find_map(|branch| {
    branch
      .commits
      .iter()
      .find(|commit| commit.id == conflict.commit)
      .map(|commit| (branch.name.as_str(), commit))
  });
  let commit = match found {
    Some((branch, commit)) => format!("{} ({}, of {branch})", short(&conflict.commit), commit.stripped_subject),
    None => short(&conflict.commit),
  };
  format!(
    "{} doesn't build as one branch: {commit} conflicts in {} with the commits of the stack before it, though each branch builds",
    Subject::whole_stack(root).words(),
    join_names(&conflict.files)
  )
}

/// "the stack branch of auth is not the build of its commits: login conflicts, so the stack branch was not rebuilt": why
/// a stack branch that is not built can't land, the member that keeps it from being built or the reason
pub fn stack_not_built_words(update: &StackBranchUpdate) -> String {
  let why = match (&update.blocked_by, &update.error) {
    (Some(block), _) => {
      let reason = match block.reason {
        BlockReason::Conflict => "conflicts",
        BlockReason::Problem => "has a stack problem",
        BlockReason::Error => "failed",
        BlockReason::Blocked => "is blocked by its parent",
      };
      format!("{} {reason}, so the stack branch was not rebuilt", block.member)
    }
    (None, Some(error)) => error.clone(),
    (None, None) => "the stack branch was not rebuilt".to_string(),
  };
  format!("the stack branch of {} is not the build of its commits: {why}", update.root)
}

impl RemoteActions<'_> {
  /// The facts of the checks: one full list of the user's robots (a list Patronus doesn't give fails the action), then,
  /// in a blocking step, the view of the kept table with them (`guard_facts`) and `then` over it; a repository not
  /// synced yet is refused with `nothing` ("Nothing was started")
  async fn with_facts<T: Send + 'static>(
    &self,
    repository_path: &str,
    branch_prefix: &str,
    nothing: &'static str,
    then: impl FnOnce(&GuardFacts) -> Result<T> + Send + 'static,
  ) -> Result<T> {
    let robots = self.robots_for_action(branch_prefix).await?;
    let (git, views, path, prefix) = (self.git_executor.clone(), self.views.clone(), repository_path.to_string(), branch_prefix.to_string());
    tokio::task::spawn_blocking(in_current_span(move || {
      let facts = views
        .guard_facts(&git, &path, &prefix, robots)
        .ok_or_else(|| anyhow!("{nothing}: the branches are not synced yet; sync first"))?;
      then(&facts)
    }))
    .await?
  }

  /// Starts a Safe Push or a Dry Run of `params` as the app does: the checks (`check_start`), then the landing gate with
  /// the commits merged Safe Pushes landed as the view names them (`landed_commits`) and the target
  /// (`prepare_robot_start`), then the start. `cache` opens the object cache of the gate, in the blocking step
  #[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, stack = params.stack, mode = ?params.mode))]
  pub async fn start_robot(&self, cache: impl FnOnce() -> ObjectCache + Send + 'static, params: &StartPatronusRobotParams) -> Result<PatronusRobot> {
    let (service, repository) = self.patronus()?;
    let (git, start) = (self.git_executor.clone(), params.clone());
    let branches = self
      .with_facts(&params.repository_path, &params.branch_prefix, "Nothing was started", move |facts| {
        let subject = if start.stack {
          Subject::whole_stack(&start.branch_name)
        } else {
          Subject::branch(&start.branch_name)
        };
        if let Err(refusal) = check_start(facts, &subject, start.mode) {
          bail!(refusal);
        }
        let mut landed: Vec<String> = landed_commits(&facts.view).into_iter().collect();
        landed.sort_unstable();
        prepare_robot_start(&git, &cache(), &start, landed)
      })
      .await?;
    service
      .start(
        repository,
        &params.branch_prefix,
        &branches.source_branch,
        &branches.target_branch,
        params.mode,
        params.profile_id.clone(),
        params.merge_request.clone(),
      )
      .await
      .map_err(|e| anyhow!(e))
  }

  /// The landing gate as the popover asks it before a start (`check_landing_gate`), with the commits merged Safe Pushes
  /// landed as the view names them (`landed_commits`) on the robots Rust reads now (`robots_for_landing_check`: the list
  /// of a robot action, the same input the start gives its own gate, `start_robot`): the page's robots never decide it.
  /// A list Patronus doesn't give fails the check instead of counting nothing landed, which would refuse a start whose
  /// need a merged Safe Push landed; the page then says "Not checked whether …" and the start checks again. Off Space or
  /// before the first sync nothing landed; `cache` opens the object cache, in the blocking step
  #[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, stack = params.stack))]
  pub async fn check_landing(&self, cache: impl FnOnce() -> ObjectCache + Send + 'static, params: &LandingGateParams) -> Result<LandingCheck> {
    let robots = self.robots_for_landing_check(&params.branch_prefix).await?;
    let (git, views, params) = (self.git_executor.clone(), self.views.clone(), params.clone());
    tokio::task::spawn_blocking(in_current_span(move || {
      let landed: HashSet<String> = views
        .guard_facts(&git, &params.repository_path, &params.branch_prefix, robots)
        .map_or_default(|facts| landed_commits(&facts.view));
      landing_check(&git, &cache(), &params, &landed)
    }))
    .await?
  }

  /// Restarts the robot of `params` as the app does: the checks (`check_restart`), then the restart
  #[instrument(skip_all, fields(repository_path = %params.repository_path, robot_id = %params.robot_id))]
  pub async fn restart_robot(&self, params: &PatronusRobotParams) -> Result<PatronusRobot> {
    let (service, _) = self.patronus()?;
    let robot_id = params.robot_id.clone();
    self
      .with_facts(&params.repository_path, &params.branch_prefix, "Nothing was restarted", move |facts| {
        check_restart(facts, &robot_id).map_err(|refusal| anyhow!(refusal))
      })
      .await?;
    service.restart(&params.branch_prefix, &params.robot_id).await.map_err(|e| anyhow!(e))
  }

  /// Cancels the robot of `params` as the app does: one list of the user's robots (`check_cancel`), then the cancel; no
  /// table: a cancel changes no branch. The robot a start or a push replaces (`replaced`) that finished meanwhile is
  /// answered as listed, and Patronus is not asked
  #[instrument(skip_all, fields(repository_path = %params.repository_path, robot_id = %params.robot_id, replaced = params.replaced))]
  pub async fn cancel_robot(&self, params: &PatronusRobotParams) -> Result<PatronusRobot> {
    let (service, _) = self.patronus()?;
    let robots = self.robots_for_action(&params.branch_prefix).await?;
    match check_cancel(&robots, &params.robot_id, params.replaced).map_err(|refusal| anyhow!(refusal))? {
      CancelCheck::AlreadyFinished(robot) => Ok(*robot),
      CancelCheck::Cancel => service.cancel(&params.branch_prefix, &params.robot_id).await.map_err(|e| anyhow!(e)),
    }
  }
}

#[cfg(test)]
#[path = "start_guard_test.rs"]
mod start_guard_test;
