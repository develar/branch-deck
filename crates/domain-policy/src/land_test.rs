use crate::facts::{RemoteBranch, Robots};
use crate::land::{
  BranchStart, BranchSubject, LandFacts, LandsBranch, StackStart, StackSubject, Start, SubjectFacts, TargetConflictFact, allows_mode, decide, default_profile, offered_profiles,
  on_last_commit, restart_refusal, start_refusal, took_as_is, unaccepted,
};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{missing_approvals, pull_request, robot, robot_of, stack_commit, stack_robot};
use domain_types::landing::{ApprovalsSkip, Confirmation, FactWait, Land, LandDecision, LandFact, LandingFix, LandingRefusal, PlannedEffect, ProfileChoice, StartRoute, Subject};
use domain_types::patronus::{
  LandingAllowances, PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotProfile, PatronusRobotStatus, PatronusStartOptions, ReviewReadiness, ReviewReadinessBranch,
  UnavailableReadinessReason, UnknownReadinessReason,
};
use domain_types::pull_request::{BranchRules, PullRequest, PullRequestAvailability, Reviewer, ReviewerState};
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView, RobotOf, SpanView};
use domain_types::session::{Fact, FactState, Parked, PullRequestsView, SubjectReadiness};
use domain_types::stack::{BlockReason, BranchStack, StackBlock, StackBranchState, StackBranchUpdate};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

const SAFE_PUSH: PatronusPushMode = PatronusPushMode::RebaseAutosquash;
const DRY_RUN: PatronusPushMode = PatronusPushMode::DryRun;

fn running(id: &str, branch: &str) -> PatronusRobot {
  robot(id, Some(branch), PatronusRobotStatus::Running, 1)
}

/// login, with nothing in its way
fn start<'a>() -> BranchStart<'a> {
  BranchStart {
    name: "login",
    not_built: None,
    stack: None,
    last_commit: None,
    span: None,
    landing_ancestor: None,
    hold: None,
    pushes: true,
  }
}

/// The robot of the commit a branch is at, by its hashes or by its up-to-date remote branch: a running or canceling one
/// refuses a start of the same commits; a finished one, or one on a commit the branch moved past, does not
#[test]
fn a_robot_on_the_last_commit_is_one_that_runs_on_the_branch_as_it_is() {
  let remote = RemoteBranch {
    tip: Some("tip"),
    up_to_date: true,
    drops: 0,
    others: 0,
  };
  let up_to_date = Some(remote);
  assert!(took_as_is("tip", Some("tip"), None));
  assert!(took_as_is("tip", Some("rebuilt"), up_to_date));
  assert!(!took_as_is("tip", None, up_to_date));
  assert!(!took_as_is("tip", Some("new"), Some(RemoteBranch { up_to_date: false, ..remote })));

  let at = |status: PatronusRobotStatus| on_last_commit(&robot("r", Some("login"), status, 1), Some("tip"), None);
  assert!(at(PatronusRobotStatus::Running));
  assert!(at(PatronusRobotStatus::Canceling));
  assert!(!at(PatronusRobotStatus::Failure));
  assert!(!at(PatronusRobotStatus::Canceled));
  assert!(!on_last_commit(&running("r", "login"), Some("new"), None));
}

/// A start of a branch, in the order of the refusals: blocked, a robot on the last commit, then for a Safe Push the
/// robot of a branch built on it that lands it, then one that lands an ancestor
#[test]
fn a_safe_push_waits_for_a_block_the_last_commit_a_robot_that_lands_it_then_one_that_lands_an_ancestor() {
  let last = running("last", "login");
  let lands_it = running("s", "session");
  let lands_ancestor = running("a", "auth");
  let robots = Robots::new([&last, &lands_it, &lands_ancestor]);
  let span = SpanView {
    by: robot_of("session", "s"),
    carried: None,
  };
  let ancestor = robot_of("auth", "a");
  let stack = BranchStack {
    blocked_by: Some(StackBlock {
      parent: "auth".to_string(),
      reason: BlockReason::Conflict,
    }),
    ..BranchStack::default()
  };
  let all = BranchStart {
    not_built: Some(BlockReason::Blocked),
    stack: Some(&stack),
    last_commit: Some(&last),
    span: Some(&span),
    landing_ancestor: Some(&ancestor),
    ..start()
  };
  let refusal = |start: BranchStart<'_>| start_refusal(&Start::Branch(start), SAFE_PUSH, &robots);
  assert_eq!(
    refusal(all),
    Some(Reason::Blocked {
      branch: "login".to_string(),
      by: Some("auth".to_string())
    })
  );
  let all = BranchStart { not_built: None, ..all };
  assert_eq!(
    refusal(all),
    Some(Reason::LastCommit {
      robot_id: "last".to_string(),
      mode: SAFE_PUSH
    })
  );
  let all = BranchStart { last_commit: None, ..all };
  assert_eq!(
    refusal(all),
    Some(Reason::Descendant {
      branch: "login".to_string(),
      by: robot_of("session", "s"),
      mode: SAFE_PUSH,
      carried: None
    })
  );
  let all = BranchStart { span: None, ..all };
  assert_eq!(
    refusal(all),
    Some(Reason::Ancestor {
      landing: ancestor.clone(),
      mode: SAFE_PUSH
    })
  );
  assert_eq!(refusal(start()), None);
  // an error of the branch is its own step, not a refusal of the start
  assert_eq!(
    refusal(BranchStart {
      not_built: Some(BlockReason::Error),
      ..start()
    }),
    None
  );
  // a fact whose robot the user's robots lack refuses nothing
  assert_eq!(
    start_refusal(&Start::Branch(BranchStart { span: Some(&span), ..start() }), SAFE_PUSH, &Robots::default()),
    None
  );
}

/// A Dry Run lands nothing: it waits only for a block and the robot on its last commit, and for a Safe Push of a branch
/// built on it while its own push would go into what that Safe Push lands (D15); its own Safe Push on an older commit
/// is the hold of another rule
#[test]
fn a_dry_run_waits_for_the_last_commit_and_for_the_span_hold_of_the_push_it_makes() {
  let lands_it = running("s", "session");
  let robots = Robots::new([&lands_it]);
  let span = SpanView {
    by: robot_of("session", "s"),
    carried: None,
  };
  let hold = HoldView {
    kind: HoldKind::Span,
    by: robot_of("session", "s"),
    carried: None,
  };
  let held = BranchStart {
    span: Some(&span),
    landing_ancestor: Some(&span.by),
    hold: Some(&hold),
    ..start()
  };
  assert_eq!(
    start_refusal(&Start::Branch(held), DRY_RUN, &robots),
    Some(Reason::Held {
      branch: "login".to_string(),
      hold: hold.clone(),
      mode: SAFE_PUSH,
      target: "master".to_string()
    })
  );
  // nothing to push: the dry run starts from the remote, which the Safe Push doesn't need
  assert_eq!(start_refusal(&Start::Branch(BranchStart { pushes: false, ..held }), DRY_RUN, &robots), None);
  let own = HoldView {
    kind: HoldKind::Own,
    ..hold.clone()
  };
  assert_eq!(start_refusal(&Start::Branch(BranchStart { hold: Some(&own), ..held }), DRY_RUN, &robots), None);
}

fn stack(state: StackBranchState) -> StackBranchUpdate {
  StackBranchUpdate {
    root: "auth".to_string(),
    branch_name: "user/stack/auth".to_string(),
    members: vec!["auth".to_string(), "login".to_string()],
    commit_count: 2,
    state,
    tip: Some("stack-tip".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  }
}

/// A start of the whole stack: a stack branch the sync did not build, the robot on its commit, then for a Safe Push the
/// robot of a member that lands a part of it; a Dry Run waits for no member
#[test]
fn a_whole_stack_waits_for_its_build_its_last_commit_then_a_member_that_lands_a_part() {
  let whole = stack_robot("w", "auth", PatronusRobotStatus::Running, "stack-tip");
  let member = running("l", "login");
  let robots = Robots::new([&whole, &member]);
  let built = stack(StackBranchState::Built);
  let landing = robot_of("login", "l");
  let start = StackStart {
    stack: &built,
    last_commit: Some(&whole),
    landing_member: Some(&landing),
  };
  let refusal = |start: StackStart<'_>, mode| start_refusal(&Start::WholeStack(start), mode, &robots);
  let conflict = stack(StackBranchState::Conflict);
  assert!(matches!(
    refusal(StackStart { stack: &conflict, ..start }, SAFE_PUSH),
    Some(Reason::StackNotBuilt {
      state: StackBranchState::Conflict,
      ..
    })
  ));
  assert!(matches!(refusal(start, DRY_RUN), Some(Reason::LastCommit { .. })));
  let start = StackStart { last_commit: None, ..start };
  assert_eq!(
    refusal(start, SAFE_PUSH),
    Some(Reason::Ancestor {
      landing: landing.clone(),
      mode: SAFE_PUSH
    })
  );
  assert_eq!(refusal(start, DRY_RUN), None);
}

/// A restart is the start of its robot's subject that the robot itself does not refuse and that pushes nothing; a
/// subject landed whole refuses it last
#[test]
fn a_restart_is_refused_by_the_table_first_then_as_a_start_without_its_own_robot_then_by_a_landing() {
  let own = running("own", "login");
  let lands_it = running("s", "session");
  let robots = Robots::new([&own, &lands_it]);
  let hold = HoldView {
    kind: HoldKind::Span,
    by: robot_of("session", "s"),
    carried: None,
  };
  let restart = BranchStart {
    last_commit: Some(&own),
    hold: Some(&hold),
    ..start()
  };
  assert_eq!(restart_refusal(&Start::Branch(restart), DRY_RUN, &robots, None, None), None);
  let landed = Reason::Landed {
    branch: "login".to_string(),
    by: robot_of("session", "m"),
    own: false,
    target: "master".to_string(),
  };
  assert_eq!(restart_refusal(&Start::Branch(restart), DRY_RUN, &robots, None, Some(landed.clone())), Some(landed.clone()));
  // a parked subject stays local: a restart would land it
  let parked = Reason::Parked {
    branch: "login".to_string(),
    with: None,
    note: String::new(),
  };
  assert_eq!(restart_refusal(&Start::Branch(restart), DRY_RUN, &robots, Some(parked.clone()), Some(landed)), Some(parked));
  let span = SpanView {
    by: robot_of("session", "s"),
    carried: None,
  };
  assert!(matches!(
    restart_refusal(&Start::Branch(BranchStart { span: Some(&span), ..restart }), SAFE_PUSH, &robots, None, None),
    Some(Reason::Descendant { .. })
  ));
}

// ---- part 2: `decide` over the facts of a start, every fact also without an answer ----

/// The facts of one start, owned: `decide` borrows them
struct World {
  robots: Vec<PatronusRobot>,
  profiles: Fact<PatronusStartOptions>,
  last_profile: Option<PatronusRobotProfile>,
  pull_requests: PullRequestsView,
  readiness: Option<Fact<SubjectReadiness>>,
  rules: Option<Fact<BranchRules>>,
  landing: Option<Vec<LandingRefusal>>,
  /// login on auth: the chain root first, each `(remote, landed whole, held)`: held by a running Safe Push of a branch
  /// built on it, other than the one a start replaces
  chain: Vec<(&'static str, Option<RemoteBranch<'static>>, bool, bool)>,
  hold: Option<HoldView>,
  replaces: Option<&'static str>,
  parked: Option<Parked>,
  not_built: Option<BlockReason>,
  conflict: Option<Vec<String>>,
}

const UNPUSHED: RemoteBranch<'static> = RemoteBranch {
  tip: None,
  up_to_date: false,
  drops: 0,
  others: 0,
};
const ON_REMOTE: RemoteBranch<'static> = RemoteBranch {
  tip: Some("tip"),
  up_to_date: true,
  ..UNPUSHED
};

fn fresh<T>(value: T) -> Fact<T> {
  Fact {
    value: Some(value),
    state: FactState::Fresh,
    ..Fact::default()
  }
}

fn failed<T>(message: &str) -> Fact<T> {
  Fact {
    value: None,
    state: FactState::Unavailable {
      reason: ApiError::new(ErrorCode::NetworkError, message),
    },
    ..Fact::default()
  }
}

fn profile(id: &str, skips_review: Option<bool>) -> PatronusProfile {
  PatronusProfile {
    id: id.to_string(),
    name: id.to_uppercase(),
    description: None,
    allowed_push_modes: Vec::new(),
    skips_review,
  }
}

fn options(supported: bool) -> PatronusStartOptions {
  PatronusStartOptions {
    target_branch: "master".to_string(),
    supported,
    profiles: vec![profile("lsc", None), profile("fast", Some(false))],
    emergency_profiles: Vec::new(),
    default_profile_id: None,
  }
}

fn listed(pull_requests: &[(&str, PullRequest)]) -> PullRequestsView {
  PullRequestsView {
    head: fresh(PullRequestAvailability::Available {
      owner: "acme".to_string(),
      name: "monorepo".to_string(),
    }),
    pull_requests: pull_requests.iter().map(|(name, pull_request)| ((*name).to_string(), pull_request.clone())).collect(),
    stack_pull_requests: BTreeMap::new(),
  }
}

fn readiness(answer: ReviewReadiness) -> Fact<SubjectReadiness> {
  fresh(SubjectReadiness {
    ask: ReviewReadinessBranch::starting_from("login".to_string(), None, "login-tip".to_string(), None),
    requirement: None,
    readiness: answer,
  })
}

fn approved(mut pull_request: PullRequest) -> PullRequest {
  pull_request.reviewers = vec![Reviewer {
    login: "ikorennoy".to_string(),
    state: ReviewerState::Approved,
    requested: false,
  }];
  pull_request
}

/// login on auth, both unpushed, every fact answered: nothing refuses it, it waits for nothing
fn world() -> World {
  World {
    robots: Vec::new(),
    profiles: fresh(options(true)),
    last_profile: None,
    pull_requests: listed(&[]),
    readiness: Some(readiness(ReviewReadiness::NotRequired)),
    rules: None,
    landing: Some(Vec::new()),
    chain: vec![("auth", Some(UNPUSHED), false, false), ("login", Some(UNPUSHED), false, false)],
    hold: None,
    replaces: None,
    parked: None,
    not_built: None,
    conflict: None,
  }
}

fn land(mode: PatronusPushMode) -> Land {
  Land {
    subject: Subject::branch("login"),
    mode,
    profile: ProfileChoice::Default,
    without_approvals: false,
    create_pull_request: false,
    allow: LandingAllowances::default(),
  }
}

impl World {
  fn decide(&self, land: &Land) -> LandDecision {
    let robots = Robots::new(&self.robots);
    let conflict = self.conflict.as_deref();
    let span = HoldView {
      kind: HoldKind::Span,
      by: robot_of("oauth", "oauth-run"),
      carried: None,
    };
    let facts = LandFacts {
      target: "master",
      prefix: "user",
      github: true,
      robots: &robots,
      subject: SubjectFacts::Branch(BranchSubject {
        start: BranchStart {
          not_built: self.not_built,
          hold: self.hold.as_ref(),
          ..start()
        },
        parked: self.parked.clone(),
        landed: None,
        chain: self
          .chain
          .iter()
          .map(|(name, remote, landed_whole, held)| LandsBranch {
            name,
            landed_whole: *landed_whole,
            in_baseline: false,
            remote: *remote,
            hold: held.then_some(&span),
            not_built: false,
          })
          .collect(),
        tip: Some("login-tip"),
        replaces: self.replaces,
        whole_stack_pull_request: None,
      }),
      profiles: &self.profiles,
      last_profile: self.last_profile.as_ref(),
      pull_requests: &self.pull_requests,
      readiness: self.readiness.as_ref(),
      rules: self.rules.as_ref(),
      landing: self.landing.as_deref(),
      target_conflict: conflict.map(|files| TargetConflictFact { commit: "c1", files }),
    };
    decide(&facts, land)
  }
}

fn codes(decision: &LandDecision) -> Vec<String> {
  decision
    .blockers
    .iter()
    .map(|reason| serde_json::to_value(reason).unwrap()["code"].as_str().unwrap().to_string())
    .collect()
}

fn start_route(decision: &LandDecision) -> Option<&StartRoute> {
  decision.start_route()
}

/// With every fact answered and nothing in its way, a Safe Push of a stacked branch lands its parent with it, pushes
/// both, root first, and starts through the API with the default checks; it confirms only that it lands more than its
/// row. A Dry Run checks no review
#[test]
fn a_start_with_nothing_in_its_way_pushes_what_it_lands_root_first_then_starts() {
  let decision = world().decide(&land(SAFE_PUSH));
  assert_eq!(
    decision,
    LandDecision {
      blockers: Vec::new(),
      confirmations: vec![Confirmation::LandsMoreThanRow { count: 1 }],
      waits: Vec::new(),
      effects: vec![
        PlannedEffect::Push {
          branches: vec!["auth".to_string(), "login".to_string()],
          force: false,
        },
        PlannedEffect::StartRobot {
          route: StartRoute::Api { profile: None }
        },
      ],
      lands: vec!["auth".to_string(), "login".to_string()],
      pull_request: None,
      approvals_skipped_by: None,
    }
  );
  let world = World { readiness: None, ..world() };
  assert_eq!((world.decide(&land(DRY_RUN)).waits, world.decide(&land(SAFE_PUSH)).waits.len()), (Vec::new(), 1));
}

/// Every fact without an answer is a wait, never "no": the remote of a branch, the profiles, the pull requests, the
/// review, the rules of a push into an approved pull request and the landing gate; a failed read of the profiles says
/// why. A failed listing of the pull requests and a review Patronus couldn't answer are confirmations instead (D14),
/// the rules GitHub didn't answer too; each waits or confirms, none refuses
#[test]
fn every_fact_without_an_answer_waits_or_confirms_and_never_refuses() {
  let wait = |fact: LandFact| FactWait { fact, failed: None };
  let unknown_remote = World {
    chain: vec![("auth", None, false, false), ("login", Some(UNPUSHED), false, false)],
    ..world()
  };
  assert_eq!(unknown_remote.decide(&land(SAFE_PUSH)).waits, vec![wait(LandFact::Remote)]);
  for loading in [
    Fact::default(),
    Fact {
      state: FactState::Unknown,
      ..Fact::default()
    },
  ] {
    let profiles = World { profiles: loading, ..world() }.decide(&land(SAFE_PUSH));
    assert_eq!((profiles.waits.clone(), codes(&profiles)), (vec![wait(LandFact::Profiles)], Vec::<String>::new()));
  }
  let failed_profiles = World {
    profiles: failed("Patronus is down"),
    ..world()
  }
  .decide(&land(SAFE_PUSH));
  assert_eq!(failed_profiles.waits[0].failed.as_ref().map(|error| error.message.as_str()), Some("Patronus is down"));
  let stale_profiles = World {
    profiles: Fact {
      state: FactState::Stale { reason: None },
      ..fresh(options(true))
    },
    ..world()
  };
  assert_eq!(stale_profiles.decide(&land(SAFE_PUSH)).waits, vec![wait(LandFact::Profiles)]);

  let mut loading_listing = world();
  loading_listing.pull_requests.head = Fact::default();
  assert_eq!(loading_listing.decide(&land(DRY_RUN)).waits, vec![wait(LandFact::PullRequests)]);
  let mut failed_listing = world();
  failed_listing.pull_requests.head = failed("GitHub is down");
  let decision = failed_listing.decide(&land(SAFE_PUSH));
  assert_eq!(
    (decision.waits.clone(), decision.confirmations.last().cloned(), start_route(&decision).cloned()),
    (Vec::new(), Some(Confirmation::ApprovalsJudgedAtStart), Some(StartRoute::Api { profile: None }))
  );

  for readiness in [
    None,
    Some(Fact::default()),
    Some(Fact {
      state: FactState::Stale { reason: None },
      ..readiness(ReviewReadiness::NotRequired)
    }),
  ] {
    let decision = World { readiness, ..world() }.decide(&land(SAFE_PUSH));
    assert_eq!((decision.waits.clone(), codes(&decision)), (vec![wait(LandFact::Readiness)], Vec::<String>::new()));
  }
  for unanswered in [
    failed("pre-check failed"),
    readiness(ReviewReadiness::Unavailable {
      reason: UnavailableReadinessReason::Error,
      message: "pre-check failed".to_string(),
    }),
  ] {
    let decision = World {
      readiness: Some(unanswered),
      ..world()
    }
    .decide(&land(SAFE_PUSH));
    assert_eq!(
      (codes(&decision), decision.confirmations.last().cloned()),
      (
        Vec::<String>::new(),
        Some(Confirmation::ReviewUnknown {
          message: "pre-check failed".to_string()
        })
      )
    );
  }
  let landing = World { landing: None, ..world() }.decide(&land(DRY_RUN));
  assert_eq!(landing.waits, vec![wait(LandFact::LandingNeeds)]);
}

/// What the table alone refuses comes first: the sync did not build the branch, then it stays local; then a target
/// Patronus doesn't protect; a push that drops commits of someone else refuses unless the user allowed it (D12), which
/// makes it a confirmation and the push a force push
#[test]
fn the_table_refuses_first_then_the_target_then_a_push_that_drops_remote_commits_unless_allowed() {
  let parked = Parked {
    note: "IJPL-1".to_string(),
    with: Some("auth".to_string()),
  };
  let not_built = World {
    not_built: Some(BlockReason::Conflict),
    parked: Some(parked.clone()),
    ..world()
  };
  assert_eq!(
    not_built.decide(&land(SAFE_PUSH)).blockers[0],
    Reason::CantPush {
      branch: "login".to_string(),
      block: BlockReason::Conflict,
    }
  );
  let decision = World { parked: Some(parked), ..world() }.decide(&land(DRY_RUN));
  assert_eq!(
    decision.blockers,
    vec![Reason::Parked {
      branch: "login".to_string(),
      with: Some("auth".to_string()),
      note: "IJPL-1".to_string(),
    }]
  );
  let unprotected = World {
    profiles: fresh(options(false)),
    ..world()
  };
  assert_eq!(codes(&unprotected.decide(&land(DRY_RUN))), ["unprotected"]);

  let behind = RemoteBranch { drops: 2, ..UNPUSHED };
  let dropping = World {
    chain: vec![("auth", Some(ON_REMOTE), false, false), ("login", Some(behind), false, false)],
    ..world()
  };
  let refused = dropping.decide(&land(SAFE_PUSH));
  assert_eq!(
    refused.blockers,
    vec![Reason::DropsRemote {
      branch: "login".to_string(),
      behind: 2,
    }]
  );
  let mut forced = land(SAFE_PUSH);
  forced.allow.allow_force = true;
  let decision = dropping.decide(&forced);
  assert_eq!(
    (codes(&decision), decision.confirmations.last().cloned(), decision.effects[0].clone()),
    (
      Vec::<String>::new(),
      Some(Confirmation::ForcePushDrops {
        branch: "login".to_string(),
        commits: 2
      }),
      PlannedEffect::Push {
        branches: vec!["login".to_string()],
        force: true
      }
    )
  );
}

/// A Safe Push of a branch whose own Safe Push runs on an older commit replaces it: it confirms the cancel, which goes
/// before the push (also without a push, unless it only opens the pull request). A Dry Run replaces none: its push into
/// the held branch is refused, and without a push it starts beside the Safe Push. A held ancestor ends the push below it,
/// an ancestor a merged Safe Push landed whole is neither pushed nor landed
#[test]
fn a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor() {
  let own = robot("old", Some("login"), PatronusRobotStatus::Running, 1);
  let replaced = World {
    robots: vec![own],
    hold: Some(HoldView {
      kind: HoldKind::Own,
      by: robot_of("login", "old"),
      carried: None,
    }),
    replaces: Some("old"),
    ..world()
  };
  let decision = replaced.decide(&land(SAFE_PUSH));
  assert_eq!(
    (codes(&decision), decision.confirmations.clone(), decision.effects[0].clone()),
    (
      Vec::<String>::new(),
      vec![Confirmation::LandsMoreThanRow { count: 1 }, Confirmation::CancelsRobot { robot_id: "old".to_string() }],
      PlannedEffect::CancelRobot { robot_id: "old".to_string() }
    )
  );
  assert_eq!(codes(&replaced.decide(&land(DRY_RUN))), ["held"]);
  let on_remote = World {
    chain: vec![("auth", Some(ON_REMOTE), false, false), ("login", Some(ON_REMOTE), false, false)],
    ..replaced
  };
  let mut opens = land(SAFE_PUSH);
  opens.create_pull_request = true;
  assert_eq!(
    (
      codes(&on_remote.decide(&land(DRY_RUN))),
      on_remote.decide(&opens).confirmations,
      on_remote.decide(&land(SAFE_PUSH)).confirmations.len()
    ),
    (Vec::<String>::new(), vec![Confirmation::LandsMoreThanRow { count: 1 }], 2)
  );

  let held_ancestor = World {
    chain: vec![("auth", Some(UNPUSHED), false, true), ("login", Some(UNPUSHED), false, false)],
    ..world()
  };
  assert_eq!(
    held_ancestor.decide(&land(DRY_RUN)).effects[0],
    PlannedEffect::Push {
      branches: vec!["login".to_string()],
      force: false,
    }
  );
  let landed_ancestor = World {
    chain: vec![("auth", Some(UNPUSHED), true, false), ("login", Some(ON_REMOTE), false, false)],
    ..world()
  };
  let decision = landed_ancestor.decide(&land(SAFE_PUSH));
  assert_eq!((decision.lands, decision.confirmations, decision.effects.len()), (vec!["login".to_string()], Vec::new(), 1));
  // one that landed whole is left as it is, someone's commits on its remote too, and the push goes on below it, as Push
  // Stack of the row does (`push::start_push`); a held one ends it
  let behind = RemoteBranch { drops: 2, ..UNPUSHED };
  let below_landed = World {
    chain: vec![
      ("base", Some(UNPUSHED), false, false),
      ("auth", Some(behind), true, false),
      ("login", Some(UNPUSHED), false, false),
    ],
    ..world()
  };
  let decision = below_landed.decide(&land(SAFE_PUSH));
  assert_eq!(
    (codes(&decision), decision.effects[0].clone()),
    (
      Vec::<String>::new(),
      PlannedEffect::Push {
        branches: vec!["base".to_string(), "login".to_string()],
        force: false,
      }
    )
  );
  let above_held = World {
    chain: vec![
      ("base", Some(UNPUSHED), false, true),
      ("auth", Some(UNPUSHED), true, false),
      ("login", Some(ON_REMOTE), false, false),
    ],
    ..world()
  };
  assert_eq!(above_held.decide(&land(DRY_RUN)).effects.len(), 1, "nothing to push: a StartRobot only");
}

/// The landing gate refuses what would not build unless allowed, kind by kind; a conflict with the newest target refuses
/// it after them
#[test]
fn the_landing_gate_refuses_unless_allowed_then_a_conflict_with_the_target() {
  let missing = LandingRefusal::MissingDeclaration {
    branch: "login".to_string(),
    identifier: "parse".to_string(),
    file: "a.kt".to_string(),
    line: 1,
    used_by: stack_commit("c0", "", ""),
    declared_by: stack_commit("c0", "", ""),
    fix: LandingFix::Restack {
      branch: "auth".to_string(),
      onto: "util".to_string(),
    },
    landing_in: None,
  };
  let gated = World {
    landing: Some(vec![missing.clone()]),
    conflict: Some(vec!["a.kt".to_string()]),
    ..world()
  };
  assert_eq!(
    gated.decide(&land(DRY_RUN)).blockers,
    vec![
      Reason::LandingRefused { refusal: Box::new(missing) },
      Reason::TargetConflict {
        commit: "c1".to_string(),
        files: vec!["a.kt".to_string()],
      },
    ]
  );
  let mut allowed = land(DRY_RUN);
  allowed.allow.allow_missing = true;
  assert_eq!(codes(&gated.decide(&allowed)), ["targetConflict"]);
  allowed.allow = LandingAllowances {
    allow_dangling: true,
    ..LandingAllowances::default()
  };
  assert_eq!(codes(&gated.decide(&allowed)), ["landingRefused", "targetConflict"]);
}

/// The pull request of the branch decides the route: one into the target, or into a virtual branch once it is
/// retargeted first (D11), takes a `/patronus` comment, which runs the default checks; a draft refuses a Safe Push. One
/// into another base starts nothing from it and refuses only a Safe Push whose approvals count (D3b): a Dry Run, a start
/// without approvals and a profile that skips the review go through the API
#[test]
fn the_pull_request_decides_the_route_a_draft_refuses_and_another_base_refuses_only_counted_approvals() {
  let into = |base: &str, draft: bool| {
    let mut pull_request = pull_request("login", 7);
    pull_request.base_ref_name = base.to_string();
    pull_request.is_draft = draft;
    World {
      pull_requests: listed(&[("login", pull_request)]),
      ..world()
    }
  };
  let comment = into("master", false).decide(&land(SAFE_PUSH));
  assert_eq!((start_route(&comment).cloned(), comment.pull_request), (Some(StartRoute::Comment { number: 7 }), Some(7)));
  let mut chosen = land(SAFE_PUSH);
  chosen.profile = ProfileChoice::Id { id: "lsc".to_string() };
  assert_eq!(into("master", false).decide(&chosen).confirmations, vec![Confirmation::LandsMoreThanRow { count: 1 }]);
  let virtual_base = into("user/virtual/auth", false).decide(&land(SAFE_PUSH));
  assert_eq!(
    &virtual_base.effects[..1],
    [PlannedEffect::Retarget {
      branch: "login".to_string(),
      number: 7,
      from: "user/virtual/auth".to_string(),
      to: "master".to_string(),
    }]
  );
  assert_eq!(start_route(&virtual_base), Some(&StartRoute::Comment { number: 7 }));
  assert_eq!(
    (codes(&into("master", true).decide(&land(SAFE_PUSH))), codes(&into("master", true).decide(&land(DRY_RUN)))),
    (vec!["draft".to_string()], Vec::new())
  );

  let other = into("release/253", false);
  assert_eq!(
    other.decide(&land(SAFE_PUSH)).blockers,
    vec![Reason::OtherBase {
      number: 7,
      base: "release/253".to_string(),
      target: "master".to_string(),
    }]
  );
  let mut without = land(SAFE_PUSH);
  without.without_approvals = true;
  let skipping = World {
    last_profile: Some(PatronusRobotProfile {
      id: Some("lsc".to_string()),
      name: None,
      skips_review: Some(true),
    }),
    ..into("release/253", false)
  };
  for decision in [other.decide(&land(DRY_RUN)), other.decide(&without), skipping.decide(&land(SAFE_PUSH))] {
    assert_eq!(
      (
        codes(&decision),
        decision.pull_request,
        start_route(&decision).is_some_and(|route| matches!(route, StartRoute::Api { .. }))
      ),
      (Vec::new(), None, true)
    );
  }
}

/// Missing approvals refuse a Safe Push unless a start without them or a profile that skips the review skips them (D2),
/// or the start opens the pull request where they count; Patronus judging them only at the start is a confirmation
#[test]
fn missing_approvals_refuse_unless_skipped_and_approvals_judged_at_the_start_confirm() {
  let missing = World {
    readiness: Some(readiness(missing_approvals("Air", None))),
    ..world()
  };
  assert_eq!(
    missing.decide(&land(SAFE_PUSH)).blockers,
    vec![Reason::ApprovalsMissing { groups: vec!["Air".to_string()] }]
  );
  let mut without = land(SAFE_PUSH);
  without.without_approvals = true;
  let decision = missing.decide(&without);
  assert_eq!(
    (codes(&decision), decision.approvals_skipped_by, decision.confirmations.last().cloned()),
    (Vec::<String>::new(), Some(ApprovalsSkip::WithoutApprovals), Some(Confirmation::WithoutApprovals))
  );
  let mut opens = land(SAFE_PUSH);
  opens.create_pull_request = true;
  let decision = missing.decide(&opens);
  assert_eq!(
    (codes(&decision), decision.effects.last().cloned()),
    (Vec::<String>::new(), Some(PlannedEffect::CreatePullRequest))
  );
  let skipping = World {
    last_profile: Some(PatronusRobotProfile {
      id: Some("lsc".to_string()),
      name: None,
      skips_review: Some(true),
    }),
    ..missing
  };
  let decision = skipping.decide(&land(SAFE_PUSH));
  assert_eq!((codes(&decision), decision.approvals_skipped_by), (Vec::<String>::new(), Some(ApprovalsSkip::Profile)));
  let unknown = World {
    readiness: Some(readiness(ReviewReadiness::Unknown {
      reason: UnknownReadinessReason::ApprovalsNotEvaluated,
    })),
    ..world()
  };
  assert_eq!(unknown.decide(&land(SAFE_PUSH)).confirmations.last(), Some(&Confirmation::ApprovalsJudgedAtStart));
}

/// A Safe Push that pushes new commits into the approved pull request it starts from: rules that dismiss the approvals
/// Patronus counts refuse it, other dismissed approvals are a confirmation, rules GitHub didn't answer too, rules not
/// read yet a wait; no rules are asked for a Dry Run, a start without a push or the pull request it opens
#[test]
fn the_rules_of_a_push_into_an_approved_pull_request_refuse_confirm_or_wait() {
  let into = |rules: Option<Fact<BranchRules>>, readiness_answer: ReviewReadiness| World {
    pull_requests: listed(&[("login", approved(pull_request("login", 7)))]),
    rules,
    readiness: Some(readiness(readiness_answer)),
    ..world()
  };
  let dismiss = Some(fresh(BranchRules {
    dismiss_stale_reviews_on_push: true,
  }));
  let counted = ReviewReadiness::Approved { groups: Vec::new() };
  assert_eq!(
    into(dismiss.clone(), counted.clone()).decide(&land(SAFE_PUSH)).blockers,
    vec![Reason::DismissesApprovals {
      number: 7,
      target: "master".to_string(),
    }]
  );
  let confirmation = |world: World| world.decide(&land(SAFE_PUSH)).confirmations.last().cloned();
  assert_eq!(
    confirmation(into(dismiss, ReviewReadiness::NotRequired)),
    Some(Confirmation::DismissesApprovals { number: 7 })
  );
  assert_eq!(
    confirmation(into(Some(failed("no token")), counted.clone())),
    Some(Confirmation::RulesUnknown { number: 7 })
  );
  let loading = into(None, counted.clone()).decide(&land(SAFE_PUSH));
  assert_eq!(
    loading.waits,
    vec![FactWait {
      fact: LandFact::Rules { number: 7 },
      failed: None,
    }]
  );
  assert_eq!(into(None, counted.clone()).decide(&land(DRY_RUN)).waits, Vec::new());
  let mut on_remote = into(None, counted);
  on_remote.chain = vec![("auth", Some(ON_REMOTE), false, false), ("login", Some(ON_REMOTE), false, false)];
  assert_eq!(on_remote.decide(&land(SAFE_PUSH)).waits, Vec::new());
}

/// The default profile is the last robot's while the target offers it for the mode, with what the robot said of the
/// review; a chosen one is taken by id, `None` is the default checks; a profile is a confirmation. A profile that lists
/// its modes allows only those
#[test]
fn the_default_profile_is_the_last_robots_while_offered_and_a_profile_is_confirmed() {
  let last = PatronusRobotProfile {
    id: Some("lsc".to_string()),
    name: None,
    skips_review: Some(true),
  };
  let mut offered = options(true);
  assert_eq!(default_profile(&offered, Some(&last), SAFE_PUSH), Some(profile("lsc", Some(true))));
  assert_eq!(default_profile(&offered, None, SAFE_PUSH), None);
  offered.default_profile_id = Some("fast".to_string());
  assert_eq!(default_profile(&offered, None, SAFE_PUSH).map(|profile| profile.id), Some("fast".to_string()));
  offered.profiles[0].allowed_push_modes = vec![DRY_RUN];
  assert!(!allows_mode(&offered.profiles[0], SAFE_PUSH) && allows_mode(&offered.profiles[0], DRY_RUN));
  assert_eq!(default_profile(&offered, Some(&last), SAFE_PUSH).map(|profile| profile.id), Some("fast".to_string()));

  let world = World {
    last_profile: Some(last),
    ..world()
  };
  let profile_of = |choice: ProfileChoice| {
    let mut land = land(SAFE_PUSH);
    land.profile = choice;
    let decision = world.decide(&land);
    let route = start_route(&decision).cloned();
    (
      route,
      decision
        .confirmations
        .iter()
        .any(|confirmation| matches!(confirmation, Confirmation::NonDefaultProfile { .. })),
    )
  };
  let api = |id: Option<&str>| {
    Some(StartRoute::Api {
      profile: id.map(|id| if id == "lsc" { profile(id, Some(true)) } else { profile(id, Some(false)) }),
    })
  };
  assert_eq!(profile_of(ProfileChoice::Default), (api(Some("lsc")), true));
  assert_eq!(profile_of(ProfileChoice::Id { id: "fast".to_string() }), (api(Some("fast")), true));
  assert_eq!(profile_of(ProfileChoice::None), (api(None), false));
}

/// A start offers the profiles its mode allows, the last robot's described by what its answer said of the review; a
/// start without approvals also the emergency ones, and as its default only a profile known to skip the review (the
/// user chooses otherwise); an emergency profile is taken by id only there
#[test]
fn a_start_offers_the_profiles_of_its_mode_and_without_approvals_the_emergency_ones_and_a_skipping_default() {
  let last = PatronusRobotProfile {
    id: Some("lsc".to_string()),
    name: None,
    skips_review: Some(true),
  };
  let mut target = options(true);
  target.emergency_profiles = vec![profile("panic", None)];
  target.profiles.push(PatronusProfile {
    allowed_push_modes: vec![DRY_RUN],
    ..profile("check", None)
  });
  let ids = |profiles: &[PatronusProfile]| profiles.iter().map(|profile| profile.id.clone()).collect::<Vec<_>>();
  let with = offered_profiles(&target, Some(&last), SAFE_PUSH, false);
  assert_eq!(
    (ids(&with.offered), ids(&with.emergency), with.default.as_deref()),
    (names(&["lsc", "fast"]), Vec::new(), Some("lsc"))
  );
  assert_eq!(with.offered[0].skips_review, Some(true));
  let without = offered_profiles(&target, Some(&last), SAFE_PUSH, true);
  assert_eq!((ids(&without.emergency), without.default.as_deref()), (names(&["panic"]), Some("lsc")));
  // the last robot's profile checks the review: a start without approvals has no default, the usual start keeps it
  let checking = PatronusRobotProfile {
    skips_review: Some(false),
    ..last
  };
  assert_eq!(offered_profiles(&target, Some(&checking), SAFE_PUSH, true).default, None);
  assert_eq!(offered_profiles(&target, Some(&checking), SAFE_PUSH, false).default.as_deref(), Some("lsc"));
  assert_eq!(ids(&offered_profiles(&target, None, DRY_RUN, false).offered), names(&["lsc", "fast", "check"]));

  let world = World {
    profiles: fresh(target),
    last_profile: Some(checking),
    readiness: Some(readiness(missing_approvals("Air", None))),
    ..world()
  };
  let mut land = land(SAFE_PUSH);
  land.without_approvals = true;
  let api = |profile| Some(StartRoute::Api { profile });
  assert_eq!(start_route(&world.decide(&land)).cloned(), api(None));
  land.profile = ProfileChoice::Id { id: "panic".to_string() };
  assert_eq!(start_route(&world.decide(&land)).cloned(), api(Some(profile("panic", None))));
  // the usual start takes no emergency profile: the default checks
  land.without_approvals = false;
  assert_eq!(start_route(&world.decide(&land)).cloned(), api(None));
}

fn names(names: &[&str]) -> Vec<String> {
  names.iter().map(ToString::to_string).collect()
}

/// Of the confirmations, `bd` accepts by a flag only the cancel of the robot it replaces, a review Patronus couldn't
/// answer and rules GitHub didn't answer; the others it says
#[test]
fn bd_accepts_by_an_allowance_only_the_cancel_an_unknown_review_and_unknown_rules() {
  let confirmations = [
    Confirmation::LandsMoreThanRow { count: 1 },
    Confirmation::CancelsRobot { robot_id: "old".to_string() },
    Confirmation::ReviewUnknown { message: "down".to_string() },
    Confirmation::RulesUnknown { number: 7 },
    Confirmation::ApprovalsJudgedAtStart,
  ];
  assert_eq!(unaccepted(&confirmations, &LandingAllowances::default()), confirmations[1..4].iter().collect::<Vec<_>>());
  let allow = LandingAllowances {
    cancel_running: true,
    allow_unknown_review: true,
    allow_unknown_rules: true,
    ..LandingAllowances::default()
  };
  assert_eq!(unaccepted(&confirmations, &allow), [] as [&Confirmation; 0]);
}

/// The whole stack: a stack branch that is no build of its commits is said first and asks no review, then a push that
/// drops remote commits, then (of a built one) a robot that lands a part of it; its own robot on an older stack branch is replaced by a
/// Safe Push and holds the push of a Dry Run; its pull request into a virtual branch is another base, never retargeted
#[test]
fn the_whole_stack_says_its_build_first_and_its_own_robot_holds_or_is_replaced() {
  let world = world();
  let built = stack(StackBranchState::Built);
  let own = stack_robot("w", "auth", PatronusRobotStatus::Running, "0000");
  let member = running("m", "login");
  let robots_list = vec![own.clone(), member];
  let decide_stack =
    |stack: &StackBranchUpdate, remote: RemoteBranch<'static>, landing_member: Option<&RobotOf>, held_by: Option<&PatronusRobot>, mode, pull_requests: &PullRequestsView| {
      let robots = Robots::new(&robots_list);
      let facts = LandFacts {
        target: "master",
        prefix: "user",
        github: true,
        robots: &robots,
        subject: SubjectFacts::WholeStack(StackSubject {
          start: StackStart {
            stack,
            last_commit: None,
            landing_member,
          },
          parked: None,
          landed: None,
          members: vec![("auth", false), ("login", false)],
          remote: Some(remote),
          replaces: held_by.map(|robot| robot.id.as_str()),
          held_by,
        }),
        profiles: &world.profiles,
        last_profile: None,
        pull_requests,
        readiness: None,
        rules: None,
        landing: Some(&[]),
        target_conflict: None,
      };
      let mut land = land(mode);
      land.subject = Subject::whole_stack("auth");
      decide(&facts, &land)
    };
  let behind = RemoteBranch { drops: 1, ..UNPUSHED };
  let landing = robot_of("login", "m");
  let not_built = decide_stack(&stack(StackBranchState::Conflict), behind, Some(&landing), None, SAFE_PUSH, &listed(&[]));
  assert_eq!(
    (codes(&not_built), not_built.waits),
    (vec!["stackNotBuilt".to_string(), "dropsRemote".to_string()], Vec::new())
  );
  let landing_member = decide_stack(&built, behind, Some(&landing), None, SAFE_PUSH, &listed(&[]));
  assert_eq!(codes(&landing_member), ["dropsRemote", "ancestor"]);
  let replaced = decide_stack(&built, UNPUSHED, None, Some(&own), SAFE_PUSH, &listed(&[]));
  assert_eq!(
    (replaced.lands, replaced.effects[..2].to_vec()),
    (
      vec!["auth".to_string(), "login".to_string()],
      vec![
        PlannedEffect::CancelRobot { robot_id: "w".to_string() },
        PlannedEffect::Push {
          branches: vec!["auth".to_string()],
          force: false
        }
      ]
    )
  );
  assert_eq!(codes(&decide_stack(&built, UNPUSHED, None, Some(&own), DRY_RUN, &listed(&[]))), ["stackHeld"]);
  let mut pull_requests = listed(&[]);
  let mut stack_pull_request = pull_request("auth", 12);
  stack_pull_request.base_ref_name = "user/virtual/auth".to_string();
  pull_requests.stack_pull_requests.insert("auth".to_string(), stack_pull_request);
  let other = decide_stack(&built, ON_REMOTE, None, None, SAFE_PUSH, &pull_requests);
  assert_eq!(
    (codes(&other), other.effects.iter().any(|effect| matches!(effect, PlannedEffect::Retarget { .. }))),
    (vec!["otherBase".to_string()], false)
  );
}
