//! What `bd push` decides of the branches its intents cover besides their plans, and how it words what it refuses and
//! leaves out (docs/design/git-operations.md, "Agent push"). Each covered branch does what the rule of the push says
//! (`domain_policy::push::push_plan` over `derive::with_push_states`, `derive::alone_push_of`; the stack branch of a
//! whole stack by `stack_branch_along`): the verdicts the plans of `operations::push` are made of. Where the app's push
//! leaves a covered branch out to its row, `bd push` refuses the whole push, nothing pushed: a branch the sync didn't
//! build (`push_refused`), one a running Safe Push holds (`robot_running`: the branch the push moves, or the commits the
//! robot lands), one whose push drops remote commits it lacks without `--force` (`push_refused`, with its remote status
//! and the leased push that replaces it), a named parked branch (`parked`). A branch a merged Safe Push landed whole
//! and, with `--all` or `--with-children`, a parked one are left out (`skipped`); the push is refused as `parked` or
//! `landed` only when nothing else is left. An ancestor taken along that a robot holds, that landed or that the remote
//! has as it is stays as it is: its child carries its commits. A stack branch `--with-children` can't push is a warning.

use crate::landed::{LandedBySummary, landed_branches};
use crate::push_command::{LANDED_NEXT, PushRun, SkippedBranch, not_synced_reason};
use bd_core::bd_command::shell_word;
use bd_core::context::RepoContext;
use bd_core::error::{CliError, ErrorBranch, NextStep};
use bd_core::sync_command::RemoteSummary;
use domain_policy::push::{PushScope as PlanScope, Verdict, push_plan};
use domain_types::branch_names::{BranchKind, qualified_branch_name, to_final_branch_name};
use domain_types::error::ErrorCode;
use domain_types::landing::Subject;
use domain_types::operation::{Push, PushScope};
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::reason::Reason;
use domain_types::repository_view::HoldKind;
use domain_types::session::StackBranchAlong;
use domain_types::stack::StackBranchState;
use operations::land_execute::Planned;
use operations::push::{ExpectRemote, push_planned};
use repository_model::derive::{Holds, alone_push_of, with_push_states};
use repository_model::one_shot::OneShot;
use repository_model::push_guard::{every_branch_landed, own_hold_words, span_hold_words, stack_hold_words};
use repository_model::refusal_words::start_refusal_text;
use sync_core::patronus_start::push_mode_name;

/// How a branch is in the push of a named one; a branch two intents cover counts as the strongest
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
  /// A branch it is built on, taken along while it has work (Push Stack)
  Along,
  /// A branch stacked on it (`--with-children`)
  Child,
  /// The named branch itself, every branch with `--all`
  Named,
}

/// A branch an intent covers: what it does in the push, and why a verdict that keeps it out does
struct Step {
  name: String,
  verdict: Verdict,
  reason: Option<Reason>,
  role: Role,
}

/// What the push says of the branches its intents cover besides their plans
#[derive(Default)]
pub(crate) struct Covered {
  /// The named branches and the children `--with-children` covers that the remote has as they are: nothing to push,
  /// listed `up_to_date`
  pub(crate) up_to_date: Vec<String>,
  pub(crate) skipped: Vec<SkippedBranch>,
  pub(crate) stack_warnings: Vec<String>,
}

/// The plans `bd push` runs as one push, none of them refused, and what it says of the rest
pub(crate) struct Decided {
  pub(crate) plans: Vec<Planned>,
  pub(crate) covered: Covered,
}

/// What refuses the push, by kind, in the order the error names them
#[derive(Default)]
struct Refusals {
  parked: Vec<String>,
  held: Vec<String>,
  held_steps: Vec<NextStep>,
  refused: Vec<String>,
  refused_steps: Vec<NextStep>,
  behind: Vec<ErrorBranch>,
}

/// The plans of `intents` over the one-shot `session` of the run, and what the push says of the branches they cover; an
/// error for what refuses the push (module docs)
pub(crate) fn decide(run: &PushRun<'_>, session: &OneShot, intents: &[Push]) -> Result<Decided, CliError> {
  let expect = if run.args.expect_remote.is_some() {
    ExpectRemote::OfIntent
  } else {
    ExpectRemote::OfSession
  };
  let holds = Holds {
    view: session.view(),
    robots: session.robots(),
  };
  let mut plans = Vec::new();
  let mut steps: Vec<Step> = Vec::new();
  let mut refusals = Refusals::default();
  let mut covered = Covered::default();
  let mut goes = false;
  for push in intents {
    let plan = |push: &Push| push_planned(session.state(), session.facts(), &run.context.prefix, push, None, expect).ok_or_else(|| no_plan(run.context, push));
    let mut planned = plan(push)?;
    let (covers, along) = steps_of(session, holds, push, &planned);
    for step in covers {
      match steps.iter_mut().find(|known| known.name == step.name) {
        Some(known) => known.role = known.role.max(step.role),
        None => steps.push(step),
      }
    }
    let root = push.subject.name();
    match along {
      None | Some(StackBranchAlong::Unpublished) => {}
      Some(StackBranchAlong::Held { reason } | StackBranchAlong::NotBuilt { reason }) => covered.stack_warnings.push(stack_warning(run, session, root, &reason)),
      // its remote status is of `origin`, which the sync compares with
      Some(_) if run.remote != crate::safe_push_command::STACK_REMOTE => {
        let stack = stack_branch_name(run, root);
        covered
          .stack_warnings
          .push(format!("{stack} was not pushed: its remote status is of origin, not of {}", run.remote));
        planned.pushes.retain(|pushed| !pushed.whole_stack);
      }
      Some(StackBranchAlong::UpToDate) => {}
      Some(StackBranchAlong::Along) => goes = true,
      Some(StackBranchAlong::DropsRemote) if run.args.force => {
        // the push of a stack sends a stack branch that drops remote commits only alone, as the push a refused start offers
        let whole_stack = Push {
          subject: Subject::whole_stack(root),
          scope: PushScope::Alone,
          expect_remote: None,
          force: true,
          restart_running: false,
        };
        plans.push(plan(&whole_stack)?);
        goes = true;
      }
      Some(StackBranchAlong::DropsRemote) => refusals.stack_drops(run, root),
    }
    plans.push(planned);
  }
  let mut landed: Vec<(String, Role, String)> = Vec::new();
  let mut parked_skipped: Vec<(String, String)> = Vec::new();
  for Step { name, verdict, reason, role } in &steps {
    let words = || reason.as_ref().map(|reason| start_refusal_text(reason, name, session.robots()));
    match (verdict, role) {
      (Verdict::Goes, _) => goes = true,
      (Verdict::AsksFirst, _) if run.args.force => goes = true,
      (Verdict::AsksFirst, role) => refusals.drops(run, session, name, *role == Role::Along),
      // what a branch built on it carries stays as it is: held, landed, on the remote as it is, or no build of its own
      (_, Role::Along) => {}
      (Verdict::UpToDate, _) => covered.up_to_date.push(name.clone()),
      (Verdict::NotBuilt, _) => refusals.not_synced(run, name, reason.as_ref(), session.robots()),
      (Verdict::Held, _) => refusals.held(run.context, session, name, reason.as_ref()),
      (Verdict::Landed, role) => {
        let words = words().unwrap_or_else(|| format!("{name} is in {}: nothing of it is left to push", run.synced.baseline));
        landed.push((name.clone(), *role, words));
      }
      (Verdict::Parked, role) => {
        let words = words().unwrap_or_else(|| format!("{name} is parked"));
        if *role == Role::Named && !run.args.all {
          refusals.parked.push(words);
        } else {
          parked_skipped.push((name.clone(), words));
        }
      }
    }
  }
  let left = goes || !covered.up_to_date.is_empty();
  refusals.refuse(run.context, left, &landed, &parked_skipped)?;
  covered.skipped = skipped(run.context, session, landed, parked_skipped);
  // what bd skips the plans of refuse nothing: they send nothing
  plans.retain(|planned| planned.plan.blockers.is_empty());
  Ok(Decided { plans, covered })
}

/// What `push` covers, by its scope, and of Push with Children of a root what goes of its stack branch
fn steps_of(session: &OneShot, holds: Holds<'_>, push: &Push, planned: &Planned) -> (Vec<Step>, Option<StackBranchAlong>) {
  let (state, facts) = (session.state(), session.facts());
  let name = push.subject.name();
  let step = |verdict: Verdict, reason: Option<Reason>| Step {
    name: name.to_string(),
    verdict,
    reason,
    role: Role::Named,
  };
  match &push.scope {
    PushScope::Alone => {
      // the Safe Push the push restarts keeps nothing out (`alone_planned`)
      let replaces = session
        .branch_robot(name)
        .filter(|robot| push.restart_running && robot.status == PatronusRobotStatus::Running)
        .map(|robot| robot.id.as_str());
      let alone = alone_push_of(state, facts, holds, name, replaces);
      (alone.map(|alone| step(alone.verdict, alone.blocker)).into_iter().collect(), None)
    }
    PushScope::WithAncestors | PushScope::WithChildren => with_push_states(state, facts, holds, |rules| {
      let (scope, others) = if push.scope == PushScope::WithAncestors {
        (PlanScope::WithAncestors, Role::Along)
      } else {
        (PlanScope::WithChildren, Role::Child)
      };
      let steps = push_plan(&rules.repo, |branch| rules.state(branch), name, scope)
        .steps
        .iter()
        .map(|&(branch, verdict)| Step {
          name: branch.to_string(),
          verdict,
          reason: rules.refusal(branch, verdict),
          role: if branch == name { Role::Named } else { others },
        })
        .collect();
      let along = (push.scope == PushScope::WithChildren).then(|| rules.stack_branch_along(name)).flatten();
      (steps, along)
    }),
    PushScope::Unapplied => {
      let verdict = if !planned.plan.confirmations.is_empty() {
        Verdict::AsksFirst
      } else if planned.pushes.is_empty() {
        Verdict::UpToDate
      } else {
        Verdict::Goes
      };
      (vec![step(verdict, None)], None)
    }
    PushScope::UpTo { .. } => (Vec::new(), None),
  }
}

impl Refusals {
  /// `name` (taken `along` by a branch built on it) drops remote commits it lacks: the leased push the sync names for it
  /// replaces them once the user saw them, or `--alone` pushes without it
  fn drops(&mut self, run: &PushRun<'_>, session: &OneShot, name: &str, along: bool) {
    let option = run.option();
    let remote = run.remotes.get(name).cloned().or_else(|| unapplied_remote(run, session, name));
    let behind = remote.as_ref().map_or(0, |remote| remote.behind);
    let alone = if along { ", or --alone to push the named branches without it" } else { "" };
    self.refused.push(format!(
      "{name}: the remote branch has {behind} commits the branch doesn't (`dropped_commits`); show them to the user, and once the user approved, --force replaces them{alone}"
    ));
    let synced = run.synced.branches.iter().find(|branch| branch.name == name);
    let replace = synced
      .and_then(|branch| branch.push.clone())
      .filter(|_| option.is_empty())
      .unwrap_or_else(|| format!("{} push {} --force{option}", run.context.bd, shell_word(name)));
    self.refused_steps.push(NextStep::new(replace, true));
    if let Some(remote) = remote {
      self.behind.push(ErrorBranch { name: name.to_string(), remote });
    }
  }

  /// The stack branch of `root` drops remote commits it lacks: `--with-children --force` replaces them
  fn stack_drops(&mut self, run: &PushRun<'_>, root: &str) {
    let Some(stack) = run.synced.report.stack_branches.iter().find(|stack| stack.root == root) else {
      return;
    };
    let Some(remote) = &stack.remote else {
      return;
    };
    self.refused.push(format!(
      "{}: the remote branch has {} commits the stack branch doesn't (`dropped_commits`); show them to the user, and once the user approved, --force replaces them",
      stack.branch_name, remote.commits_behind
    ));
    self.refused_steps.push(NextStep::new(
      format!("{} push {} --with-children --force{}", run.context.bd, shell_word(root), run.option()),
      true,
    ));
    let short = run.context.short_ids(RemoteSummary::ids(remote));
    self.behind.push(ErrorBranch {
      name: stack.branch_name.clone(),
      remote: RemoteSummary::new(remote, usize::try_from(stack.commit_count).unwrap_or(usize::MAX), &short),
    });
  }

  /// The sync didn't build `name`: its error or conflict as the sync reported them
  fn not_synced(&mut self, run: &PushRun<'_>, name: &str, reason: Option<&Reason>, robots: &[PatronusRobot]) {
    let synced = run.synced.branches.iter().find(|branch| branch.name == name);
    let words = match synced.and_then(|branch| not_synced_reason(&run.synced.report, branch)) {
      Some(why) => format!("{name} is not synced ({why}): fix it first, `bd sync` says what holds it"),
      None => reason.map_or_else(|| format!("{name} is not synced"), |reason| start_refusal_text(reason, name, robots)),
    };
    self.refused.push(words);
    self.refused_steps.push(NextStep::new(format!("{} sync", run.context.bd), false));
  }

  /// A running Safe Push holds `name` (`reason`): its own, which a push into the branch it took its commit from fails,
  /// or the robot of a branch built on it or of its whole stack, which lands its commits as it took them
  fn held(&mut self, context: &RepoContext, session: &OneShot, name: &str, reason: Option<&Reason>) {
    let Some(reason @ Reason::Held { hold, .. }) = reason else {
      self
        .held
        .push(reason.map_or_else(|| format!("a running Safe Push holds {name}"), |reason| start_refusal_text(reason, name, session.robots())));
      return;
    };
    let by = hold.by.subject();
    let words = match session.robot(&hold.by.robot_id) {
      Some(robot) if hold.kind == HoldKind::Own => {
        let source = to_final_branch_name(&context.prefix, name).ok();
        let remedy = "once the user approved, --restart-running cancels it, pushes and restarts it";
        own_hold_words(name, robot, source.as_deref(), remedy)
      }
      Some(robot) => span_hold_words(name, &by, robot, hold.carried.as_ref()),
      None => start_refusal_text(reason, name, session.robots()),
    };
    self.held.push(words);
    self.held_steps.push(NextStep::new(robot_command(context, &by), false));
    if hold.kind == HoldKind::Own {
      self
        .held_steps
        .push(NextStep::new(format!("{} push {} --restart-running", context.bd, shell_word(name)), true));
    }
  }

  /// The error of the refusals, in their order: a named parked branch; nothing left but parked branches; nothing left but
  /// what merged Safe Pushes landed (the named ones say it), before any running robot; a running Safe Push; a branch not
  /// synced or behind its remote. `left`: something goes, or is listed up to date
  fn refuse(self, context: &RepoContext, left: bool, landed: &[(String, Role, String)], parked: &[(String, String)]) -> Result<(), CliError> {
    if !self.parked.is_empty() {
      return Err(CliError::new(
        ErrorCode::Parked,
        format!("Nothing was pushed: {}; it stays local until the user unparks it", self.parked.join("; ")),
      ));
    }
    let only = !left && self.held.is_empty() && self.refused.is_empty();
    if only && landed.is_empty() && !parked.is_empty() {
      let reasons: Vec<&str> = parked.iter().map(|(_, words)| words.as_str()).collect();
      return Err(CliError::new(
        ErrorCode::Parked,
        format!("Nothing was pushed: {}; tell the user, never unpark them yourself", reasons.join("; ")),
      ));
    }
    if only && !landed.is_empty() {
      let named: Vec<&str> = landed.iter().filter(|(_, role, _)| *role == Role::Named).map(|(.., words)| words.as_str()).collect();
      let reasons = if named.is_empty() {
        landed.iter().map(|(.., words)| words.as_str()).collect()
      } else {
        named
      };
      let error = CliError::new(ErrorCode::Landed, format!("Nothing was pushed: {}; {LANDED_NEXT}", reasons.join("; ")));
      return Err(error.with_next_step(format!("{} update", context.bd), true));
    }
    if !self.held.is_empty() {
      let mut error = CliError::new(ErrorCode::RobotRunning, format!("Nothing was pushed: {}", self.held.join("; ")));
      for step in self.held_steps {
        error.push_next_step(step);
      }
      return Err(error);
    }
    if !self.refused.is_empty() {
      let mut error = CliError::new(ErrorCode::PushRefused, format!("Nothing was pushed: {}", self.refused.join("; ")));
      error.branches = self.behind;
      for step in self.refused_steps {
        error.push_next_step(step);
      }
      return Err(error);
    }
    Ok(())
  }
}

/// The remote status of the unapplied branch `name` a push drops remote commits of, as the sync listed it
fn unapplied_remote(run: &PushRun<'_>, session: &OneShot, name: &str) -> Option<RemoteSummary> {
  let branch = session.state().sections().unapplied.iter().find(|branch| branch.branch == name)?;
  let remote = branch.remote.as_ref()?;
  let short = run.context.short_ids(RemoteSummary::ids(remote));
  Some(RemoteSummary::new(remote, remote.unpushed_commits.len(), &short))
}

/// `{prefix}/stack/<root>`, as the sync named the stack branch of `root`
fn stack_branch_name(run: &PushRun<'_>, root: &str) -> String {
  let stack = run.synced.report.stack_branches.iter().find(|stack| stack.root == root);
  stack.map_or_else(|| format!("the stack branch of {root}"), |stack| stack.branch_name.clone())
}

/// Why the stack branch of `root` is not pushed (`hold`), as `bd` says it: "this sync didn't build it (conflict; …)", "it
/// holds parked login; …", "every branch of the whole auth stack landed in master with …: nothing of it is left to
/// push", "a Safe Push of the whole auth stack is running on it: …" (`stack_hold_words`)
fn stack_warning(run: &PushRun<'_>, session: &OneShot, root: &str, hold: &Reason) -> String {
  let what = Subject::whole_stack(root).words();
  let why = match hold {
    Reason::StackNotBuilt { state, .. } => format!("this sync didn't build it ({}; `stacks` of `bd sync` says why)", state_word(*state)),
    Reason::StackParked { member, .. } => format!("it holds parked {member}; land the stack in pieces"),
    Reason::StackLanded { by, .. } => {
      let landed: Vec<(Subject, &PatronusRobot)> = by.iter().filter_map(|of| Some((of.subject(), session.robot(&of.robot_id)?))).collect();
      format!("{}: nothing of it is left to push", every_branch_landed(&what, &landed))
    }
    Reason::StackHeld { robot_id, mode, .. } => match session.robot(robot_id) {
      Some(robot) => stack_hold_words(root, robot),
      None => format!("a {} of {what} is running on it", push_mode_name(*mode)),
    },
    other => start_refusal_text(other, &what, session.robots()),
  };
  format!("{} was not pushed: {why}", stack_branch_name(run, root))
}

/// `blocked`, `conflict`, `stale`, as `stacks` names a state
const fn state_word(state: StackBranchState) -> &'static str {
  match state {
    StackBranchState::Built => "built",
    StackBranchState::Blocked => "blocked",
    StackBranchState::Conflict => "conflict",
    StackBranchState::Stale => "stale",
  }
}

/// The branches the push leaves out: what merged Safe Pushes landed whole, with who landed it (`landed_by` of `bd
/// status`), then the parked ones
fn skipped(context: &RepoContext, session: &OneShot, landed: Vec<(String, Role, String)>, parked: Vec<(String, String)>) -> Vec<SkippedBranch> {
  let mut by = landed_branches(session);
  let ids: Vec<String> = landed
    .iter()
    .filter_map(|(name, ..)| by.get(name))
    .flat_map(|landed| landed.landed.iter().chain(&landed.after).cloned())
    .collect();
  let short = context.short_ids(ids.iter().map(String::as_str));
  let landed = landed.into_iter().map(|(name, _, message)| {
    let landed_by = by.remove(&name).map(|landed| LandedBySummary::new(context, &landed, &short));
    SkippedBranch {
      name,
      code: ErrorCode::Landed,
      message,
      landed_by,
    }
  });
  let parked = parked.into_iter().map(|(name, message)| SkippedBranch {
    name,
    code: ErrorCode::Parked,
    message,
    landed_by: None,
  });
  landed.chain(parked).collect()
}

/// The push has no plan of `push` in the sync: no such branch, or no such unapplied branch
fn no_plan(context: &RepoContext, push: &Push) -> CliError {
  let name = push.subject.name();
  match push.scope {
    PushScope::Unapplied => {
      let full = qualified_branch_name(&context.prefix, BranchKind::Unapplied, name).unwrap_or_else(|_| name.to_string());
      CliError::new(ErrorCode::UnknownBranch, format!("No unapplied branch {full}: `unapplied` of `bd status` lists them"))
    }
    _ => CliError::new(ErrorCode::UnknownBranch, format!("No branch named {name:?}")),
  }
}

/// `bd robot <branch>`, or `bd robot <root> --whole-stack`: what follows the robot that runs on `subject`, a read
pub(crate) fn robot_command(context: &RepoContext, subject: &Subject) -> String {
  match subject {
    Subject::Branch { name } => format!("{} robot {}", context.bd, shell_word(name)),
    Subject::WholeStack { root } => format!("{} robot {} --whole-stack", context.bd, shell_word(root)),
  }
}
