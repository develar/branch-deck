//! Whether and how a Safe Push or a Dry Run of a branch or of a whole stack starts, for the app and `bd` alike
//! (docs/design/rust-domain-authority.md, R4, R6, D2, D3b, D11, D12, D14, D15; spec/branch/patronus.spec.md, "Starting a
//! Safe Push or a Dry Run"; spec/cli/agent-cli.spec.md, "bd safe-push").
//!
//! Part 1, the refusals of the subject and its robots (`start_refusal`). A start is refused:
//!
//! - for a branch the sync left blocked (its build lacks its parent's state), for a whole stack whose stack branch is no
//!   build of its commits;
//! - while a robot runs on the commit the subject is at, or on its remote branch that has the same commits rebuilt on a
//!   newer baseline (`on_last_commit`): Patronus would refuse it, the robot tests and lands the subject as it is;
//! - for a Safe Push, while a running robot of a branch built on it, or of its whole stack, lands the branch with more
//!   (`Descendant`), or one lands a part of what it would land (`Ancestor`: an ancestor, a side branch built on one, for
//!   the whole stack a branch of it): two robots would land the same commits;
//! - for a Dry Run, while a running Safe Push of a branch built on it lands it and the start pushes (D15): the push
//!   would fail that Safe Push at the end, as it fails any push into what a running Safe Push lands.
//!
//! A restart is a start of the robot's subject in its mode that the robot itself does not refuse and that pushes
//! nothing; a subject merged Safe Pushes landed whole has nothing left to land (`restart_refusal`).
//!
//! Part 2, the decision of a start (`decide`, the contract's `Land` → `LandDecision`) over the facts of the subject and
//! of the remote, each of which may have no answer yet (a wait, never "no"):
//!
//! - blockers, in this order: what the table alone refuses (`table_blocker`: the sync did not build the branch, it stays
//!   local), a target Patronus doesn't protect, merged Safe Pushes that landed it whole, the robots of part 1, a push
//!   that drops commits of a remote branch unless the user said so (D12), a push into a branch a running Safe Push holds
//!   that the start doesn't replace (a Dry Run never replaces one), what the landing gate refuses unless allowed, a
//!   conflict with the newest target, a draft pull request, a pull request into another base when the approvals count
//!   (D3b), missing approvals unless skipped (D2), a push that dismisses the approvals Patronus counts;
//! - confirmations, what the user accepts first: more than the row lands, a profile, the cancel of the Safe Push it
//!   replaces, approvals Patronus judges only at the start, a review Patronus couldn't answer (D14), approvals the push
//!   dismisses, rules GitHub didn't answer, a start without approvals, a piece of a stack whose pull request is open, a
//!   force push the user allowed; `unaccepted` names the ones an allowance of `bd` accepts;
//! - effects, in order: the retargets of the pull requests into a virtual branch (D11), the cancel, the push, then the
//!   start (`StartRoute`: a `/patronus` comment on the pull request it starts from, else the API with the profile) or
//!   the pull request it opens.
//!
//! The profiles of a mode (`allows_mode`), the default one (`default_profile`: the last robot's while the target offers
//! it) and what a start offers (`offered_profiles`: of a start without approvals the emergency ones too, and as the
//! default only one that skips the review) are read here too.

use crate::facts::{RemoteBranch, Robots};
use crate::holds::is_active;
use crate::pull_request::{RulesAnswer, base_of, dismisses_approvals, start_base, starts_from};
use crate::reasons::{ancestor, blocked, descendant, held, last_commit, stack_not_built};
use crate::review::{ReviewState, review_state, skips_approvals};
use domain_types::landing::{ApprovalsSkip, Confirmation, FactWait, Land, LandDecision, LandFact, LandProfiles, LandingRefusal, PlannedEffect, ProfileChoice, StartRoute};
use domain_types::patronus::{LandingAllowances, PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotProfile, PatronusStartOptions, ReviewReadiness};
use domain_types::pull_request::{BranchRules, PullRequest, PullRequestBase, PullRequestState, ReviewerState};
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView, RobotOf, SpanView};
use domain_types::session::{Fact, FactState, Parked, ParkedMember, PullRequestsView, SubjectReadiness};
use domain_types::stack::{BlockReason, BranchStack, StackBranchUpdate};

/// The robot took its subject as it is: its tip, or its remote branch while that has the same commits by content
#[must_use]
pub fn took_as_is(source: &str, tip: Option<&str>, remote: Option<RemoteBranch<'_>>) -> bool {
  tip == Some(source) || (tip.is_some() && remote.is_some_and(|remote| remote.up_to_date && remote.tip == Some(source)))
}

/// A start of the subject waits for `robot`: one still running (a cancel included) that took the subject as it is
/// (`took_as_is`); a finished one blocks nothing, nor one on a commit the subject built new ones on
#[must_use]
pub fn on_last_commit(robot: &PatronusRobot, tip: Option<&str>, remote: Option<RemoteBranch<'_>>) -> bool {
  is_active(robot) && robot.source_revision.as_deref().is_some_and(|source| took_as_is(source, tip, remote))
}

/// What a start of a branch is decided on
#[derive(Clone, Copy, Debug)]
pub struct BranchStart<'a> {
  pub name: &'a str,
  /// Why the last sync did not build it; only a block refuses the start here (an error of the branch is its own step)
  pub not_built: Option<BlockReason>,
  /// Where it sits in its stack: the branch a block names
  pub stack: Option<&'a BranchStack>,
  /// The robot on the commit the branch is at (`on_last_commit`)
  pub last_commit: Option<&'a PatronusRobot>,
  /// The first robot that lands it with more (`BranchPatronus::span`)
  pub span: Option<&'a SpanView>,
  /// The nearest ancestor a robot lands (`BranchPatronus::landing_ancestor`)
  pub landing_ancestor: Option<&'a RobotOf>,
  /// The hold of its push (`holds::branch_hold`)
  pub hold: Option<&'a HoldView>,
  /// The start pushes: the branch, or an ancestor its push takes along, has work for the remote
  pub pushes: bool,
}

/// What a start of the whole stack of a root is decided on
#[derive(Clone, Copy, Debug)]
pub struct StackStart<'a> {
  /// Its stack branch, while the stack forks
  pub stack: &'a StackBranchUpdate,
  /// The robot on the commit of its stack branch (`on_last_commit`)
  pub last_commit: Option<&'a PatronusRobot>,
  /// The first member whose robot lands a part of it (`StackPatronus::landing_member`)
  pub landing_member: Option<&'a RobotOf>,
}

/// The subject of a start
#[derive(Clone, Copy, Debug)]
pub enum Start<'a> {
  Branch(BranchStart<'a>),
  WholeStack(StackStart<'a>),
}

/// Why a start of `start` in `mode` is refused (module docs), in this order; `None` when nothing refuses. The robots a
/// reason names come from `robots`: a fact whose robot the user's robots lack refuses nothing
#[must_use]
pub fn start_refusal(start: &Start<'_>, mode: PatronusPushMode, robots: &Robots<'_>) -> Option<Reason> {
  let dry_run = mode == PatronusPushMode::DryRun;
  match start {
    Start::Branch(branch) => {
      if branch.not_built == Some(BlockReason::Blocked) {
        return Some(blocked(branch.name, branch.stack));
      }
      if let Some(robot) = branch.last_commit {
        return Some(last_commit(robot));
      }
      if dry_run {
        // a Safe Push of a branch built on it lands it: the push of the dry run fails it at the end (D15)
        let hold = branch.hold.filter(|hold| branch.pushes && hold.kind == HoldKind::Span)?;
        return robots.get(&hold.by.robot_id).map(|robot| held(branch.name, hold, robot));
      }
      let span = branch.span.and_then(|span| Some(descendant(branch.name, span, robots.get(&span.by.robot_id)?)));
      span.or_else(|| branch.landing_ancestor.and_then(|landing| Some(ancestor(landing, robots.get(&landing.robot_id)?))))
    }
    Start::WholeStack(stack) => {
      if let Some(reason) = stack_not_built(stack.stack) {
        return Some(reason);
      }
      if let Some(robot) = stack.last_commit {
        return Some(last_commit(robot));
      }
      if dry_run {
        return None;
      }
      stack.landing_member.and_then(|landing| Some(ancestor(landing, robots.get(&landing.robot_id)?)))
    }
  }
}

/// Why a restart of a robot in `mode` is refused: a start of its subject that the robot itself does not refuse (it is
/// the robot on the last commit) and that pushes nothing, then `landed`, what tells that merged Safe Pushes landed the
/// subject whole
#[must_use]
pub fn restart_refusal(start: &Start<'_>, mode: PatronusPushMode, robots: &Robots<'_>, landed: Option<Reason>) -> Option<Reason> {
  let start = match *start {
    Start::Branch(branch) => Start::Branch(BranchStart {
      last_commit: None,
      pushes: false,
      ..branch
    }),
    Start::WholeStack(stack) => Start::WholeStack(StackStart { last_commit: None, ..stack }),
  };
  start_refusal(&start, mode, robots).or(landed)
}

/// A branch of what a start of a branch lands: one it is built on, or the branch itself
#[derive(Clone, Copy, Debug)]
pub struct LandsBranch<'a> {
  pub name: &'a str,
  /// A merged Safe Push landed all of it: its commits are in the target, the robot lands the rest
  pub landed_whole: bool,
  /// What is known of its remote branch; `None` before a remote status said it
  pub remote: Option<RemoteBranch<'a>>,
  /// A running Safe Push other than the one the start replaces holds it: the push of what the start lands stops below it
  pub held: bool,
}

/// What a start of a branch is decided on besides the remote services
#[derive(Clone, Debug)]
pub struct BranchSubject<'a> {
  /// Its robots (part 1); `pushes` is decided here, over `chain`
  pub start: BranchStart<'a>,
  /// Why it stays local: parked, or built on a parked branch (`parked::parked_state`)
  pub parked: Option<Parked>,
  /// What landed it whole (`reasons::landed`)
  pub landed: Option<Reason>,
  /// What it lands, root first: the branches it is built on, then the branch itself
  pub chain: Vec<LandsBranch<'a>>,
  /// The commit it starts from; none until the sync built it
  pub tip: Option<&'a str>,
  /// The robot a Safe Push start replaces: its own, running on an older commit
  pub replaces: Option<&'a str>,
  /// The open pull request of the whole stack the branch is in: its approvals count for the whole stack only
  pub whole_stack_pull_request: Option<u32>,
}

/// What a start of the whole stack of a root is decided on besides the remote services
#[derive(Clone, Debug)]
pub struct StackSubject<'a> {
  /// Its robots (part 1)
  pub start: StackStart<'a>,
  /// The member that keeps its stack branch local (`parked::whole_stack_parked`)
  pub parked: Option<ParkedMember>,
  /// What landed every branch of it whole (`reasons::stack_landed`)
  pub landed: Option<Reason>,
  /// Its branches in the order of the stack, each whether a merged Safe Push landed it whole
  pub members: Vec<(&'a str, bool)>,
  /// What is known of the remote branch of its stack branch
  pub remote: Option<RemoteBranch<'a>>,
  /// The robot of the whole stack a Safe Push start replaces, running on an older stack branch
  pub replaces: Option<&'a str>,
  /// Its own robot, which holds the pushes of the stack branch
  pub held_by: Option<&'a PatronusRobot>,
}

/// The subject of a decision
#[derive(Clone, Debug)]
pub enum SubjectFacts<'a> {
  Branch(BranchSubject<'a>),
  WholeStack(StackSubject<'a>),
}

/// A conflict of what lands with the newest target, as the check before the start found it
#[derive(Clone, Copy, Debug)]
pub struct TargetConflictFact<'a> {
  /// The copy of the virtual branch that conflicts
  pub commit: &'a str,
  pub files: &'a [String],
}

/// Everything a start is decided on: the subject as the table and the robots have it, and the facts of the remote
/// services, each as the session holds it (`bd` holds its own reads as `Fresh`)
#[derive(Clone, Debug)]
pub struct LandFacts<'a> {
  /// The branch of origin the robot pushes into
  pub target: &'a str,
  /// The branch prefix of the virtual branches (a pull request into one goes into the target)
  pub prefix: &'a str,
  /// Patronus names the GitHub repository the branches are mirrored to: a robot can start from a pull request
  pub github: bool,
  pub robots: &'a Robots<'a>,
  pub subject: SubjectFacts<'a>,
  /// The merge profiles of the target
  pub profiles: &'a Fact<PatronusStartOptions>,
  /// The profile of the last robot of the subject, when an answer named it (the default)
  pub last_profile: Option<&'a PatronusRobotProfile>,
  /// The pull requests of the branches and of the whole stacks, with how their listing stands
  pub pull_requests: &'a PullRequestsView,
  /// The code-owner review of the subject; `None` while nobody asked
  pub readiness: Option<&'a Fact<SubjectReadiness>>,
  /// The rules of the target, for a push into an approved pull request; `None` while nobody asked
  pub rules: Option<&'a Fact<BranchRules>>,
  /// What the landing gate refuses of what lands; `None` until it answered
  pub landing: Option<&'a [LandingRefusal]>,
  /// A conflict the check against the newest target found; `None` without one, or when nothing checked
  pub target_conflict: Option<TargetConflictFact<'a>>,
}

/// The profile allows the mode: one that lists none allows every mode
#[must_use]
pub fn allows_mode(profile: &PatronusProfile, mode: PatronusPushMode) -> bool {
  profile.allowed_push_modes.is_empty() || profile.allowed_push_modes.contains(&mode)
}

/// The default profile of a start in `mode`: the profile of the last robot (`last`, as an answer named it) while the
/// target offers it for the mode, with what that answer said of the review when the profiles don't tell; else the
/// default the profiles name; else none, the default checks
#[must_use]
pub fn default_profile(options: &PatronusStartOptions, last: Option<&PatronusRobotProfile>, mode: PatronusPushMode) -> Option<PatronusProfile> {
  let offered = |id: &str| options.profiles.iter().find(|profile| profile.id == id && allows_mode(profile, mode));
  if let Some(last) = last
    && let Some(profile) = last.id.as_deref().and_then(offered)
  {
    let mut profile = profile.clone();
    profile.skips_review = profile.skips_review.or(last.skips_review);
    return Some(profile);
  }
  options.default_profile_id.as_deref().and_then(offered).cloned()
}

/// The profiles a start of `land` offers over the options of its target (`LandProfiles`): those the mode allows, the
/// last robot's (`last`) with what its answer said of the review when the options don't tell; of a start without
/// approvals also the emergency profiles the mode allows, and as the default only one known to skip the review (the
/// user chooses otherwise: a profile that checks the review fails such a start at once)
#[must_use]
pub fn offered_profiles(options: &PatronusStartOptions, last: Option<&PatronusRobotProfile>, mode: PatronusPushMode, without_approvals: bool) -> LandProfiles {
  let described = |profile: &PatronusProfile| {
    let mut profile = profile.clone();
    if let Some(last) = last.filter(|last| last.id.as_deref() == Some(profile.id.as_str())) {
      profile.skips_review = profile.skips_review.or(last.skips_review);
    }
    profile
  };
  let allowed = |profiles: &[PatronusProfile]| profiles.iter().filter(|profile| allows_mode(profile, mode)).map(described).collect::<Vec<_>>();
  let default = default_profile(options, last, mode).filter(|profile| !without_approvals || profile.skips_review == Some(true));
  LandProfiles {
    offered: allowed(&options.profiles),
    emergency: if without_approvals { allowed(&options.emergency_profiles) } else { Vec::new() },
    default: default.map(|profile| profile.id),
  }
}

/// The profile a start of `land` through the API runs with (`ProfileChoice`): the default of `offered_profiles`, one of
/// them by id, or none, the default checks
fn chosen_profile(options: &PatronusStartOptions, last: Option<&PatronusRobotProfile>, land: &Land) -> Option<PatronusProfile> {
  let profiles = offered_profiles(options, last, land.mode, land.without_approvals);
  let id = match &land.profile {
    ProfileChoice::None => return None,
    ProfileChoice::Id { id } => id.as_str(),
    ProfileChoice::Default => profiles.default.as_deref()?,
  };
  profiles.offered.iter().chain(&profiles.emergency).find(|profile| profile.id == id).cloned()
}

/// What the table alone refuses of a start, before any fact of the remote: the sync did not build the branch, the
/// branch stays local (parked, or built on parked), the stack branch of a whole stack holds a parked member
#[must_use]
pub fn table_blocker(subject: &SubjectFacts<'_>) -> Option<Reason> {
  match subject {
    SubjectFacts::Branch(branch) => {
      let name = branch.start.name.to_string();
      match (branch.start.not_built, &branch.parked) {
        (Some(block), _) => Some(Reason::CantPush { branch: name, block }),
        (None, Some(parked)) => Some(Reason::Parked {
          branch: name,
          with: parked.with.clone(),
          note: parked.note.clone(),
        }),
        (None, None) => None,
      }
    }
    SubjectFacts::WholeStack(stack) => stack.parked.as_ref().map(|parked| Reason::StackParked {
      root: stack.start.stack.root.clone(),
      member: parked.parked.with.clone().unwrap_or_else(|| parked.member.clone()),
    }),
  }
}

/// The confirmations of `confirmations` that `allow` doesn't accept, of those an allowance accepts in `bd` (a flag): the
/// cancel of the robot the start replaces, a review Patronus couldn't answer, rules GitHub didn't answer. The others
/// the start says and `bd` prints; the app's popover asks every one
#[must_use]
pub fn unaccepted<'c>(confirmations: &'c [Confirmation], allow: &LandingAllowances) -> Vec<&'c Confirmation> {
  confirmations
    .iter()
    .filter(|confirmation| match confirmation {
      Confirmation::CancelsRobot { .. } => !allow.cancel_running,
      Confirmation::ReviewUnknown { .. } => !allow.allow_unknown_review,
      Confirmation::RulesUnknown { .. } => !allow.allow_unknown_rules,
      _ => false,
    })
    .collect()
}

/// What the subject part of a decision found: what lands and the push, the pull request it starts from or one into
/// another base, and whether Patronus can review it
struct SubjectPart<'a> {
  lands: Vec<String>,
  /// The branches the push takes, root first (of a whole stack its root)
  push: Vec<String>,
  force: bool,
  /// The open pull request of the subject, where it goes
  own: Option<(&'a PullRequest, PullRequestBase)>,
  retargets: Vec<PlannedEffect>,
  /// The robot it replaces, once the start replaces one
  cancels: Option<&'a str>,
  forced: Vec<Confirmation>,
  /// A whole stack whose stack branch is no build of its commits has no commits to review
  reviewable: bool,
  lands_more: u32,
  whole_stack_pull_request: Option<u32>,
}

/// What a start of `land` does and what stands in its way over `facts` (module docs). Every fact without an answer is
/// a wait; the effects say what the start does once nothing refuses it and the user accepted its confirmations
#[must_use]
pub fn decide(facts: &LandFacts<'_>, land: &Land) -> LandDecision {
  let mut decision = LandDecision::default();
  let safe_push = land.mode != PatronusPushMode::DryRun;
  decision.blockers.extend(table_blocker(&facts.subject));
  let options = standing(facts.profiles);
  match options {
    Some(options) if !options.supported => decision.blockers.push(Reason::Unprotected {
      target: options.target_branch.clone(),
    }),
    Some(_) => {}
    None => decision.waits.push(wait(LandFact::Profiles, facts.profiles)),
  }
  let listed = standing(&facts.pull_requests.head).map(|_| facts.pull_requests);
  if listed.is_none() && !matches!(facts.pull_requests.head.state, FactState::Unavailable { .. }) {
    decision.waits.push(wait(LandFact::PullRequests, &facts.pull_requests.head));
  }
  let part = match &facts.subject {
    SubjectFacts::Branch(branch) => branch_part(facts, land, branch, listed, &mut decision),
    SubjectFacts::WholeStack(stack) => stack_part(facts, land, stack, listed, &mut decision),
  };
  match facts.landing {
    None => decision.waits.push(FactWait {
      fact: LandFact::LandingNeeds,
      failed: None,
    }),
    Some(refusals) => decision.blockers.extend(
      refusals
        .iter()
        .filter(|refusal| match refusal {
          LandingRefusal::DanglingReference { .. } => !land.allow.allow_dangling,
          LandingRefusal::MissingDeclaration { .. } => !land.allow.allow_missing,
        })
        .map(|refusal| Reason::LandingRefused {
          refusal: Box::new(refusal.clone()),
        }),
    ),
  }
  if let Some(conflict) = facts.target_conflict {
    decision.blockers.push(Reason::TargetConflict {
      commit: conflict.commit.to_string(),
      files: conflict.files.to_vec(),
    });
  }

  // a robot starts from the open pull request of the subject into the target, of a branch also from one into a virtual
  // branch once it is retargeted (D11); a `/patronus` comment takes no profile
  let start_pull_request = part
    .own
    .as_ref()
    .filter(|(_, base)| facts.github && starts_from(base))
    .map(|(pull_request, _)| *pull_request);
  let profile = match (start_pull_request, options) {
    (Some(_), _) | (None, None) => None,
    (None, Some(options)) => chosen_profile(options, facts.last_profile, land),
  };
  let skipped = skips_approvals(land.mode, land.create_pull_request, land.without_approvals, start_pull_request.is_some(), profile.as_ref());
  decision.approvals_skipped_by = skipped;
  if let Some(pull_request) = start_pull_request.filter(|pull_request| pull_request.is_draft && safe_push) {
    decision.blockers.push(Reason::Draft { number: pull_request.number });
  }
  // D3b: Patronus merges a pull request into its base, so the approvals of one into another base count for no Safe Push
  // into the target; it may be meant, so it is never retargeted silently
  if let Some((pull_request, PullRequestBase::Other { base })) = &part.own
    && safe_push
    && skipped.is_none()
  {
    decision.blockers.push(Reason::OtherBase {
      number: pull_request.number,
      base: base.clone(),
      target: facts.target.to_string(),
    });
  }
  let (review, missing) = review_part(facts, land, &part, start_pull_request, skipped, &mut decision);

  let mut confirmations = Vec::new();
  if part.lands_more > 0 {
    confirmations.push(Confirmation::LandsMoreThanRow { count: part.lands_more });
  }
  if let Some(profile) = &profile {
    confirmations.push(Confirmation::NonDefaultProfile { id: profile.id.clone() });
  }
  if let Some(robot_id) = part.cancels {
    confirmations.push(Confirmation::CancelsRobot { robot_id: robot_id.to_string() });
  }
  confirmations.extend(review);
  confirmations.extend(rules_part(facts, land, &part, start_pull_request, &mut decision));
  if missing && skipped == Some(ApprovalsSkip::WithoutApprovals) {
    confirmations.push(Confirmation::WithoutApprovals);
  }
  if let Some(number) = part.whole_stack_pull_request.filter(|_| safe_push) {
    confirmations.push(Confirmation::StackPullRequestApprovalsDontCount { number });
  }
  confirmations.extend(part.forced.iter().cloned());
  decision.confirmations = confirmations;

  decision.effects.extend(part.retargets.iter().cloned());
  if let Some(robot_id) = part.cancels {
    decision.effects.push(PlannedEffect::CancelRobot { robot_id: robot_id.to_string() });
  }
  if !part.push.is_empty() {
    decision.effects.push(PlannedEffect::Push {
      branches: part.push.clone(),
      force: part.force,
    });
  }
  let found = part.own.as_ref().filter(|(_, base)| starts_from(base)).map(|(pull_request, _)| *pull_request);
  if land.create_pull_request {
    decision.pull_request = found.map(|pull_request| pull_request.number);
    if found.is_none() {
      decision.effects.push(PlannedEffect::CreatePullRequest);
    }
  } else {
    decision.pull_request = start_pull_request.map(|pull_request| pull_request.number);
    let route = match start_pull_request {
      Some(pull_request) => StartRoute::Comment { number: pull_request.number },
      None => StartRoute::Api { profile },
    };
    decision.effects.push(PlannedEffect::StartRobot { route });
  }
  decision.lands = part.lands;
  decision
}

/// What the start of a branch lands and pushes, and what refuses it of the table and the robots
fn branch_part<'a>(facts: &LandFacts<'a>, land: &Land, branch: &BranchSubject<'a>, listed: Option<&'a PullRequestsView>, decision: &mut LandDecision) -> SubjectPart<'a> {
  let name = branch.start.name;
  let safe_push = land.mode != PatronusPushMode::DryRun;
  // the push takes the branch and the ancestors with work, nearest first up to the first one a running Safe Push holds;
  // one a merged Safe Push landed whole stays as it is
  let mut push: Vec<&LandsBranch<'a>> = Vec::new();
  let mut known = branch.tip.is_some();
  for (index, step) in branch.chain.iter().rev().enumerate() {
    let ancestor = index > 0;
    if ancestor && step.held {
      break;
    }
    if ancestor && step.landed_whole {
      continue;
    }
    match step.remote {
      None => known = false,
      Some(remote) if !remote.up_to_date => push.push(step),
      Some(_) => {}
    }
  }
  push.reverse();
  if !known {
    decision.waits.push(FactWait {
      fact: LandFact::Remote,
      failed: None,
    });
  }
  let pushes = !push.is_empty();
  decision.blockers.extend(branch.landed.clone());
  let robot = start_refusal(&Start::Branch(BranchStart { pushes, ..branch.start }), land.mode, facts.robots);
  let robot_refuses = robot.is_some();
  decision.blockers.extend(robot);
  let forced = drops(push.iter().map(|step| (step.name, step.remote)), land, decision);
  // a push into the branch while a running Safe Push the start doesn't replace holds it (a Dry Run replaces none)
  // fails that Safe Push at the end
  if !robot_refuses
    && branch.landed.is_none()
    && pushes
    && let Some(hold) = branch.start.hold.filter(|_| !safe_push || branch.replaces.is_none())
    && let Some(robot) = facts.robots.get(&hold.by.robot_id)
  {
    decision.blockers.push(held(name, hold, robot));
  }
  let open = |name: &str| {
    listed
      .and_then(|listed| listed.pull_requests.get(name))
      .filter(|pull_request| pull_request.state == PullRequestState::Open)
  };
  // the pull requests of what the push takes along and of the branch go into the target first, where every pull request
  // of a virtual branch goes (D11)
  let retargets = push
    .iter()
    .map(|step| step.name)
    .filter(|step| *step != name)
    .chain(std::iter::once(name))
    .filter_map(|of| {
      let pull_request = open(of)?;
      matches!(base_of(pull_request, facts.target, facts.prefix), PullRequestBase::Virtual { .. }).then(|| PlannedEffect::Retarget {
        branch: of.to_string(),
        number: pull_request.number,
        from: pull_request.base_ref_name.clone(),
        to: facts.target.to_string(),
      })
    })
    .collect();
  let lands: Vec<String> = branch
    .chain
    .iter()
    .filter(|step| step.name == name || !step.landed_whole)
    .map(|step| step.name.to_string())
    .collect();
  SubjectPart {
    lands_more: u32::try_from(lands.len().saturating_sub(1)).unwrap_or(u32::MAX),
    lands,
    push: push.iter().map(|step| step.name.to_string()).collect(),
    force: !forced.is_empty(),
    own: open(name).map(|pull_request| (pull_request, start_base(pull_request, facts.target, facts.prefix, false))),
    retargets,
    cancels: branch.replaces.filter(|_| safe_push && (pushes || !land.create_pull_request)),
    forced,
    reviewable: true,
    whole_stack_pull_request: branch.whole_stack_pull_request,
  }
}

/// What the start of a whole stack lands and pushes (its stack branch), and what refuses it of the table and the robots
fn stack_part<'a>(facts: &LandFacts<'a>, land: &Land, stack: &StackSubject<'a>, listed: Option<&'a PullRequestsView>, decision: &mut LandDecision) -> SubjectPart<'a> {
  let root = stack.start.stack.root.as_str();
  let safe_push = land.mode != PatronusPushMode::DryRun;
  decision.blockers.extend(stack.landed.clone());
  let robot = start_refusal(&Start::WholeStack(stack.start), land.mode, facts.robots);
  // a stack branch that is no build of its commits is said first: nothing of it is reviewed or pushed as it should be
  let not_built = matches!(robot, Some(Reason::StackNotBuilt { .. }));
  let (not_built_refusal, robot) = if not_built { (robot, None) } else { (None, robot) };
  decision.blockers.extend(not_built_refusal);
  let pushes = match stack.remote {
    None => {
      decision.waits.push(FactWait {
        fact: LandFact::Remote,
        failed: None,
      });
      false
    }
    Some(remote) => !remote.up_to_date,
  };
  let forced = if pushes {
    drops(std::iter::once((root, stack.remote)), land, decision)
  } else {
    Vec::new()
  };
  let robot_refuses = robot.is_some();
  decision.blockers.extend(robot);
  if !robot_refuses
    && !not_built
    && stack.landed.is_none()
    && pushes
    && let Some(robot) = stack.held_by.filter(|_| !safe_push || stack.replaces.is_none())
  {
    decision.blockers.push(Reason::StackHeld {
      root: root.to_string(),
      robot_id: robot.id.clone(),
      mode: robot.push_mode,
    });
  }
  // the whole stack's pull request is the one of its stack branch, which no retarget touches: one into a virtual branch
  // is another base for it
  let own = listed
    .and_then(|listed| listed.stack_pull_requests.get(root))
    .filter(|pull_request| pull_request.state == PullRequestState::Open)
    .map(|pull_request| (pull_request, start_base(pull_request, facts.target, facts.prefix, true)));
  SubjectPart {
    lands: stack.members.iter().filter(|(_, landed)| !landed).map(|(name, _)| (*name).to_string()).collect(),
    push: if pushes { vec![root.to_string()] } else { Vec::new() },
    force: !forced.is_empty(),
    own,
    retargets: Vec::new(),
    cancels: stack.replaces.filter(|_| safe_push && (pushes || !land.create_pull_request)),
    forced,
    reviewable: !not_built,
    // the whole stack lands every branch of it with its root, always more than the root's row
    lands_more: u32::try_from(stack.members.len().saturating_sub(1)).unwrap_or(u32::MAX),
    whole_stack_pull_request: None,
  }
}

/// The remote branches of `pushed` whose push drops commits the branch lacks (someone's work): each refuses the start
/// unless the user allowed it (D12), which makes it a confirmation
fn drops<'a>(pushed: impl Iterator<Item = (&'a str, Option<RemoteBranch<'a>>)>, land: &Land, decision: &mut LandDecision) -> Vec<Confirmation> {
  let mut forced = Vec::new();
  for (branch, remote) in pushed {
    let Some(remote) = remote.filter(|remote| remote.drops > 0) else {
      continue;
    };
    if land.allow.allow_force {
      forced.push(Confirmation::ForcePushDrops {
        branch: branch.to_string(),
        commits: remote.drops,
      });
    } else {
      decision.blockers.push(Reason::DropsRemote {
        branch: branch.to_string(),
        behind: remote.drops,
      });
    }
  }
  forced
}

/// The code-owner review of a Safe Push (never a Dry Run, nor a whole stack whose stack branch is no build of its
/// commits): missing approvals refuse it unless skipped (or the start opens the pull request where they count),
/// approvals Patronus judges only at the start and a review it couldn't answer are confirmations; with whether the
/// approvals are missing
fn review_part(
  facts: &LandFacts<'_>,
  land: &Land,
  part: &SubjectPart<'_>,
  start_pull_request: Option<&PullRequest>,
  skipped: Option<ApprovalsSkip>,
  decision: &mut LandDecision,
) -> (Vec<Confirmation>, bool) {
  let mut confirmations = Vec::new();
  if land.mode == PatronusPushMode::DryRun || !part.reviewable {
    return (confirmations, false);
  }
  // GitHub didn't list the pull requests: the pull request a robot would start from, and so where the approvals count, is
  // not known
  if matches!(facts.pull_requests.head.state, FactState::Unavailable { .. }) {
    confirmations.push(Confirmation::ApprovalsJudgedAtStart);
  }
  let answer = match facts.readiness {
    None => {
      decision.waits.push(FactWait {
        fact: LandFact::Readiness,
        failed: None,
      });
      return (confirmations, false);
    }
    Some(Fact {
      state: FactState::Unavailable { reason },
      ..
    }) => {
      confirmations.push(Confirmation::ReviewUnknown { message: reason.message.clone() });
      return (confirmations, false);
    }
    Some(Fact {
      state: FactState::Fresh,
      value: Some(answer),
      ..
    }) => answer,
    // an earlier answer of other inputs, or none yet: the review of the subject as it is is not known
    Some(fact) => {
      decision.waits.push(wait(LandFact::Readiness, fact));
      return (confirmations, false);
    }
  };
  let mut missing = false;
  match review_state(Some(&answer.readiness), answer.requirement.as_ref(), facts.github, start_pull_request) {
    ReviewState::Missing { groups, .. } => {
      missing = true;
      if skipped.is_none() && !land.create_pull_request {
        decision.blockers.push(Reason::ApprovalsMissing {
          groups: groups.iter().map(|group| group.name.clone()).collect(),
        });
      }
    }
    ReviewState::Unknown => {
      if !confirmations.contains(&Confirmation::ApprovalsJudgedAtStart) {
        confirmations.push(Confirmation::ApprovalsJudgedAtStart);
      }
    }
    ReviewState::Unavailable { message } => confirmations.push(Confirmation::ReviewUnknown { message: message.to_string() }),
    ReviewState::Pending | ReviewState::NotRequired | ReviewState::Approved => {}
  }
  (confirmations, missing)
}

/// What a Safe Push that pushes new commits into the pull request it starts from, which someone approved, does to the
/// approvals by the rules of the target: the approvals Patronus counts dismissed refuse it, others dismissed are said,
/// rules GitHub didn't answer are a confirmation (never "keeps them"), rules not read yet a wait
fn rules_part(facts: &LandFacts<'_>, land: &Land, part: &SubjectPart<'_>, start_pull_request: Option<&PullRequest>, decision: &mut LandDecision) -> Option<Confirmation> {
  if land.mode == PatronusPushMode::DryRun || part.push.is_empty() || land.create_pull_request {
    return None;
  }
  let pull_request = start_pull_request.filter(|pull_request| pull_request.reviewers.iter().any(|reviewer| reviewer.state == ReviewerState::Approved))?;
  let number = pull_request.number;
  let counted = facts
    .readiness
    .and_then(|fact| fact.value.as_ref().filter(|_| fact.state == FactState::Fresh))
    .is_some_and(|answer| matches!(answer.readiness, ReviewReadiness::Approved { .. }));
  match dismisses_approvals(facts.rules, pull_request) {
    RulesAnswer::Keep => None,
    RulesAnswer::Dismiss if counted => {
      decision.blockers.push(Reason::DismissesApprovals {
        number,
        target: facts.target.to_string(),
      });
      None
    }
    RulesAnswer::Dismiss => Some(Confirmation::DismissesApprovals { number }),
    RulesAnswer::Unknown { .. } => Some(Confirmation::RulesUnknown { number }),
    RulesAnswer::Loading => {
      decision.waits.push(FactWait {
        fact: LandFact::Rules { number },
        failed: None,
      });
      None
    }
  }
}

/// The answer of `fact` that stands: the newest, or an earlier one after a read that failed; none while it is read for
/// the first time or again for other inputs
#[must_use]
pub const fn standing<T>(fact: &Fact<T>) -> Option<&T> {
  match fact.state {
    FactState::Fresh | FactState::Stale { reason: Some(_) } => fact.value.as_ref(),
    _ => None,
  }
}

/// A wait for `fact`, with why its read failed when it did
fn wait<T>(of: LandFact, fact: &Fact<T>) -> FactWait {
  FactWait {
    fact: of,
    failed: match &fact.state {
      FactState::Unavailable { reason } => Some(reason.clone()),
      _ => None,
    },
  }
}
