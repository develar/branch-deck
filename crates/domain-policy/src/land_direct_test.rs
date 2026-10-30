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
use domain_types::pull_request::{
  ChecksState, ChecksSummary, MergeMethod, MergeMethods, MergeStateStatus, PullRequest, PullRequestAvailability, PullRequestStackEntry, PullRequestState, ReviewDecision, Reviewer,
  ReviewerState,
};
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
    let route = LandingRoute::Push {
      gate,
      remote: Some("origin".to_owned()),
    };
    self.decide_on(&route, land)
  }

  /// The decision of a repository that lands by a merge of the pull request on GitHub, its gate `merge`
  fn merge(&self, merge: RouteGate, land: &Land) -> LandDecision {
    self.decide_on(&LandingRoute::PullRequest { merge }, land)
  }

  fn decide_on(&self, route: &LandingRoute, land: &Land) -> LandDecision {
    let robots = Robots::new(&[]);
    let profiles = Fact::<PatronusStartOptions>::default();
    let chain = ["auth", "login"].map(|name| LandsBranch {
      name,
      landed_whole: false,
      in_baseline: false,
      remote: None,
      hold: None,
      not_built: false,
    });
    let facts = LandFacts {
      route,
      patronus: false,
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
      patronus: false,
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
fn the_update_follows_a_push_into_the_target_that_landed_or_found_it_moved() {
  let pushed = |status| vec![EffectRecord::new(push_effect("login-tip"), status)];
  assert_eq!(update_after(&pushed(EffectStatus::Done)), Some(Intent::UpdateProject { fetch: true }));
  // the copy is built on the fork point: only the update onto the moved target builds it anew
  let moved = EffectStatus::Failed {
    error: ApiError::new(ErrorCode::BaselineMoved, "master moved"),
  };
  assert_eq!(update_after(&pushed(moved)), Some(Intent::UpdateProject { fetch: true }));
  let refused = EffectStatus::Failed {
    error: ApiError::new(ErrorCode::BaselineRefused, "remote: master is protected"),
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

/// The open pull request #7 of login into master, on the tip of its copy, with GitHub's `mergeStateStatus` `state`
fn merging(state: MergeStateStatus) -> PullRequest {
  PullRequest {
    merge_state: Some(state),
    ..passing(pull_request("login", 7))
  }
}

/// login with its pull request `pull_request` in a repository that allows `methods`
fn merge_world(pull_request: PullRequest, methods: Option<MergeMethods>) -> World {
  let mut pull_requests = listed(&[("login", pull_request)]);
  pull_requests.merge_methods = methods;
  World { pull_requests, ..world() }
}

fn merge_effect(method: MergeMethod) -> PlannedEffect {
  PlannedEffect::MergePullRequest {
    subject: Subject::branch("login"),
    number: 7,
    method,
    head: "login-tip".to_string(),
    target: "master".to_string(),
  }
}

const ALL_METHODS: MergeMethods = MergeMethods {
  rebase: true,
  merge: true,
  squash: true,
  auto_merge: true,
};

/// A pull request GitHub can merge (`CLEAN`, `HAS_HOOKS`) merges on its head, which the copy is, by rebase where the
/// repository allows it, else a merge commit, else squash, which asks first: it pairs no original with what lands; one
/// whose checks that its base doesn't require fail (`UNSTABLE`) merges once the user accepts it
#[test]
fn a_clean_pull_request_merges_by_the_first_method_the_repository_allows_and_an_unstable_one_asks_first() {
  let safe_push = land(PatronusPushMode::RebaseAutosquash);
  let decision = merge_world(merging(MergeStateStatus::Clean), Some(ALL_METHODS)).merge(OPEN, &safe_push);
  assert_eq!(
    decision,
    LandDecision {
      confirmations: vec![Confirmation::LandsMoreThanRow { count: 1 }],
      effects: vec![merge_effect(MergeMethod::Rebase)],
      lands: vec!["auth".to_string(), "login".to_string()],
      pull_request: Some(7),
      ..LandDecision::default()
    }
  );
  let only = |methods: MergeMethods| merge_world(merging(MergeStateStatus::HasHooks), Some(methods)).merge(OPEN, &safe_push);
  let merge_commit = only(MergeMethods { rebase: false, ..ALL_METHODS });
  assert_eq!(merge_commit.effects, vec![merge_effect(MergeMethod::Merge)]);
  let squash = only(MergeMethods {
    squash: true,
    ..MergeMethods::default()
  });
  assert_eq!(squash.effects, vec![merge_effect(MergeMethod::Squash)]);
  assert_eq!(squash.confirmations[1..], [Confirmation::SquashOnly { number: 7 }]);
  // GitHub didn't name the methods: the rebase
  assert_eq!(
    merge_world(merging(MergeStateStatus::Clean), None).merge(OPEN, &safe_push).effects,
    vec![merge_effect(MergeMethod::Rebase)]
  );

  let unstable = merge_world(merging(MergeStateStatus::Unstable), Some(ALL_METHODS)).merge(OPEN, &safe_push);
  assert_eq!(unstable.confirmations[1..], [Confirmation::ChecksNotRequired { number: 7 }]);
  assert_eq!(unstable.effects, vec![merge_effect(MergeMethod::Rebase)]);
  let allowed = Land {
    allow: LandingAllowances {
      allow_without_checks: true,
      ..LandingAllowances::default()
    },
    ..safe_push
  };
  assert_eq!(unaccepted(&unstable.confirmations, &allowed.allow), Vec::<&Confirmation>::new());
}

/// What holds a merge back refuses it with its reason (`BLOCKED` by changes requested, by approvals the base requires,
/// by failing required checks, by something GitHub doesn't name; `DIRTY`, `BEHIND`, a draft); required checks that still
/// run enable the auto-merge where the repository allows it; GitHub that has not computed it (`UNKNOWN`, or not said)
/// is a wait, never "no"
#[test]
fn what_holds_the_merge_back_refuses_it_running_checks_enable_the_auto_merge_and_an_unknown_state_waits() {
  let safe_push = land(PatronusPushMode::RebaseAutosquash);
  let decided = |pull_request: PullRequest, methods: Option<MergeMethods>| merge_world(pull_request, methods).merge(OPEN, &safe_push);
  let blocked = |change: fn(&mut PullRequest)| {
    let mut pull_request = merging(MergeStateStatus::Blocked);
    change(&mut pull_request);
    decided(pull_request, Some(ALL_METHODS)).blockers
  };
  assert_eq!(
    blocked(|it| it.review_decision = Some(ReviewDecision::ChangesRequested)),
    [Reason::MergeChangesRequested { number: 7, by: Vec::new() }]
  );
  assert_eq!(
    blocked(|it| it.review_decision = Some(ReviewDecision::ReviewRequired)),
    [Reason::MergeNeedsApprovals { number: 7 }]
  );
  assert_eq!(
    blocked(|it| it.checks.as_mut().unwrap().state = ChecksState::Failing),
    [Reason::MergeChecksFail { number: 7 }]
  );
  assert_eq!(blocked(|_| {}), [Reason::MergeBlocked { number: 7 }]);
  assert_eq!(decided(merging(MergeStateStatus::Dirty), None).blockers, [Reason::MergeConflict { number: 7 }]);
  assert_eq!(
    decided(merging(MergeStateStatus::Behind), None).blockers,
    [Reason::MergeBehind {
      number: 7,
      target: "master".to_string()
    }]
  );
  let draft = PullRequest {
    is_draft: true,
    ..merging(MergeStateStatus::Draft)
  };
  assert_eq!(decided(draft, None).blockers, [Reason::MergeDraft { number: 7 }]);

  let mut running = merging(MergeStateStatus::Blocked);
  running.checks.as_mut().unwrap().state = ChecksState::Pending;
  let auto = decided(running.clone(), Some(ALL_METHODS));
  assert_eq!(
    auto.effects,
    vec![PlannedEffect::EnableAutoMerge {
      subject: Subject::branch("login"),
      number: 7,
      method: MergeMethod::Rebase,
      head: "login-tip".to_string(),
      target: "master".to_string(),
    }]
  );
  assert_eq!(auto.blockers, []);
  let no_auto = MergeMethods { auto_merge: false, ..ALL_METHODS };
  assert_eq!(decided(running, Some(no_auto)).blockers, [Reason::MergeChecksRunning { number: 7 }]);

  for unknown in [Some(MergeStateStatus::Unknown), None] {
    let pull_request = PullRequest {
      merge_state: unknown,
      ..merging(MergeStateStatus::Clean)
    };
    let decision = decided(pull_request, Some(ALL_METHODS));
    assert_eq!(
      (decision.waits, decision.effects, decision.blockers),
      (
        vec![FactWait {
          fact: LandFact::MergeState { number: 7 },
          failed: None
        }],
        Vec::new(),
        Vec::new()
      )
    );
  }
}

/// A pull request that shows another commit than the copy is pushed first, then GitHub merges the pushed commit once
/// its checks pass (a merge now would merge the old head); without auto-merge the push is the user's first; one whose
/// auto-merge is enabled starts nothing; no open pull request into the target refuses it (Create Pull Request first),
/// one into another base too; the merge gate refuses it below write access
#[test]
fn a_head_behind_the_copy_is_pushed_then_auto_merged_and_without_a_pull_request_nothing_merges() {
  let safe_push = land(PatronusPushMode::RebaseAutosquash);
  let behind = PullRequest {
    head_ref_oid: "older".to_string(),
    ..merging(MergeStateStatus::Clean)
  };
  let pushed = merge_world(behind.clone(), Some(ALL_METHODS)).merge(OPEN, &safe_push);
  assert_eq!(
    pushed.effects,
    vec![
      PlannedEffect::Push {
        branches: vec!["login".to_string()],
        force: false,
      },
      PlannedEffect::EnableAutoMerge {
        subject: Subject::branch("login"),
        number: 7,
        method: MergeMethod::Rebase,
        head: "login-tip".to_string(),
        target: "master".to_string(),
      },
    ]
  );
  let no_auto = MergeMethods { auto_merge: false, ..ALL_METHODS };
  assert_eq!(merge_world(behind, Some(no_auto)).merge(OPEN, &safe_push).blockers, [Reason::MergeHeadBehind { number: 7 }]);

  let auto_merging = PullRequest {
    auto_merge: Some(MergeMethod::Rebase),
    ..merging(MergeStateStatus::Blocked)
  };
  let decision = merge_world(auto_merging, Some(ALL_METHODS)).merge(OPEN, &safe_push);
  assert_eq!((decision.effects, decision.blockers), (Vec::new(), Vec::new()));

  let none = World {
    pull_requests: listed(&[]),
    ..world()
  };
  assert_eq!(none.merge(OPEN, &safe_push).blockers, [Reason::NoPullRequestToMerge { branch: "login".to_string() }]);
  let other = PullRequest {
    base_ref_name: "release/253".to_string(),
    ..merging(MergeStateStatus::Clean)
  };
  assert_eq!(
    merge_world(other, None).merge(OPEN, &safe_push).blockers,
    [Reason::OtherBase {
      number: 7,
      base: "release/253".to_string(),
      target: "master".to_string()
    }]
  );
  let no_access = RouteGate::Refused {
    reason: Reason::NoWriteAccess {
      repository: "acme/monorepo".to_string(),
      target: "master".to_string(),
    },
  };
  let refused = merge_world(merging(MergeStateStatus::Clean), None).merge(no_access, &safe_push);
  assert_eq!(codes(&refused), ["noWriteAccess"]);
  // a Dry Run is a robot of Patronus
  assert_eq!(
    codes(&merge_world(merging(MergeStateStatus::Clean), None).merge(OPEN, &land(PatronusPushMode::DryRun))),
    ["dryRunNeedsPatronus"]
  );
}

/// Update Project follows a merge by the click; an auto-merge GitHub completes later runs none
#[test]
fn the_update_follows_a_merge_of_the_pull_request_and_no_auto_merge() {
  let ran = |effect: PlannedEffect, status| vec![EffectRecord::new(effect, status)];
  assert_eq!(
    update_after(&ran(merge_effect(MergeMethod::Rebase), EffectStatus::Done)),
    Some(Intent::UpdateProject { fetch: true })
  );
  let refused = EffectStatus::Failed {
    error: ApiError::new(ErrorCode::NotMergeable, "Pull request is not mergeable"),
  };
  assert_eq!(update_after(&ran(merge_effect(MergeMethod::Rebase), refused)), None);
  let auto = PlannedEffect::EnableAutoMerge {
    subject: Subject::branch("login"),
    number: 7,
    method: MergeMethod::Rebase,
    head: "login-tip".to_string(),
    target: "master".to_string(),
  };
  assert_eq!(update_after(&ran(auto, EffectStatus::Done)), None);
}

/// login as the layer #7 of a stack of GitHub above auth's #6 into master (`stack` 4), both on the tips of their copies;
/// `auth` is auth's pull request as GitHub lists it
fn stacked_world(auth: PullRequest) -> World {
  let entry = |position| PullRequestStackEntry {
    number: 4,
    base_ref_name: "master".to_owned(),
    size: 2,
    position,
  };
  let login = PullRequest {
    base_ref_name: "user/virtual/auth".to_owned(),
    stack: Some(entry(2)),
    ..merging(MergeStateStatus::Clean)
  };
  let auth = PullRequest { stack: Some(entry(1)), ..auth };
  let mut pull_requests = listed(&[("auth", auth), ("login", login)]);
  pull_requests.merge_methods = Some(ALL_METHODS);
  World { pull_requests, ..world() }
}

/// The open #6 of auth, the layer below, with GitHub's `mergeStateStatus` `state`
fn below(state: MergeStateStatus) -> PullRequest {
  PullRequest {
    merge_state: Some(state),
    ..passing(pull_request("auth", 6))
  }
}

/// A layer of a stack of GitHub merges with the layers below it in one asynchronous merge of its own pull request
/// (docs/design/github-stacks.md): a piece in one move; the first layer below a merge can't take holds it by that
/// layer's number, and no auto-merge is offered a stack
#[test]
fn a_layer_of_a_stack_merges_with_the_layers_below_it_and_the_first_one_that_cant_holds_it() {
  let safe_push = land(PatronusPushMode::RebaseAutosquash);
  let decision = stacked_world(below(MergeStateStatus::Clean)).merge(OPEN, &safe_push);
  assert_eq!(
    decision.effects,
    vec![PlannedEffect::MergeStack {
      subject: Subject::branch("login"),
      number: 7,
      method: MergeMethod::Rebase,
      head: "login-tip".to_string(),
      target: "master".to_string(),
      layers: vec![6],
    }]
  );
  assert_eq!(decision.pull_request, Some(7));

  let mut approvals = below(MergeStateStatus::Blocked);
  approvals.review_decision = Some(ReviewDecision::ReviewRequired);
  assert_eq!(stacked_world(approvals).merge(OPEN, &safe_push).blockers, vec![Reason::MergeNeedsApprovals { number: 6 }]);
  assert_eq!(
    stacked_world(below(MergeStateStatus::Dirty)).merge(OPEN, &safe_push).blockers,
    vec![Reason::MergeConflict { number: 6 }]
  );
  // GitHub has not computed the layer below: a wait, never "no"
  let unknown = stacked_world(below(MergeStateStatus::Unknown)).merge(OPEN, &safe_push);
  assert_eq!((unknown.blockers, unknown.effects), (Vec::new(), Vec::new()));
  assert_eq!(
    unknown.waits,
    vec![FactWait {
      fact: LandFact::MergeState { number: 6 },
      failed: None
    }]
  );

  // required checks that still run hold a stack: GitHub has no auto-merge of one
  let mut running = merging(MergeStateStatus::Blocked);
  running.checks = Some(ChecksSummary {
    state: ChecksState::Pending,
    passed: 0,
    failed: 0,
    pending: 1,
    failed_names: Vec::new(),
  });
  let mut world = stacked_world(below(MergeStateStatus::Clean));
  let login = world.pull_requests.pull_requests.get_mut("login").unwrap();
  login.merge_state = running.merge_state;
  login.checks = running.checks;
  assert_eq!(world.merge(OPEN, &safe_push).blockers, vec![Reason::MergeChecksRunning { number: 7 }]);
}

/// A pull request into the branch of the layer below that GitHub lists in no stack would merge into that branch: its
/// merge is refused until the stack is linked
#[test]
fn a_layer_outside_a_stack_is_not_merged() {
  let safe_push = land(PatronusPushMode::RebaseAutosquash);
  let mut world = stacked_world(below(MergeStateStatus::Clean));
  world.pull_requests.pull_requests.get_mut("login").unwrap().stack = None;
  // a pull request into a virtual branch outside a stack is retargeted to the target, as any such
  assert_eq!(
    world.merge(OPEN, &safe_push).blockers,
    vec![Reason::OtherBase {
      number: 7,
      base: "auth".to_owned(),
      target: "master".to_owned(),
    }]
  );
}
