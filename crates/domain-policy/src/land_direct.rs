//! The start of a branch or of a whole stack in a repository that lands by a push of the copy into the target, for the
//! app and `bd land` alike (spec/branch/landing-routes.spec.md, "Push to master"; docs/design/landing.md): the tail of
//! `land::decide` for `LandingRoute::Push`. What lands is the copy the sync built on the baseline: the tip of the virtual
//! branch of the piece (the branch with the branches it is built on), of a whole stack its stack branch; the push is a
//! fast-forward of the target, which the server takes or refuses whole.
//!
//! - blockers, in this order: a Dry Run (a robot of Patronus, which no push runs), the refusal of the gate of the route
//!   (`landing_route::route`), what the table alone refuses (`land::table_blocker`: not built, parked), what landed it
//!   whole (a merged Safe Push, the fetched baseline, a push of this session), the robots of `land::start_refusal` (a
//!   robot on its commit, one that lands it or a part of it; a whole stack whose stack branch is no build of its
//!   commits), what the landing gate refuses unless allowed, a conflict with the newest target;
//! - waits: the facts of the gate (the permission, the rules of the target), the tip of the copy, the pull requests
//!   GitHub lists (whether CI passed on the tip), the landing gate; never read as "no";
//! - confirmations, in this order: more than the row lands; the checks of CI did not pass on the tip (`Unchecked`: no
//!   open pull request, its head another commit, its checks failing or still running, GitHub lists none); changes
//!   requested on its pull request; what the gate of the route asks (`RouteRule`: a rule the user may bypass, the
//!   required status checks, a refusal of the server before);
//! - effects: the push of the copy into the target (`PlannedEffect::PushBaseline`).
//!
//! After a push that landed, Update Project follows as an operation of its own (`update_after`): the app runs it, `bd
//! land` names it as the next step.

use crate::land::{BranchStart, LandFacts, Start, SubjectFacts, branch_lands, conflict_blocker, gate_blockers, more_than_row, stack_lands, start_refusal, table_blocker};
use crate::next_step::{Listing, changes_requested_by};
use domain_types::landing::{Confirmation, FactWait, Land, LandDecision, LandFact, PlannedEffect, Unchecked};
use domain_types::landing_route::{RouteFact, RouteGate};
use domain_types::operation::{EffectRecord, EffectStatus, Intent};
use domain_types::patronus::PatronusPushMode;
use domain_types::pull_request::{ChecksState, PullRequest, PullRequestState, ReviewDecision};
use domain_types::reason::Reason;

/// What a push of the copy of the subject of `land` into the target does and what stands in its way over `facts`, with
/// `gate` the gate of the route (module docs)
#[must_use]
pub(crate) fn decide_push(facts: &LandFacts<'_>, land: &Land, gate: &RouteGate) -> LandDecision {
  let mut decision = LandDecision::default();
  if land.mode == PatronusPushMode::DryRun {
    decision.blockers.push(Reason::DryRunNeedsPatronus);
  }
  let asked: &[Reason] = match gate {
    RouteGate::Open => &[],
    RouteGate::Refused { reason } => {
      decision.blockers.push(reason.clone());
      &[]
    }
    RouteGate::Wait { fact, failed } => {
      let fact = match fact {
        RouteFact::Permission => LandFact::Permission,
        RouteFact::TargetRules => LandFact::TargetRules,
      };
      decision.waits.push(FactWait { fact, failed: failed.clone() });
      &[]
    }
    RouteGate::Confirm { confirmations } => confirmations,
  };
  decision.blockers.extend(table_blocker(&facts.subject));
  // the robots refuse it as they refuse a Safe Push: one on its commit, one that lands it or a part of it
  let safe_push = PatronusPushMode::RebaseAutosquash;
  let (lands, tip, pull_request) = match &facts.subject {
    SubjectFacts::Branch(branch) => {
      decision.blockers.extend(branch.landed.clone());
      decision
        .blockers
        .extend(start_refusal(&Start::Branch(BranchStart { pushes: false, ..branch.start }), safe_push, facts.robots));
      let pull_request = facts.pull_requests.pull_requests.get(branch.start.name);
      (branch_lands(branch), branch.tip, pull_request)
    }
    SubjectFacts::WholeStack(stack) => {
      decision.blockers.extend(stack.landed.clone());
      decision.blockers.extend(start_refusal(&Start::WholeStack(stack.start), safe_push, facts.robots));
      let pull_request = facts.pull_requests.stack_pull_requests.get(&stack.start.stack.root);
      (stack_lands(stack), stack.start.stack.tip.as_deref(), pull_request)
    }
  };
  if tip.is_none() {
    decision.waits.push(FactWait {
      fact: LandFact::Remote,
      failed: None,
    });
  }
  gate_blockers(facts, land, &mut decision);
  conflict_blocker(facts, &mut decision);

  if lands.len() > 1 {
    decision.confirmations.push(Confirmation::LandsMoreThanRow {
      count: more_than_row(lands.len()),
    });
  }
  match Listing::of(&facts.pull_requests.head) {
    // whether CI passed on the tip is not known yet: never "it did not"
    Listing::Loading => decision.waits.push(FactWait {
      fact: LandFact::PullRequests,
      failed: None,
    }),
    Listing::Unlisted { .. } => decision.confirmations.push(Confirmation::Unchecked { why: Unchecked::Unlisted }),
    Listing::Listed => {
      let open = pull_request.filter(|pull_request| pull_request.state == PullRequestState::Open);
      decision.confirmations.extend(checks_of(open, tip));
    }
  }
  decision.confirmations.extend(asked.iter().map(|reason| Confirmation::RouteRule { reason: reason.clone() }));
  if let Some(tip) = tip {
    decision.effects.push(PlannedEffect::PushBaseline {
      subject: land.subject.clone(),
      tip: tip.to_owned(),
      target: facts.target.to_owned(),
    });
  }
  decision.lands = lands;
  decision
}

/// What the open pull request of the subject tells of the commit `tip` a push lands: the checks of CI did not pass on it
/// (none open, it shows another commit, they fail or run), and the changes requested on it
fn checks_of(open: Option<&PullRequest>, tip: Option<&str>) -> Vec<Confirmation> {
  let Some(pull_request) = open else {
    return vec![Confirmation::Unchecked { why: Unchecked::NoPullRequest }];
  };
  let number = pull_request.number;
  let mut confirmations = Vec::new();
  let unchecked = if tip.is_some_and(|tip| tip != pull_request.head_ref_oid) {
    Some(Unchecked::OtherHead { number })
  } else {
    match pull_request.checks.as_ref().map(|checks| checks.state) {
      Some(ChecksState::Failing) => Some(Unchecked::Failing { number }),
      Some(ChecksState::Pending) => Some(Unchecked::Running { number }),
      // a commit without checks: the repository runs no CI on it
      Some(ChecksState::Passing) | None => None,
    }
  };
  confirmations.extend(unchecked.map(|why| Confirmation::Unchecked { why }));
  let by = changes_requested_by(pull_request);
  if !by.is_empty() || pull_request.review_decision == Some(ReviewDecision::ChangesRequested) {
    confirmations.push(Confirmation::ChangesRequested { number, by });
  }
  confirmations
}

/// What follows the start whose effects ran as `effects`: Update Project with its fetch, as an operation of its own,
/// once its push into the target landed; nothing otherwise (a push that failed, a start of a robot)
#[must_use]
pub fn update_after(effects: &[EffectRecord]) -> Option<Intent> {
  effects
    .iter()
    .any(|record| matches!(record.effect, PlannedEffect::PushBaseline { .. }) && record.status == EffectStatus::Done)
    .then_some(Intent::UpdateProject { fetch: true })
}
