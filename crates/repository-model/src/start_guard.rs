//! The checks of the app's restart and cancel of a robot (`restart_patronus_robot`, `cancel_patronus_robot` and the E2E
//! server's routes of them) in Rust, by the rules of `bd safe-push` over the Patronus view (`derive_patronus`) and the
//! robots Rust read itself, never the page's. A restart is a start of the robot's subject that the robot itself does not
//! refuse: a parked subject (a branch, a branch built on parked, a whole stack with a parked member) stays local, a
//! subject merged Safe Pushes landed whole has nothing left to land, a subject the last sync did not build (a stack
//! branch that is not `built`) is no build of its commits, and a robot that runs on its last commit or lands a part of
//! it refuses a second one (`domain_policy::land::start_refusal`, as the view decides it); a robot of a temporary ref
//! (the IDE, the CLI) is not restarted: it would test its old commit. A cancel needs a running robot of the user; the
//! cancel of the robot a push replaces answers one that finished meanwhile as it is (`CancelCheck::AlreadyFinished`).
//! The start of a Safe Push or a Dry Run is an operation of the engine (`operations::land_execute`), refused by its plan.
//!
//! `RemoteActions::restart_robot` and `cancel_robot` (`actions`) are the whole sequence of both hosts: one list of the
//! robots asked now (a failed list fails the action), the checks, then the call of Patronus. The words of the robot
//! refusals are `bd`'s and the engine's too (`start_refusal_text`).

use crate::actions::RemoteActions;
use crate::derive::LandingFacts;
use crate::push_guard::{GuardFacts, guard_facts, parked_words, row_refusal};
use crate::push_guard::{own_hold_words, span_hold_words};
use anyhow::{Result, anyhow};
use domain_policy::holds::is_active;
use domain_policy::land::{BranchStart, StackStart, Start, start_refusal as robot_refusal};
use domain_policy::parked::whole_stack_parked;
use domain_types::GroupedBranchInfo;
use domain_types::landing::Subject;
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotParams};
use domain_types::reason::Reason;
use domain_types::repository_view::HoldKind;
use domain_types::session::{Parked, ParkedMember};
use domain_types::stack::{BlockReason, StackBranchState, StackBranchUpdate};
use sync_core::patronus_start::{carried_ids, carried_words, join_names, push_mode_name};
use sync_core::push::not_synced_refusal;
use sync_utils::span::in_current_span;
use tracing::instrument;

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
      Some(robot) if hold.kind == HoldKind::Own => own_hold_words(subject, robot, None, "restart it on the current branch"),
      Some(robot) => span_hold_words(subject, &hold.by.subject(), robot, hold.carried.as_ref()),
      None => format!("a running Safe Push of {} lands {subject}: a push won't land", hold.by.subject().words()),
    },
    Reason::Blocked { by: Some(by), .. } => format!("{subject} is blocked by {by}: fix it first, then sync"),
    Reason::Blocked { by: None, .. } => format!("{subject} is blocked: fix its parent first, then sync"),
    Reason::Landed { .. } | Reason::StackLanded { .. } => format!("{subject} landed: nothing of it is left to land"),
    Reason::LandedIn { upstream } => format!("{subject} is in {upstream}: the update archives it"),
    Reason::PullRequestMerged { number } => format!("pull request #{number} of {subject} is merged"),
    Reason::NoStackBranch { .. } => format!("{subject} has no stack branch: the stack doesn't fork"),
    Reason::StackNotBuilt { root, .. } => format!("the stack branch of {root} is not the build of its commits; fix it, then sync"),
    Reason::StackParked { member, .. } => format!("{subject} holds parked {member}; land the stack in pieces"),
    Reason::StackHeld { mode, .. } => format!("a {} of {subject} is running on its stack branch", push_mode_name(*mode)),
    Reason::CantPush { branch, .. } => format!("{branch} is not synced"),
    Reason::DropsRemote { branch, behind } => format!("the remote branch of {branch} has {behind} commits {branch} lacks"),
    Reason::Parked { branch, with, note } => parked_words(
      branch,
      &Parked {
        note: note.clone(),
        with: with.clone(),
      },
    ),
    Reason::Unprotected { target } => format!("Patronus does not protect {target}: a robot can't be started into it"),
    Reason::Draft { number } => format!("Pull request #{number} is a draft: Patronus merges it once it is ready for review"),
    Reason::OtherBase { number, base, target } => {
      format!("pull request #{number} of {subject} goes into {base}, and Patronus merges a pull request into its base: its approvals count for no Safe Push into {target}")
    }
    Reason::ApprovalsMissing { groups } => format!(
      "The code owners haven't approved {subject}: a Safe Push fails without these approvals (needs review by {})",
      join_names(groups)
    ),
    Reason::DismissesApprovals { number, target } => format!(
      "the push dismisses the approvals of pull request #{number} that Patronus counts (the rules of {target} dismiss stale reviews on push), and the robot would fail without them"
    ),
    Reason::LandingRefused { refusal } => format!("{subject} would not build: {}", refusal.reason()),
    Reason::TargetConflict { files, .. } => format!("{subject} conflicts with the newest target in {}: Patronus would fail it at once", join_names(files)),
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
