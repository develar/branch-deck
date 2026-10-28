use crate::facts::Robots;
use crate::land::{BranchStart, BranchSubject, LandFacts, LandsBranch, StackStart, StackSubject, SubjectFacts, TargetConflictFact, decide, unaccepted};
use crate::land_direct::update_after;
use crate::land_test::{codes, land, listed};
use crate::next_step_test::fresh;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{pull_request, robot, stack_commit};
use domain_types::landing::{Confirmation, FactWait, Land, LandDecision, LandFact, LandingFix, LandingRefusal, PlannedEffect, Subject, Unchecked};
use domain_types::landing_route::{LandingRoute, RouteFact, RouteGate};
use domain_types::operation::{EffectRecord, EffectStatus, Intent};
use domain_types::patronus::{LandingAllowances, PatronusPushMode, PatronusRobotStatus, PatronusStartOptions};
use domain_types::pull_request::{ChecksState, ChecksSummary, PullRequest, PullRequestAvailability, PullRequestState, ReviewDecision, Reviewer, ReviewerState};
use domain_types::reason::Reason;
use domain_types::session::{Fact, FactState, Parked, PullRequestsView};
use domain_types::stack::{BlockReason, StackBranchState, StackBranchUpdate};
use pretty_assertions::assert_eq;

const OPEN: RouteGate = RouteGate::Open;

/// login, built on auth, at `login-tip`; its pull request #7 into master shows that commit, its checks passing
struct World {
  not_built: Option<BlockReason>,
  parked: Option<Parked>,
  landed: Option<Reason>,
  tip: Option<&'static str>,
  pull_requests: PullRequestsView,
  landing: Option<Vec<LandingRefusal>>,
  conflict: Option<Vec<String>>,
}

fn passing(mut pull_request: PullRequest) -> PullRequest {
  pull_request.checks = Some(ChecksSummary {
    state: ChecksState::Passing,
    passed: 3,
    failed: 0,
    pending: 0,
    failed_names: Vec::new(),
  });
  pull_request
}

fn world() -> World {
  World {
    not_built: None,
    parked: None,
    landed: None,
    tip: Some("login-tip"),
    pull_requests: listed(&[("login", passing(pull_request("login", 7)))]),
    landing: Some(Vec::new()),
    conflict: None,
  }
}

impl World {
  fn decide(&self, gate: RouteGate, land: &Land) -> LandDecision {
    let robots = Robots::new(&[]);
    let profiles = Fact::<PatronusStartOptions>::default();
    let route = LandingRoute::Push {
      gate,
      remote: Some("origin".to_owned()),
    };
    let chain = ["auth", "login"].map(|name| LandsBranch {
      name,
      landed_whole: false,
      in_baseline: false,
      remote: None,
      hold: None,
      not_built: false,
    });
    let facts = LandFacts {
      route: &route,
      target: "master",
      prefix: "user",
      github: false,
      robots: &robots,
      subject: SubjectFacts::Branch(BranchSubject {
        start: BranchStart {
          name: "login",
          not_built: self.not_built,
          stack: None,
          last_commit: None,
          span: None,
          landing_ancestor: None,
          hold: None,
          pushes: false,
        },
        parked: self.parked.clone(),
        landed: self.landed.clone(),
        chain: chain.to_vec(),
        tip: self.tip,
        replaces: None,
        whole_stack_pull_request: None,
      }),
      profiles: &profiles,
      last_profile: None,
      pull_requests: &self.pull_requests,
      readiness: None,
      rules: None,
      landing: self.landing.as_deref(),
      target_conflict: self.conflict.as_deref().map(|files| TargetConflictFact { commit: "c1", files }),
    };
    decide(&facts, land)
  }
}

fn push_effect(tip: &str) -> PlannedEffect {
  PlannedEffect::PushBaseline {
    subject: Subject::branch("login"),
    tip: tip.to_string(),
    remote: "origin".to_string(),
    target: "master".to_string(),
  }
}

/// A piece lands by one push of its copy into the target: the tip of its virtual branch, with the branches it is built
/// on, which is more than its row; its pull request shows that commit and its checks passed, so nothing else is asked,
/// and nothing of Patronus is waited for (no profiles, no review)
#[test]
fn a_piece_lands_by_a_push_of_its_copy_into_the_target_and_says_it_lands_more_than_its_row() {
  let decision = world().decide(OPEN, &land(PatronusPushMode::RebaseAutosquash));
  assert_eq!(
    decision,
    LandDecision {
      confirmations: vec![Confirmation::LandsMoreThanRow { count: 1 }],
      effects: vec![push_effect("login-tip")],
      lands: vec!["auth".to_string(), "login".to_string()],
      ..LandDecision::default()
    }
  );
}

/// A push of the copy runs no CI: the checks that did not pass on its tip are a confirmation (no open pull request, one
/// that shows another commit, failing or running checks, GitHub listing none), never a blocker; changes requested on it
/// too; a listing not answered yet is a wait, never "it did not pass", a stale one still stands
#[test]
fn the_checks_that_did_not_pass_on_the_tip_and_the_changes_requested_are_confirmations() {
  let safe_push = land(PatronusPushMode::RebaseAutosquash);
  let asked = |pull_requests: PullRequestsView| {
    let decision = World { pull_requests, ..world() }.decide(OPEN, &safe_push);
    (decision.confirmations[1..].to_vec(), decision.waits)
  };
  let unchecked = |why| (vec![Confirmation::Unchecked { why }], Vec::new());
  assert_eq!(asked(listed(&[])), unchecked(Unchecked::NoPullRequest));
  let mut closed = passing(pull_request("login", 7));
  closed.state = PullRequestState::Closed;
  assert_eq!(asked(listed(&[("login", closed)])), unchecked(Unchecked::NoPullRequest));
  let mut behind = passing(pull_request("login", 7));
  behind.head_ref_oid = "older".to_string();
  assert_eq!(asked(listed(&[("login", behind)])), unchecked(Unchecked::OtherHead { number: 7 }));
  let with_checks = |state| {
    let mut pull_request = passing(pull_request("login", 7));
    if let Some(checks) = pull_request.checks.as_mut() {
      checks.state = state;
    }
    listed(&[("login", pull_request)])
  };
  assert_eq!(asked(with_checks(ChecksState::Failing)), unchecked(Unchecked::Failing { number: 7 }));
  assert_eq!(asked(with_checks(ChecksState::Pending)), unchecked(Unchecked::Running { number: 7 }));
  // a commit without checks: the repository runs no CI on it
  assert_eq!(asked(listed(&[("login", pull_request("login", 7))])), (Vec::new(), Vec::new()));

  let mut changes = passing(pull_request("login", 7));
  changes.reviewers = vec![Reviewer {
    login: "ikorennoy".to_string(),
    state: ReviewerState::ChangesRequested,
    requested: false,
  }];
  assert_eq!(
    asked(listed(&[("login", changes)])),
    (
      vec![Confirmation::ChangesRequested {
        number: 7,
        by: vec!["ikorennoy".to_string()]
      }],
      Vec::new()
    )
  );
  let mut decided = passing(pull_request("login", 7));
  decided.review_decision = Some(ReviewDecision::ChangesRequested);
  assert_eq!(
    asked(listed(&[("login", decided)])),
    (vec![Confirmation::ChangesRequested { number: 7, by: Vec::new() }], Vec::new())
  );

  let unlisted = PullRequestsView {
    head: fresh(PullRequestAvailability::NotGitHub),
    ..PullRequestsView::default()
  };
  assert_eq!(asked(unlisted), unchecked(Unchecked::Unlisted));
  assert_eq!(
    asked(PullRequestsView::default()),
    (
      Vec::new(),
      vec![FactWait {
        fact: LandFact::PullRequests,
        failed: None
      }]
    )
  );
  let stale = PullRequestsView {
    head: Fact {
      state: FactState::Stale { reason: None },
      ..listed(&[]).head
    },
    ..listed(&[])
  };
  assert_eq!(asked(stale), unchecked(Unchecked::NoPullRequest));
}

/// The gate of the route comes first: a refusal is its first blocker after a Dry Run, which is a robot of Patronus; a
/// fact it has no answer of is a wait with why its read failed; what it asks is a confirmation after the checks
#[test]
fn the_gate_of_the_route_refuses_waits_or_asks_and_a_dry_run_needs_patronus() {
  let world = world();
  let safe_push = land(PatronusPushMode::RebaseAutosquash);
  let no_access = Reason::NoWriteAccess {
    repository: "acme/monorepo".to_string(),
    target: "master".to_string(),
  };
  let refused = world.decide(RouteGate::Refused { reason: no_access }, &safe_push);
  assert_eq!(codes(&refused), ["noWriteAccess"]);
  assert_eq!(codes(&world.decide(OPEN, &land(PatronusPushMode::DryRun))), ["dryRunNeedsPatronus"]);

  let failed = ApiError::new(ErrorCode::NetworkError, "GitHub is down");
  let waits = world.decide(
    RouteGate::Wait {
      fact: RouteFact::TargetRules,
      failed: Some(failed.clone()),
    },
    &safe_push,
  );
  assert_eq!(
    (codes(&waits), waits.waits),
    (
      Vec::new(),
      vec![FactWait {
        fact: LandFact::TargetRules,
        failed: Some(failed)
      }]
    )
  );

  let checks = Reason::BaselineRequiresStatusChecks { target: "master".to_string() };
  let asks = world.decide(
    RouteGate::Confirm {
      confirmations: vec![checks.clone()],
    },
    &safe_push,
  );
  assert_eq!(
    asks.confirmations,
    [Confirmation::LandsMoreThanRow { count: 1 }, Confirmation::RouteRule { reason: checks }]
  );
  assert_eq!(asks.effects, [push_effect("login-tip")]);
}

/// After the gate, what the subject refuses, in this order: what the table alone refuses (not built, parked), what
/// landed it whole (a push of this session too), what the landing gate refuses unless allowed, a conflict with the
/// newest target; a copy not built yet waits and pushes nothing
#[test]
fn the_subject_refuses_in_the_order_of_the_table_what_landed_it_the_landing_gate_and_the_target() {
  let safe_push = land(PatronusPushMode::RebaseAutosquash);
  let missing = LandingRefusal::MissingDeclaration {
    branch: "login".to_string(),
    identifier: "Token".to_string(),
    file: "a.kt".to_string(),
    line: 3,
    used_by: stack_commit("u1", "Use token", "login"),
    declared_by: stack_commit("d1", "Add token", "tokens"),
    fix: LandingFix::Restack {
      branch: "auth".to_string(),
      onto: "tokens".to_string(),
    },
    landing_in: None,
  };
  let all = World {
    not_built: Some(BlockReason::Conflict),
    landed: Some(Reason::LandedIn {
      upstream: "origin/master".to_string(),
    }),
    landing: Some(vec![missing]),
    conflict: Some(vec!["a.kt".to_string()]),
    ..world()
  };
  assert_eq!(codes(&all.decide(OPEN, &safe_push)), ["cantPush", "landedIn", "landingRefused", "targetConflict"]);
  let mut allowed = safe_push.clone();
  allowed.allow.allow_missing = true;
  assert_eq!(codes(&all.decide(OPEN, &allowed)), ["cantPush", "landedIn", "targetConflict"]);
  let parked = World {
    parked: Some(Parked { note: String::new(), with: None }),
    ..world()
  };
  assert_eq!(codes(&parked.decide(OPEN, &safe_push)), ["parked"]);

  let unbuilt = World {
    tip: None,
    landing: None,
    ..world()
  };
  let decision = unbuilt.decide(OPEN, &safe_push);
  assert_eq!(
    (decision.effects, decision.waits.iter().map(|wait| wait.fact.clone()).collect::<Vec<_>>()),
    (Vec::new(), vec![LandFact::Remote, LandFact::LandingNeeds])
  );
}

/// The whole stack lands by a push of its stack branch, only one the sync built of its commits; a robot on it refuses it
/// as it refuses a Safe Push
#[test]
fn a_whole_stack_pushes_its_stack_branch_only_once_the_sync_built_it() {
  let pull_requests = listed(&[]);
  let decide_stack = |stack: &StackBranchUpdate, last_commit| {
    let robots = Robots::new(&[]);
    let profiles = Fact::<PatronusStartOptions>::default();
    let route = LandingRoute::Push {
      gate: OPEN,
      remote: Some("origin".to_owned()),
    };
    let facts = LandFacts {
      route: &route,
      target: "master",
      prefix: "user",
      github: false,
      robots: &robots,
      subject: SubjectFacts::WholeStack(StackSubject {
        start: StackStart {
          stack,
          last_commit,
          landing_member: None,
        },
        parked: None,
        landed: None,
        members: vec![("auth", false), ("login", false)],
        remote: None,
        replaces: None,
        held_by: None,
      }),
      profiles: &profiles,
      last_profile: None,
      pull_requests: &pull_requests,
      readiness: None,
      rules: None,
      landing: Some(&[]),
      target_conflict: None,
    };
    let mut land = land(PatronusPushMode::RebaseAutosquash);
    land.subject = Subject::whole_stack("auth");
    decide(&facts, &land)
  };
  let built = crate::holds_test::stack(StackBranchState::Built, Some("stack-tip"));
  let decision = decide_stack(&built, None);
  assert_eq!(
    (codes(&decision), decision.effects, decision.lands),
    (
      Vec::new(),
      vec![PlannedEffect::PushBaseline {
        subject: Subject::whole_stack("auth"),
        tip: "stack-tip".to_string(),
        remote: "origin".to_string(),
        target: "master".to_string(),
      }],
      vec!["auth".to_string(), "login".to_string()]
    )
  );
  let conflict = crate::holds_test::stack(StackBranchState::Conflict, Some("stack-tip"));
  assert_eq!(codes(&decide_stack(&conflict, None)), ["stackNotBuilt"]);
  let running = robot("w", None, PatronusRobotStatus::Running, 1);
  assert_eq!(codes(&decide_stack(&built, Some(&running))), ["lastCommit"]);
}

/// `bd` accepts the checks that did not pass and the changes requested only with `--without-checks`, what the gate of the
/// route asks only with `--bypass-rules`
#[test]
fn bd_accepts_the_checks_and_what_the_gate_asks_only_by_their_flags() {
  let confirmations = [
    Confirmation::LandsMoreThanRow { count: 1 },
    Confirmation::Unchecked { why: Unchecked::NoPullRequest },
    Confirmation::ChangesRequested { number: 7, by: Vec::new() },
    Confirmation::RouteRule {
      reason: Reason::BaselineRequiresStatusChecks { target: "master".to_string() },
    },
  ];
  let none = LandingAllowances::default();
  assert_eq!(unaccepted(&confirmations, &none), confirmations[1..].iter().collect::<Vec<_>>());
  let checks = LandingAllowances {
    allow_without_checks: true,
    ..none
  };
  assert_eq!(unaccepted(&confirmations, &checks), [&confirmations[3]]);
  let both = LandingAllowances { bypass_rules: true, ..checks };
  assert_eq!(unaccepted(&confirmations, &both), [] as [&Confirmation; 0]);
}

/// Update Project with its fetch follows only a push into the target that landed: not one that failed, not a start of a
/// robot
#[test]
fn the_update_follows_only_a_push_into_the_target_that_landed() {
  let pushed = |status| vec![EffectRecord::new(push_effect("login-tip"), status)];
  assert_eq!(update_after(&pushed(EffectStatus::Done)), Some(Intent::UpdateProject { fetch: true }));
  let refused = EffectStatus::Failed {
    error: ApiError::new(ErrorCode::BaselineMoved, "master moved"),
  };
  assert_eq!(update_after(&pushed(refused)), None);
  let started = vec![EffectRecord::new(
    PlannedEffect::Push {
      branches: vec!["login".to_string()],
      force: false,
    },
    EffectStatus::Done,
  )];
  assert_eq!(update_after(&started), None);
}
