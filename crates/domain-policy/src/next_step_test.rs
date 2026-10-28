use crate::facts::RemoteBranch;
use crate::next_step::{Listing, Needs, PatronusStanding, StepFacts, StepPullRequest, StepRobot, next_step};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{missing_approvals, pull_request, robot, robot_of, stack_commit};
use domain_types::landing::{Landed, LandedBy, LandingFix, LandingRefusal, Subject};
use domain_types::landing_route::{LandingRoute, RouteFact, RouteGate};
use domain_types::next_step::{NextStep, StackPullRequest, StepBlocker, StepFact, StepFailure, StepHelp, StepKind, StepRetarget, StepRow, WithoutReview};
use domain_types::patronus::{
  MissingApprovalsReason, MissingReviewGroup, PatronusAvailability, PatronusPullRequestLink, PatronusRobot, PatronusRobotStatus, Person, ReviewReadiness, ReviewReadinessBranch,
  UnknownReadinessReason,
};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestBase, Reviewer, ReviewerState};
use domain_types::reason::Reason;
use domain_types::repository_view::LandingNeeds;
use domain_types::session::{Fact, FactState, Parked, RobotsHead, SubjectReadiness};
use pretty_assertions::assert_eq;

/// The facts of a subject as a test sets them, each of the remote as the session holds it
#[derive(Clone)]
#[expect(clippy::struct_excessive_bools, reason = "the facts of the rule, each a bool of its own")]
pub(crate) struct Fixture {
  pub subject: Subject,
  pub robots_head: Fact<RobotsHead>,
  pub view: FactState,
  pub syncing: bool,
  pub sync_failed: bool,
  pub problem: Option<String>,
  pub commit_count: usize,
  pub blocked: bool,
  pub start: Option<Reason>,
  pub dry_run_start: Option<Reason>,
  pub landing_part: bool,
  pub held: bool,
  pub parked: Option<Parked>,
  pub landed: Option<Landed>,
  pub tip: Option<String>,
  /// The remote tip, and whether it has what the subject has
  pub remote: Option<(Option<String>, bool)>,
  /// Of the remote commits a push drops, those of someone else (`others_behind`)
  pub others: u32,
  /// The robot, why it failed (`needs_review`), why Patronus didn't tell, whether it offers a restart
  pub robot: Option<(PatronusRobot, Option<bool>, Option<String>, bool)>,
  pub own_robot_landed: bool,
  pub listing: Fact<PullRequestAvailability>,
  pub pull_request: Option<(PullRequest, Option<PullRequestBase>)>,
  pub needs: Option<LandingNeeds>,
  pub needs_state: FactState,
  pub readiness: Option<Fact<SubjectReadiness>>,
  pub stack_pull_request: Option<StackPullRequest>,
  pub repository: bool,
  /// The branches its Safe Push lands
  pub lands: u32,
  /// The route of the repository into master: Patronus unless a test names another
  pub route: LandingRoute,
}

pub(crate) fn fresh<T>(value: T) -> Fact<T> {
  Fact {
    value: Some(value),
    state: FactState::Fresh,
    seq: 1,
    observed_at: 1,
  }
}

pub(crate) fn failure(message: &str) -> ApiError {
  ApiError::new(ErrorCode::PatronusError, message)
}

pub(crate) fn available_head() -> Fact<RobotsHead> {
  fresh(RobotsHead {
    availability: PatronusAvailability::Available,
    github_repository: Some("acme/monorepo".to_string()),
  })
}

pub(crate) fn listed() -> Fact<PullRequestAvailability> {
  fresh(PullRequestAvailability::Available {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  })
}

pub(crate) fn answered(readiness: ReviewReadiness) -> Fact<SubjectReadiness> {
  fresh(SubjectReadiness {
    ask: ReviewReadinessBranch::starting_from("auth".to_string(), None, "t1".to_string(), None),
    requirement: None,
    readiness,
  })
}

impl Fixture {
  /// The branch auth with one commit, pushed, on Space and GitHub, no pull request, nothing needed, no approval needed:
  /// row 11
  pub(crate) fn branch() -> Self {
    Self {
      subject: Subject::branch("auth"),
      robots_head: available_head(),
      view: FactState::Fresh,
      syncing: false,
      sync_failed: false,
      problem: None,
      commit_count: 1,
      blocked: false,
      start: None,
      dry_run_start: None,
      landing_part: false,
      held: false,
      parked: None,
      landed: None,
      tip: Some("t1".to_string()),
      remote: Some((Some("t1".to_string()), true)),
      others: 0,
      robot: None,
      own_robot_landed: false,
      listing: listed(),
      pull_request: None,
      needs: Some(LandingNeeds::Known { refusals: Vec::new() }),
      needs_state: FactState::Fresh,
      readiness: Some(answered(ReviewReadiness::NotRequired)),
      stack_pull_request: None,
      repository: true,
      lands: 2,
      route: LandingRoute::Patronus { gate: RouteGate::Open },
    }
  }

  pub(crate) fn standing(&self) -> PatronusStanding {
    PatronusStanding::of(&self.robots_head, &self.view)
  }

  /// The facts of the rule over `standing` (of `standing()`)
  pub(crate) fn facts<'a>(&'a self, standing: &'a PatronusStanding) -> StepFacts<'a> {
    StepFacts {
      route: &self.route,
      subject: &self.subject,
      patronus: standing.answer(),
      syncing: self.syncing,
      sync_failed: self.sync_failed,
      problem: self.problem.as_deref(),
      commit_count: self.commit_count,
      blocked: self.blocked,
      start: self.start.as_ref(),
      dry_run_start: self.dry_run_start.as_ref(),
      landing_part: self.landing_part,
      held: self.held,
      parked: self.parked.as_ref(),
      landed: self.landed.as_ref(),
      tip: self.tip.as_deref(),
      remote: self.remote.as_ref().map(|(tip, up_to_date)| RemoteBranch {
        tip: tip.as_deref(),
        up_to_date: *up_to_date,
        drops: self.others,
        others: self.others,
      }),
      robot: self.robot.as_ref().map(|(robot, needs_review, problems_failed, restart)| StepRobot {
        robot,
        needs_review: *needs_review,
        problems_failed: problems_failed.as_deref(),
        failing: false,
        restart: *restart,
      }),
      own_robot_landed: self.own_robot_landed,
      listing: Listing::of(&self.listing),
      pull_request: self.pull_request.as_ref().map(|(pull_request, base)| StepPullRequest {
        pull_request,
        base: base.as_ref(),
      }),
      target: "master",
      needs: Needs::of(self.needs.as_ref(), &self.needs_state),
      readiness: self.readiness.as_ref(),
      stack_pull_request: self.stack_pull_request.as_ref(),
      repository: self.repository,
      lands: self.lands,
    }
  }

  pub(crate) fn step(&self) -> NextStep {
    let standing = self.standing();
    next_step(&self.facts(&standing))
  }

  pub(crate) fn with(mut self, change: impl FnOnce(&mut Self)) -> Self {
    change(&mut self);
    self
  }
}

/// A robot of auth on its tip `t1`
pub(crate) fn robot_on_tip(status: PatronusRobotStatus) -> PatronusRobot {
  PatronusRobot {
    source_revision: Some("t1".to_string()),
    ..robot("r1", Some("auth"), status, 1)
  }
}

/// A failed robot of auth: `needs_review` why it failed, as Patronus told it
pub(crate) fn failed(needs_review: Option<bool>, restart: bool) -> impl FnOnce(&mut Fixture) {
  move |fixture| fixture.robot = Some((robot_on_tip(PatronusRobotStatus::Failure), needs_review, None, restart))
}

/// The open pull request 7 of auth into `base` as the rules read it
pub(crate) fn open(base: PullRequestBase, change: impl FnOnce(&mut PullRequest)) -> impl FnOnce(&mut Fixture) {
  move |fixture| {
    let mut pull_request = pull_request("auth", 7);
    change(&mut pull_request);
    fixture.pull_request = Some((pull_request, Some(base)));
  }
}

pub(crate) fn person(login: &str) -> Person {
  Person {
    login: Some(login.to_string()),
    full_name: None,
  }
}

pub(crate) fn reviewer(login: &str, state: ReviewerState, requested: bool) -> Reviewer {
  Reviewer {
    login: login.to_string(),
    state,
    requested,
  }
}

/// A missing group `name` with the people its pull request asks (`requested`), those Patronus suggests and those who
/// requested changes
pub(crate) fn group(name: &str, requested: &[&str], suggested: &[&str], changes: &[&str]) -> MissingReviewGroup {
  MissingReviewGroup {
    name: name.to_string(),
    needed: Some(1),
    approved_by: Vec::new(),
    requested: requested.iter().map(|login| person(login)).collect(),
    suggested: suggested.iter().map(|login| person(login)).collect(),
    changes_requested_by: changes.iter().map(|login| person(login)).collect(),
  }
}

pub(crate) fn missing(groups: Vec<MissingReviewGroup>, reason: MissingApprovalsReason) -> ReviewReadiness {
  ReviewReadiness::Missing {
    groups,
    not_counted: Vec::new(),
    reason,
  }
}

/// A listing GitHub can't give: no token
pub(crate) fn signed_out() -> Fact<PullRequestAvailability> {
  Fact {
    state: FactState::Unavailable { reason: failure("signed out") },
    ..fresh(PullRequestAvailability::SignedOut)
  }
}

pub(crate) fn readiness(answer: ReviewReadiness) -> impl FnOnce(&mut Fixture) {
  move |fixture| fixture.readiness = Some(answered(answer))
}

#[expect(clippy::unnecessary_wraps, reason = "the blocker a step expects, which is optional")]
fn rule(reason: Reason) -> Option<StepBlocker> {
  Some(StepBlocker::Rule { reason })
}

/// The row, the kind and the two blockers of a step
fn shape(step: &NextStep) -> (StepRow, StepKind, Option<StepBlocker>, Option<StepBlocker>) {
  (step.row, step.kind.clone(), step.safe_push.clone(), step.dry_run.clone())
}

/// What a merged Safe Push of oauth landed: all of the subject
pub(crate) fn landed_by_oauth() -> Landed {
  Landed {
    by: LandedBy::Robots {
      by: vec![robot_of("oauth", "r9")],
      own: false,
      target: "master".to_string(),
    },
    after: Vec::new(),
  }
}

/// Row 1: a sync, a failure, no commits, a block, a hold, parked, landed, a robot that runs or merged: nothing to start,
/// the Status cell says why; Safe Push and Dry Run say the first reason of the subject
#[test]
fn row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts() {
  let settled = |fixture: Fixture, blocker: Option<StepBlocker>| {
    assert_eq!(shape(&fixture.step()), (StepRow::One, StepKind::Settled, blocker.clone(), blocker));
  };
  settled(Fixture::branch().with(|it| it.syncing = true), Some(StepBlocker::Syncing));
  settled(
    Fixture::branch().with(|it| {
      it.sync_failed = true;
      it.problem = Some("auth declares a parent twice".to_string());
    }),
    Some(StepBlocker::SyncFailed {
      problem: Some("auth declares a parent twice".to_string()),
    }),
  );
  settled(Fixture::branch().with(|it| it.commit_count = 0), Some(StepBlocker::NoCommits));
  let parked = Parked {
    note: "after the release".to_string(),
    with: Some("core".to_string()),
  };
  settled(
    Fixture::branch().with(|it| it.parked = Some(parked.clone())),
    rule(Reason::Parked {
      branch: "auth".to_string(),
      with: Some("core".to_string()),
      note: "after the release".to_string(),
    }),
  );
  let blocked = Reason::Blocked {
    branch: "auth".to_string(),
    by: Some("core".to_string()),
  };
  settled(
    Fixture::branch().with(|it| {
      it.blocked = true;
      it.start = Some(blocked.clone());
      it.dry_run_start = Some(blocked.clone());
    }),
    rule(blocked),
  );
  // held by a Safe Push that lands it: its start says the robot
  let descendant = Reason::Descendant {
    branch: "auth".to_string(),
    by: robot_of("oauth", "r9"),
    mode: domain_types::patronus::PatronusPushMode::RebaseAutosquash,
    carried: None,
  };
  settled(
    Fixture::branch().with(|it| {
      it.held = true;
      it.start = Some(descendant.clone());
      it.dry_run_start = Some(descendant.clone());
    }),
    rule(descendant),
  );
  let landed = Fixture::branch().with(|it| it.landed = Some(landed_by_oauth()));
  let reason = rule(Reason::Landed {
    branch: "auth".to_string(),
    by: robot_of("oauth", "r9"),
    own: false,
    target: "master".to_string(),
  });
  settled(landed.clone(), reason);
  assert_eq!(landed.step().landed, Some(landed_by_oauth()), "the step says what landed it");
  // in the baseline: until the update archives it
  let baseline = Landed {
    by: LandedBy::Baseline {
      upstream: "origin/master".to_string(),
    },
    after: Vec::new(),
  };
  settled(
    Fixture::branch().with(|it| it.landed = Some(baseline)),
    rule(Reason::LandedIn {
      upstream: "origin/master".to_string(),
    }),
  );
  // a robot that runs: the start replaces an older one, the view says one on its commit
  settled(
    Fixture::branch().with(|it| it.robot = Some((robot_on_tip(PatronusRobotStatus::Running), None, None, false))),
    None,
  );
  // its robot merged, before the view says what landed it
  let merged = Fixture::branch().with(|it| it.robot = Some((robot_on_tip(PatronusRobotStatus::Success), None, None, false)));
  let own = Reason::Landed {
    branch: "auth".to_string(),
    by: robot_of("auth", "r1"),
    own: true,
    target: "master".to_string(),
  };
  settled(merged, rule(own));
  // work added after what landed is the only work left: it starts
  let after = Fixture::branch().with(|it| {
    it.landed = Some(Landed {
      after: vec!["c2".to_string()],
      ..landed_by_oauth()
    });
  });
  assert_eq!((after.step().row, after.step().landed), (StepRow::Eleven, None));
}

fn loading<T>() -> Fact<T> {
  Fact::default()
}

/// Row 0: before Patronus answers (or the view of the robots it listed first), GitHub's listing, the landing needs, why
/// the robot failed or the first answer of the readiness, the step waits: "Checking…", Safe Push disabled, a Dry Run
/// only while Patronus hasn't answered. A reason of the subject outranks it, a known landing need too
#[test]
fn row_0_waits_for_a_fact_no_answer_told_yet_and_a_reason_of_the_subject_outranks_it() {
  let waits = |fixture: Fixture, fact: StepFact, dry_run: Option<StepBlocker>| {
    let step = fixture.step();
    assert_eq!(shape(&step), (StepRow::Zero, StepKind::Pending { fact }, Some(StepBlocker::Step), dry_run));
    assert_eq!(step.needs_readiness, fact == StepFact::Readiness, "the session asks the readiness only for its own wait");
  };
  waits(Fixture::branch().with(|it| it.robots_head = loading()), StepFact::Patronus, Some(StepBlocker::Step));
  waits(Fixture::branch().with(|it| it.view = FactState::Loading), StepFact::Patronus, Some(StepBlocker::Step));
  waits(Fixture::branch().with(|it| it.listing = loading()), StepFact::PullRequests, None);
  waits(
    Fixture::branch().with(|it| {
      it.needs = Some(LandingNeeds::Unknown);
      it.needs_state = FactState::Loading;
    }),
    StepFact::LandingNeeds,
    None,
  );
  waits(Fixture::branch().with(failed(None, true)), StepFact::RobotProblems, None);
  waits(Fixture::branch().with(|it| it.readiness = None), StepFact::Readiness, None);
  waits(Fixture::branch().with(|it| it.readiness = Some(loading())), StepFact::Readiness, None);
  // a reason of the subject says more than "Checking…": a robot of a branch below, a sync
  let ancestor = Reason::Ancestor {
    landing: robot_of("core", "r5"),
    mode: domain_types::patronus::PatronusPushMode::RebaseAutosquash,
  };
  let below = Fixture::branch().with(|it| {
    it.listing = loading();
    it.start = Some(ancestor.clone());
  });
  assert_eq!(shape(&below.step()), (StepRow::Eleven, StepKind::SafePush, rule(ancestor), None), "a dry run may run");
  let syncing = Fixture::branch().with(|it| {
    it.robots_head = loading();
    it.syncing = true;
  });
  assert_eq!(
    shape(&syncing.step()),
    (StepRow::One, StepKind::Settled, Some(StepBlocker::Syncing), Some(StepBlocker::Syncing))
  );
  // a robot Patronus listed says why nothing starts while its view comes
  let running = Fixture::branch().with(|it| {
    it.view = FactState::Loading;
    it.robot = Some((robot_on_tip(PatronusRobotStatus::Running), None, None, false));
  });
  assert_eq!(running.step().row, StepRow::One);
  // a known landing need comes before the listing
  let needs = Fixture::branch().with(|it| {
    it.listing = loading();
    it.needs = Some(LandingNeeds::Known {
      refusals: vec![missing_declaration()],
    });
  });
  assert_eq!(needs.step().row, StepRow::OnePrime);
}

pub(crate) fn missing_declaration() -> LandingRefusal {
  LandingRefusal::MissingDeclaration {
    branch: "auth".to_string(),
    identifier: "Codec".to_string(),
    file: "src/codec.rs".to_string(),
    line: 3,
    used_by: stack_commit("c1", "Use the codec", "auth"),
    declared_by: stack_commit("c9", "Add the codec", "codec"),
    fix: LandingFix::Restack {
      branch: "auth".to_string(),
      onto: "codec".to_string(),
    },
    landing_in: None,
  }
}

/// Row 0 of a failed read: a login to copy, a refresh of Patronus, a restart disabled while why the robot failed is
/// unknown; an error after a list leaves the steps on the robots it listed, a view that failed after one on that view
#[test]
fn row_0_a_read_that_failed_decides_no_start_and_offers_what_helps() {
  let head = |availability: PatronusAvailability, state: FactState| Fact {
    value: Some(RobotsHead {
      availability,
      github_repository: None,
    }),
    state,
    seq: 2,
    observed_at: 2,
  };
  let unavailable = |fixture: Fixture, failure: StepFailure, help: Option<StepHelp>, robot_id: Option<&str>, dry_run: Option<StepBlocker>| {
    let kind = StepKind::Unavailable {
      failure,
      help,
      robot_id: robot_id.map(str::to_string),
    };
    assert_eq!(shape(&fixture.step()), (StepRow::Zero, kind, Some(StepBlocker::Step), dry_run));
  };
  let error = PatronusAvailability::Error {
    message: "502 Bad Gateway".to_string(),
  };
  unavailable(
    Fixture::branch().with(|it| it.robots_head = head(PatronusAvailability::NotLoggedIn, FactState::Unavailable { reason: failure("not logged in") })),
    StepFailure::NotLoggedIn,
    Some(StepHelp::CopyLoginCommand),
    None,
    Some(StepBlocker::Step),
  );
  unavailable(
    Fixture::branch().with(|it| it.robots_head = head(error.clone(), FactState::Unavailable { reason: failure("502") })),
    StepFailure::PatronusError {
      message: "502 Bad Gateway".to_string(),
    },
    Some(StepHelp::RefreshPatronus),
    None,
    Some(StepBlocker::Step),
  );
  let after_list = Fixture::branch().with(|it| it.robots_head = head(error, FactState::Stale { reason: Some(failure("502")) }));
  assert_eq!(after_list.step().row, StepRow::Eleven, "the steps stand on the robots it listed");
  unavailable(
    Fixture::branch().with(|it| it.view = FactState::Unavailable { reason: failure("no view") }),
    StepFailure::ViewFailed { reason: failure("no view") },
    Some(StepHelp::RefreshPatronus),
    None,
    Some(StepBlocker::Step),
  );
  let over_earlier = Fixture::branch().with(|it| it.view = FactState::Stale { reason: Some(failure("no view")) });
  assert_eq!(over_earlier.step().row, StepRow::Eleven, "an earlier view stands");
  // why the robot failed: Patronus didn't give it, the restart waits and says it; a Dry Run is not held back
  let problems = Fixture::branch().with(|it| it.robot = Some((robot_on_tip(PatronusRobotStatus::Failure), None, Some("timeout".to_string()), true)));
  unavailable(problems, StepFailure::ProblemsFailed { message: "timeout".to_string() }, None, Some("r1"), None);
  // a reason of the subject comes first
  let parked = Fixture::branch().with(|it| {
    it.robots_head = head(PatronusAvailability::NotLoggedIn, FactState::Unavailable { reason: failure("not logged in") });
    it.parked = Some(Parked { note: String::new(), with: None });
  });
  assert_eq!(parked.step().row, StepRow::One);
}

/// Row 1′: the build of what the Safe Push lands needs a commit of a branch outside it: the step names it, the Safe
/// Push stays enabled (its popover says why before the start)
#[test]
fn row_1_prime_needs_a_branch_outside_what_the_safe_push_lands_and_keeps_the_start() {
  let needs = Fixture::branch().with(|it| {
    it.needs = Some(LandingNeeds::Known {
      refusals: vec![missing_declaration()],
    });
  });
  let kind = StepKind::NeedsBranch {
    refusals: vec![missing_declaration()],
  };
  assert_eq!(shape(&needs.step()), (StepRow::OnePrime, kind, None, None));
}

/// Row 1″: someone else pushed to the remote branch commits the branch lacks: they come in first, before a restart or a
/// Safe Push, whose push would drop them; the Safe Push stays enabled (its start asks before that push)
#[test]
fn row_1_double_prime_takes_in_the_commits_of_others_on_the_remote_before_a_start() {
  let theirs = Fixture::branch().with(|it| {
    it.remote = Some((Some("theirs".to_string()), false));
    it.others = 1;
  });
  assert_eq!(shape(&theirs.step()), (StepRow::OneDoublePrime, StepKind::PickRemoteCommits { count: 1 }, None, None));
  let failed_too = theirs.with(failed(Some(false), true));
  assert_eq!(failed_too.step().kind, StepKind::PickRemoteCommits { count: 1 }, "before the restart, which pushes first");
  let parked = Fixture::branch().with(|it| {
    it.others = 1;
    it.parked = Some(Parked { note: String::new(), with: None });
  });
  assert_eq!(parked.step().row, StepRow::One, "a parked branch starts nothing");
}

/// Rows 2 and 3: a robot that failed (or was canceled) not for the approvals restarts, or offers Safe Push where Patronus
/// can't restart it; while a robot of a branch below lands a part of it, the failure waits for that robot and the steps
/// of the pull request stay. One that failed for the approvals restarts only from the pull request they now count in,
/// approved; while nobody can tell whether they count, the step is the pull request (to create, or open)
#[test]
fn rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell() {
  let restart = StepKind::Restart { robot_id: "r1".to_string() };
  let row2 = Fixture::branch().with(failed(Some(false), true));
  assert_eq!((row2.step().row, row2.step().kind, row2.step().needs_readiness), (StepRow::Two, restart.clone(), false));
  let canceled = Fixture::branch().with(|it| it.robot = Some((robot_on_tip(PatronusRobotStatus::Canceled), None, None, true)));
  assert_eq!(canceled.step().kind, restart);
  let cannot = Fixture::branch().with(failed(Some(false), false));
  assert_eq!((cannot.step().row, cannot.step().kind), (StepRow::Two, StepKind::SafePush));
  // the robot of a branch below lands a part of it: no restart, its start waits for that robot
  let ancestor = Reason::Ancestor {
    landing: robot_of("core", "r5"),
    mode: domain_types::patronus::PatronusPushMode::RebaseAutosquash,
  };
  let below = Fixture::branch().with(failed(Some(false), true)).with(|it| {
    it.landing_part = true;
    it.start = Some(ancestor.clone());
  });
  assert_eq!(shape(&below.step()), (StepRow::Eleven, StepKind::SafePush, rule(ancestor), None));

  // row 3: started from #7 into the target, approved since: the restart counts the approvals
  let from_seven = |fixture: &mut Fixture| {
    fixture.robot = Some((
      PatronusRobot {
        pull_request: Some(PatronusPullRequestLink {
          number: 7,
          url: "https://github.com/acme/monorepo/pull/7".to_string(),
        }),
        ..robot_on_tip(PatronusRobotStatus::Failure)
      },
      Some(true),
      None,
      true,
    ));
  };
  let approved = ReviewReadiness::Approved { groups: Vec::new() };
  let row3 = Fixture::branch()
    .with(from_seven)
    .with(open(PullRequestBase::Target, |_| {}))
    .with(readiness(approved.clone()));
  assert_eq!((row3.step().row, row3.step().kind, row3.step().needs_readiness), (StepRow::Three, restart, true));
  // approved, but the robot started through the API: a new start from the pull request counts them
  let api = Fixture::branch()
    .with(failed(Some(true), true))
    .with(open(PullRequestBase::Target, |_| {}))
    .with(readiness(approved));
  assert_eq!((api.step().row, api.step().kind), (StepRow::Eleven, StepKind::SafePush));
  // nobody can tell whether they count: the pull request, to open or to create; without the GitHub repository Safe Push
  let unknown = ReviewReadiness::Unknown {
    reason: UnknownReadinessReason::ApprovalsNotEvaluated,
  };
  let opened = Fixture::branch()
    .with(from_seven)
    .with(open(PullRequestBase::Target, |_| {}))
    .with(readiness(unknown.clone()));
  assert_eq!((opened.step().row, opened.step().kind), (StepRow::Three, StepKind::RobotPullRequest { number: Some(7) }));
  let to_create = Fixture::branch().with(failed(Some(true), true)).with(readiness(unknown.clone()));
  assert_eq!(to_create.step().kind, StepKind::RobotPullRequest { number: None });
  let no_repository = Fixture::branch().with(failed(Some(true), true)).with(readiness(unknown)).with(|it| it.repository = false);
  assert_eq!((no_repository.step().row, no_repository.step().kind), (StepRow::Three, StepKind::SafePush));
  // before the first answer: it waits
  let first = Fixture::branch().with(failed(Some(true), true)).with(|it| it.readiness = None);
  assert_eq!(first.step().kind, StepKind::Pending { fact: StepFact::Readiness });
}

/// Row 5: an open pull request into a virtual branch is retargeted into the target, the Safe Push enabled (its start
/// retargets first, D11); 5′: one into another base, which may be meant, opens, the Safe Push disabled with why; "⋯"
/// offers the retarget of both. The pull request of a whole stack is never retargeted
#[test]
fn row_5_retargets_a_pull_request_into_a_virtual_branch_and_5_prime_opens_one_into_another_base() {
  let virtual_base = PullRequestBase::Virtual {
    from: "core".to_string(),
    retarget: "master".to_string(),
  };
  let into_core = |pull_request: &mut PullRequest| pull_request.base_ref_name = "user/virtual/core".to_string();
  let retarget = Some(StepRetarget {
    number: 7,
    base: "user/virtual/core".to_string(),
    target: "master".to_string(),
  });
  let row5 = Fixture::branch().with(open(virtual_base.clone(), into_core));
  let kind = StepKind::Retarget {
    number: 7,
    base: "user/virtual/core".to_string(),
  };
  assert_eq!((shape(&row5.step()), row5.step().retarget), ((StepRow::Five, kind, None, None), retarget));
  let release = PullRequestBase::Other { base: "release-252".to_string() };
  let row5_prime = Fixture::branch().with(open(release, |it| it.base_ref_name = "release-252".to_string()));
  let kind = StepKind::OtherBase {
    number: 7,
    base: "release-252".to_string(),
  };
  assert_eq!(shape(&row5_prime.step()), (StepRow::FivePrime, kind, Some(StepBlocker::Step), None));
  assert!(row5_prime.step().retarget.is_some(), "the retarget stays in the menu");
  let stack = Fixture::branch().with(open(virtual_base, into_core)).with(|it| it.subject = Subject::whole_stack("auth"));
  assert_eq!(stack.step().row, StepRow::FivePrime);
}

/// Row 6: Patronus merges no draft: the step opens it, the Safe Push says so, a Dry Run runs
#[test]
fn row_6_a_draft_is_a_step_that_opens_it_the_safe_push_waits_a_dry_run_runs() {
  let draft = Fixture::branch().with(open(PullRequestBase::Target, |it| it.is_draft = true));
  assert_eq!(shape(&draft.step()), (StepRow::Six, StepKind::Draft { number: 7 }, Some(StepBlocker::Step), None));
}

/// Rows 7 to 10: the approvals that count are missing. Without an open pull request it is to create (beside it a
/// Safe Push without review), or the pull request of the whole stack the branch is in; with one, changes requested by a
/// member of a missing group, a group that asks fewer people than it needs (nobody can approve: no request helps), or
/// the wait; right after Request Review… a suggested member the pull request asks counts as requested
#[test]
fn rows_7_to_10_the_missing_approvals_and_where_they_can_come_from() {
  let groups = |requested: &[&str], suggested: &[&str], changes: &[&str]| vec![group("IJPL Code Platform", requested, suggested, changes)];
  let approvals = |groups| missing(groups, MissingApprovalsReason::ApprovalsMissing);
  let row7 = Fixture::branch().with(readiness(approvals(groups(&[], &["ikorennoy"], &[]))));
  let kind = StepKind::CreatePullRequest {
    groups: groups(&[], &["ikorennoy"], &[]),
    stack: None,
    repository: true,
  };
  let step = row7.step();
  assert_eq!(shape(&step), (StepRow::Seven, kind, Some(StepBlocker::Step), None));
  assert_eq!((step.needs_readiness, step.without_review), (true, Some(WithoutReview { blocker: None })));
  let stack = StackPullRequest {
    number: 12,
    root: "air".to_string(),
  };
  let in_stack = Fixture::branch()
    .with(readiness(missing_approvals("IJPL Code Platform", Some(1))))
    .with(|it| it.stack_pull_request = Some(stack.clone()));
  assert!(matches!(in_stack.step().kind, StepKind::CreatePullRequest { stack: Some(found), .. } if found == stack));

  let with_open = |answer| Fixture::branch().with(open(PullRequestBase::Target, |_| {})).with(readiness(answer));
  let row8 = with_open(approvals(groups(&["ikorennoy"], &[], &["ikorennoy"])));
  let kind = StepKind::ChangesRequested {
    number: 7,
    groups: groups(&["ikorennoy"], &[], &["ikorennoy"]),
  };
  assert_eq!(shape(&row8.step()), (StepRow::Eight, kind, Some(StepBlocker::Step), None));
  let row9 = with_open(approvals(groups(&[], &["ikorennoy"], &[])));
  let kind = StepKind::RequestReview {
    number: 7,
    groups: groups(&[], &["ikorennoy"], &[]),
    no_eligible: false,
  };
  assert_eq!(shape(&row9.step()), (StepRow::Nine, kind, Some(StepBlocker::Step), None));
  // a group that needs two approvals and asks one still needs a request
  let mut two = group("Air Platform", &["alice"], &[], &[]);
  two.needed = Some(2);
  assert_eq!(with_open(approvals(vec![two])).step().row, StepRow::Nine);
  let nobody = with_open(missing(groups(&["ikorennoy"], &[], &[]), MissingApprovalsReason::NoEligibleReviewer));
  assert!(matches!(nobody.step().kind, StepKind::RequestReview { no_eligible: true, .. }));
  let row10 = with_open(approvals(groups(&["ikorennoy"], &[], &[])));
  let kind = StepKind::Awaiting {
    number: 7,
    groups: groups(&["ikorennoy"], &[], &[]),
  };
  assert_eq!(shape(&row10.step()), (StepRow::Ten, kind, Some(StepBlocker::Step), None));
  // right after Request Review…: Patronus answers again with the people requested before, the pull request asks him
  let asked = Fixture::branch()
    .with(open(PullRequestBase::Target, |it| {
      it.reviewers = vec![reviewer("IKorennoy", ReviewerState::Requested, true)];
    }))
    .with(readiness(approvals(groups(&[], &["ikorennoy"], &[]))));
  let kind = StepKind::Awaiting {
    number: 7,
    groups: groups(&["ikorennoy"], &["ikorennoy"], &[]),
  };
  assert_eq!((asked.step().row, asked.step().kind), (StepRow::Ten, kind));
}

/// Rows 8′ and 10′: while Patronus can't judge the approvals, the wait GitHub tells (bots left out), the Safe Push
/// still enabled: its start checks them
#[test]
fn rows_8_prime_and_10_prime_tell_the_wait_github_tells_while_patronus_cannot_judge() {
  let unknown = ReviewReadiness::Unknown {
    reason: UnknownReadinessReason::ApprovalsNotEvaluated,
  };
  let unavailable = ReviewReadiness::Unavailable {
    reason: domain_types::patronus::UnavailableReadinessReason::Error,
    message: "502".to_string(),
  };
  let reviewers = |list: Vec<Reviewer>| open(PullRequestBase::Target, move |it| it.reviewers = list);
  let changes = Fixture::branch()
    .with(reviewers(vec![
      reviewer("ikorennoy", ReviewerState::ChangesRequested, false),
      reviewer("copilot", ReviewerState::ChangesRequested, false),
    ]))
    .with(readiness(unknown.clone()));
  let kind = StepKind::ChangesRequestedUnjudged {
    number: 7,
    by: vec!["ikorennoy".to_string()],
  };
  assert_eq!(shape(&changes.step()), (StepRow::EightPrime, kind, None, None));
  let waits = Fixture::branch()
    .with(reviewers(vec![
      reviewer("ikorennoy", ReviewerState::Requested, true),
      reviewer("bot[bot]", ReviewerState::Requested, true),
    ]))
    .with(readiness(unavailable));
  let kind = StepKind::AwaitingUnjudged {
    number: 7,
    reviewers: vec!["ikorennoy".to_string()],
  };
  assert_eq!(shape(&waits.step()), (StepRow::TenPrime, kind, None, None));
  assert!(waits.step().needs_readiness);
  // asked again after the changes: a wait, not changes
  let again = Fixture::branch()
    .with(reviewers(vec![reviewer("ikorennoy", ReviewerState::ChangesRequested, true)]))
    .with(readiness(unknown.clone()));
  assert_eq!(again.step().row, StepRow::TenPrime);
  let no_wait = Fixture::branch().with(reviewers(Vec::new())).with(readiness(unknown));
  assert_eq!(no_wait.step().row, StepRow::Eleven);
}

/// Row 11: approved, not required, or Patronus can't judge and GitHub tells no wait. While GitHub can't list the pull
/// requests, rows 5 to 10 don't apply, the readiness isn't asked, and a Safe Push without review is offered beside
/// (row 7 may apply); off GitHub none is
#[test]
fn row_11_safe_push_and_the_start_without_review_while_github_is_signed_out() {
  let approved = Fixture::branch().with(readiness(ReviewReadiness::Approved { groups: Vec::new() }));
  let step = approved.step();
  assert_eq!((shape(&step), step.needs_readiness), ((StepRow::Eleven, StepKind::SafePush, None, None), true));
  let signed_out = Fixture::branch().with(|it| {
    it.listing = signed_out();
    it.readiness = None;
    it.pull_request = Some((pull_request("auth", 7), Some(PullRequestBase::Target)));
  });
  let step = signed_out.step();
  assert_eq!(
    (step.row, step.needs_readiness, step.without_review),
    (StepRow::Eleven, false, Some(WithoutReview { blocker: None }))
  );
  let off_github = Fixture::branch().with(|it| {
    it.listing = fresh(PullRequestAvailability::NotGitHub);
    it.readiness = None;
  });
  assert_eq!((off_github.step().row, off_github.step().without_review), (StepRow::Eleven, None));
}

/// A stale answer of the readiness keeps its step while the new one loads, so the row doesn't flicker; its action
/// waits for the new answer, as does every Safe Push
#[test]
fn a_stale_readiness_keeps_its_step_and_its_action_waits_for_the_new_answer() {
  let stale = |answer: ReviewReadiness| {
    Fixture::branch().with(open(PullRequestBase::Target, |_| {})).with(|it| {
      it.readiness = Some(Fact {
        state: FactState::Stale { reason: None },
        ..answered(answer)
      });
    })
  };
  let approved = stale(ReviewReadiness::Approved { groups: Vec::new() }).step();
  assert_eq!(
    (approved.row, approved.stale_readiness, approved.safe_push),
    (StepRow::Eleven, true, Some(StepBlocker::StaleReadiness))
  );
  let awaiting = stale(missing(
    vec![group("IJPL Code Platform", &["ikorennoy"], &[], &[])],
    MissingApprovalsReason::ApprovalsMissing,
  ))
  .step();
  assert_eq!((awaiting.row, awaiting.stale_readiness, awaiting.safe_push), (StepRow::Ten, true, Some(StepBlocker::Step)));
  // a read that failed after an answer is no stale answer the start would wait on: it is unavailable
  let failed = Fixture::branch().with(|it| {
    it.readiness = Some(Fact {
      state: FactState::Stale { reason: Some(failure("502")) },
      ..answered(ReviewReadiness::Approved { groups: Vec::new() })
    });
  });
  assert_eq!((failed.step().row, failed.step().stale_readiness), (StepRow::Eleven, false));
}

/// A repository not on Space has no slot, and keeps the reasons of the subject for every surface
#[test]
fn a_repository_not_on_space_has_no_slot_and_keeps_the_reasons_of_the_subject() {
  let off_space = Fixture::branch().with(|it| {
    it.robots_head = fresh(RobotsHead {
      availability: PatronusAvailability::NotPatronus,
      github_repository: None,
    });
    it.syncing = true;
  });
  assert_eq!(
    shape(&off_space.step()),
    (StepRow::NoSlot, StepKind::Hidden, Some(StepBlocker::Syncing), Some(StepBlocker::Syncing))
  );
}

/// The whole stack takes the same table over its stack branch: a parked member holds it local, what landed every
/// branch of it says it, its own pull request into a virtual branch is no retarget
#[test]
fn the_whole_stack_takes_the_rows_of_its_stack_branch() {
  let stack = || Fixture::branch().with(|it| it.subject = Subject::whole_stack("air"));
  let parked = stack().with(|it| {
    it.parked = Some(Parked {
      note: String::new(),
      with: Some("bazel".to_string()),
    });
  });
  let reason = rule(Reason::StackParked {
    root: "air".to_string(),
    member: "bazel".to_string(),
  });
  assert_eq!(shape(&parked.step()), (StepRow::One, StepKind::Settled, reason.clone(), reason));
  let landed = stack().with(|it| it.landed = Some(landed_by_oauth()));
  let reason = rule(Reason::StackLanded {
    root: "air".to_string(),
    by: vec![robot_of("oauth", "r9")],
    own: false,
    target: "master".to_string(),
  });
  assert_eq!(landed.step().safe_push, reason);
  assert_eq!(stack().step().row, StepRow::Eleven);
}

/// A family of the facts of the remote the step reads
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
  Patronus,
  PatronusView,
  PullRequests,
  LandingNeeds,
  RobotProblems,
  Readiness,
}

/// A fact without a fresh answer
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Without {
  Loading,
  Stale,
  Unavailable,
  Unknown,
}

impl Without {
  fn state(self) -> FactState {
    match self {
      Self::Loading => FactState::Loading,
      Self::Stale => FactState::Stale {
        reason: Some(failure("a refresh failed")),
      },
      Self::Unavailable => FactState::Unavailable {
        reason: failure("the read failed"),
      },
      Self::Unknown => FactState::Unknown,
    }
  }
}

/// `fixture` with `family` as it stands `without` a fresh answer: no value while it loads or nobody asks, the earlier
/// answer while it is stale, the failure of a read before any answer
fn degraded(fixture: &Fixture, family: Family, without: Without) -> Fixture {
  let state = without.state();
  let answered_before = matches!(without, Without::Stale | Without::Unavailable);
  fixture.clone().with(|it| match family {
    Family::Patronus => {
      it.robots_head = Fact {
        state,
        value: answered_before.then(|| RobotsHead {
          availability: PatronusAvailability::Error { message: "502".to_string() },
          github_repository: None,
        }),
        ..Fact::default()
      }
    }
    Family::PatronusView => it.view = state,
    Family::PullRequests => {
      it.listing = Fact {
        state,
        value: answered_before.then(|| PullRequestAvailability::Error { message: "offline".to_string() }),
        ..Fact::default()
      }
    }
    Family::LandingNeeds => {
      it.needs = Some(LandingNeeds::Unknown);
      it.needs_state = state;
    }
    Family::RobotProblems => {
      if let Some((robot, needs_review, problems, _)) = &mut it.robot
        && robot.status == PatronusRobotStatus::Failure
      {
        match without {
          Without::Loading | Without::Unknown => *needs_review = None,
          Without::Stale => {}
          Without::Unavailable => {
            *needs_review = None;
            *problems = Some("the read failed".to_string());
          }
        }
      }
    }
    Family::Readiness => {
      it.readiness = Some(match without {
        Without::Stale => Fact {
          state: FactState::Stale { reason: None },
          ..it.readiness.clone().unwrap_or_else(|| answered(ReviewReadiness::NotRequired))
        },
        _ => Fact { state, ..Fact::default() },
      });
    }
  })
}

/// A fresh fixture of each row past 1, by name
fn rows() -> Vec<(&'static str, Fixture)> {
  let from_seven = |fixture: &mut Fixture| {
    let robot = PatronusRobot {
      pull_request: Some(PatronusPullRequestLink { number: 7, url: String::new() }),
      ..robot_on_tip(PatronusRobotStatus::Failure)
    };
    fixture.robot = Some((robot, Some(true), None, true));
  };
  let target = || open(PullRequestBase::Target, |_| {});
  let unknown = ReviewReadiness::Unknown {
    reason: UnknownReadinessReason::ApprovalsNotEvaluated,
  };
  let groups = |requested: &[&str], changes: &[&str]| {
    missing(
      vec![group("IJPL Code Platform", requested, &["ikorennoy"], changes)],
      MissingApprovalsReason::ApprovalsMissing,
    )
  };
  let reviewers = |list: Vec<Reviewer>| open(PullRequestBase::Target, move |it| it.reviewers = list);
  vec![
    (
      "1′",
      Fixture::branch().with(|it| {
        it.needs = Some(LandingNeeds::Known {
          refusals: vec![missing_declaration()],
        });
      }),
    ),
    ("2", Fixture::branch().with(failed(Some(false), true))),
    ("3", Fixture::branch().with(from_seven).with(target()).with(readiness(ReviewReadiness::NotRequired))),
    ("3 unknown", Fixture::branch().with(from_seven).with(target()).with(readiness(unknown.clone()))),
    (
      "5",
      Fixture::branch().with(open(
        PullRequestBase::Virtual {
          from: "core".to_string(),
          retarget: "master".to_string(),
        },
        |_| {},
      )),
    ),
    ("5′", Fixture::branch().with(open(PullRequestBase::Other { base: "release".to_string() }, |_| {}))),
    ("6", Fixture::branch().with(open(PullRequestBase::Target, |it| it.is_draft = true))),
    ("7", Fixture::branch().with(readiness(groups(&[], &[])))),
    ("8", Fixture::branch().with(target()).with(readiness(groups(&["ikorennoy"], &["ikorennoy"])))),
    ("9", Fixture::branch().with(target()).with(readiness(groups(&[], &[])))),
    ("10", Fixture::branch().with(target()).with(readiness(groups(&["ikorennoy"], &[])))),
    (
      "8′",
      Fixture::branch()
        .with(reviewers(vec![reviewer("ikorennoy", ReviewerState::ChangesRequested, false)]))
        .with(readiness(unknown.clone())),
    ),
    (
      "10′",
      Fixture::branch()
        .with(reviewers(vec![reviewer("ikorennoy", ReviewerState::Requested, true)]))
        .with(readiness(unknown)),
    ),
    ("11", Fixture::branch()),
  ]
}

/// Rows 7 to 10: the approvals are missing, a "no" only an answer of the readiness gives
const fn says_missing(row: StepRow) -> bool {
  matches!(row, StepRow::Seven | StepRow::Eight | StepRow::Nine | StepRow::Ten)
}

/// Every row with every fact it reads without a fresh answer: one that loads or that nobody asks waits (row 0, every
/// Safe Push disabled), never a step a later answer turns into another; a stale one keeps the step of the answer before
/// (a stale readiness holds its action); a read that failed is an answer that says so: Patronus offers what helps, a
/// listing or a readiness that failed never says the approvals are missing (the start checks them, its popover says
/// it), a computation of the landing needs that failed leaves them to the gate of the start
#[test]
fn every_row_with_every_fact_without_a_fresh_answer_never_answers_an_action_the_fresh_one_would_replace() {
  let families = [
    Family::Patronus,
    Family::PatronusView,
    Family::PullRequests,
    Family::LandingNeeds,
    Family::RobotProblems,
    Family::Readiness,
  ];
  let states = [Without::Loading, Without::Stale, Without::Unavailable, Without::Unknown];
  let mut failures = Vec::new();
  for (name, fixture) in rows() {
    let fresh = fixture.step();
    assert!(fresh.row != StepRow::Zero && fresh.row != StepRow::One, "row {name} decides a step: {fresh:?}");
    for family in families {
      for without in states {
        let step = degraded(&fixture, family, without).step();
        let same = step.row == fresh.row && step.kind == fresh.kind;
        let waits = step.row == StepRow::Zero && step.safe_push.is_some();
        let ok = match without {
          Without::Loading | Without::Unknown => same || waits,
          Without::Stale if family == Family::Readiness && fresh.needs_readiness => same && step.stale_readiness && step.safe_push.is_some(),
          Without::Stale if family == Family::LandingNeeds => same || waits,
          Without::Stale => same,
          Without::Unavailable => match family {
            Family::Patronus | Family::PatronusView | Family::RobotProblems => same || (waits && matches!(step.kind, StepKind::Unavailable { .. })),
            Family::PullRequests | Family::Readiness | Family::LandingNeeds => {
              same
                || (!says_missing(step.row)
                  && matches!(
                    step.kind,
                    StepKind::SafePush
                      | StepKind::Restart { .. }
                      | StepKind::RobotPullRequest { .. }
                      | StepKind::ChangesRequestedUnjudged { .. }
                      | StepKind::AwaitingUnjudged { .. }
                  ))
            }
          },
        };
        if !ok {
          failures.push(format!(
            "row {name}, {family:?} {without:?}: {:?} {:?} {:?} instead of {:?} {:?}",
            step.row, step.kind, step.safe_push, fresh.row, fresh.kind
          ));
        }
      }
    }
  }
  assert_eq!(failures, Vec::<String>::new());
}

/// A repository that lands by a push into the target takes the table of its route, on Space or off it: row 1 while the
/// Status cell says why, row 0 while the gate of the route or the pull requests have no answer, 1′ before the push,
/// then "Push to master", disabled with the refusal of the gate; a Dry Run is a robot of Patronus, never offered
#[test]
fn a_push_into_the_target_is_the_step_of_its_route_and_its_gate_holds_it_back() {
  let push = |gate: RouteGate| Fixture::branch().with(|it| it.route = LandingRoute::Push { gate });
  let no_dry_run = rule(Reason::DryRunNeedsPatronus);
  let push_step = StepKind::PushToBaseline { target: "master".to_string() };
  assert_eq!(shape(&push(RouteGate::Open).step()), (StepRow::Push, push_step.clone(), None, no_dry_run.clone()));
  assert_eq!(push(RouteGate::Open).step().lands, 2);
  let off_space = push(RouteGate::Open).with(|it| {
    it.robots_head = fresh(RobotsHead {
      availability: PatronusAvailability::NotPatronus,
      github_repository: None,
    });
  });
  assert_eq!(shape(&off_space.step()), (StepRow::Push, push_step.clone(), None, no_dry_run.clone()));

  let no_access = Reason::NoWriteAccess {
    repository: "acme/monorepo".to_string(),
    target: "master".to_string(),
  };
  let refused = push(RouteGate::Refused { reason: no_access.clone() });
  assert_eq!(shape(&refused.step()), (StepRow::Push, push_step, rule(no_access), no_dry_run.clone()));
  let confirm = push(RouteGate::Confirm {
    confirmations: vec![Reason::BaselineRequiresStatusChecks { target: "master".to_string() }],
  });
  assert_eq!(confirm.step().safe_push, None);

  let waits = push(RouteGate::Wait {
    fact: RouteFact::Permission,
    failed: None,
  });
  assert_eq!(
    shape(&waits.step()),
    (StepRow::Zero, StepKind::Pending { fact: StepFact::Permission }, Some(StepBlocker::Step), no_dry_run.clone())
  );
  // a reason of the subject outranks the wait
  let syncing = push(RouteGate::Wait {
    fact: RouteFact::TargetRules,
    failed: None,
  })
  .with(|it| it.syncing = true);
  assert_eq!(shape(&syncing.step()), (StepRow::One, StepKind::Settled, Some(StepBlocker::Syncing), no_dry_run));
  let listing = push(RouteGate::Open).with(|it| it.listing = Fact::default());
  assert_eq!(listing.step().kind, StepKind::Pending { fact: StepFact::PullRequests });
  let needs = push(RouteGate::Open).with(|it| {
    it.needs = Some(LandingNeeds::Known {
      refusals: vec![missing_declaration()],
    });
  });
  assert_eq!(needs.step().row, StepRow::OnePrime);
}
