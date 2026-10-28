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
//! The merge tail (`decide_merge`, spec/branch/landing-routes.spec.md, "Merge"), of `LandingRoute::PullRequest` where no
//! robot of Patronus lands the repository: GitHub merges the open pull request of the subject into the target (of a
//! branch its own, of a whole stack the one of its stack branch), by the first of rebase, a merge commit and squash the
//! repository allows (rebase keeps author, time and message, so the update pairs the commits with their originals;
//! squash pairs none, a confirmation). What GitHub tells of the pull request decides (`merge_of`, the one answer of the
//! plan and of the next step):
//!
//! - none open into the target: Create Pull Request first (`NoPullRequestToMerge`); one into another base, a draft:
//!   refused;
//! - its auto-merge enabled: GitHub merges it once its checks pass; nothing to start;
//! - its head another commit than the tip of the copy: the push of the branch (of a whole stack its stack branch), then
//!   the auto-merge on the pushed commit, whose checks the push restarts (a merge now would merge the old head); without
//!   auto-merge the push first (`MergeHeadBehind`);
//! - by `mergeStateStatus`: `CLEAN` and `HAS_HOOKS` merge now, `UNSTABLE` too with a confirmation (checks that are not
//!   required fail or run); `BLOCKED` by what the pull request tells (changes requested, approvals required, required
//!   checks failing) refuses, checks that still run enable the auto-merge where the repository allows it; `BEHIND` and
//!   `DIRTY` refuse (the update of the branch and its push; a conflict); `UNKNOWN` (GitHub computes it lazily) waits,
//!   never "no".
//!
//! Its witness is the head GitHub shows, which `expectedHeadOid` checks again on GitHub: a head that moved meanwhile
//! refuses the merge (`plan_changed`).
//!
//! After a push that landed, after a target that moved refused it, and after a merge by the click, Update Project follows
//! as an operation of its own (`update_after`): the app runs it, `bd land` names it as the next step. An auto-merge that
//! GitHub completes later runs none: the news of the session tells it (`news::auto_merged`).

use crate::facts::RemoteBranch;
use crate::land::{BranchStart, LandFacts, Start, SubjectFacts, branch_lands, conflict_blocker, drops, gate_blockers, more_than_row, stack_lands, start_refusal, table_blocker};
use crate::next_step::{Listing, changes_requested_by};
use crate::pull_request::start_base;
use domain_types::error::ErrorCode;
use domain_types::landing::{Confirmation, FactWait, Land, LandDecision, LandFact, PlannedEffect, Unchecked};
use domain_types::landing_route::{RouteFact, RouteGate};
use domain_types::operation::{EffectRecord, EffectStatus, Intent};
use domain_types::patronus::PatronusPushMode;
use domain_types::pull_request::{ChecksState, MergeMethod, MergeMethods, MergeStateStatus, PullRequest, PullRequestBase, PullRequestState, ReviewDecision};
use domain_types::reason::Reason;

/// What a push of the copy of the subject of `land` into the target on `remote` does and what stands in its way over
/// `facts`, with `gate` the gate of the route (module docs); a local baseline (no `remote`), which the gate refuses,
/// has no effect
#[must_use]
pub(crate) fn decide_push(facts: &LandFacts<'_>, land: &Land, gate: &RouteGate, remote: Option<&str>) -> LandDecision {
  let mut decision = LandDecision::default();
  let asked = gate_part(land, gate, &mut decision);
  let SubjectPart { lands, tip, pull_request, .. } = subject_part(facts, land, &mut decision);
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
  if let (Some(tip), Some(remote)) = (tip, remote) {
    decision.effects.push(PlannedEffect::PushBaseline {
      subject: land.subject.clone(),
      tip: tip.to_owned(),
      remote: remote.to_owned(),
      target: facts.target.to_owned(),
    });
  }
  decision.lands = lands;
  decision
}

/// What a landing without a robot lands of its subject, and where: what the subject part of `land` found
struct SubjectPart<'a> {
  lands: Vec<String>,
  /// The tip of the copy: of a branch its virtual branch, of a whole stack its stack branch
  tip: Option<&'a str>,
  /// The pull request of the subject, any state
  pull_request: Option<&'a PullRequest>,
  /// The branch, or the root of a whole stack
  name: &'a str,
  /// Its remote branch (of a whole stack the stack branch's)
  remote: Option<RemoteBranch<'a>>,
}

/// The subject part of a landing without a robot, its blockers and waits into `decision`: what the table refuses, what
/// landed it whole, the robots as they refuse a Safe Push (one on its commit, one that lands it or a part of it), the
/// tip of the copy, the landing gate, a conflict with the newest target; more than its row a confirmation
fn subject_part<'a>(facts: &LandFacts<'a>, land: &Land, decision: &mut LandDecision) -> SubjectPart<'a> {
  decision.blockers.extend(table_blocker(&facts.subject));
  let safe_push = PatronusPushMode::RebaseAutosquash;
  let part = match &facts.subject {
    SubjectFacts::Branch(branch) => {
      decision.blockers.extend(branch.landed.clone());
      decision
        .blockers
        .extend(start_refusal(&Start::Branch(BranchStart { pushes: false, ..branch.start }), safe_push, facts.robots));
      SubjectPart {
        lands: branch_lands(branch),
        tip: branch.tip,
        pull_request: facts.pull_requests.pull_requests.get(branch.start.name),
        name: branch.start.name,
        remote: branch.chain.last().and_then(|own| own.remote),
      }
    }
    SubjectFacts::WholeStack(stack) => {
      decision.blockers.extend(stack.landed.clone());
      decision.blockers.extend(start_refusal(&Start::WholeStack(stack.start), safe_push, facts.robots));
      let root = stack.start.stack.root.as_str();
      SubjectPart {
        lands: stack_lands(stack),
        tip: stack.start.stack.tip.as_deref(),
        pull_request: facts.pull_requests.stack_pull_requests.get(root),
        name: root,
        remote: stack.remote,
      }
    }
  };
  if part.tip.is_none() {
    decision.waits.push(FactWait {
      fact: LandFact::Remote,
      failed: None,
    });
  }
  gate_blockers(facts, land, decision);
  conflict_blocker(facts, decision);
  if part.lands.len() > 1 {
    decision.confirmations.push(Confirmation::LandsMoreThanRow {
      count: more_than_row(part.lands.len()),
    });
  }
  part
}

/// A Dry Run refused (a robot of Patronus, which no push or merge runs), and the gate of the route: its refusal a
/// blocker, its wait a wait; what it asks the user to accept
fn gate_part<'g>(land: &Land, gate: &'g RouteGate, decision: &mut LandDecision) -> &'g [Reason] {
  if land.mode == PatronusPushMode::DryRun {
    decision.blockers.push(Reason::DryRunNeedsPatronus);
  }
  match gate {
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
  }
}

/// What GitHub tells of a merge of the open pull request of a subject now (module docs): the one answer of the plan
/// (`decide_merge`) and of the next step (`next_step`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeAnswer<'a> {
  /// No open pull request of it into the target, nor into another base
  NoPullRequest,
  /// Its open pull request goes into another base (`base`): of a branch a virtual branch, which a retarget fixes
  OtherBase { pull_request: &'a PullRequest, base: PullRequestBase },
  /// GitHub has not computed whether it can merge the open pull request `number` (`UNKNOWN`, or not said): asked again,
  /// it answers
  Waits { number: u32 },
  /// The merge of the open pull request `number` can't now: `reason`
  Held { number: u32, reason: Reason },
  /// GitHub merges the open pull request `number` by `method` while it shows `head`: now, or (`auto`) once the checks of
  /// it pass; `push`: the branch is pushed first, `head` the commit it pushes; `unstable`: checks that are not required
  /// fail or run
  Merge {
    number: u32,
    method: MergeMethod,
    head: &'a str,
    auto: bool,
    push: bool,
    unstable: bool,
  },
  /// The auto-merge of the open pull request `number` is enabled: GitHub merges it by `method` once its checks pass
  AutoMerging { number: u32, method: MergeMethod },
}

/// What a merge of `pull_request`, the pull request of a subject in any state, does now (module docs): `base` where an
/// open one goes (`pull_request::start_base`), `tip` the commit of the copy the merge would land, `methods` what the
/// repository allows (the rebase while GitHub didn't tell)
#[must_use]
pub fn merge_of<'a>(
  pull_request: Option<&'a PullRequest>,
  base: impl FnOnce(&PullRequest) -> PullRequestBase,
  tip: Option<&'a str>,
  methods: Option<MergeMethods>,
  target: &str,
) -> MergeAnswer<'a> {
  let Some(pull_request) = pull_request.filter(|pull_request| pull_request.state == PullRequestState::Open) else {
    return MergeAnswer::NoPullRequest;
  };
  let number = pull_request.number;
  let base = base(pull_request);
  if base != PullRequestBase::Target {
    return MergeAnswer::OtherBase { pull_request, base };
  }
  let held = |reason: Reason| MergeAnswer::Held { number, reason };
  if pull_request.is_draft || pull_request.merge_state == Some(MergeStateStatus::Draft) {
    return held(Reason::MergeDraft { number });
  }
  let auto_merge = methods.is_some_and(|methods| methods.auto_merge);
  let method = methods.and_then(MergeMethods::preferred).unwrap_or(MergeMethod::Rebase);
  if let Some(method) = pull_request.auto_merge {
    return MergeAnswer::AutoMerging { number, method };
  }
  let merge = |head: &'a str, auto: bool, push: bool, unstable: bool| MergeAnswer::Merge {
    number,
    method,
    head,
    auto,
    push,
    unstable,
  };
  // a merge now would merge the commit it shows, not the copy: the push, then the auto-merge on the pushed commit
  if let Some(tip) = tip.filter(|tip| *tip != pull_request.head_ref_oid) {
    return if auto_merge {
      merge(tip, true, true, false)
    } else {
      held(Reason::MergeHeadBehind { number })
    };
  }
  let head = pull_request.head_ref_oid.as_str();
  match pull_request.merge_state {
    None | Some(MergeStateStatus::Unknown) => MergeAnswer::Waits { number },
    Some(MergeStateStatus::Clean | MergeStateStatus::HasHooks) => merge(head, false, false, false),
    Some(MergeStateStatus::Unstable) => merge(head, false, false, true),
    Some(MergeStateStatus::Dirty) => held(Reason::MergeConflict { number }),
    Some(MergeStateStatus::Behind) => held(Reason::MergeBehind {
      number,
      target: target.to_owned(),
    }),
    // checked first
    Some(MergeStateStatus::Draft) => held(Reason::MergeDraft { number }),
    Some(MergeStateStatus::Blocked) => {
      let by = changes_requested_by(pull_request);
      let checks = pull_request.checks.as_ref().map(|checks| checks.state);
      if !by.is_empty() || pull_request.review_decision == Some(ReviewDecision::ChangesRequested) {
        held(Reason::MergeChangesRequested { number, by })
      } else if pull_request.review_decision == Some(ReviewDecision::ReviewRequired) {
        held(Reason::MergeNeedsApprovals { number })
      } else if checks == Some(ChecksState::Failing) {
        held(Reason::MergeChecksFail { number })
      } else if checks == Some(ChecksState::Pending) {
        if auto_merge {
          merge(head, true, false, false)
        } else {
          held(Reason::MergeChecksRunning { number })
        }
      } else {
        held(Reason::MergeBlocked { number })
      }
    }
  }
}

/// What a merge of the open pull request of the subject of `land` into the target does and what stands in its way over
/// `facts`, with `gate` the gate of the route (module docs)
#[must_use]
pub(crate) fn decide_merge(facts: &LandFacts<'_>, land: &Land, gate: &RouteGate) -> LandDecision {
  let mut decision = LandDecision::default();
  gate_part(land, gate, &mut decision);
  let SubjectPart {
    lands,
    tip,
    pull_request,
    name,
    remote,
  } = subject_part(facts, land, &mut decision);
  decision.lands = lands;
  if matches!(Listing::of(&facts.pull_requests.head), Listing::Loading | Listing::Unlisted { .. }) {
    // which pull request merges is not known: never "none"
    let failed = match &facts.pull_requests.head.state {
      domain_types::session::FactState::Unavailable { reason } => Some(reason.clone()),
      _ => None,
    };
    decision.waits.push(FactWait {
      fact: LandFact::PullRequests,
      failed,
    });
    return decision;
  }
  let methods = facts.pull_requests.merge_methods;
  let whole_stack = land.subject.is_whole_stack();
  let base = |pull_request: &PullRequest| start_base(pull_request, facts.target, facts.prefix, whole_stack);
  match merge_of(pull_request, base, tip, methods, facts.target) {
    MergeAnswer::NoPullRequest => decision.blockers.push(Reason::NoPullRequestToMerge { branch: name.to_owned() }),
    MergeAnswer::OtherBase { pull_request, base } => decision.blockers.push(Reason::OtherBase {
      number: pull_request.number,
      base: match base {
        PullRequestBase::Virtual { from, .. } => from,
        PullRequestBase::Other { base } => base,
        PullRequestBase::Target => facts.target.to_owned(),
      },
      target: facts.target.to_owned(),
    }),
    MergeAnswer::Waits { number } => decision.waits.push(FactWait {
      fact: LandFact::MergeState { number },
      failed: None,
    }),
    MergeAnswer::Held { reason, .. } => decision.blockers.push(reason),
    // GitHub merges it by itself once its checks pass: nothing to start
    MergeAnswer::AutoMerging { .. } => {}
    MergeAnswer::Merge {
      number,
      method,
      head,
      auto,
      push,
      unstable,
    } => {
      decision.pull_request = Some(number);
      if unstable {
        decision.confirmations.push(Confirmation::ChecksNotRequired { number });
      }
      let by = pull_request.map_or_default(changes_requested_by);
      if !by.is_empty() {
        decision.confirmations.push(Confirmation::ChangesRequested { number, by });
      }
      if method == MergeMethod::Squash {
        decision.confirmations.push(Confirmation::SquashOnly { number });
      }
      let subject = land.subject.clone();
      if push {
        let pushed: Option<(&str, Option<RemoteBranch<'_>>)> = Some((name, remote));
        let forced = drops(pushed.into_iter(), land, &mut decision);
        decision.effects.push(PlannedEffect::Push {
          branches: vec![name.to_owned()],
          force: !forced.is_empty(),
        });
        decision.confirmations.extend(forced);
      }
      let (head, target) = (head.to_owned(), facts.target.to_owned());
      decision.effects.push(if auto {
        PlannedEffect::EnableAutoMerge {
          subject,
          number,
          method,
          head,
          target,
        }
      } else {
        PlannedEffect::MergePullRequest {
          subject,
          number,
          method,
          head,
          target,
        }
      });
    }
  }
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
/// once its push into the target landed, and once the target refused it as no fast-forward (`baseline_moved`): the
/// copies are built on the fork point of the checked out branch, so only the update onto the moved target builds them
/// anew on it; once GitHub merged its pull request by the click; nothing otherwise (a refusal of the server, another
/// failure, an auto-merge GitHub completes later, a start of a robot)
#[must_use]
pub fn update_after(effects: &[EffectRecord]) -> Option<Intent> {
  effects
    .iter()
    .any(|record| match (&record.effect, &record.status) {
      (PlannedEffect::PushBaseline { .. } | PlannedEffect::MergePullRequest { .. }, EffectStatus::Done) => true,
      (PlannedEffect::PushBaseline { .. }, EffectStatus::Failed { error }) => error.code == ErrorCode::BaselineMoved,
      _ => false,
    })
    .then_some(Intent::UpdateProject { fetch: true })
}
