//! The plan of a start of a Safe Push or a Dry Run (`Intent::Land`; docs/design/rust-domain-authority.md, "Safe Push
//! (приложение; `bd` совпадает с шага 2)", "Инварианты планов"): the decision of `domain_policy::land::decide` over the
//! facts of the session of the start (`derive::land_decision` over `LandSources`), answered as the plan of the
//! operation (`Plan`): what it does, what the user accepts first, what refuses it, the facts it waits for, the profiles
//! it offers (`alternatives`), and the fingerprint of what it was decided on (`witness_of`). A read: no lock, no effect,
//! no write of a ref, a pull request or a robot.
//!
//! - `prepare`: the plan in an open session of the app (and of the E2E server), on its actor, over the facts its
//!   scheduler holds by the demand of its windows; `None` before its Patronus view, or for a subject its table lacks
//! - `prepare_one_shot`: the plan of a command of `bd` over its one-shot session, which holds what `needs(Land)` names:
//!   the first decision, then the facts it waits for that a read answers now (the review of the subject, the rules of the
//!   target: `needs::waits`, one request each, side by side), then the decision again; with the landing gate and the
//!   check against the newest target the command ran (`LandHost`)
//! - `plan_of`: the plan over any sources, `one_shot_plan` over the one-shot session as it is (no read)
//!
//! The fingerprint hashes the plan with its witness: the tips of the virtual branches it lands and pushes (of a whole
//! stack its stack branch too) and of their remote branches as the table has them, the pull requests of what it lands,
//! and the robots of what it lands and the one it cancels. An `execute` prepares again and runs only on the same
//! fingerprint (`witness::check`).

use crate::facts::fill;
use crate::land_execute::{Planned, PushLease, PushedRef};
use crate::needs::{FactNeed, waits};
use crate::witness::Witness;
use domain_policy::land::TargetConflictFact;
use domain_types::branch_names::to_final_branch_name;
use domain_types::landing::{Land, LandDecision, LandProfiles, LandingRefusal, PlannedEffect, Subject};
use domain_types::operation::{Fingerprint, Intent, Plan};
use domain_types::session::{Fact, LandedSection, SubjectReadiness};
use git_executor::git_command_executor::GitCommandExecutor;
use remote_facts::fact_sources::FactSources;
use repository_model::branch_table::BranchTable;
use repository_model::derive::{LandSources, land_decision, with_land_sources};
use repository_model::facts::FactRecord;
use repository_model::one_shot::OneShot;
use repository_model::session::{SessionKey, Sessions};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use tracing::instrument;

/// What a command knows of its start besides the facts of its one-shot session: the table its sync left, as it compares
/// the remote, the target and whether the branches have a GitHub repository, what the landing gate it ran refuses, and the
/// conflict its check against the newest target found
#[derive(Clone, Copy)]
pub struct LandHost<'a> {
  pub table: &'a BranchTable,
  /// The landing report of its sync
  pub baseline: &'a LandedSection,
  /// The branch of origin the robot pushes into ("master")
  pub target: &'a str,
  pub github: bool,
  pub gate: &'a [LandingRefusal],
  pub target_conflict: Option<TargetConflictFact<'a>>,
}

/// The plan of `land` in the open session of `key`, over the facts it holds now; `None` for a session that is not open,
/// before its Patronus view, or for a subject its table lacks
#[instrument(skip_all, fields(subject = %land.subject.name()))]
pub async fn prepare(sessions: &Sessions, key: &SessionKey, land: Land) -> Option<Plan> {
  let prefix = key.prefix.clone();
  sessions
    .read(key, move |state, facts| {
      with_land_sources(state, facts, &prefix, |sources| plan_of(sources, &land)).flatten()
    })
    .await
    .flatten()
}

/// The plan of `land` over the one-shot session of a command: decided over what it holds, then over the facts that
/// decision waits for, read through `sources` (the rules of the target only for branches on GitHub); `None` for a
/// subject the table lacks
pub async fn prepare_one_shot(session: &mut OneShot, sources: &Arc<dyn FactSources>, git: &GitCommandExecutor, land: &Land, host: &LandHost<'_>) -> Option<Planned> {
  let first = one_shot_planned(session, land, host)?;
  let then: Vec<FactNeed> = waits(&first.plan.waits, &land.subject, host.target)
    .into_iter()
    .filter(|need| host.github || !matches!(need, FactNeed::Rules { .. }))
    .collect();
  if then.is_empty() {
    return Some(first);
  }
  fill(session, sources, git, &then).await;
  one_shot_planned(session, land, host)
}

/// The plan of `land` over the one-shot session of a command as it is now: no read
#[must_use]
pub fn one_shot_plan(session: &OneShot, land: &Land, host: &LandHost<'_>) -> Option<Plan> {
  one_shot_planned(session, land, host).map(|planned| planned.plan)
}

/// `one_shot_plan` with what its run needs (`planned_of`)
#[must_use]
pub fn one_shot_planned(session: &OneShot, land: &Land, host: &LandHost<'_>) -> Option<Planned> {
  with_one_shot_sources(session, host, |sources| planned_of(sources, land))
}

/// `decide` over the sources of the one-shot session of a command as it is now, with what the command knows of its
/// start besides (`LandHost`)
pub fn with_one_shot_sources<T>(session: &OneShot, host: &LandHost<'_>, decide: impl FnOnce(&LandSources<'_>) -> T) -> T {
  let facts = session.facts();
  let readiness = |key: &str| -> Option<&Fact<SubjectReadiness>> { facts.readiness.fact(key) };
  let sources = LandSources {
    table: host.table,
    remote: &host.table.remote,
    view: session.view(),
    robots: session.robots(),
    target: host.target,
    prefix: &session.key().prefix,
    github: host.github,
    profiles: facts.profiles.fact(),
    pull_requests: facts.pull_requests.view(),
    readiness: &readiness,
    rules: facts.rules.get(host.target).map(FactRecord::fact),
    landing_needs: &facts.patronus.part().landing_needs,
    baseline: host.baseline,
    gate: Some(host.gate),
    target_conflict: host.target_conflict,
  };
  decide(&sources)
}

/// The plan of `land` over `sources`; `None` for a subject the table lacks, or a whole stack that doesn't fork
#[must_use]
pub fn plan_of(sources: &LandSources<'_>, land: &Land) -> Option<Plan> {
  planned_of(sources, land).map(|planned| planned.plan)
}

/// The plan of `land` over `sources` with what its run needs (`land_execute::execute`): the witness it was decided on,
/// the branches its push sends with their parents, the ref the robot starts from, the merge request whose approvals
/// count; `None` for a subject the table lacks, or a whole stack that doesn't fork
#[must_use]
pub fn planned_of(sources: &LandSources<'_>, land: &Land) -> Option<Planned> {
  let (decision, profiles) = land_decision(sources, land)?;
  let witness = witness_of(sources, land, &decision);
  let (pushes, source_ref) = pushes_of(sources, land, &decision)?;
  let merge_request = (sources.readiness)(&land.subject.key()).and_then(|fact| fact.value.as_ref()?.requirement.as_ref()?.merge_request.clone());
  let plan = plan(land, decision, profiles, &witness);
  Some(Planned {
    plan,
    witness,
    pushes,
    source_ref,
    merge_request,
    notes: Vec::new(),
    subject: land.subject.clone(),
    mode: land.mode,
  })
}

/// The branches the push of `decision` sends, root first, and the ref the robot starts from: of a whole stack its stack
/// branch, of a branch its virtual branch with the branch each pushed one is built on
fn pushes_of(sources: &LandSources<'_>, land: &Land, decision: &LandDecision) -> Option<(Vec<PushedRef>, String)> {
  let pushed = decision.effects.iter().find_map(|effect| match effect {
    PlannedEffect::Push { branches, .. } => Some(branches.as_slice()),
    _ => None,
  });
  match &land.subject {
    Subject::WholeStack { root } => {
      let stack = sources.table.stack_branches.iter().find(|stack| stack.root == *root)?;
      let pushes = pushed
        .map(|_| PushedRef {
          name: root.clone(),
          full_name: stack.branch_name.clone(),
          parent_ref: None,
          whole_stack: true,
          unapplied: false,
          up_to: None,
          lease: PushLease::Frozen,
        })
        .into_iter()
        .collect();
      Some((pushes, format!("refs/heads/{}", stack.branch_name)))
    }
    Subject::Branch { name } => {
      let full = |branch: &str| to_final_branch_name(sources.prefix, branch).ok();
      let pushes = pushed
        .unwrap_or_default()
        .iter()
        .filter_map(|branch| {
          let base = sources.table.branch(branch)?.stack.as_ref().and_then(|stack| stack.effective_base.as_deref());
          Some(PushedRef {
            name: branch.clone(),
            full_name: full(branch)?,
            parent_ref: base.and_then(full),
            whole_stack: false,
            unapplied: false,
            up_to: None,
            lease: PushLease::Frozen,
          })
        })
        .collect();
      Some((pushes, format!("refs/heads/{}", full(name)?)))
    }
  }
}

/// `decision` of `land` as its plan, fingerprinted on `witness`
fn plan(land: &Land, decision: LandDecision, profiles: LandProfiles, witness: &Witness) -> Plan {
  let LandDecision {
    blockers,
    confirmations,
    waits,
    effects,
    lands,
    pull_request,
    approvals_skipped_by,
  } = decision;
  let mut plan = Plan {
    intent: Intent::Land(land.clone()),
    fingerprint: Fingerprint(String::new()),
    lands,
    effects,
    confirmations,
    blockers,
    waits,
    alternatives: Some(profiles),
    pull_request,
    approvals_skipped_by,
  };
  plan.fingerprint = witness.fingerprint(&plan);
  plan
}

/// What the plan of `land` was decided on besides the facts its decision names: the tips of the branches it lands and
/// pushes and of their remote branches (of a whole stack its stack branch too), their pull requests, their robots and the
/// one it cancels, as `sources` have them
fn witness_of(sources: &LandSources<'_>, land: &Land, decision: &LandDecision) -> Witness {
  let mut refs = BTreeMap::new();
  let mut pull_requests = Vec::new();
  let mut robots: BTreeSet<&str> = BTreeSet::new();
  if let Subject::WholeStack { root } = &land.subject
    && let Some(stack) = sources.table.stack_branches.iter().find(|stack| stack.root == *root)
  {
    let remote = sources.remote.stacks.get(root).and_then(|fact| fact.remote_tip.clone());
    refs.insert(format!("refs/heads/{}", stack.branch_name), stack.tip.clone());
    refs.insert(format!("refs/remotes/origin/{}", stack.branch_name), remote);
    pull_requests.extend(sources.pull_requests.stack_pull_requests.get(root));
    robots.extend(sources.view.stacks.get(root).and_then(|stack| stack.robot_id.as_deref()));
  }
  for effect in &decision.effects {
    if let PlannedEffect::CancelRobot { robot_id } = effect {
      robots.insert(robot_id);
    }
  }
  let pushed = decision.effects.iter().find_map(|effect| match effect {
    PlannedEffect::Push { branches, .. } => Some(branches.as_slice()),
    _ => None,
  });
  for name in decision.lands.iter().chain(pushed.unwrap_or_default()) {
    let Ok(full) = to_final_branch_name(sources.prefix, name) else {
      continue;
    };
    let remote = sources.remote.branches.get(name).and_then(|fact| fact.remote_tip.clone());
    refs.insert(format!("refs/heads/{full}"), sources.table.tips.get(&full).cloned());
    refs.insert(format!("refs/remotes/origin/{full}"), remote);
    pull_requests.extend(sources.pull_requests.pull_requests.get(name));
    robots.extend(sources.view.branches.get(name).and_then(|branch| branch.robot_id.as_deref()));
  }
  let robots = sources.robots.iter().filter(|robot| robots.contains(robot.id.as_str()));
  Witness::of(refs, pull_requests, robots)
}

#[cfg(test)]
#[path = "land_test.rs"]
mod land_test;
