//! The next step of a branch or of a whole stack, for the app and `bd` alike (spec/branch/next-step.spec.md, "The
//! table", "Facts without an answer", "The whole stack"; docs/design/rust-domain-authority.md, R6): `next_step` is the
//! first row of the table that matches over `StepFacts`, the facts of the subject and of the remote as the session (or
//! `bd`) holds them, each of which may have no answer yet.
//!
//! - Row 0 waits for the first fact the rows below would read without an answer (Patronus, the pull requests, the
//!   landing needs, why the robot failed, the readiness): never a step a later answer turns into another. A reason of
//!   the subject (a sync, a failure, a block, parked, landed, a robot of a branch below) outranks it, a known landing
//!   need too. A read that failed is an answer, never "no": the step is what helps (`StepHelp`), the starts disabled.
//! - Row 1 has nothing to start: the Status cell says why. Row 1′ needs a branch outside what the Safe Push lands. Rows
//!   2 and 3 restart a failed robot; 5 and 5′ a pull request into another base; 6 a draft; 7 to 10 the approvals that
//!   count are missing; 8′ and 10′ the wait GitHub tells while Patronus can't judge them; 11 "Safe Push".
//! - A stale answer of the readiness keeps its step, its action waits for the new one (`stale_readiness`).
//! - "Safe Push elsewhere" is the step's `safe_push`; a Dry Run (`dry_run`) is never held back by the approvals, a
//!   draft, another base or a robot that lands an ancestor.
//!
//! The whole stack of a root that forks takes the same table over the facts of its stack branch: its robot, the robot
//! of a branch of it that lands a part of it, its own pull request (never retargeted), the readiness of `stack:<root>`.
//! What runs on the row (an operation, a start, a push) is the page's until the operations move into Rust (s10).

use crate::facts::RemoteBranch;
use crate::holds::is_active;
use crate::readiness::waiting_reviewers;
use domain_types::error::ErrorCode;
use domain_types::landing::{Landed, LandedBy, LandingRefusal, Subject};
use domain_types::next_step::{NextStep, StackPullRequest, StepBlocker, StepFact, StepFailure, StepHelp, StepKind, StepRetarget, StepRow, WithoutReview};
use domain_types::patronus::{
  MissingApprovalsReason, MissingReviewGroup, PatronusAvailability, PatronusPushMode, PatronusRobot, PatronusRobotStatus, ReviewReadiness, UnavailableReadinessReason,
};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestBase, PullRequestState, ReviewerState};
use domain_types::reason::Reason;
use domain_types::repository_view::LandingNeeds;
use domain_types::session::{Fact, FactState, Parked, RobotsHead, SubjectReadiness};

/// How Patronus answers for the repository, as the next step reads it
#[derive(Clone, Copy, Debug)]
pub enum PatronusAnswer<'a> {
  /// No answer yet, or the view of the robots it listed first isn't in: nobody knows what they hold and land
  Pending,
  /// The repository is not on Space: no slot
  NotOnSpace,
  /// It answers for the repository; `failure` a read of it that failed (the Safe Push CLI not logged in, an error
  /// before it ever listed the robots, a view the session couldn't derive)
  Answers { failure: Option<&'a StepFailure> },
}

/// How Patronus stands for the repository, read once of its facts (`of`): what `PatronusAnswer` borrows
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatronusStanding {
  Pending,
  NotOnSpace,
  Answers(Option<StepFailure>),
}

impl PatronusStanding {
  /// Of the head of the robots of the session (`RobotsView::head`) and how the view of what they hold and land stands
  /// (`PatronusPart::state`). No answer is pending; an answer that the repository is available is pending until the
  /// view of the robots it listed first is in. A failure is an answer: not logged in, an error before any list (an
  /// error after one leaves the steps on the robots it listed, `Stale`), a view that failed without an earlier one
  #[must_use]
  pub fn of(head: &Fact<RobotsHead>, view: &FactState) -> Self {
    let Some(answered) = &head.value else {
      return Self::Pending;
    };
    let failure = match &answered.availability {
      PatronusAvailability::NotPatronus => return Self::NotOnSpace,
      PatronusAvailability::NotLoggedIn => Some(StepFailure::NotLoggedIn),
      PatronusAvailability::Error { message } if !matches!(head.state, FactState::Stale { .. }) => Some(StepFailure::PatronusError { message: message.clone() }),
      PatronusAvailability::Available if matches!(view, FactState::Loading | FactState::Unknown) => return Self::Pending,
      PatronusAvailability::Available | PatronusAvailability::Error { .. } => None,
    };
    let failure = failure.or_else(|| match view {
      FactState::Unavailable { reason } => Some(StepFailure::ViewFailed { reason: reason.clone() }),
      _ => None,
    });
    Self::Answers(failure)
  }

  #[must_use]
  pub const fn answer(&self) -> PatronusAnswer<'_> {
    match self {
      Self::Pending => PatronusAnswer::Pending,
      Self::NotOnSpace => PatronusAnswer::NotOnSpace,
      Self::Answers(failure) => PatronusAnswer::Answers { failure: failure.as_ref() },
    }
  }
}

/// What GitHub lists of the pull requests of the repository
#[derive(Clone, Copy, Debug)]
pub enum Listing<'a> {
  /// No answer yet: no pull request is known, never "none"
  Loading,
  /// A listing stands: GitHub lists them, or failed for a while after a listing, whose pull requests stay
  Listed,
  /// GitHub can't list them; `why` the availability that says so, `None` off GitHub
  Unlisted { why: Option<&'a PullRequestAvailability> },
}

impl<'a> Listing<'a> {
  /// Of the head of the listing of the session (`PullRequestsView::head`): none before GitHub's first answer; a listing
  /// stands while GitHub lists them and through a failed refresh after one (`Stale`); otherwise GitHub can't list them
  #[must_use]
  pub const fn of(head: &'a Fact<PullRequestAvailability>) -> Self {
    match (&head.value, &head.state) {
      (None, _) => Self::Loading,
      (Some(_), FactState::Stale { .. }) | (Some(PullRequestAvailability::Available { .. }), FactState::Fresh) => Self::Listed,
      (Some(PullRequestAvailability::NotGitHub), _) => Self::Unlisted { why: None },
      (Some(availability), _) => Self::Unlisted { why: Some(availability) },
    }
  }
}

/// What the Safe Push of a branch needs of a branch outside what it lands, as the session computes it
#[derive(Clone, Copy, Debug)]
pub enum Needs<'a> {
  /// Computed for the view the rows read: the step waits for it
  Computing,
  /// The refusals the landing gate would give its start; empty: nothing
  Known(&'a [LandingRefusal]),
  /// Not computed (a failed computation, a whole stack: the gate of its start checks it)
  Unknown,
}

impl<'a> Needs<'a> {
  /// Of what the view has of a branch (`BranchPatronus::landing_needs`; `None` none of it) and how the landing needs of
  /// the view stand (`PatronusPart::landing_needs`): computing while they have no answer for a branch they don't know
  /// yet; a computation that failed waits for nothing (the gate of the start checks), nor a branch they answered none
  /// of
  #[must_use]
  pub fn of(needs: Option<&'a LandingNeeds>, state: &FactState) -> Self {
    match needs {
      Some(LandingNeeds::Known { refusals }) => Self::Known(refusals),
      _ if matches!(state, FactState::Loading | FactState::Stale { .. } | FactState::Unknown) => Self::Computing,
      _ => Self::Unknown,
    }
  }
}

/// The robot of the subject, with what the session learned of it
#[derive(Clone, Copy, Debug)]
pub struct StepRobot<'a> {
  pub robot: &'a PatronusRobot,
  /// It failed for missing approvals: `Some(true)`, not for them `Some(false)`, `None` while Patronus hasn't told why
  pub needs_review: Option<bool>,
  /// Why Patronus didn't give the problems of its failure
  pub problems_failed: Option<&'a str>,
  /// A required check of the running robot failed: it will fail
  pub failing: bool,
  /// It offers a restart (`RepositoryView.patronus.view.restarts`)
  pub restart: bool,
}

/// The pull request of the subject in any state, with where an open one goes as the rules read it
#[derive(Clone, Copy, Debug)]
pub struct StepPullRequest<'a> {
  pub pull_request: &'a PullRequest,
  /// `None` for one that isn't open, or of no known base
  pub base: Option<&'a PullRequestBase>,
}

/// Everything the next step of a subject is decided on: the subject as the table and the robots have it, and the facts
/// of the remote services as the session (or `bd`) holds them
#[derive(Clone, Copy, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts of the subject, each a row of the table")]
pub struct StepFacts<'a> {
  /// A branch, or the whole stack of a root that forks
  pub subject: &'a Subject,
  pub patronus: PatronusAnswer<'a>,
  /// The sync runs on it (of a whole stack: on a branch of it)
  pub syncing: bool,
  /// It failed to sync
  pub sync_failed: bool,
  /// The reason of its stack problem, where the sync says it
  pub problem: Option<&'a str>,
  pub commit_count: usize,
  /// A stacked branch whose parent failed to build
  pub blocked: bool,
  /// Why a Safe Push can't start by the robots (`landing.start` of the view: a robot on its commit, one that lands it
  /// or an ancestor of it), and a Dry Run
  pub start: Option<&'a Reason>,
  pub dry_run_start: Option<&'a Reason>,
  /// A running Safe Push of another branch lands a part of it (an ancestor; of a whole stack a branch of it): the
  /// failure of its robot, of an earlier piece, waits for that robot
  pub landing_part: bool,
  /// A running Safe Push holds its pushes: it runs on it, or lands it
  pub held: bool,
  /// It stays local: its own line or a parked branch it is built on (of a whole stack a parked member, `with`)
  pub parked: Option<&'a Parked>,
  /// What landed it, as the view says (`landedStates`)
  pub landed: Option<&'a Landed>,
  /// The tip of its virtual branch (of the stack branch), and its remote branch as a remote status told it
  pub tip: Option<&'a str>,
  pub remote: Option<RemoteBranch<'a>>,
  pub robot: Option<StepRobot<'a>>,
  /// Its merged robot landed all of it as it is now, by content: a rebuild since gave it new hashes, not new work
  pub own_robot_landed: bool,
  pub listing: Listing<'a>,
  pub pull_request: Option<StepPullRequest<'a>>,
  /// The branch Patronus pushes into, as a pull request names its base ("master")
  pub target: &'a str,
  pub needs: Needs<'a>,
  /// The readiness of the subject; `None` while nobody asked
  pub readiness: Option<&'a Fact<SubjectReadiness>>,
  /// The open pull request of the whole stack the branch is in
  pub stack_pull_request: Option<&'a StackPullRequest>,
  /// The GitHub repository of the branches is known: a pull request can be created
  pub repository: bool,
  /// The branches a Safe Push of it lands now (1 for a lone branch): it and those it is built on that are not landed
  /// whole; of a whole stack what is left of it
  pub lands: u32,
}

/// The robot of the row, as its slot shows it (`getSafePushSlot` of the page before)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
  /// No robot to show, nothing to start (Patronus doesn't answer, the branch can't be pushed)
  None,
  /// None to show: "Safe Push" starts one (also for a robot of an earlier state of the subject)
  Start,
  /// The live robot: `dry_run` checks it, `failing` a required check failed
  Running { dry_run: bool, failing: bool },
  /// A finished robot of the subject as it is
  Finished(Chip),
}

/// How a finished robot ended
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Chip {
  Failed,
  /// Failed for missing code-owner approvals
  NeedsReview,
  Canceled,
  Merged,
  /// A Dry Run that passed
  Passed,
  Unknown,
}

/// The slot of the robot of the subject
pub(crate) fn slot(facts: &StepFacts<'_>) -> Slot {
  let answers = matches!(facts.patronus, PatronusAnswer::Answers { .. });
  let offered = answers && !facts.sync_failed && facts.commit_count > 0;
  let Some(robot) = facts.robot else {
    return if offered { Slot::Start } else { Slot::None };
  };
  let it = robot.robot;
  if is_active(it) {
    return Slot::Running {
      dry_run: it.push_mode == PatronusPushMode::DryRun,
      failing: it.status == PatronusRobotStatus::Running && robot.failing,
    };
  }
  // a merged robot is done; a subject with other commits since (the same name, new work) starts anew
  if offered && !facts.own_robot_landed && is_outdated(it.source_revision.as_deref(), facts.tip, facts.remote) {
    return Slot::Start;
  }
  Slot::Finished(match it.status {
    PatronusRobotStatus::Failure if robot.needs_review == Some(true) => Chip::NeedsReview,
    PatronusRobotStatus::Failure => Chip::Failed,
    PatronusRobotStatus::Success if it.push_mode == PatronusPushMode::DryRun => Chip::Passed,
    PatronusRobotStatus::Success => Chip::Merged,
    PatronusRobotStatus::Canceled => Chip::Canceled,
    PatronusRobotStatus::Running | PatronusRobotStatus::Canceling | PatronusRobotStatus::Unknown => Chip::Unknown,
  })
}

/// The robot took other commits than the subject has: changed locally since (the remote lacks work), or on the remote
fn is_outdated(source: Option<&str>, tip: Option<&str>, remote: Option<RemoteBranch<'_>>) -> bool {
  let (Some(source), Some(remote)) = (source, remote) else {
    return false;
  };
  source != tip.unwrap_or_default() && (!remote.up_to_date || remote.tip != Some(source))
}

/// The next step of the subject of `facts`: the first row of the table that matches (module docs)
#[must_use]
pub fn next_step(facts: &StepFacts<'_>) -> NextStep {
  let slot = slot(facts);
  let landed = landed_whole(facts, slot);
  let general = start_reason(facts, facts.start, landed.as_ref());
  let dry_run = start_reason(facts, facts.dry_run_start, landed.as_ref());
  let mut step = decide(facts, slot, general, dry_run, landed.as_ref());
  step.landed = landed;
  step.lands = facts.lands;
  step.unlisted = match facts.listing {
    Listing::Unlisted { why } => why.cloned(),
    Listing::Loading | Listing::Listed => None,
  };
  step
}

/// What landed all of the subject: the view's landed state with nothing after it; before the view, its own robot that
/// merged
pub(crate) fn landed_whole(facts: &StepFacts<'_>, slot: Slot) -> Option<Landed> {
  if let Some(landed) = facts.landed.filter(|landed| landed.is_whole()) {
    return Some(landed.clone());
  }
  let robot = facts.robot.filter(|_| slot == Slot::Finished(Chip::Merged))?.robot;
  Some(Landed {
    by: LandedBy::Robots {
      by: vec![facts.subject.robot_of(&robot.id)],
      own: true,
      target: robot.target_branch.clone(),
    },
    after: Vec::new(),
  })
}

/// The reason a start says of what landed all of `subject`
fn landed_reason(subject: &Subject, landed: &Landed) -> Reason {
  match &landed.by {
    LandedBy::Baseline { upstream } => Reason::LandedIn { upstream: upstream.clone() },
    LandedBy::PullRequest { number } => Reason::PullRequestMerged { number: *number },
    LandedBy::Robots { by, own, target } => match subject {
      Subject::Branch { name } => Reason::Landed {
        branch: name.clone(),
        by: by.first().cloned().unwrap_or_else(|| subject.robot_of("")),
        own: *own,
        target: target.clone(),
      },
      Subject::WholeStack { root } => Reason::StackLanded {
        root: root.clone(),
        by: by.clone(),
        own: *own,
        target: target.clone(),
      },
    },
  }
}

/// The reason a subject parked stays local with
fn parked_reason(subject: &Subject, parked: &Parked) -> Reason {
  match subject {
    Subject::Branch { name } => Reason::Parked {
      branch: name.clone(),
      with: parked.with.clone(),
      note: parked.note.clone(),
    },
    Subject::WholeStack { root } => Reason::StackParked {
      root: root.clone(),
      member: parked.with.clone().unwrap_or_else(|| root.clone()),
    },
  }
}

/// Why no start is possible now, whatever the approvals: the first of the reasons of the subject; `start` the reason of
/// the robots for the mode
fn start_reason(facts: &StepFacts<'_>, start: Option<&Reason>, landed: Option<&Landed>) -> Option<StepBlocker> {
  if facts.sync_failed {
    return Some(StepBlocker::SyncFailed {
      problem: facts.problem.map(str::to_owned),
    });
  }
  if facts.commit_count == 0 {
    return Some(StepBlocker::NoCommits);
  }
  if facts.syncing {
    return Some(StepBlocker::Syncing);
  }
  let rule = facts
    .parked
    .map(|parked| parked_reason(facts.subject, parked))
    .or_else(|| start.cloned())
    .or_else(|| landed.map(|landed| landed_reason(facts.subject, landed)))?;
  Some(StepBlocker::Rule { reason: rule })
}

/// A step of `row` and `kind` with its blockers, nothing else
const fn step(row: StepRow, kind: StepKind, safe_push: Option<StepBlocker>, dry_run: Option<StepBlocker>) -> NextStep {
  NextStep {
    row,
    kind,
    safe_push,
    dry_run,
    needs_readiness: false,
    stale_readiness: false,
    landed: None,
    retarget: None,
    without_review: None,
    lands: 1,
    unlisted: None,
  }
}

/// Row 1: nothing to start now, the Status cell says why
const fn settled(general: Option<StepBlocker>, dry_run: Option<StepBlocker>) -> NextStep {
  step(StepRow::One, StepKind::Settled, general, dry_run)
}

/// Row 0: `fact` has no answer yet; every Safe Push says it, a Dry Run only while Patronus hasn't answered
fn pending(fact: StepFact, general: Option<StepBlocker>, dry_run: Option<StepBlocker>) -> NextStep {
  let dry_run = if fact == StepFact::Patronus { dry_run.or(Some(StepBlocker::Step)) } else { dry_run };
  NextStep {
    needs_readiness: fact == StepFact::Readiness,
    ..step(StepRow::Zero, StepKind::Pending { fact }, general.or(Some(StepBlocker::Step)), dry_run)
  }
}

/// Row 0 of a failed read: what helps (a login to copy, a refresh), else the restart of the robot disabled with the
/// failure; while Patronus fails every Dry Run says it too
fn unavailable(facts: &StepFacts<'_>, failure: &StepFailure, general: Option<StepBlocker>, dry_run: Option<StepBlocker>) -> NextStep {
  let help = match failure {
    StepFailure::NotLoggedIn => Some(StepHelp::CopyLoginCommand),
    StepFailure::PatronusError { .. } | StepFailure::ViewFailed { .. } => Some(StepHelp::RefreshPatronus),
    StepFailure::ProblemsFailed { .. } => None,
  };
  let robot_id = facts.robot.filter(|robot| help.is_none() && robot.restart).map(|robot| robot.robot.id.clone());
  let dry_run = if failure.fact() == StepFact::Patronus {
    dry_run.or(Some(StepBlocker::Step))
  } else {
    dry_run
  };
  let kind = StepKind::Unavailable {
    failure: failure.clone(),
    help,
    robot_id,
  };
  step(StepRow::Zero, kind, general.or(Some(StepBlocker::Step)), dry_run)
}

/// The robot of the subject failed and Patronus hasn't told why yet: a restart may fail again for the approvals
pub(crate) fn problems_pending(robot: Option<StepRobot<'_>>) -> bool {
  robot.is_some_and(|robot| robot.robot.status == PatronusRobotStatus::Failure && robot.needs_review.is_none() && robot.problems_failed.is_none())
}

/// Patronus didn't give why the robot of the subject failed
pub(crate) fn problems_failed(robot: Option<StepRobot<'_>>) -> Option<StepFailure> {
  let robot = robot.filter(|robot| robot.robot.status == PatronusRobotStatus::Failure)?;
  Some(StepFailure::ProblemsFailed {
    message: robot.problems_failed?.to_owned(),
  })
}

/// Row 1: the subject has nothing to start now, and its Status cell says why
const fn is_settled(facts: &StepFacts<'_>, slot: Slot, landed: Option<&Landed>) -> bool {
  !matches!(facts.patronus, PatronusAnswer::Answers { .. })
    || facts.syncing
    || facts.sync_failed
    || facts.commit_count == 0
    || facts.blocked
    || facts.held
    || facts.parked.is_some()
    || landed.is_some()
    || matches!(slot, Slot::Running { .. } | Slot::None)
}

fn decide(facts: &StepFacts<'_>, slot: Slot, general: Option<StepBlocker>, dry_run: Option<StepBlocker>, landed: Option<&Landed>) -> NextStep {
  let failure = match facts.patronus {
    // before the answer of Patronus nobody knows whether it lands the subject, nor what its robots hold and land; what
    // the sync tells and a robot it listed (one that runs, one that merged) still say why nothing starts
    PatronusAnswer::Pending => {
      return if general.is_some() || matches!(slot, Slot::Running { .. } | Slot::Finished(Chip::Merged)) {
        settled(general, dry_run)
      } else {
        pending(StepFact::Patronus, general, dry_run)
      };
    }
    // the surfaces offer a start only where Patronus answers: the blockers stay those of the subject
    PatronusAnswer::NotOnSpace if slot == Slot::None => return step(StepRow::NoSlot, StepKind::Hidden, general, dry_run),
    PatronusAnswer::NotOnSpace => None,
    PatronusAnswer::Answers { failure } => failure,
  };
  if is_settled(facts, slot, landed) {
    return settled(general, dry_run);
  }
  // Patronus answers with a failure: its robots are unknown, no start is decided on them
  if let Some(failure) = failure
    && general.is_none()
  {
    return unavailable(facts, failure, general, dry_run);
  }
  let needs = match facts.needs {
    Needs::Known(refusals) if !refusals.is_empty() => Some(refusals),
    _ => None,
  };
  // the rows past 1′ read the pull requests and why a robot failed: a reason of the subject that holds every start
  // back says more than "Checking…"
  if needs.is_none() && general.is_none() {
    let waits = if matches!(facts.listing, Listing::Loading) {
      Some(StepFact::PullRequests)
    } else if matches!(facts.needs, Needs::Computing) {
      Some(StepFact::LandingNeeds)
    } else {
      problems_pending(facts.robot).then_some(StepFact::RobotProblems)
    };
    if let Some(fact) = waits {
      return pending(fact, general, dry_run);
    }
    // why the robot failed is unknown: a restart may fail again for the approvals
    if failure.is_none()
      && let Some(failed) = problems_failed(facts.robot)
    {
      return unavailable(facts, &failed, general, dry_run);
    }
  }
  match needs {
    // row 1′: the Safe Push stays enabled, its popover says why before the start
    Some(refusals) => step(StepRow::OnePrime, StepKind::NeedsBranch { refusals: refusals.to_vec() }, general, dry_run),
    None => Rows::of(facts, slot, general, dry_run).decide(),
  }
}

/// The readiness of a subject as the step reads it: none while it loads or nobody asked; a failed read is
/// `unavailable`, never a stale answer the start would wait on; `stale` while it is asked again for new inputs
fn readiness_of(fact: Option<&Fact<SubjectReadiness>>) -> Option<(ReviewReadiness, bool)> {
  let fact = fact?;
  let failure = match &fact.state {
    FactState::Loading | FactState::Unknown => return None,
    FactState::Unavailable { reason } | FactState::Stale { reason: Some(reason) } => Some(reason),
    FactState::Fresh | FactState::Stale { reason: None } => None,
  };
  if let Some(reason) = failure {
    let unavailable = if reason.code == ErrorCode::NotLoggedIn {
      UnavailableReadinessReason::NotLoggedIn
    } else {
      UnavailableReadinessReason::Error
    };
    return Some((
      ReviewReadiness::Unavailable {
        reason: unavailable,
        message: reason.message.clone(),
      },
      false,
    ));
  }
  let value = fact.value.as_ref()?;
  Some((value.readiness.clone(), fact.state != FactState::Fresh))
}

/// Rows 2 to 11 of a subject that can start; `general` is why a start can't whatever the approvals
struct Rows<'f, 'a> {
  facts: &'f StepFacts<'a>,
  slot: Slot,
  general: Option<StepBlocker>,
  dry_run: Option<StepBlocker>,
  /// The open pull request, while GitHub lists them
  open: Option<StepPullRequest<'a>>,
  /// The readiness, while GitHub lists the pull requests: without them the approvals are unknown
  readiness: Option<ReviewReadiness>,
  /// The answer of the readiness is stale: a new one loads
  stale: bool,
  /// Patronus judges the local commits: a subject on GitHub is asked, pushed or not
  asks: bool,
}

impl<'f, 'a> Rows<'f, 'a> {
  fn of(facts: &'f StepFacts<'a>, slot: Slot, general: Option<StepBlocker>, dry_run: Option<StepBlocker>) -> Self {
    let asks = matches!(facts.listing, Listing::Listed);
    let answer = readiness_of(facts.readiness);
    Self {
      facts,
      slot,
      general,
      dry_run,
      open: facts.pull_request.filter(|it| asks && it.pull_request.state == PullRequestState::Open),
      readiness: answer.as_ref().filter(|_| asks).map(|(readiness, _)| readiness.clone()),
      stale: answer.is_some_and(|(_, stale)| stale),
      asks,
    }
  }

  /// A step of these rows, its blockers the subject's, or the step's own reason behind them
  fn step(&self, row: StepRow, kind: StepKind, own_reason: bool) -> NextStep {
    let safe_push = self.general.clone().or_else(|| own_reason.then_some(StepBlocker::Step));
    step(row, kind, safe_push, self.dry_run.clone())
  }

  /// A step that depends on the readiness: the session asks it; a step of a stale answer stays, its action waits for
  /// the new one
  fn judged(&self, mut step: NextStep) -> NextStep {
    step.needs_readiness = self.asks;
    if self.stale {
      step.stale_readiness = true;
      step.safe_push = step.safe_push.or(Some(StepBlocker::StaleReadiness));
    }
    step
  }

  /// The readiness the rows read has no answer yet: they wait for it; a start waits for it itself
  const fn unanswered(&self) -> bool {
    self.asks && self.readiness.is_none() && self.general.is_none()
  }

  const fn approved(&self) -> bool {
    matches!(self.readiness, Some(ReviewReadiness::Approved { .. } | ReviewReadiness::NotRequired))
  }

  const fn unjudged(&self) -> bool {
    matches!(self.readiness, Some(ReviewReadiness::Unknown { .. } | ReviewReadiness::Unavailable { .. }))
  }

  /// "Safe Push" of row 11 (row 2 where Patronus can't restart, row 3 approved outside the robot's pull request)
  fn safe_push(&self, row: StepRow) -> NextStep {
    self.step(row, StepKind::SafePush, false)
  }
}

impl Rows<'_, '_> {
  fn decide(&self) -> NextStep {
    if let (Slot::Finished(chip), Some(robot)) = (self.slot, self.facts.robot) {
      let id = &robot.robot.id;
      match chip {
        // row 2; while a robot of a branch below lands a part of it, the failure waits for that robot: the steps of its
        // pull request stay, the Safe Push says why it waits
        Chip::Failed | Chip::Canceled if !self.facts.landing_part => {
          return if robot.restart {
            self.step(StepRow::Two, StepKind::Restart { robot_id: id.clone() }, false)
          } else {
            self.safe_push(StepRow::Two)
          };
        }
        Chip::NeedsReview => {
          if self.unanswered() {
            return pending(StepFact::Readiness, self.general.clone(), self.dry_run.clone());
          }
          // row 3: the restart keeps the pull request of the robot, it counts the approvals it has by now
          let started_from = robot.robot.pull_request.as_ref().map(|pull_request| pull_request.number);
          let linked = self.open.is_some_and(|open| into_target(open) && started_from == Some(open.pull_request.number));
          if linked && self.approved() && robot.restart {
            return self.judged(self.step(StepRow::Three, StepKind::Restart { robot_id: id.clone() }, false));
          }
          // nobody can tell whether the approvals count: a restart may fail again, the pull request collects them
          if self.readiness.is_none() || self.unjudged() {
            let number = self.open.map(|open| open.pull_request.number);
            return self.judged(if number.is_some() || self.facts.repository {
              self.step(StepRow::Three, StepKind::RobotPullRequest { number }, false)
            } else {
              self.safe_push(StepRow::Three)
            });
          }
        }
        Chip::Failed | Chip::Canceled | Chip::Merged | Chip::Passed | Chip::Unknown => {}
      }
    }
    if let Some(open) = self.open {
      let number = open.pull_request.number;
      // row 5: Patronus merges a pull request into its base, so a start from it would not land in the target
      if !into_target(open) {
        let base = open.pull_request.base_ref_name.clone();
        let retarget = Some(StepRetarget {
          number,
          base: base.clone(),
          target: self.facts.target.to_owned(),
        });
        // a parent's branch: every pull request of a virtual branch goes into the target, a start retargets it first
        // (D11); another base may be meant (a release branch): the step opens it, the retarget is in "⋯"
        let (row, kind, own) = if self.facts.subject.is_whole_stack() || !matches!(open.base, Some(PullRequestBase::Virtual { .. })) {
          (StepRow::FivePrime, StepKind::OtherBase { number, base }, true)
        } else {
          (StepRow::Five, StepKind::Retarget { number, base }, false)
        };
        return NextStep {
          retarget,
          ..self.step(row, kind, own)
        };
      }
      // row 6
      if open.pull_request.is_draft {
        return self.step(StepRow::Six, StepKind::Draft { number }, true);
      }
      // rows 8′ and 10′: Patronus can't judge the approvals, GitHub tells what the pull request waits for; the start
      // stays possible, it checks them
      if self.unjudged()
        && let Some(step) = self.unjudged_wait(open.pull_request)
      {
        return self.judged(step);
      }
    }
    if self.unanswered() {
      return pending(StepFact::Readiness, self.general.clone(), self.dry_run.clone());
    }
    if let Some(ReviewReadiness::Missing { groups, reason, .. }) = &self.readiness {
      return self.judged(self.missing(groups, *reason));
    }
    // row 11; row 3 for a robot whose approvals arrived, but not in the pull request it was started from. GitHub signed
    // out or failing leaves row 7 possible: the start without review stays the user's choice
    let unlisted = matches!(self.facts.listing, Listing::Unlisted { why: Some(_) });
    NextStep {
      without_review: unlisted.then(|| WithoutReview { blocker: self.general.clone() }),
      ..self.judged(self.safe_push(StepRow::Eleven))
    }
  }

  /// Rows 8′ and 10′: a reviewer requested changes, or GitHub waits for a requested reviewer; `None` without a wait
  fn unjudged_wait(&self, pull_request: &PullRequest) -> Option<NextStep> {
    let number = pull_request.number;
    let by = changes_requested_by(pull_request);
    if !by.is_empty() {
      return Some(self.step(StepRow::EightPrime, StepKind::ChangesRequestedUnjudged { number, by }, false));
    }
    let reviewers: Vec<String> = waiting_reviewers(&pull_request.reviewers).into_iter().cloned().collect();
    (!reviewers.is_empty()).then(|| self.step(StepRow::TenPrime, StepKind::AwaitingUnjudged { number, reviewers }, false))
  }

  /// Rows 7 to 10: the approvals that count are missing, where they can come from
  fn missing(&self, groups: &[MissingReviewGroup], reason: MissingApprovalsReason) -> NextStep {
    let Some(open) = self.open else {
      // row 7: a start through the API counts no approval; the pull request of the whole stack collects the approvals
      // of its own Safe Push only
      let kind = StepKind::CreatePullRequest {
        groups: groups.to_vec(),
        stack: self.facts.stack_pull_request.cloned(),
        repository: self.facts.repository,
      };
      return NextStep {
        without_review: Some(WithoutReview { blocker: self.general.clone() }),
        ..self.step(StepRow::Seven, kind, true)
      };
    };
    let number = open.pull_request.number;
    let groups = with_requested_on_pull_request(groups, &waiting_reviewers(&open.pull_request.reviewers));
    // row 8: only the changes requested by a member of a group still short of approvals block, as Patronus decides
    if groups.iter().any(|group| !group.changes_requested_by.is_empty()) {
      return self.step(StepRow::Eight, StepKind::ChangesRequested { number, groups }, true);
    }
    // row 9: nobody of a group can approve (no request helps), or a group asks fewer people than it needs
    let no_eligible = reason == MissingApprovalsReason::NoEligibleReviewer;
    if no_eligible || groups.iter().any(is_short_of_requests) {
      let short: Vec<MissingReviewGroup> = groups.iter().filter(|group| is_short_of_requests(group)).cloned().collect();
      let groups = if short.is_empty() { groups } else { short };
      return self.step(StepRow::Nine, StepKind::RequestReview { number, groups, no_eligible }, true);
    }
    // row 10
    self.step(StepRow::Ten, StepKind::Awaiting { number, groups }, true)
  }
}

/// An open pull request merges into the target, or its base is unknown
fn into_target(open: StepPullRequest<'_>) -> bool {
  open.base.is_none_or(|base| matches!(base, PullRequestBase::Target))
}

/// The reviewers whose latest review requests changes and who weren't asked again since; bots left out
pub(crate) fn changes_requested_by(pull_request: &PullRequest) -> Vec<String> {
  pull_request
    .reviewers
    .iter()
    .filter(|reviewer| reviewer.state == ReviewerState::ChangesRequested && !reviewer.requested && !crate::readiness::is_bot(&reviewer.login))
    .map(|reviewer| reviewer.login.clone())
    .collect()
}

/// A missing group asks fewer people than the approvals it still needs: waiting can't satisfy it, Request Review… can
fn is_short_of_requests(group: &MissingReviewGroup) -> bool {
  u32::try_from(group.requested.len()).unwrap_or(u32::MAX) < group.needed.unwrap_or(1)
}

/// The groups with the review requests the pull request has now: a suggested member GitHub waits for (`waiting`) is
/// requested. Patronus is asked again once the pull request changes, and its kept answer lists the people requested
/// before: right after Request Review… the step is the wait for them, not the request again
fn with_requested_on_pull_request(groups: &[MissingReviewGroup], waiting: &[&String]) -> Vec<MissingReviewGroup> {
  let same = |login: &str, other: Option<&String>| other.is_some_and(|other| other.eq_ignore_ascii_case(login));
  groups
    .iter()
    .map(|group| {
      let asked: Vec<_> = group
        .suggested
        .iter()
        .filter(|person| {
          let login = person.login.as_ref();
          waiting.iter().any(|waits| same(waits, login))
            && !group.requested.iter().any(|it| login.is_some_and(|login| same(login, it.login.as_ref())))
            && !group.approved_by.iter().any(|it| login.is_some_and(|login| same(login, it.login.as_ref())))
        })
        .cloned()
        .collect();
      let mut group = group.clone();
      group.requested.extend(asked);
      group
    })
    .collect()
}
