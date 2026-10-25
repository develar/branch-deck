//! The checks of the app's start, restart and cancel of a robot (`start_patronus_robot`, `restart_patronus_robot`,
//! `cancel_patronus_robot` and the E2E server's routes of them) in Rust, by the rules of `bd safe-push` over the
//! Patronus view (`derive_patronus`) and the robots Rust read itself, never the page's: a parked subject (a branch, a
//! branch built on parked, a whole stack with a parked member) stays local, a subject merged Safe Pushes landed whole
//! has nothing left to land, a subject the last sync did not build (a stack branch that is not `built`) is no build of
//! its commits, and a robot that runs on its last commit or lands a part of it refuses a second one, a running Safe Push
//! that lands the branch the push of a Dry Run too (`domain_policy::land::start_refusal`, as the view decides it). The
//! page checks the same before it asks and says it in its words; these refuse a start that raced with another one (the
//! IDE, `bd`) or came by IPC.
//!
//! A restart is a start of the robot's subject that the robot itself does not refuse; a robot of a temporary ref (the
//! IDE, the CLI) is not restarted: it would test its old commit. A cancel needs a running robot of the user; the cancel
//! of the robot a start or a push replaces answers one that finished meanwhile as it is (`CancelCheck::AlreadyFinished`).
//!
//! `RemoteActions::start_robot`, `restart_robot` and `cancel_robot` (`actions`) are the whole sequence of both hosts, and
//! `check_start_from_pull_request` the same checks before the `/patronus` comment of the other route of a start: one
//! list of the robots asked now (a failed list fails the action), the checks, then the landing gate and the target
//! (`prepare_robot_start`) and the call of Patronus; `check_landing` is the landing check of the popover on the same
//! robots. Both checks go through the landing gate of the host (`LandingGate`, which keeps the check of the same inputs),
//! so a start right after the check of its popover reuses it while the refs, the config and the landed commits are the
//! same. The words of the robot refusals are `bd`'s too (`start_refusal_text`).

use crate::actions::RemoteActions;
use crate::derive::LandingFacts;
use crate::landing::LandingGate;
use crate::patronus_view::landed_commits;
use crate::push_guard::span_hold_words;
use crate::push_guard::{GuardFacts, guard_facts, parked_words, row_refusal};
use anyhow::{Result, anyhow, bail};
use domain_policy::holds::is_active;
use domain_policy::land::{BranchStart, StackStart, Start, start_refusal as robot_refusal};
use domain_policy::parked::whole_stack_parked;
use domain_types::GroupedBranchInfo;
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::landing::{LandingCheck, LandingGateParams, Subject, ensure_landing_starts};
use domain_types::patronus::{PatronusCommandSubject, PatronusPushMode, PatronusRobot, PatronusRobotParams, PostPatronusCommandParams, StartPatronusRobotParams};
use domain_types::reason::Reason;
use domain_types::session::ParkedMember;
use domain_types::stack::{BlockReason, StackBranchState, StackBranchUpdate};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashSet;
use std::sync::Arc;
use sync_core::patronus::required_target_branch;
use sync_core::patronus_start::{carried_ids, carried_words, join_names, push_mode_name};
use sync_core::push::not_synced_refusal;
use sync_utils::span::in_current_span;
use tracing::instrument;

/// The branches of a start Patronus is asked for
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotStartBranches {
  /// `refs/heads/{prefix}/virtual/<name>`, or `refs/heads/{prefix}/stack/<root>` for the whole stack
  pub source_branch: String,
  /// The branch of origin the baseline tracks
  pub target_branch: String,
}

/// What the app checks before it asks Patronus to start `params`, after the checks of the Patronus view
/// (`check_start`): the landing gate, as `bd safe-push` refuses it (`ensure_landing_starts`), with `landed_commits`, the
/// commits (original ids) merged Safe Pushes landed as the view names them, whose names the target has, through `gate`
/// (the check of the popover when its inputs are the same); then the branch the robot pushes into. The full ref of the
/// pushed virtual branch, or of the stack branch with `stack`, under `branch_prefix`. Blocking
pub fn prepare_robot_start(
  git_executor: &GitCommandExecutor,
  gate: &dyn LandingGate,
  branch_prefix: &str,
  params: &StartPatronusRobotParams,
  landed_commits: &HashSet<String>,
) -> Result<RobotStartBranches> {
  let start = LandingGateParams {
    repository_path: params.repository_path.clone(),
    branch_name: params.branch_name.clone(),
    stack: params.stack,
    allow: params.allow,
  };
  ensure_landing_starts(&start, gate.check(branch_prefix, &start, landed_commits)?)?;
  let target_branch = required_target_branch(git_executor, &params.repository_path, &params.baseline_branch)?;
  let source = if params.stack {
    to_stack_branch_name(branch_prefix, &params.branch_name)?
  } else {
    to_final_branch_name(branch_prefix, &params.branch_name)?
  };
  Ok(RobotStartBranches {
    source_branch: format!("refs/heads/{source}"),
    target_branch,
  })
}

/// Why a start of `subject` ("auth", "the whole auth stack") is not possible, in the words of `bd` and of the guards
/// of the app, for the robot part of the decision (`domain_policy::land::start_refusal`): the commits a descendant lands
/// named by their ids, the robot of a hold by its URL (`robots`)
pub fn start_refusal_text(refusal: &Reason, subject: &str, robots: &[PatronusRobot]) -> String {
  match refusal {
    Reason::LastCommit { mode, .. } => format!("A {} of the last commit is running", push_mode_name(*mode)),
    Reason::Descendant { by, mode, carried, .. } => format!(
      "A {} of {} is running: it lands {} with it{}",
      push_mode_name(*mode),
      by.subject().words(),
      carried_words(subject, carried.as_ref()),
      carried_ids(carried.as_ref())
    ),
    Reason::Ancestor { landing, mode } => format!("A {} of {} is running: wait until it merges", push_mode_name(*mode), landing.subject().words()),
    Reason::Held { hold, .. } => match robots.iter().find(|robot| robot.id == hold.by.robot_id) {
      Some(robot) => span_hold_words(subject, &hold.by.subject(), robot, hold.carried.as_ref()),
      None => format!("a running Safe Push of {} lands {subject}: a push won't land", hold.by.subject().words()),
    },
    Reason::Blocked { by: Some(by), .. } => format!("{subject} is blocked by {by}: fix it first, then sync"),
    Reason::Blocked { by: None, .. } => format!("{subject} is blocked: fix its parent first, then sync"),
    Reason::Landed { .. } | Reason::StackLanded { .. } => format!("{subject} landed: nothing of it is left to land"),
    Reason::NoStackBranch { .. } => format!("{subject} has no stack branch: the stack doesn't fork"),
    Reason::StackNotBuilt { root, .. } => format!("the stack branch of {root} is not the build of its commits; fix it, then sync"),
    Reason::StackParked { member, .. } => format!("{subject} holds parked {member}; land the stack in pieces"),
    Reason::StackHeld { mode, .. } => format!("a {} of {subject} is running on its stack branch", push_mode_name(*mode)),
    Reason::CantPush { branch, .. } => format!("{branch} is not synced"),
    Reason::DropsRemote { branch, behind } => format!("the remote branch of {branch} has {behind} commits {branch} lacks"),
  }
}

/// Why a start of `subject` in `mode` is refused: "Nothing was started: …"; `Ok` when nothing refuses
pub fn check_start(facts: &GuardFacts, subject: &Subject, mode: PatronusPushMode) -> Result<(), String> {
  match start_refusal(facts, subject, mode == PatronusPushMode::DryRun, None) {
    Some(refusal) => Err(format!("Nothing was started: {refusal}")),
    None => Ok(()),
  }
}

/// The subject a start names: the branch, or with `stack` the whole stack of the root `name`
fn start_subject(name: &str, stack: bool) -> Subject {
  if stack { Subject::whole_stack(name) } else { Subject::branch(name) }
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
    Some(robot) if !is_active(robot) && replaced => Ok(CancelCheck::AlreadyFinished(Box::new(robot.clone()))),
    Some(robot) if !is_active(robot) => Err(format!("Nothing was canceled: the {} is not running ({})", push_mode_name(robot.push_mode), robot.url)),
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

/// The robot part of a start in `mode` (`domain_policy::land::start_refusal`): `excluded` is the robot a restart
/// replaces, which refuses nothing as the robot of the last commit; a restart pushes nothing
fn robot_part(landing: &LandingFacts<'_>, start: Start<'_>, dry_run: bool, excluded: Option<&str>) -> Option<Reason> {
  let not_excluded = |robot: &&PatronusRobot| Some(robot.id.as_str()) != excluded;
  let start = match start {
    Start::Branch(branch) => Start::Branch(BranchStart {
      last_commit: branch.last_commit.filter(not_excluded),
      pushes: branch.pushes && excluded.is_none(),
      ..branch
    }),
    Start::WholeStack(stack) => Start::WholeStack(StackStart {
      last_commit: stack.last_commit.filter(not_excluded),
      ..stack
    }),
  };
  let mode = if dry_run { PatronusPushMode::DryRun } else { PatronusPushMode::RebaseAutosquash };
  robot_refusal(&start, mode, &landing.robots)
}

fn branch_refusal(facts: &GuardFacts, name: &str, dry_run: bool, excluded: Option<&str>) -> Option<String> {
  if let Some(refusal) = row_refusal(facts, name, "land") {
    return Some(refusal);
  }
  let table = facts.table();
  if let Some(reason) = table.not_built.get(name) {
    return Some(not_synced_refusal(name, *reason));
  }
  let landing = facts.landing();
  let start = landing.branch_start(name, landing.start_pushes(name));
  let refusal = robot_part(&landing, Start::Branch(start), dry_run, excluded)?;
  Some(start_refusal_text(&refusal, name, &facts.robots))
}

fn stack_refusal(facts: &GuardFacts, root: &str, dry_run: bool, excluded: Option<&str>) -> Option<String> {
  let table = facts.table();
  let what = Subject::whole_stack(root).words();
  let Some(stack) = table.stack(root) else {
    return Some(format!("{what} has no stack branch: the stack doesn't fork"));
  };
  if let Some(ParkedMember { member, parked }) = whole_stack_parked(&table.repository(), stack.members.iter().map(String::as_str)) {
    return Some(format!("{what} holds what is parked: {}; land the stack in pieces", parked_words(&member, &parked)));
  }
  if let Some(Reason::StackLanded { by, .. }) = facts.stack_hold(root) {
    return Some(format!("{}: nothing of it is left to land", facts.stack_landed_words(root, by)));
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
  let landing = facts.landing();
  let refusal = robot_part(&landing, Start::WholeStack(landing.stack_start(stack)), dry_run, excluded)?;
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
  /// The facts of the checks: one full list of the user's robots (a list Patronus doesn't give fails the action) and the
  /// table of the last sync of the repository's session, then, in a blocking step, the view of both (`guard_facts`) and
  /// `then` over it; a repository not synced yet is refused with `nothing` ("Nothing was started")
  async fn with_facts<T: Send + 'static>(&self, repository_path: &str, nothing: &'static str, then: impl FnOnce(&GuardFacts) -> Result<T> + Send + 'static) -> Result<T> {
    let robots = self.robots_for_action().await?;
    let table = self
      .table(repository_path)
      .await
      .ok_or_else(|| anyhow!("{nothing}: the branches are not synced yet; sync first"))?;
    let (git, path) = (self.git_executor.clone(), repository_path.to_string());
    tokio::task::spawn_blocking(in_current_span(move || then(&guard_facts(&git, &path, &table, robots)))).await?
  }

  /// Starts a Safe Push or a Dry Run of `params` as the app does: the checks (`check_start`), then the landing gate with
  /// the commits merged Safe Pushes landed as the view names them (`landed_commits`) and the target
  /// (`prepare_robot_start`), then the start
  #[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, stack = params.stack, mode = ?params.mode))]
  pub async fn start_robot(&self, params: &StartPatronusRobotParams) -> Result<PatronusRobot> {
    let (service, repository) = self.patronus()?;
    let (git, gate, start, prefix) = (self.git_executor.clone(), Arc::clone(self.gate), params.clone(), self.branch_prefix.to_owned());
    let branches = self
      .with_facts(&params.repository_path, "Nothing was started", move |facts| {
        if let Err(refusal) = check_start(facts, &start_subject(&start.branch_name, start.stack), start.mode) {
          bail!(refusal);
        }
        prepare_robot_start(&git, &*gate, &prefix, &start, &landed_commits(&facts.view))
      })
      .await?;
    service
      .start(
        repository,
        self.branch_prefix,
        &branches.source_branch,
        &branches.target_branch,
        params.mode,
        params.profile_id.clone(),
        params.merge_request.clone(),
      )
      .await
      .map_err(|e| anyhow!(e))
  }

  /// The checks of a start from the pull request of the user's branch (the `/patronus` comment, `post_patronus_command`
  /// of both hosts), before the comment: those of `start_robot` before it asks Patronus, on one list of the user's
  /// robots: `check_start` (parked, landed whole, not built, a robot of the last commit or one that lands a part of it),
  /// then the landing gate with the commits the view names landed; the pull request names the target. A comment on a
  /// pull request of someone else (`ReviewedPullRequest`) has no branch of the table to check; an `OwnBranch` comment
  /// without its branch is refused
  #[instrument(skip_all, fields(repository_path = %params.repository_path, pull_request = params.pull_request_number, mode = ?params.mode))]
  pub async fn check_start_from_pull_request(&self, params: &PostPatronusCommandParams) -> Result<()> {
    if params.subject == PatronusCommandSubject::ReviewedPullRequest {
      return Ok(());
    }
    let Some(start) = &params.start else {
      bail!(
        "Nothing was started: the comment on pull request #{} names no branch of the user",
        params.pull_request_number
      );
    };
    let (gate, path, mode, prefix, start) = (
      Arc::clone(self.gate),
      params.repository_path.clone(),
      params.mode,
      self.branch_prefix.to_owned(),
      start.clone(),
    );
    self
      .with_facts(&params.repository_path, "Nothing was started", move |facts| {
        if let Err(refusal) = check_start(facts, &start_subject(&start.branch_name, start.stack), mode) {
          bail!(refusal);
        }
        let check = LandingGateParams {
          repository_path: path,
          branch_name: start.branch_name,
          stack: start.stack,
          allow: start.allow,
        };
        Ok(ensure_landing_starts(&check, gate.check(&prefix, &check, &landed_commits(&facts.view))?)?)
      })
      .await
  }

  /// The landing gate as the popover asks it before a start (`check_landing_gate`), with the commits merged Safe Pushes
  /// landed as the view names them (`landed_commits`) on the robots Rust reads now (`robots_for_landing_check`: the list
  /// of a robot action, the same input the start gives its own gate, `start_robot`): the page's robots never decide it.
  /// A list Patronus doesn't give fails the check instead of counting nothing landed, which would refuse a start whose
  /// need a merged Safe Push landed; the page then says "Not checked whether …" and the start checks again. Off Space or
  /// before the first sync nothing landed
  #[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, stack = params.stack))]
  pub async fn check_landing(&self, params: &LandingGateParams) -> Result<LandingCheck> {
    let robots = self.robots_for_landing_check().await?;
    let table = self.table(&params.repository_path).await;
    let (git, gate, params, prefix) = (self.git_executor.clone(), Arc::clone(self.gate), params.clone(), self.branch_prefix.to_owned());
    tokio::task::spawn_blocking(in_current_span(move || {
      let landed: HashSet<String> = table.map_or_default(|table| landed_commits(&guard_facts(&git, &params.repository_path, &table, robots).view));
      gate.check(&prefix, &params, &landed)
    }))
    .await?
  }

  /// Restarts the robot of `params` as the app does: the checks (`check_restart`), then the restart
  #[instrument(skip_all, fields(repository_path = %params.repository_path, robot_id = %params.robot_id))]
  pub async fn restart_robot(&self, params: &PatronusRobotParams) -> Result<PatronusRobot> {
    let (service, _) = self.patronus()?;
    let robot_id = params.robot_id.clone();
    self
      .with_facts(&params.repository_path, "Nothing was restarted", move |facts| {
        check_restart(facts, &robot_id).map_err(|refusal| anyhow!(refusal))
      })
      .await?;
    service.restart(self.branch_prefix, &params.robot_id).await.map_err(|e| anyhow!(e))
  }

  /// Cancels the robot of `params` as the app does: one list of the user's robots (`check_cancel`), then the cancel; no
  /// table: a cancel changes no branch. The robot a start or a push replaces (`replaced`) that finished meanwhile is
  /// answered as listed, and Patronus is not asked
  #[instrument(skip_all, fields(repository_path = %params.repository_path, robot_id = %params.robot_id, replaced = params.replaced))]
  pub async fn cancel_robot(&self, params: &PatronusRobotParams) -> Result<PatronusRobot> {
    let (service, _) = self.patronus()?;
    let robots = self.robots_for_action().await?;
    match check_cancel(&robots, &params.robot_id, params.replaced).map_err(|refusal| anyhow!(refusal))? {
      CancelCheck::AlreadyFinished(robot) => Ok(*robot),
      CancelCheck::Cancel => service.cancel(self.branch_prefix, &params.robot_id).await.map_err(|e| anyhow!(e)),
    }
  }
}

#[cfg(test)]
#[path = "start_guard_test.rs"]
mod start_guard_test;
