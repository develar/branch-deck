//! The restart and the cancel of a robot of the user (`Intent::RestartRobot`, `Intent::CancelRobot`;
//! spec/branch/patronus.spec.md, "Restart and cancel"), one family of the engine for every surface of the app: the
//! robot menu, its details, the robots indicator, a toast, the hold of a push.
//!
//! - The plan of a restart is the restart the view offers (`PatronusView.restarts` by robot id, one rule:
//!   `domain_policy::holds::restart_action`, refused by `domain_policy::land::restart_refusal`): a plain restart while
//!   the remote has the branch (`[RestartRobot]`), else its push first (`[Retarget, CancelRobot, Push, RestartRobot]`:
//!   the pull request of the branch into a virtual branch goes into the target, a running robot is canceled before the
//!   push into its branch). A push that drops commits of the remote the branch lacks refuses it (`DropsRemote`): the
//!   push button asks first. It runs as a start runs (`land_execute::run_planned`): the witness and the lock only for
//!   the refs its push moves and leases, the restart last, skipped when its pushed branch moved since or a robot of its
//!   subject started meanwhile.
//! - The plan of a cancel is the cancel (`[CancelRobot]`); a robot that finished meanwhile (as the facts of the session
//!   or its status, one request, say) is as it ended: nothing is canceled, and the outcome says so
//!   (`RobotResult::already_finished`). The cancel of the robot a start or a restart replaces is the same (`cancel`).
//!
//! Every robot an operation restarted or canceled goes into the facts of the session, so every window shows it at once;
//! the robots it decides on are the session's, asked no list of its own (C2). It holds the robot (`Resource::Robot`):
//! a restart and a cancel of one robot run one after the other, a second click of the same joins it; a restart holds
//! what its robot lands too (`Resource::Lands`) and the ref it pushes (`Resource::Ref`).

use crate::engine::{check_admitted, outcome_of, run, run_planned_in};
use crate::events::EventSink;
use crate::land_execute::{Accepted, Planned, PushLease, PushedRef};
use crate::land_session::session_start;
use crate::registry::{Answer, OperationRequest, Registry};
use crate::robot_start::patronus_failure;
use crate::witness::Witness;
use domain_policy::holds::is_active;
use domain_policy::pull_request::base_of;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{PlannedEffect, Subject};
use domain_types::operation::{
  EffectRecord, EffectStatus, Fingerprint, Intent, NewsKey, OperationId, OperationResult, OperationStep, Outcome, Plan, Resource, RobotResult, RunningMove,
};
use domain_types::patronus::PatronusRobot;
use domain_types::pull_request::{PullRequestBase, PullRequestState};
use domain_types::reason::Reason;
use domain_types::repository_view::RestartAction;
use domain_types::session::Version;
use patronus::service::PatronusService;
use remote_facts::service_sources::ServiceHost;
use repository_model::derive::{LandSources, with_land_sources};
use repository_model::facts::listed_robots;
use repository_model::session::{SessionKey, Sessions};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::{debug, instrument};

/// How a cancel went: the robot as Patronus answered it (canceling), or as it ended before the cancel
#[derive(Debug, Clone)]
pub struct Canceled {
  pub robot: PatronusRobot,
  /// It had finished: nothing was canceled
  pub already_finished: bool,
  /// The version of the view after the robot went into the facts; `None` when nothing was written
  pub version: Option<Version>,
}

/// Cancels `robot_id` of the user: one that finished as `robots` (the facts of the session) have it, or as its status
/// (one request) says since, is as it ended, and Patronus is asked nothing more; a status that can't be read asks for
/// the cancel all the same. The robot Patronus answered, or the finished one with its status, goes into the facts
/// (`write`)
///
/// # Errors
/// What Patronus answered the cancel
pub async fn cancel<W, F>(patronus: &PatronusService, prefix: &str, robots: &[PatronusRobot], robot_id: &str, write: &W) -> Result<Canceled, ApiError>
where
  W: Fn(PatronusRobot) -> F + Sync,
  F: Future<Output = Option<Version>> + Send,
{
  let listed = robots.iter().find(|robot| robot.id == robot_id);
  if let Some(finished) = listed.filter(|robot| !is_active(robot)) {
    debug!(robot_id, "the robot to cancel had finished");
    return Ok(Canceled {
      robot: finished.clone(),
      already_finished: true,
      version: None,
    });
  }
  if let Some(listed) = listed
    && let Ok(status) = patronus.robot_status(robot_id).await
    && status.is_finished()
  {
    debug!(robot_id, ?status, "the robot to cancel finished since it was listed");
    let finished = PatronusRobot { status, ..listed.clone() };
    let version = write(finished.clone()).await;
    return Ok(Canceled {
      robot: finished,
      already_finished: true,
      version,
    });
  }
  let robot = patronus.cancel_robot(prefix, robot_id).await.map_err(patronus_failure)?;
  let version = write(robot.clone()).await;
  Ok(Canceled {
    robot,
    already_finished: false,
    version,
  })
}

/// The plan of the restart of `robot_id` over `sources`, with what its run needs (module docs); `None` for a robot the
/// user's robots lack, or one the view offers no restart of (`PatronusView.restarts`: a robot of a temporary ref, one
/// that runs on the commit its branch has, a merged one)
#[must_use]
pub fn restart_planned(sources: &LandSources<'_>, robot_id: &str) -> Option<Planned> {
  let robot = sources.robots.iter().find(|robot| robot.id == robot_id)?;
  let restart = sources.view.restarts.get(robot_id)?;
  let subject = Subject::of_robot(robot)?;
  let mut blockers: Vec<Reason> = restart.refusal.iter().cloned().collect();
  let mut effects = Vec::new();
  let mut pushes = Vec::new();
  let mut refs = BTreeMap::new();
  let mut pull_requests = Vec::new();
  if restart.action == RestartAction::PushAndRestart {
    let (pushed, tip, remote) = match &subject {
      Subject::Branch { name } => {
        let pushed = PushedRef::branch(sources.table, sources.prefix, name, PushLease::Frozen)?;
        let tip = sources.table.tips.get(&pushed.full_name).cloned();
        (pushed, tip, sources.remote.branches.get(name))
      }
      Subject::WholeStack { root } => {
        let stack = sources.table.stack_branches.iter().find(|stack| stack.root == *root)?;
        let pushed = PushedRef::stack_branch(stack, PushLease::Frozen);
        (pushed, stack.tip.clone(), sources.remote.stacks.get(root))
      }
    };
    // the pull request of the branch goes into the target before the push, where every pull request of a virtual
    // branch goes (D3a)
    if let Subject::Branch { name } = &subject
      && let Some(pull_request) = sources
        .pull_requests
        .pull_requests
        .get(name)
        .filter(|pull_request| pull_request.state == PullRequestState::Open)
      && matches!(base_of(pull_request, sources.target, sources.prefix), PullRequestBase::Virtual { .. })
    {
      effects.push(PlannedEffect::Retarget {
        branch: name.clone(),
        number: pull_request.number,
        from: pull_request.base_ref_name.clone(),
        to: sources.target.to_string(),
      });
      pull_requests.push(pull_request);
    }
    // a running robot of an older commit: the push into its branch fails it at the end, so it is canceled first
    if is_active(robot) {
      effects.push(PlannedEffect::CancelRobot { robot_id: robot_id.to_string() });
    }
    if let Some(drops) = remote.map(|remote| remote.drops).filter(|drops| *drops > 0) {
      blockers.push(Reason::DropsRemote {
        branch: subject.name().to_string(),
        behind: drops,
      });
    }
    effects.push(PlannedEffect::Push {
      branches: vec![subject.name().to_string()],
      force: false,
    });
    refs.insert(format!("refs/heads/{}", pushed.full_name), tip);
    refs.insert(format!("refs/remotes/origin/{}", pushed.full_name), remote.and_then(|remote| remote.remote_tip.clone()));
    pushes.push(pushed);
  }
  effects.push(PlannedEffect::RestartRobot { robot_id: robot_id.to_string() });
  let lands = match &subject {
    Subject::Branch { name } => vec![name.clone()],
    Subject::WholeStack { root } => sources
      .table
      .stack_branches
      .iter()
      .find(|stack| stack.root == *root)
      .map_or_else(|| vec![root.clone()], |stack| stack.members.clone()),
  };
  let witness = Witness::of(refs, pull_requests, std::iter::empty::<&PatronusRobot>());
  let mut plan = Plan {
    intent: Intent::RestartRobot { robot_id: robot_id.to_string() },
    fingerprint: Fingerprint(String::new()),
    lands,
    effects,
    confirmations: Vec::new(),
    blockers,
    waits: Vec::new(),
    alternatives: None,
    pull_request: None,
    approvals_skipped_by: None,
  };
  plan.fingerprint = witness.fingerprint(&plan);
  Some(Planned {
    plan,
    witness,
    pushes,
    source_ref: String::new(),
    merge_request: None,
    notes: Vec::new(),
    subject,
    mode: robot.push_mode,
  })
}

/// The plan of the cancel of `robot_id` among `robots`, the user's robots of the session; `None` for one they lack. It
/// cancels whatever the robot does by then: one that finished is as it ended (`cancel`), so its fingerprint is the plan
/// alone
#[must_use]
pub fn cancel_plan(robots: &[PatronusRobot], robot_id: &str) -> Option<Plan> {
  robots.iter().find(|robot| robot.id == robot_id)?;
  let mut plan = Plan {
    intent: Intent::CancelRobot { robot_id: robot_id.to_string() },
    fingerprint: Fingerprint(String::new()),
    lands: Vec::new(),
    effects: vec![PlannedEffect::CancelRobot { robot_id: robot_id.to_string() }],
    confirmations: Vec::new(),
    blockers: Vec::new(),
    waits: Vec::new(),
    alternatives: None,
    pull_request: None,
    approvals_skipped_by: None,
  };
  plan.fingerprint = Witness::default().fingerprint(&plan);
  Some(plan)
}

/// The plan of the restart of `robot_id` in the open session of `key`, over the facts it holds now (`restart_planned`);
/// `None` before its Patronus view, or for a robot it offers no restart of
pub async fn prepare_restart(sessions: &Sessions, key: &SessionKey, robot_id: &str) -> Option<Planned> {
  let (prefix, robot_id) = (key.prefix.clone(), robot_id.to_string());
  sessions
    .read(key, move |state, facts| {
      with_land_sources(state, facts, &prefix, |sources| restart_planned(sources, &robot_id)).flatten()
    })
    .await
    .flatten()
}

/// The plan of the cancel of `robot_id` in the open session of `key` (`cancel_plan`) and the user's robots it holds
pub async fn prepare_cancel(sessions: &Sessions, key: &SessionKey, robot_id: &str) -> Option<(Plan, Vec<PatronusRobot>)> {
  let robot_id = robot_id.to_string();
  sessions
    .read(key, move |_, facts| {
      let robots = listed_robots(facts.robots.view());
      cancel_plan(&robots, &robot_id).map(|plan| (plan, robots))
    })
    .await
    .flatten()
}

/// What a restart asks the registry for: its robot, what the robot lands and the ref its push writes (of `planned`, its
/// plan as the session has it now), the move of the row of its subject from the click on
pub(crate) fn restart_request(robot_id: &str, planned: Option<&Planned>) -> OperationRequest {
  let mut resources = vec![Resource::Robot { id: robot_id.to_string() }];
  if let Some(planned) = planned {
    resources.extend(planned.plan.lands.iter().map(|branch| Resource::Lands { branch: branch.clone() }));
    resources.extend(planned.pushes.iter().map(|pushed| Resource::Ref { name: pushed.full_name.clone() }));
  }
  OperationRequest {
    intent: Intent::RestartRobot { robot_id: robot_id.to_string() },
    resources,
    step: OperationStep::Checking,
    moves: planned
      .iter()
      .map(|planned| RunningMove::Restarting {
        subject: planned.subject.key(),
        mode: planned.mode,
      })
      .collect(),
  }
}

/// What a cancel asks the registry for: its robot
pub(crate) fn cancel_request(robot_id: &str) -> OperationRequest {
  OperationRequest {
    intent: Intent::CancelRobot { robot_id: robot_id.to_string() },
    resources: vec![Resource::Robot { id: robot_id.to_string() }],
    step: OperationStep::CancelingRobot,
    moves: Vec::new(),
  }
}

/// The robot no plan of the session has: "Nothing was restarted: the session offers no restart of robot r1"
fn unknown_robot(robot_id: &str, done: &str, action: &str) -> ApiError {
  ApiError::new(
    ErrorCode::InvalidArguments,
    format!("Nothing was {done}: the session offers no {action} of robot {robot_id}"),
  )
}

/// Runs the restart of `robot_id` in the session of `key` on the plan its caller accepted: admitted over the operations
/// of the session with what its plan as the session has it now holds (the robot, what it lands, the ref of its push;
/// a second restart of the robot joins it), then planned again and run as a start runs (`land_execute::run_planned`),
/// over the services of `services` (`session_start`), its steps in the view and its outcome to `events`
///
/// # Errors
/// What refuses it before its first effect, a robot the session offers no restart of, a session that closed
#[instrument(skip_all, fields(robot_id))]
pub async fn execute_restart(registry: &Registry, key: &SessionKey, robot_id: &str, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  let planned = prepare_restart(sessions, key, robot_id).await;
  let request = restart_request(robot_id, planned.as_ref());
  run(registry, key, request, events, async |guard| {
    // decided again once admitted: an operation it waited for changed what it decides on
    let planned = prepare_restart(sessions, key, robot_id)
      .await
      .ok_or_else(|| unknown_robot(robot_id, "restarted", "restart"))?;
    run_planned_in(guard, session_start(services, sessions, key).await?, planned, &accepted, guard.queued()).await
  })
  .await
}

/// Runs the cancel of `robot_id` in the session of `key` on the plan its caller accepted: admitted over the operations
/// of the session (it holds the robot: a restart of it runs first, a second cancel joins it), then the cancel (`cancel`:
/// a robot that finished is as it ended) with the Patronus service of `services`; the robot into the session. A cancel
/// Patronus refused is the status of its effect in the outcome
///
/// # Errors
/// A robot the session lacks, a changed plan, a session that closed
#[instrument(skip_all, fields(robot_id))]
pub async fn execute_cancel(registry: &Registry, key: &SessionKey, robot_id: &str, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  run(registry, key, cancel_request(robot_id), events, async |guard| {
    let (plan, robots) = prepare_cancel(sessions, key, robot_id).await.ok_or_else(|| unknown_robot(robot_id, "canceled", "cancel"))?;
    // after the restart of the robot it waited for, the robot as that left it
    check_admitted(guard, &accepted, &plan)?;
    let patronus = services.patronus(key)?;
    let write = |robot: PatronusRobot| sessions.write_robot(key, robot);
    let canceled = cancel(&patronus, &key.prefix, &robots, robot_id, &write).await;
    Ok(cancel_outcome(guard.id().clone(), plan, canceled))
  })
  .await
}

/// The outcome of a cancel: its one effect with how it went, the robot as it is now
fn cancel_outcome(op: OperationId, plan: Plan, canceled: Result<Canceled, ApiError>) -> Outcome {
  let robot_id = match &plan.intent {
    Intent::CancelRobot { robot_id } => robot_id.clone(),
    _ => String::new(),
  };
  let (status, result, version) = match canceled {
    Ok(canceled) => (
      EffectStatus::Done,
      RobotResult {
        robot: Some(canceled.robot),
        canceled: None,
        already_finished: canceled.already_finished,
      },
      canceled.version,
    ),
    Err(error) => (EffectStatus::Failed { error }, RobotResult::default(), None),
  };
  let effects = plan.effects.into_iter().map(|effect| EffectRecord { effect, status: status.clone() }).collect();
  outcome_of(op, effects, Some(NewsKey::Robot { robot_id }), version, OperationResult::Robot(Box::new(result)))
}

#[cfg(test)]
#[path = "robot_test.rs"]
pub(crate) mod robot_test;
