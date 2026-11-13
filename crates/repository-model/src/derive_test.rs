use crate::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use crate::derive::derive_budget_test::{answer_patronus, at, synced_facts};
use crate::derive::{LandSources, attention_of, blocking_parent, decide_lands, land_decision, open_pull_requests_view, repository};
use crate::facts::{FactResult, FactStore, RobotsRead};
use crate::patronus_view::patronus_view_test::chain;
use crate::state::View;
use crate::table::TableState;
use crate::table::table_test::{branch_status, grouped, remote_status, run_sync, synced};
use domain_policy::parked::parked_state;
use domain_types::actions::{ActionBlocker, ActionId, ActionState, Availability};
use domain_types::error::ApiError;
use domain_types::fixtures::branches_grouped;
use domain_types::fixtures::{pull_request, robot, robot_of, stack_held, stack_landed_by_own, stack_robot, whole_stack_robot_of, whole_stack_view};
use domain_types::landed::LandedBranch;
use domain_types::landing::{Confirmation, FactWait, Land, LandFact, LandProfiles, PlannedEffect, ProfileChoice, StartRoute, Subject};
use domain_types::landing_route::{LandingRoute, RouteGate};
use domain_types::next_step::{LandingNodeKind, StepKind, StepRow};
use domain_types::patronus::{
  LandingAllowances, MissingApprovalsReason, MissingReviewGroup, PatronusAvailability, PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotStatus, PatronusRobotsResult,
  PatronusStartOptions, Person, ReviewReadiness, ReviewReadinessBranch,
};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestBase, PullRequestState, PullRequestView, ReviewCandidate};
use domain_types::reason::Reason;
use domain_types::repository_view::{BranchLanding, BranchPatronus, HoldKind, HoldView, PatronusView, RobotOf, StackLanding, StackPatronus, WholeStackRest};
use domain_types::session::{
  BranchPush, BranchSection, BranchStatus, BranchStructure, ChildrenPush, Fact, FactState, LandedSection, Parked, ParkedMember, PullRequestsView, PushUpTo, StackBranchAlong,
  SubjectReadiness, TableHeader, WholeStackView,
};
use domain_types::stack::{BranchStack, StackBranchState, StackBranchUpdate};
use domain_types::{BranchSyncStatus, CommitSyncStatus, GroupedBranchInfo, PushAction, RemoteStatusUpdate, SyncEvent};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn on(name: &str, parent: Option<&str>) -> GroupedBranchInfo {
  on_with(name, parent, &[&format!("{name}-1")])
}

/// `name` built on its declared parent `parent`, its commits `commits`, newest first
fn on_with(name: &str, parent: Option<&str>, commits: &[&str]) -> GroupedBranchInfo {
  GroupedBranchInfo {
    stack: Some(BranchStack {
      declared_parent: parent.map(str::to_string),
      effective_base: parent.map(str::to_string),
      ..BranchStack::default()
    }),
    ..grouped(name, commits)
  }
}

fn grouping(branches: Vec<GroupedBranchInfo>, parked: &[(&str, &str)], stack_branches: &[&str]) -> SyncEvent {
  let mut grouping = branches_grouped(branches);
  if let SyncEvent::BranchesGrouped {
    parked: lines,
    stack_branches: roots,
    ..
  } = &mut grouping
  {
    *lines = parked.iter().map(|(name, note)| ((*name).to_string(), (*note).to_string())).collect();
    *roots = stack_branches.iter().map(ToString::to_string).collect();
  }
  grouping
}

fn stack_branch(root: &str, members: &[&str]) -> SyncEvent {
  SyncEvent::StackBranchUpdate(Box::new(StackBranchUpdate {
    root: root.to_string(),
    branch_name: format!("test/stack/{root}"),
    members: members.iter().map(ToString::to_string).collect(),
    commit_count: 3,
    my_email: None,
    state: StackBranchState::Built,
    tip: Some("stack-tip".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  }))
}

fn names(names: &[&str]) -> Vec<String> {
  names.iter().map(ToString::to_string).collect()
}

/// oauth on login on auth, session on auth, docs alone, newest first
fn tree() -> Vec<GroupedBranchInfo> {
  vec![
    on("oauth", Some("login")),
    on("docs", None),
    on("login", Some("auth")),
    on("session", Some("auth")),
    on("auth", None),
  ]
}

/// The view decides where each branch is: its parent, children, ancestors, descendants and main line, whether it is
/// parked and its section, so the page splits the order into the rows of each section and lays them out without a rule
/// of its own; the patch sends the order once, never a second list of the same names
#[test]
fn a_grouping_places_every_branch_with_its_section() {
  let mut table = TableState::with_prefix("user");
  table.begin_sync();
  table.apply(grouping(tree(), &[("login", "waits for IJPL-1")], &[]));
  let structure = |name: &str| table.branch(name).map(|branch| branch.structure.clone()).unwrap();
  assert_eq!(
    structure("auth"),
    BranchStructure {
      parent: None,
      children: names(&["login", "session"]),
      ancestors: Vec::new(),
      descendants: names(&["login", "session", "oauth"]),
      main_line: names(&["login", "oauth"]),
      parked: None,
      stack_parked: None,
      needs: Vec::new(),
      // the stack forks, and no stack branch of it is known yet
      whole_stack: None,
      section: BranchSection::Main,
    }
  );
  let oauth = structure("oauth");
  assert_eq!((oauth.parent.as_deref(), oauth.ancestors), (Some("login"), names(&["auth", "login"])));
  assert_eq!(
    (oauth.parked, oauth.section),
    (
      Some(Parked {
        note: "waits for IJPL-1".to_string(),
        with: Some("login".to_string()),
      }),
      BranchSection::Parked
    )
  );
  let section = |name: &str| structure(name).section;
  let (parked, main): (Vec<&str>, Vec<&str>) = table.order().iter().map(String::as_str).partition(|name| section(name) == BranchSection::Parked);
  assert_eq!((main, parked), (vec!["docs", "session", "auth"], vec!["oauth", "login"]));
  let mut view = View::new(1);
  let patch = view.diff(&table, &FactStore::new()).unwrap();
  assert_eq!(patch.changes.order.as_deref(), Some(table.order()));
  assert_eq!(patch.changes.branches["auth"].structure, Some(structure("auth")));
  assert_eq!(
    patch.changes.branches["oauth"].structure.as_ref().map(|structure| structure.section),
    Some(BranchSection::Parked)
  );
  // `bd` reads the same rule over the same input
  assert_eq!(parked_state(&repository(&table), "oauth"), structure("oauth").parked);
}

/// A grouping that changes nothing of the structure sends none; one that parks a branch sends the structures that
/// changed, the section of the parked one with it; the stack branch of a root reports its parked member into the root's
/// structure
#[test]
fn a_patch_carries_only_the_structures_that_changed() {
  let mut table = TableState::with_prefix("user");
  table.begin_sync();
  table.apply(grouping(tree(), &[], &["auth"]));
  let mut view = View::new(1);
  let _ = view.diff(&table, &FactStore::new());
  table.apply(grouping(tree(), &[], &["auth"]));
  let again = view.diff(&table, &FactStore::new());
  assert!(again.is_none_or(|patch| patch.changes.branches.values().all(|change| change.structure.is_none())));

  table.apply(grouping(tree(), &[("session", "")], &["auth"]));
  let patch = view.diff(&table, &FactStore::new()).unwrap();
  let changed: Vec<&String> = patch
    .changes
    .branches
    .iter()
    .filter(|(_, change)| change.structure.is_some())
    .map(|(name, _)| name)
    .collect();
  assert_eq!(changed, ["session"]);
  assert_eq!(
    patch.changes.branches["session"].structure.as_ref().map(|structure| structure.section),
    Some(BranchSection::Parked)
  );

  table.apply(stack_branch("auth", &["auth", "login", "session", "oauth"]));
  let patch = view.diff(&table, &FactStore::new()).unwrap();
  assert_eq!(
    patch.changes.branches["auth"].structure.as_ref().and_then(|structure| structure.stack_parked.clone()),
    Some(ParkedMember {
      member: "session".to_string(),
      parked: Parked { note: String::new(), with: None },
    })
  );
}

/// The table of `tree`, the stack branch of auth reported
fn tree_with_its_stack_branch() -> TableState {
  let mut table = TableState::with_prefix("user");
  table.begin_sync();
  table.apply(grouping(tree(), &[], &["auth"]));
  table.apply(stack_branch("auth", &["auth", "session", "login", "oauth"]));
  table
}

/// The root of a stack carries its whole stack: its stack branch once the sync reported it; the last branch of a
/// straight chain, a parked one at its end left local
#[test]
fn the_root_of_a_stack_carries_its_whole_stack() {
  let table = tree_with_its_stack_branch();
  let whole_stack = |table: &TableState, name: &str| table.branch(name).and_then(|branch| branch.structure.whole_stack.clone());
  assert_eq!(whole_stack(&table, "auth"), Some(WholeStackView::Forking { not_built: None }));
  assert_eq!((whole_stack(&table, "login"), whole_stack(&table, "docs")), (None, None));

  let chain = vec![on("oauth", Some("login")), on("login", Some("auth")), on("auth", None)];
  let mut table = TableState::with_prefix("user");
  table.begin_sync();
  table.apply(grouping(chain.clone(), &[], &[]));
  assert_eq!(whole_stack(&table, "auth"), Some(WholeStackView::Chain { leaf: "oauth".to_string() }));
  table.apply(grouping(chain, &[("login", "")], &[]));
  assert_eq!(whole_stack(&table, "auth"), None);
}

/// The view sends what the pushes of each branch send, as the rules order them over its table: Push Stack of a stacked
/// branch takes the ancestors with work along, Push with Children of the root the children with work; a branch whose
/// pushes send it alone is not listed
#[test]
fn the_view_orders_the_pushes_of_each_branch() {
  let mut table = TableState::with_prefix("user");
  table.begin_sync();
  table.apply(grouping(tree(), &[], &["auth"]));
  for (branch, unpushed) in [("auth", &["a"][..]), ("login", &["l"]), ("oauth", &["o"]), ("session", &[]), ("docs", &["d"])] {
    table.apply(SyncEvent::RemoteStatusUpdate(remote_status(branch, unpushed)));
  }
  let mut view = View::new(1);
  // a running sync sends none: its remote statuses come one by one
  assert_eq!(view.diff(&table, &FactStore::new()).map(|patch| patch.changes.pushes.len()), Some(0));
  table.finish_sync(Ok(()));
  let patch = view.diff(&table, &FactStore::new()).unwrap();
  let pushes = patch.changes.pushes;
  assert_eq!(
    pushes["oauth"],
    BranchPush {
      button: true,
      ..BranchPush::default()
    }
  );
  assert_eq!(
    pushes["auth"].children,
    Some(ChildrenPush {
      // session has nothing to push
      order: Some(names(&["auth", "login", "oauth"])),
      offers: true,
      ..ChildrenPush::default()
    })
  );
  assert!(!pushes.contains_key("docs"), "{pushes:?}");

  // the stack branch of the whole stack goes along once it is on the remote
  let SyncEvent::StackBranchUpdate(mut update) = stack_branch("auth", &["auth", "session", "login", "oauth"]) else {
    unreachable!()
  };
  update.remote = Some(remote_status("test/stack/auth", &["s"]));
  table.begin_sync();
  table.apply(SyncEvent::StackBranchUpdate(update));
  table.finish_sync(Ok(()));
  let patch = view.diff(&table, &FactStore::new()).unwrap();
  assert_eq!(
    patch.changes.pushes["auth"].children.as_ref().and_then(|children| children.stack_branch.clone()),
    Some(StackBranchAlong::Along)
  );
}

/// Push Up to Here of the commits a branch has for the remote is sent only where it differs from the common answer
/// (every commit the remote lacks, after the branches of Push Stack, nothing stopping it): a branch whose remote has an
/// older version of it pushes whole, and one stacked on an ancestor whose push drops remote commits stops at it
#[test]
fn the_view_sends_push_up_to_here_only_where_it_differs_from_the_common_answer() {
  let mut table = TableState::with_prefix("user");
  table.begin_sync();
  table.apply(grouping(tree(), &[], &[]));
  let rewritten = RemoteStatusUpdate {
    remote_exists: true,
    fast_forward: false,
    commits_behind: 1,
    ..remote_status("login", &["l"])
  };
  let rewritten = RemoteStatusUpdate {
    action: rewritten.push_action(),
    ..rewritten
  };
  table.apply(SyncEvent::RemoteStatusUpdate(rewritten));
  for (branch, unpushed) in [("auth", &["a"][..]), ("oauth", &["o"]), ("session", &["s"])] {
    table.apply(SyncEvent::RemoteStatusUpdate(remote_status(branch, unpushed)));
  }
  table.finish_sync(Ok(()));
  let mut view = View::new(1);
  let pushes = view.diff(&table, &FactStore::new()).unwrap().changes.pushes;
  let up_to = |name: &str| pushes.get(name).and_then(|push| push.up_to.clone());
  assert_eq!(up_to("session"), None, "the common answer");
  assert_eq!(
    up_to("login"),
    Some(PushUpTo {
      blocker: Some(Reason::RewrittenSincePush { branch: "login".to_string() }),
      ancestors: Some(Vec::new()),
      ..PushUpTo::default()
    })
  );
  assert_eq!(
    up_to("oauth"),
    Some(PushUpTo {
      blocker: Some(Reason::DropsRemote {
        branch: "login".to_string(),
        behind: 1
      }),
      ancestors: Some(Vec::new()),
      ..PushUpTo::default()
    })
  );
}

/// The open pull request of each subject as the rules read it: auth's into the target with the code owner Request
/// Review… offers, login's into the virtual branch of its parent (retargeted), session's draft into a release branch;
/// docs' closed one and one of a branch the table lacks are not listed; the whole stack of auth under `stack:auth`
#[test]
fn the_view_reads_where_each_open_pull_request_goes_and_whom_request_review_offers() {
  let table = tree_with_its_stack_branch();
  let into = |name: &str, number: u32, base: &str| PullRequest {
    base_ref_name: base.to_string(),
    ..pull_request(name, number)
  };
  let pull_requests = PullRequestsView {
    pull_requests: BTreeMap::from([
      ("auth".to_string(), into("auth", 7, "master")),
      ("login".to_string(), into("login", 8, "user/virtual/auth")),
      (
        "session".to_string(),
        PullRequest {
          is_draft: true,
          ..into("session", 9, "release-252")
        },
      ),
      (
        "docs".to_string(),
        PullRequest {
          state: PullRequestState::Closed,
          ..into("docs", 10, "master")
        },
      ),
      ("gone".to_string(), into("gone", 11, "master")),
    ]),
    stack_pull_requests: BTreeMap::from([("auth".to_string(), into("auth", 12, "master"))]),
    ..PullRequestsView::default()
  };
  let alice = Person {
    login: Some("alice".to_string()),
    full_name: Some("Alice".to_string()),
  };
  let missing = ReviewReadiness::Missing {
    groups: vec![MissingReviewGroup {
      name: "UI".to_string(),
      needed: Some(1),
      approved_by: Vec::new(),
      requested: Vec::new(),
      suggested: vec![alice],
      changes_requested_by: Vec::new(),
    }],
    not_counted: Vec::new(),
    reason: MissingApprovalsReason::ApprovalsMissing,
  };
  let readiness = |key: &str| (key == "auth").then_some(&missing);
  let view = |number: u32, base: PullRequestBase, draft: bool, candidates: Vec<ReviewCandidate>| PullRequestView { number, base, draft, candidates };
  let alice = ReviewCandidate {
    login: "alice".to_string(),
    full_name: Some("Alice".to_string()),
    groups: vec!["UI".to_string()],
    preselected: true,
  };
  let retarget = PullRequestBase::Virtual {
    from: "auth".to_string(),
    retarget: "master".to_string(),
  };
  assert_eq!(
    open_pull_requests_view(
      "master",
      |name| table.parts.branches.contains_key(name),
      |root| table.parts.stacks.contains_key(root),
      &pull_requests,
      readiness,
      "user",
      |_| None
    ),
    BTreeMap::from([
      ("auth".to_string(), view(7, PullRequestBase::Target, false, vec![alice])),
      ("login".to_string(), view(8, retarget, false, Vec::new())),
      ("session".to_string(), view(9, PullRequestBase::Other { base: "release-252".to_string() }, true, Vec::new())),
      ("stack:auth".to_string(), view(12, PullRequestBase::Target, false, Vec::new())),
    ])
  );
}

fn fresh<T>(value: T) -> Fact<T> {
  Fact {
    value: Some(value),
    state: FactState::Fresh,
    ..Fact::default()
  }
}

/// The view decides the default starts of every branch over the facts of the session (`LandSources`): a Safe Push of
/// oauth lands the chain it is built on, retargets the pull request of login out of the virtual branch of auth (D11),
/// pushes the chain root first and starts through the API with the default profile of the target; login starts from its
/// retargeted pull request; landing needs not computed yet are a wait, a failed computation none. A read of another
/// intent answers its decision with the profiles it offers
#[test]
fn the_view_decides_the_default_starts_of_each_branch_and_a_read_answers_another_intent() {
  let table = chain();
  let unpushed = RemoteFact {
    remote_tip: None,
    up_to_date: false,
    known: true,
    drops: 0,
    others: 0,
  };
  let remote = RemoteFacts {
    branches: ["auth", "login", "oauth"].map(|name| (name.to_string(), unpushed.clone())).into(),
    stacks: BTreeMap::new(),
  };
  let fast = PatronusProfile {
    id: "fast".to_string(),
    name: "Fast".to_string(),
    description: None,
    allowed_push_modes: Vec::new(),
    skips_review: None,
  };
  let profiles = fresh(PatronusStartOptions {
    target_branch: "master".to_string(),
    supported: true,
    profiles: vec![fast.clone()],
    emergency_profiles: Vec::new(),
    default_profile_id: Some("fast".to_string()),
  });
  let pull_requests = PullRequestsView {
    head: fresh(PullRequestAvailability::Available {
      owner: "acme".to_string(),
      name: "monorepo".to_string(),
    }),
    pull_requests: BTreeMap::from([(
      "login".to_string(),
      PullRequest {
        base_ref_name: "test/virtual/auth".to_string(),
        ..pull_request("login", 8)
      },
    )]),
    ..PullRequestsView::default()
  };
  let ready = fresh(SubjectReadiness {
    ask: ReviewReadinessBranch::starting_from("oauth".to_string(), None, "vo1".to_string(), None),
    requirement: None,
    readiness: ReviewReadiness::NotRequired,
  });
  let readiness = |_: &str| Some(&ready);
  let failed = FactState::Unavailable {
    reason: ApiError::internal("the trial session failed"),
  };
  let route = LandingRoute::Patronus { gate: RouteGate::Open };
  let sources = LandSources {
    patronus: true,
    route: &route,
    pushed: None,
    header: &TableHeader::default(),
    table: &table,
    remote: &remote,
    view: &PatronusView::default(),
    robots: &[],
    target: "master",
    prefix: "test",
    github: true,
    profiles: &profiles,
    pull_requests: &pull_requests,
    readiness: &readiness,
    rules: None,
    landing_needs: &failed,
    baseline: &LandedSection::default(),
    gate: None,
    declared: &BTreeMap::new(),
    target_conflict: None,
    github_layers: &BTreeMap::new(),
  };
  let lands = decide_lands(&sources);
  assert_eq!(lands.keys().collect::<Vec<_>>(), ["auth", "login", "oauth"]);
  let retarget = PlannedEffect::Retarget {
    branch: "login".to_string(),
    number: 8,
    from: "test/virtual/auth".to_string(),
    to: "master".to_string(),
  };
  let push = |branches: &[&str]| PlannedEffect::Push {
    branches: names(branches),
    force: false,
  };
  let oauth = &lands["oauth"].safe_push;
  assert_eq!((&oauth.blockers, &oauth.waits), (&Vec::new(), &Vec::new()));
  assert_eq!(
    oauth.confirmations,
    vec![Confirmation::LandsMoreThanRow { count: 2 }, Confirmation::NonDefaultProfile { id: "fast".to_string() }]
  );
  assert_eq!(
    oauth.effects,
    vec![
      retarget.clone(),
      push(&["auth", "login", "oauth"]),
      PlannedEffect::StartRobot {
        route: StartRoute::Api { profile: Some(fast.clone()) }
      }
    ]
  );
  assert_eq!(oauth.lands, names(&["auth", "login", "oauth"]));
  let login = &lands["login"].safe_push;
  assert_eq!(
    login.effects,
    vec![
      retarget,
      push(&["auth", "login"]),
      PlannedEffect::StartRobot {
        route: StartRoute::Comment { number: 8 }
      }
    ]
  );
  assert_eq!(lands["oauth"].dry_run.lands, names(&["auth", "login", "oauth"]));

  // landing needs being computed: the start waits for them
  let loading = LandSources {
    patronus: true,
    landing_needs: &FactState::Loading,
    ..sources
  };
  assert_eq!(
    decide_lands(&loading)["oauth"].safe_push.waits,
    vec![FactWait {
      fact: LandFact::LandingNeeds,
      failed: None,
    }]
  );

  // a start without approvals: the default checks, as no profile is known to skip the review
  let land = Land {
    subject: Subject::branch("oauth"),
    mode: PatronusPushMode::RebaseAutosquash,
    profile: ProfileChoice::Default,
    without_approvals: true,
    create_pull_request: false,
    allow: LandingAllowances::default(),
  };
  let (decision, profiles) = land_decision(&sources, &land).unwrap();
  assert_eq!(decision.start_route(), Some(&StartRoute::Api { profile: None }));
  assert_eq!(
    profiles,
    LandProfiles {
      offered: vec![fast],
      emergency: Vec::new(),
      default: None,
    }
  );
  // a subject the table lacks has no answer
  let gone = Land {
    subject: Subject::branch("gone"),
    ..land
  };
  assert_eq!(land_decision(&sources, &gone), None);
}

/// What the session's view answers of a start of login on auth on base, auth landed, base with a new commit, and someone
/// pushed a commit onto auth's remote branch: the branches the decision of login's Safe Push pushes, the push of its row
/// (what the page sends for an action and a start: `action`, else `stack`, then the branch, `pushOrdersIn`), the
/// blockers of the decision and the gate of the app's start (`pushForAction`: `BranchPush::action_blocker`, why the
/// push of an action can't be made, of the table as it is). `by_robot`: a merged Safe Push landed auth whole, else the fetched baseline has every
/// commit of it and its remote is an older version; `reported`: the landing section of the table state has the landing
/// report that says so (`BaselineStatus`, which a running sync sends at any point of its events); `in_baseline`: the
/// table of the last sync has it (`BranchTable::in_baseline`)
fn landed_ancestor_answers(by_robot: bool, reported: bool, in_baseline: bool) -> (Option<Vec<String>>, Vec<String>, Vec<Reason>, Option<Reason>) {
  use crate::derive::decide_landing;
  use crate::facts::session_remote;
  use domain_types::repository_view::LandedByView;
  use std::collections::HashMap;

  let branches = vec![on("login", Some("auth")), on("auth", Some("base")), on("base", None)];
  let mut auth = remote_status("auth", &[]);
  // a colleague pushed a commit onto auth's remote branch: a push of auth would drop it
  auth.commits_behind = 1;
  if !by_robot {
    // the remote branch has the commit of auth that the baseline has now: an older version
    auth.commits_landed = 1;
  }
  auth.action = auth.push_action();
  let mut events = vec![grouping(branches.clone(), &[], &[])];
  // a repository Patronus knows: its start is the robot's also before Patronus answers
  if let SyncEvent::BranchesGrouped { landing, .. } = &mut events[0] {
    landing.patronus = true;
  }
  events.extend(
    [remote_status("base", &["base-2"]), auth, remote_status("login", &["login-1"])]
      .into_iter()
      .map(SyncEvent::RemoteStatusUpdate),
  );
  let whole = |landed: bool| {
    let auth = LandedBranch {
      name: Some("auth".to_string()),
      commits: Vec::new(),
      remaining: 0,
      landed_at: 0,
    };
    if landed { vec![auth] } else { Vec::new() }
  };
  events.push(SyncEvent::BaselineStatus {
    baseline_branch: "origin/master".to_string(),
    update: None,
    check_error: None,
    landing: whole(reported),
    landed: Vec::new(),
  });
  let mut state = TableState::with_prefix("user");
  run_sync(&mut state, events);

  // the table of the sync with its landing report (`BranchTable::from_report`)
  let table = BranchTable {
    prefix: "user".to_string(),
    baseline: "origin/master".to_string(),
    tips: branches
      .iter()
      .map(|branch| (format!("user/virtual/{}", branch.name), format!("{}-tip", branch.name)))
      .collect(),
    branches,
    in_baseline: crate::branch_table::in_baseline(&whole(in_baseline)).map(str::to_string).collect(),
    ..BranchTable::default()
  };
  let mut facts = synced_facts(&table);
  let mut robots = Vec::new();
  let mut view = PatronusView::default();
  if by_robot {
    let merged = robot("merged", Some("auth"), PatronusRobotStatus::Success, 10);
    robots.push(merged.clone());
    facts.apply(
      at(2),
      FactResult::Robots {
        read: RobotsRead::List { full: true },
        result: PatronusRobotsResult {
          availability: PatronusAvailability::Available,
          robots: vec![merged],
          github_repository: None,
        },
      },
    );
    view.branches.insert(
      "auth".to_string(),
      BranchPatronus {
        robot_id: Some("merged".to_string()),
        landed_by: Some(LandedByView {
          by: robot_of("auth", "merged"),
          own: true,
          landed: names(&["auth-1"]),
          after: Vec::new(),
        }),
        ..BranchPatronus::default()
      },
    );
  }
  // what the rules decide over these facts, as the session's derivation writes it into the view
  decide_landing(&mut view, &table, &session_remote(&table, &state), &robots, &HashMap::new(), &BTreeMap::new(), &[]);
  answer_patronus(&mut facts, 3, view, &table);

  let patch = View::new(1).diff(&state, &facts).unwrap();
  let decision = &patch.changes.lands["login"].safe_push;
  let decided = decision.effects.iter().find_map(|effect| match effect {
    PlannedEffect::Push { branches, .. } => Some(branches.clone()),
    _ => None,
  });
  let push = patch.changes.pushes.get("login").cloned().unwrap_or_default();
  let mut row = push.action.clone().or_else(|| push.stack.clone()).unwrap_or_default();
  row.push("login".to_string());
  (decided, row, decision.blockers.clone(), push.action_blocker)
}

/// The push of a start is the push of its row (`push::start_push` over the holds the row reads): whether a merged Safe
/// Push landed auth whole or the fetched baseline has every commit of it (the landing report of the table, its remote an
/// older version), the decision of the Safe Push of login pushes base and login, as Push Stack of the row does, and
/// leaves auth as it is; the gate of the start says nothing against it: auth's remote drops nothing of what lands
#[test]
fn the_push_of_a_start_is_the_push_of_its_row_past_an_ancestor_that_landed() {
  for by_robot in [true, false] {
    assert_eq!(
      landed_ancestor_answers(by_robot, !by_robot, !by_robot),
      (Some(names(&["base", "login"])), names(&["base", "login"]), Vec::new(), None),
      "landed by a robot: {by_robot}"
    );
  }
}

/// The view's pushes read the landing report of the table of the last sync (`BranchTable::in_baseline`), as the decision
/// of a start and its gate do, never the landing section of the table state, which a running sync moves at any point of
/// its events (`BaselineStatus`): while the table lacks the report, the row leaves auth to its own row, whose push asks
/// first, and the decision and the gate refuse its dropped remote commit; once the table has it, the three pass auth
/// alike, with or without the report in the section
#[test]
fn the_pushes_of_the_view_read_the_landing_report_of_the_table_as_the_decision_of_a_start_and_its_gate() {
  let drops_auth = Reason::DropsRemote {
    branch: "auth".to_string(),
    behind: 1,
  };
  let not_in_table = (Some(names(&["base", "auth", "login"])), names(&["login"]), vec![drops_auth.clone()], Some(drops_auth));
  let in_table = (Some(names(&["base", "login"])), names(&["base", "login"]), Vec::new(), None);
  for reported in [true, false] {
    assert_eq!(
      [false, true].map(|in_baseline| landed_ancestor_answers(false, reported, in_baseline)),
      [not_in_table.clone(), in_table.clone()],
      "the report in the section of the table state: {reported}"
    );
  }
}

/// A session has a landing in flight only while an operation holds the exact tracking ref of its target
#[test]
fn the_landing_route_sees_only_an_operation_holding_its_target_ref() {
  use super::landing_in_flight;
  use domain_types::landing_route::LandingRemote;
  use domain_types::operation::{Intent, OperationId, OperationStep, Origin, Resource, RunningOperation};
  let remote = LandingRemote {
    remote: Some("origin".to_owned()),
    target: Some("master".to_owned()),
    ..LandingRemote::default()
  };
  let operation = |reference: &str| RunningOperation {
    id: OperationId("fetch".to_owned()),
    intent: Intent::FetchBaseline,
    resources: vec![Resource::Ref { name: reference.to_owned() }],
    step: OperationStep::Fetching,
    origin: Origin::ThisProcess,
    moves: Vec::new(),
    joined: 0,
  };
  assert!(landing_in_flight(&remote, &[operation("refs/remotes/origin/master")]));
  assert!(!landing_in_flight(&remote, &[operation("refs/remotes/upstream/master")]));
  assert!(!landing_in_flight(&remote, &[operation("refs/remotes/origin/main")]));
}

/// The view carries the next step and the node of every row and of every whole stack that forks
/// (`domain_policy::next_step`, `domain_policy::landing_node`), over the table and the facts of the session; a patch
/// carries only the subjects whose step or node changed
#[test]
fn the_view_carries_the_next_step_and_the_node_of_each_subject_and_a_patch_only_those_that_changed() {
  let (state, mut facts) = crate::derive::derive_budget_test::session();
  let mut view = View::new(1);
  let first = view.diff(&state, &facts).unwrap();
  assert_eq!(first.changes.next_steps.len(), 141 + 3);
  let running = &first.changes.next_steps["lone001"];
  assert_eq!((running.step.row, running.node.kind), (StepRow::Eleven, LandingNodeKind::Ready), "approved: ready to land");
  let stack = &first.changes.next_steps["stack:stack0"];
  assert!(
    matches!(stack.step.kind, StepKind::SafePush | StepKind::CreatePullRequest { .. }),
    "the whole stack takes the rows of its stack branch: {:?}",
    stack.step
  );
  // the robot of a branch finishes merged: only its step changes
  let branch = finish_merged(&mut facts);
  let second = view.diff(&state, &facts).unwrap();
  assert_eq!(second.changes.next_steps.keys().collect::<Vec<_>>(), [&branch]);
  assert_eq!(second.changes.next_steps[&branch].node.kind, LandingNodeKind::InMaster, "merged by its robot");
}

/// The robot `r0` of the large table finishes merged, as the poll of the session answers it; its branch
fn finish_merged(facts: &mut FactStore) -> String {
  let robot = facts.robots.view().robots["r0"].clone();
  let branch = robot.branch_name.clone().unwrap();
  facts.apply(
    at(9),
    FactResult::Robots {
      read: RobotsRead::Poll,
      result: PatronusRobotsResult {
        availability: PatronusAvailability::Available,
        robots: vec![PatronusRobot {
          status: PatronusRobotStatus::Success,
          ..robot
        }],
        github_repository: None,
      },
    },
  );
  branch
}

/// The view offers the actions of each branch, whole stack and robot, each enabled or disabled with the reason, and a
/// patch carries only those of the subjects whose actions changed
#[test]
fn the_view_offers_the_actions_of_each_subject_and_a_patch_only_those_that_changed() {
  let (state, mut facts) = crate::derive::derive_budget_test::session();
  let mut view = View::new(1);
  let first = view.diff(&state, &facts).unwrap();
  let actions = &first.changes.actions;
  let availability = |key: &str, id: ActionId| actions[key].iter().find(|action| action.id == id).map(|action| action.availability.clone());
  assert_eq!(availability("lone001", ActionId::SafePush), Some(Availability::Enabled), "approved: ready to land");
  assert_eq!(availability("lone001", ActionId::OpenPullRequest), Some(Availability::Enabled));
  assert!(availability("stack:stack0", ActionId::SafePushWholeStack).is_some(), "the whole stack starts");
  assert_eq!(availability("robot:r0", ActionId::OpenRobot), Some(Availability::Enabled));
  assert_eq!(availability("robot:r0", ActionId::CancelRobot), Some(Availability::Enabled), "it runs");
  // the robot of a branch finishes merged: only the actions of the robot and of its branch change
  let branch = finish_merged(&mut facts);
  let second = view.diff(&state, &facts).unwrap();
  let changed: Vec<&String> = second.changes.actions.keys().collect();
  assert!(changed.contains(&&"robot:r0".to_string()), "{changed:?}");
  assert!(changed.iter().all(|key| **key == branch || *key == "robot:r0"), "{changed:?}");
  assert!(
    !second.changes.actions["robot:r0"].iter().any(|action| action.id == ActionId::CancelRobot),
    "merged: nothing to cancel"
  );
}

/// The cached frame of an earlier run (`TableState::cached`) shows its rows with the steps the facts decide, and the
/// actions they offer: every remote action waits for the first sync (`ActionBlocker::Cached`), a rewrite does not; the
/// sync that commits sends that the table is cached no more and gives the remote actions back
#[test]
fn a_cached_frame_shows_its_steps_and_holds_its_remote_actions_back_until_the_first_sync_commits() {
  let (mut state, facts) = crate::derive::derive_budget_test::session();
  let checked = View::new(1).diff(&state, &facts).unwrap();
  state.cached = true;
  let mut view = View::new(1);
  let first = view.diff(&state, &facts).unwrap();
  assert_eq!((first.changes.cached, view.snapshot().cached), (Some(true), true));
  assert_eq!(
    first.changes.next_steps, checked.changes.next_steps,
    "the steps and the nodes show as the facts decide them"
  );
  let availability =
    |actions: &BTreeMap<String, Vec<ActionState>>, key: &str, id: ActionId| actions[key].iter().find(|action| action.id == id).map(|action| action.availability.clone());
  let cached = Some(Availability::Disabled { reason: ActionBlocker::Cached });
  let actions = &first.changes.actions;
  for (key, id) in [
    ("lone001", ActionId::SafePush),
    ("lone001", ActionId::DryRun),
    ("lone001", ActionId::Push),
    ("stack:stack0", ActionId::SafePushWholeStack),
    ("robot:r0", ActionId::CancelRobot),
  ] {
    assert_eq!(availability(actions, key, id), cached, "{key} {id:?} waits for the first sync");
  }
  assert_eq!(
    availability(actions, "lone001", ActionId::Rename),
    Some(Availability::Enabled),
    "a rewrite runs after that sync"
  );
  assert_eq!(availability(actions, "lone001", ActionId::OpenPullRequest), Some(Availability::Enabled), "a link");
  assert_eq!(availability(actions, "robot:r0", ActionId::OpenRobot), Some(Availability::Enabled));

  state.finish_sync(Ok(()));
  let committed = view.diff(&state, &facts).unwrap();
  assert_eq!(committed.changes.cached, Some(false));
  assert_eq!(availability(&committed.changes.actions, "lone001", ActionId::SafePush), Some(Availability::Enabled));
  assert_eq!(view.snapshot().actions, checked.changes.actions, "the actions of the checked table");
}

/// The operations of the session lie over the actions of the view (`domain_policy::actions::with_operations`): a rewrite
/// admitted makes its own item running and the rewrites of every row wait behind it, a patch of the actions of every row;
/// its end gives them back, the actions as they were before it
#[test]
fn the_view_lays_the_operations_of_the_session_over_the_actions_from_the_admission_of_a_rewrite_to_its_end() {
  use crate::table::OperationPatch;
  use domain_types::operation::{Intent, OperationId, OperationStep, Origin, Resource, RunningOperation};
  use domain_types::rewrite::{RewriteIntent, TableWitness};
  let (mut state, facts) = crate::derive::derive_budget_test::session();
  let mut view = View::new(1);
  let first = view.diff(&state, &facts).unwrap();
  let before = first.changes.actions;
  let op = OperationId("u1".to_string());
  state.apply_operation(OperationPatch::Admitted(RunningOperation {
    id: op.clone(),
    intent: Intent::Rewrite(Box::new(RewriteIntent::Rename {
      branch_name: "lone001".to_string(),
      new_name: "lone".to_string(),
      witness: TableWitness::default(),
    })),
    resources: vec![Resource::Head],
    step: OperationStep::Rewriting,
    origin: Origin::ThisProcess,
    moves: Vec::new(),
    joined: 0,
  }));
  let admitted = view.diff(&state, &facts).unwrap();
  let availability = |key: &str, id: ActionId| {
    admitted.changes.actions[key]
      .iter()
      .find(|action| action.id == id)
      .map(|action| action.availability.clone())
  };
  assert_eq!(availability("lone001", ActionId::Rename), Some(Availability::Running { op: op.clone() }));
  assert_eq!(
    availability("lone002", ActionId::Rename),
    Some(Availability::Waits { op: op.clone() }),
    "another row queues behind it"
  );
  assert_eq!(
    availability("lone002", ActionId::ReviewChanges),
    Some(Availability::Enabled),
    "no rewrite: it runs beside one"
  );
  state.apply_operation(OperationPatch::Ended(op));
  let ended = view.diff(&state, &facts).unwrap();
  for (key, actions) in &ended.changes.actions {
    assert_eq!(Some(actions), before.get(key), "{key}: as before the rewrite");
  }
}

/// A branch kept outside HEAD is a subject of its own (`unapplied:<full name>`, `archived:<full name>`, the rule
/// `domain_policy::kept`), and an operation of it lies over its actions as over a branch's: its push runs
#[test]
fn the_view_offers_the_actions_of_the_kept_branches_with_their_operations() {
  use crate::table::OperationPatch;
  use domain_types::UnappliedBranchInfo;
  use domain_types::operation::{Intent, OperationId, OperationStep, Origin, Push, PushScope, Resource, RunningOperation};
  let (mut state, facts) = crate::derive::derive_budget_test::session();
  state.parts.sections.unapplied = vec![UnappliedBranchInfo {
    name: "user/unapplied/docs".to_string(),
    branch: "docs".to_string(),
    tip: "tip".to_string(),
    subject: "Document the API".to_string(),
    checked_out: false,
    remote: None,
  }];
  state.parts.sections.archived.names = vec!["user/archived/2026-10-01/old".to_string()];
  let mut view = View::new(1);
  let first = view.diff(&state, &facts).unwrap();
  let ids = |key: &str| first.changes.actions[key].iter().map(|action| action.id).collect::<Vec<_>>();
  assert_eq!(
    ids("unapplied:user/unapplied/docs"),
    [ActionId::ReviewChanges, ActionId::Reapply, ActionId::Push, ActionId::SafePush, ActionId::DeleteKeptBranch]
  );
  assert_eq!(ids("archived:user/archived/2026-10-01/old"), [ActionId::ReviewChanges, ActionId::DeleteKeptBranch]);

  let op = OperationId("p1".to_string());
  state.apply_operation(OperationPatch::Admitted(RunningOperation {
    id: op.clone(),
    intent: Intent::Push(Push {
      subject: Subject::Branch { name: "docs".to_string() },
      scope: PushScope::Unapplied,
      expect_remote: None,
      force: false,
      restart_running: false,
    }),
    resources: vec![Resource::Ref {
      name: "user/unapplied/docs".to_string(),
    }],
    step: OperationStep::Pushing,
    origin: Origin::ThisProcess,
    moves: Vec::new(),
    joined: 0,
  }));
  let pushing = view.diff(&state, &facts).unwrap();
  let push = pushing.changes.actions["unapplied:user/unapplied/docs"]
    .iter()
    .find(|action| action.id == ActionId::Push)
    .unwrap();
  assert_eq!(push.availability, Availability::Running { op });
}

/// A blocked row names the parent it waits for (the hover of its node offers to show it): the one its block names, else
/// the branch it is built on, else its declared parent; a row that is not blocked waits for none
#[test]
fn a_blocked_row_names_the_parent_it_waits_for() {
  let built_on = BranchStack {
    declared_parent: Some("core".to_string()),
    effective_base: Some("auth".to_string()),
    ..BranchStack::default()
  };
  assert_eq!(blocking_parent(BranchStatus::Blocked, Some(&built_on)), Some("auth".to_string()));
  let declared = BranchStack {
    declared_parent: Some("core".to_string()),
    ..BranchStack::default()
  };
  assert_eq!(blocking_parent(BranchStatus::Blocked, Some(&declared)), Some("core".to_string()));
  assert_eq!(blocking_parent(BranchStatus::Unchanged, Some(&built_on)), None);
  assert_eq!(blocking_parent(BranchStatus::Blocked, None), None);
}

/// The robot of a subject needs the user (`domain_policy::attention`) while it failed on its row as it is and nothing
/// else holds the subject: a lone branch, a branch once the Safe Push of a branch built on it no longer holds it, the
/// whole stack on its stack branch the sync built while no robot of a branch of it lands a part of it; never a branch
/// fixed since, a parked one, one the fetched baseline has whole or a stack branch not rebuilt, nor any of the cached frame
/// of an earlier run. A patch carries only the subjects whose answer changed
#[test]
fn the_view_names_the_robots_that_need_the_user_over_the_rows_as_they_are() {
  let names = ["login", "auth", "docs", "cache", "notes"];
  let mut events = vec![grouping(
    vec![on("login", Some("auth")), on("auth", None), on("docs", None), on("cache", None), on("notes", None)],
    &[("notes", "later")],
    &["auth"],
  )];
  for name in names {
    events.push(synced(name, &format!("{name}-1"), CommitSyncStatus::Created));
    events.push(branch_status(name, BranchSyncStatus::Created));
  }
  events.push(stack_branch("auth", &["auth", "login"]));
  let mut state = TableState::with_prefix("user");
  run_sync(&mut state, events);

  let failed = |id: &str, branch: &str, source: &str| PatronusRobot {
    source_revision: Some(source.to_string()),
    finish_time: Some(20),
    ..robot(id, Some(branch), PatronusRobotStatus::Failure, 10)
  };
  let login_run = PatronusRobot {
    source_revision: Some("copy-login-1".to_string()),
    ..robot("login-run", Some("login"), PatronusRobotStatus::Running, 11)
  };
  let robots = |robots: Vec<PatronusRobot>, read: RobotsRead| FactResult::Robots {
    read,
    result: PatronusRobotsResult {
      availability: PatronusAvailability::Available,
      robots,
      github_repository: None,
    },
  };
  let mut facts = FactStore::new();
  facts.apply(
    at(1),
    robots(
      vec![
        failed("docs-failed", "docs", "copy-docs-1"),
        // failed on a commit the branch no longer is at: fixed since
        failed("cache-failed", "cache", "old"),
        failed("notes-failed", "notes", "copy-notes-1"),
        failed("auth-failed", "auth", "copy-auth-1"),
        login_run.clone(),
        stack_robot("whole", "auth", PatronusRobotStatus::Failure, "stack-tip"),
      ],
      RobotsRead::List { full: true },
    ),
  );
  let row = |robot_id: &str| BranchPatronus {
    robot_id: Some(robot_id.to_string()),
    ..BranchPatronus::default()
  };
  // the Safe Push of login lands auth, built under it, and a part of the whole stack
  let held = BranchLanding {
    hold: Some(HoldView {
      kind: HoldKind::Span,
      by: robot_of("login", "login-run"),
      carried: None,
    }),
    ..BranchLanding::default()
  };
  let view = PatronusView {
    branches: BTreeMap::from([
      ("docs".to_string(), row("docs-failed")),
      ("cache".to_string(), row("cache-failed")),
      ("notes".to_string(), row("notes-failed")),
      (
        "auth".to_string(),
        BranchPatronus {
          landing: held,
          ..row("auth-failed")
        },
      ),
      ("login".to_string(), row("login-run")),
    ]),
    stacks: BTreeMap::from([(
      "auth".to_string(),
      StackPatronus {
        robot_id: Some("whole".to_string()),
        landing_member: Some(robot_of("login", "login-run")),
        ..StackPatronus::default()
      },
    )]),
    ..PatronusView::default()
  };
  // the part of the view, as the session publishes it after the derivation
  answer_patronus(&mut facts, 2, view, &BranchTable::default());
  let needs = |facts: &FactStore, state: &TableState| attention_of(state, facts).into_iter().map(|(key, attention)| (key, attention.robot_id)).collect::<Vec<_>>();
  let pair = |key: &str, robot_id: &str| (key.to_string(), robot_id.to_string());
  assert_eq!(needs(&facts, &state), [pair("docs", "docs-failed")], "fixed, parked, held: only docs");
  // the same rows as the cached frame of an earlier run: no count a fresh answer takes back
  state.cached = true;
  assert_eq!(needs(&facts, &state), Vec::<(String, String)>::new(), "the cached frame");
  state.cached = false;
  let mut window = View::new(1);
  let _ = window.diff(&state, &facts);

  // the Safe Push of login is canceled: it holds and lands nothing, the failures of auth and of the whole stack count
  facts.apply(
    at(3),
    robots(
      vec![PatronusRobot {
        status: PatronusRobotStatus::Canceled,
        finish_time: Some(30),
        ..login_run
      }],
      RobotsRead::Poll,
    ),
  );
  assert_eq!(
    needs(&facts, &state),
    [pair("auth", "auth-failed"), pair("docs", "docs-failed"), pair("stack:auth", "whole")]
  );
  let patch = window.diff(&state, &facts).unwrap();
  assert_eq!(patch.changes.attention.keys().collect::<Vec<_>>(), ["auth", "stack:auth"], "docs did not change");

  // the fetched baseline has all of docs, and the stack branch is no longer rebuilt: the stack lands in pieces
  state.apply(SyncEvent::BaselineStatus {
    baseline_branch: "origin/master".to_string(),
    update: None,
    check_error: None,
    landing: vec![LandedBranch {
      name: Some("docs".to_string()),
      commits: Vec::new(),
      remaining: 0,
      landed_at: 1,
    }],
    landed: Vec::new(),
  });
  state.apply(SyncEvent::StackBranchUpdate(Box::new(StackBranchUpdate {
    root: "auth".to_string(),
    branch_name: "test/stack/auth".to_string(),
    members: Vec::new(),
    commit_count: 0,
    my_email: None,
    state: StackBranchState::Stale,
    tip: Some("stack-tip".to_string()),
    blocked_by: None,
    conflict: None,
    error: Some("Not rebuilt: auth heads no stack that forks".to_string()),
    remote: None,
  })));
  assert_eq!(needs(&facts, &state), [pair("auth", "auth-failed")]);
  let patch = window.diff(&state, &facts).unwrap();
  assert_eq!(patch.changes.removed_attention, ["docs", "stack:auth"]);
}

/// The layers of the stacks of GitHub of a table (docs/design/github-stacks.md): each branch of a straight chain of two
/// or more, its parent the branch below it and the branches below it without an open pull request, which its creation
/// opens too; none of a stack that forks, of a lone branch, and none on a route of Patronus or where GitHub's schema has
/// no stacks
#[test]
fn the_view_names_the_layers_of_the_stacks_of_github_of_a_straight_chain() {
  use crate::derive::{github_layers, github_layers_over};
  use domain_policy::facts::Robots;
  use domain_policy::landed::LandedFacts;
  use domain_types::landing_route::{LandingRoute, RouteGate};
  use domain_types::session::{GitHubLayer, LandedSection};
  let table = BranchTable {
    branches: vec![on("oauth", Some("login")), on("docs", None), on("login", Some("auth")), on("auth", None)],
    ..BranchTable::default()
  };
  let layers = github_layers(&table, |_| false, |branch| branch == "auth");
  let layer = |parent: Option<&str>, creates: &[&str]| GitHubLayer {
    chain: names(&["auth", "login", "oauth"]),
    parent: parent.map(str::to_owned),
    creates: names(creates),
  };
  assert_eq!(
    layers,
    BTreeMap::from([
      ("auth".to_owned(), layer(None, &[])),
      ("login".to_owned(), layer(Some("auth"), &[])),
      ("oauth".to_owned(), layer(Some("login"), &["login"])),
    ])
  );
  // the chain starts above what landed whole
  let above = github_layers(&table, |branch| branch == "auth", |_| false);
  assert_eq!(above["oauth"].chain, names(&["login", "oauth"]));
  assert!(!above.contains_key("auth"));

  let forking = BranchTable {
    branches: vec![on("session", Some("auth")), on("login", Some("auth")), on("auth", None)],
    ..BranchTable::default()
  };
  assert_eq!(github_layers(&forking, |_| false, |_| false), BTreeMap::new());

  let robots = Robots::new(&[]);
  let baseline = LandedSection::default();
  let landed = LandedFacts {
    baseline: &baseline,
    view: None,
    robots: &robots,
    pushed: None,
    branches: None,
  };
  let mut listing = PullRequestsView {
    github_stacks: true,
    ..PullRequestsView::default()
  };
  let merge = LandingRoute::PullRequest { merge: RouteGate::Open };
  let patronus = LandingRoute::Patronus { gate: RouteGate::Open };
  assert_eq!(github_layers_over(&merge, &table, &table.remote, &landed, &listing).len(), 3);
  assert_eq!(github_layers_over(&patronus, &table, &table.remote, &landed, &listing), BTreeMap::new());
  listing.github_stacks = false;
  assert_eq!(github_layers_over(&merge, &table, &table.remote, &landed, &listing), BTreeMap::new());
}

/// The declared needs of a branch come with the grouping (`BranchesGrouped::needs`) into its structure, which Remove
/// Need of its menu names; a branch without a line has none
#[test]
fn the_structure_of_a_branch_names_the_needs_it_declared() {
  let mut table = TableState::with_prefix("user");
  table.begin_sync();
  let mut event = grouping(vec![on("auth", None), on("docs", None)], &[], &[]);
  if let SyncEvent::BranchesGrouped { needs, .. } = &mut event {
    *needs = BTreeMap::from([("auth".to_string(), "ci-config:prepare docs".to_string())]);
  }
  table.apply(event);
  let needs = |name: &str| table.branch(name).map(|branch| branch.structure.needs.clone());
  assert_eq!(needs("auth"), Some(vec!["ci-config:prepare".to_string(), "docs".to_string()]));
  assert_eq!(needs("docs"), Some(Vec::new()));
}

/// The commits of each branch of the derive tests of merged robots, newest first
fn merged_commits(name: &str) -> &'static [&'static str] {
  match name {
    "docs" => &["docs-1"],
    "cache" => &["cache-1"],
    "notes" => &["notes-1"],
    "grown" => &["grown-2", "grown-1"],
    "reworded" => &["reworded-1b"],
    "base" => &["base-2", "base-1"],
    "top" => &["top-1"],
    "kept" => &["kept-1"],
    "above" => &["above-1"],
    "air" => &["air-1"],
    _ => &["air-child-2", "air-child-1"],
  }
}

/// The session of a sync of `branches` (stacked on the parent they name, `air` the root of a stack that forks) with no
/// remote status, the merged robots of `robots` (`<branch>-merged` on the commit `robots` names, `whole` of the stack of
/// air), and the Patronus view `view`: the next step of every subject
fn merged_robot_steps(branches: &[(&str, Option<&str>)], robots: &[(&str, &str)], view: PatronusView) -> BTreeMap<String, domain_types::next_step::NextStepView> {
  let (state, facts) = merged_robot_session(branches, robots, view);
  merged_robot_steps_of(&state, &facts)
}

/// The next step of every subject of the session `state` and `facts`
fn merged_robot_steps_of(state: &TableState, facts: &FactStore) -> BTreeMap<String, domain_types::next_step::NextStepView> {
  use crate::derive::{landed_states_of, next_steps_of};
  next_steps_of(state, facts, &BTreeMap::new(), &landed_states_of(state, facts))
}

/// The session of `merged_robot_steps`
fn merged_robot_session(branches: &[(&str, Option<&str>)], robots: &[(&str, &str)], view: PatronusView) -> (TableState, FactStore) {
  merged_robot_session_with(branches, robots, view, &[])
}

/// `merged_robot_session` with the remote statuses of `unpushed`: each branch with its copies the remote lacks
fn merged_robot_session_with(branches: &[(&str, Option<&str>)], robots: &[(&str, &str)], view: PatronusView, unpushed: &[(&str, &[&str])]) -> (TableState, FactStore) {
  use std::collections::HashMap;

  let grouped_branches: Vec<GroupedBranchInfo> = branches
    .iter()
    .map(|(name, parent)| GroupedBranchInfo {
      stack: Some(BranchStack {
        declared_parent: parent.map(str::to_string),
        effective_base: parent.map(str::to_string),
        ..BranchStack::default()
      }),
      ..grouped(name, merged_commits(name))
    })
    .collect();
  let forks = branches.iter().any(|(name, _)| *name == "air");
  let mut events = vec![grouping(grouped_branches.clone(), &[], if forks { &["air"] } else { &[] })];
  for (name, _) in branches {
    for commit in merged_commits(name) {
      events.push(synced(name, commit, CommitSyncStatus::Created));
    }
    events.push(branch_status(name, BranchSyncStatus::Created));
  }
  let stack = StackBranchUpdate {
    root: "air".to_string(),
    branch_name: "user/stack/air".to_string(),
    members: names(&["air", "air-child"]),
    commit_count: 3,
    my_email: None,
    state: StackBranchState::Built,
    tip: Some("stack-tip-2".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  };
  if forks {
    events.push(SyncEvent::StackBranchUpdate(Box::new(stack.clone())));
  }
  for (name, copies) in unpushed {
    events.push(SyncEvent::RemoteStatusUpdate(remote_status(name, copies)));
  }
  let mut state = TableState::with_prefix("user");
  run_sync(&mut state, events);
  let table = BranchTable {
    prefix: "user".to_string(),
    baseline: "origin/master".to_string(),
    branches: grouped_branches,
    tips: branches
      .iter()
      .map(|(name, _)| (format!("user/virtual/{name}"), format!("copy-{}", merged_commits(name)[0])))
      .collect::<HashMap<_, _>>(),
    stack_branches: if forks { vec![stack] } else { Vec::new() },
    ..BranchTable::default()
  };
  let mut facts = synced_facts(&table);
  let merged = |name: &str, source: &str| PatronusRobot {
    source_revision: Some(source.to_string()),
    finish_time: Some(20),
    ..if name == "whole" {
      stack_robot("whole", "air", PatronusRobotStatus::Success, source)
    } else {
      robot(&format!("{name}-merged"), Some(name), PatronusRobotStatus::Success, 10)
    }
  };
  facts.apply(
    at(2),
    FactResult::Robots {
      read: RobotsRead::List { full: true },
      result: PatronusRobotsResult {
        availability: PatronusAvailability::Available,
        robots: robots.iter().map(|(name, source)| merged(name, source)).collect(),
        github_repository: Some("acme/monorepo".to_string()),
      },
    },
  );
  answer_patronus(&mut facts, 3, view, &table);
  (state, facts)
}

/// The view's answer of `name`: its robot `robot_id`, landed whole by `by` with its commits `landed` (a hold of what landed
/// beside), or none
fn merged_robot_row(name: &str, robot_id: &str, by: Option<(RobotOf, &[&str])>) -> BranchPatronus {
  use domain_types::repository_view::{LandedByView, LandingNeeds};
  let hold = by.as_ref().map(|(by, _)| HoldView {
    kind: HoldKind::Landed,
    by: by.clone(),
    carried: None,
  });
  BranchPatronus {
    robot_id: Some(robot_id.to_string()),
    landing_needs: LandingNeeds::Known { refusals: Vec::new() },
    landed_by: by.map(|(by, landed)| LandedByView {
      own: by.branch == name,
      by,
      landed: names(landed),
      after: Vec::new(),
    }),
    landing: BranchLanding { hold, ..BranchLanding::default() },
    ..BranchPatronus::default()
  }
}

/// The view's answer of `name` that its own robot `<name>-merged` landed all of it, its commits `landed`
fn own_landed_whole(name: &str, landed: &'static [&'static str]) -> BranchPatronus {
  merged_robot_row(name, &format!("{name}-merged"), Some((robot_of(name, &format!("{name}-merged")), landed)))
}

/// The view of another table that says the own robots of grown and notes landed all of them (`grown-1`, `notes-1`)
fn grown_and_notes_landed_whole() -> PatronusView {
  PatronusView {
    branches: BTreeMap::from([
      ("grown".to_string(), own_landed_whole("grown", &["grown-1"])),
      ("notes".to_string(), own_landed_whole("notes", &["notes-1"])),
    ]),
    ..PatronusView::default()
  }
}

/// A merged robot of a branch landed only the commits it took (openspec/specs/branch/next-step/spec.md, "A merged robot
/// lands only the commits it took"): before any remote status (the first view of a session, `brd status`), docs, whose
/// robot took earlier commits under its name and landed none of what it has now, is new work, not landed, its step the
/// one of new work and its robot of an earlier state (`earlier_robot`); cache, whose robot took its tip, and notes,
/// whose robot the view says landed all of it by content, are landed, "Merged by Patronus". The view the session shows
/// was derived for another table (stale while the view of this one is derived) for grown, which has a commit since, and
/// for reworded, whose landed commit has a new id: what it says landed them whole, and held their pushes, holds no more,
/// and Patronus is pending for them (row 0) until the view of this table says what landed them
#[test]
fn a_merged_robot_of_earlier_commits_lands_nothing_of_the_branch_before_a_remote_status() {
  use domain_types::next_step::{RowFact, StepBlocker, StepFact};

  let view = PatronusView {
    branches: BTreeMap::from([
      ("docs".to_string(), merged_robot_row("docs", "docs-merged", None)),
      ("cache".to_string(), merged_robot_row("cache", "cache-merged", None)),
      ("notes".to_string(), own_landed_whole("notes", &["notes-1"])),
      ("grown".to_string(), own_landed_whole("grown", &["grown-1"])),
      ("reworded".to_string(), own_landed_whole("reworded", &["reworded-1"])),
    ]),
    ..PatronusView::default()
  };
  let branches = ["docs", "cache", "notes", "grown", "reworded"].map(|name| (name, None));
  let robots = [
    ("docs", "earlier"),
    ("cache", "copy-cache-1"),
    ("notes", "rebuilt"),
    ("grown", "rebuilt"),
    ("reworded", "rebuilt"),
  ];
  let (state, facts) = merged_robot_session(&branches, &robots, view.clone());
  let actions = View::new(1).diff(&state, &facts).unwrap().changes.actions;
  let push_held = |name: &str| {
    actions[name].iter().any(|action| {
      action.id == ActionId::Push
        && matches!(
          &action.availability,
          Availability::Disabled {
            reason: ActionBlocker::Rule { reason: Reason::Held { .. } }
          }
        )
    })
  };
  assert!(push_held("notes"), "what landed holds the push of notes");
  assert!(!push_held("grown") && !push_held("reworded"), "no hold of what a view of another table says landed");
  let steps = merged_robot_steps(&branches, &robots, view);

  let docs = &steps["docs"];
  assert_eq!(
    (&docs.step.kind, &docs.step.safe_push, &docs.step.landed, &docs.step.earlier_robot),
    (
      &StepKind::Pending { fact: StepFact::PullRequests },
      &Some(StepBlocker::Step),
      &None,
      &Some("docs-merged".to_string())
    ),
    "new work under the name of a landed robot: {docs:?}"
  );
  assert_ne!(docs.node.kind, LandingNodeKind::InMaster);
  assert_eq!(docs.status.main, None, "the Status cell shows the branch as it is");
  for name in ["cache", "notes"] {
    let landed = &steps[name];
    assert_eq!((landed.step.row, &landed.step.kind), (StepRow::One, &StepKind::Settled), "{name}");
    assert!(landed.step.landed.is_some(), "{name}");
    assert_eq!(landed.step.earlier_robot, None, "{name}");
    assert_eq!(landed.node.kind, LandingNodeKind::InMaster, "{name}");
    assert_eq!(landed.status.main, Some(RowFact::Robot { running: false, failed: false }), "{name}");
  }
  for name in ["grown", "reworded"] {
    let unknown = &steps[name];
    assert_eq!(
      (
        unknown.step.row,
        &unknown.step.kind,
        &unknown.step.safe_push,
        &unknown.step.landed,
        &unknown.step.earlier_robot
      ),
      (
        StepRow::Zero,
        &StepKind::Pending { fact: StepFact::Patronus },
        &Some(StepBlocker::Step),
        &None,
        &Some(format!("{name}-merged"))
      ),
      "{name}: a view of another table, its robot of an earlier state as the facts there are say: {unknown:?}"
    );
    assert_ne!(unknown.node.kind, LandingNodeKind::InMaster, "{name}");
    assert_eq!(unknown.status.main, None, "{name}: no robot, no hold of what landed");
  }
}

/// The rules over the view read it against the table as it is now (openspec/specs/branch/next-step/spec.md, "A merged
/// robot lands only the commits it took", "A view of an older table"): top is built on base, which the view of another
/// table says a Safe Push of side landed whole, and base has a commit since: what the view says of top (what its Safe
/// Push lands, what holds it) reads what it says of base, so Patronus is pending for top too, whose own commits it
/// knows; above is built on kept, which the view knows as it is and says landed
/// whole: the Safe Push of above lands only above. The robot of the whole stack of air merged it as the view of another
/// table says, and air-child has a commit since: what is left of the stack is read of the table as it is (air-child),
/// the view publishes it (`NextStepView::rest`), and Patronus is pending for the whole stack
#[test]
fn a_view_of_another_table_leaves_pending_the_branches_built_on_one_it_does_not_know_and_their_whole_stack() {
  use domain_types::next_step::StepFact;

  let landed_by = |robot: &str, landed: &'static [&'static str]| Some((robot_of(robot, &format!("{robot}-merged")), landed));
  let by_whole = |landed: &'static [&'static str]| Some((whole_stack_robot_of("air", "whole"), landed));
  let stale_rest = WholeStackRest {
    landed: names(&["air", "air-child"]),
    by: vec![whole_stack_robot_of("air", "whole")],
    by_own_robot: true,
    ..WholeStackRest::default()
  };
  let view = PatronusView {
    branches: BTreeMap::from([
      ("base".to_string(), merged_robot_row("base", "side-merged", landed_by("side", &["base-1"]))),
      ("top".to_string(), merged_robot_row("top", "no-robot", None)),
      ("kept".to_string(), merged_robot_row("kept", "above-merged", landed_by("above", &["kept-1"]))),
      ("above".to_string(), merged_robot_row("above", "above-merged", None)),
      ("air".to_string(), merged_robot_row("air", "whole", by_whole(&["air-1"]))),
      ("air-child".to_string(), merged_robot_row("air-child", "whole", by_whole(&["air-child-1"]))),
    ]),
    stacks: BTreeMap::from([(
      "air".to_string(),
      StackPatronus {
        robot_id: Some("whole".to_string()),
        landing: StackLanding {
          rest: stale_rest,
          ..StackLanding::default()
        },
        ..StackPatronus::default()
      },
    )]),
    ..PatronusView::default()
  };
  let branches = [
    ("base", None),
    ("top", Some("base")),
    ("kept", None),
    ("above", Some("kept")),
    ("air", None),
    ("air-child", Some("air")),
  ];
  // the robot of side, another branch built on base, landed base by content
  let robots = [("side", "rebuilt"), ("above", "copy-above-1"), ("whole", "stack-tip-1")];
  let steps = merged_robot_steps(&branches, &robots, view);
  let top = &steps["top"].step;
  assert_eq!((&top.kind, &top.landed), (&StepKind::Pending { fact: StepFact::Patronus }, &None), "{top:?}");
  assert_eq!(steps["above"].step.lands, 1, "kept landed whole: {:?}", steps["above"].step);
  let stack = &steps["stack:air"];
  assert_eq!(
    (&stack.step.kind, &stack.step.landed, stack.step.lands),
    (&StepKind::Pending { fact: StepFact::Patronus }, &None, 1),
    "{stack:?}"
  );
  assert_eq!(
    stack.rest,
    Some(WholeStackRest {
      members: names(&["air-child"]),
      commit_count: 2,
      landed: names(&["air"]),
      by: vec![whole_stack_robot_of("air", "whole")],
      by_own_robot: false,
    }),
    "what is left of the stack as it is"
  );
  assert_eq!(steps["air"].rest, None, "a branch has no rest");
}

/// A derivation of the view for this table failed after the view of another one (`Stale` with its reason): of a branch
/// that view doesn't know as it is the step says the failure of the view, not a wait for Patronus; of one it knows, the
/// step of its facts
#[test]
fn a_failed_view_of_this_table_shows_its_failure_of_the_branches_the_view_before_it_does_not_know() {
  use crate::derive::{landed_states_of, next_steps_of};
  use domain_types::next_step::{StepFact, StepFailure};
  use std::time::Duration;

  let view = grown_and_notes_landed_whole();
  let (state, mut facts) = merged_robot_session(&[("grown", None), ("notes", None)], &[("grown", "rebuilt"), ("notes", "rebuilt")], view);
  let failure = ApiError::new(domain_types::error::ErrorCode::Internal, "no view");
  assert!(facts.patronus.begin(4, Duration::from_secs(4)));
  facts.patronus.answer(at(4), Err(failure.clone()));
  let table = facts.table.value().map(|table| BranchTable::clone(table)).unwrap();
  let _ = facts.patronus.publish(&table, None, &crate::patronus_view::ContentMemo::default());
  let steps = next_steps_of(&state, &facts, &BTreeMap::new(), &landed_states_of(&state, &facts));
  assert!(
    matches!(&steps["grown"].step.kind, StepKind::Unavailable { failure: StepFailure::ViewFailed { reason }, .. } if *reason == failure),
    "{:?}",
    steps["grown"].step
  );
  assert_ne!(steps["notes"].step.kind, StepKind::Pending { fact: StepFact::Patronus }, "the view knows notes");
  assert!(steps["notes"].step.landed.is_some(), "{:?}", steps["notes"].step);
}

/// The view's answer of a branch: what `by` landed of it (`landed`: whether it ran on the branch itself, the commits it
/// landed; none: nothing landed it) and `hold`, what the view decided holds its push
fn held_branch(by: &RobotOf, landed: Option<(bool, &[&str])>, hold: HoldView) -> BranchPatronus {
  use domain_types::repository_view::LandedByView;
  BranchPatronus {
    landed_by: landed.map(|(own, commits)| LandedByView {
      by: by.clone(),
      own,
      landed: names(commits),
      after: Vec::new(),
    }),
    landing: BranchLanding {
      hold: Some(hold),
      ..BranchLanding::default()
    },
    ..BranchPatronus::default()
  }
}

/// What a start reads of the view the session shows (`LandingFacts` over the holds the view decided) is read against the
/// table as it is now: auth, below login, has a commit since the view of another table said the robot of login landed
/// it whole and held its push, so the Safe Push of login lands auth too, and nothing holds it; of the table the view was
/// derived for, auth landed whole and its push is held
#[test]
fn a_start_reads_no_landing_and_no_hold_of_a_branch_whose_commits_changed_since_the_view() {
  use crate::derive::LandingFacts;
  use crate::patronus_view::patronus_view_test::table;

  let by = robot_of("login", "r");
  let hold = HoldView {
    kind: HoldKind::Landed,
    by: by.clone(),
    carried: None,
  };
  let view = PatronusView {
    branches: BTreeMap::from([("auth".to_string(), held_branch(&by, Some((false, &["a1"])), hold.clone()))]),
    ..PatronusView::default()
  };
  let robots = [PatronusRobot {
    finish_time: Some(20),
    ..robot("r", Some("login"), PatronusRobotStatus::Success, 10)
  }];
  let auth_of = |table: &BranchTable| {
    let landing = LandingFacts::new(table, &table.remote, &view, &robots, None);
    let subject = landing.branch_subject("login", None, &LandedSection::default()).unwrap();
    let auth = subject.chain.iter().find(|branch| branch.name == "auth").copied().unwrap();
    (auth.landed_whole, auth.hold.cloned())
  };
  let grown = table(&[("login", Some("auth"), &[("l1", "vl1")]), ("auth", None, &[("a2", "va2"), ("a1", "va1")])]);
  assert_eq!(auth_of(&grown), (false, None), "a commit since the view");
  let as_it_was = table(&[("login", Some("auth"), &[("l1", "vl1")]), ("auth", None, &[("a1", "va1")])]);
  assert_eq!(auth_of(&as_it_was), (true, Some(hold)), "the table of the view");
}

/// What a start reads of the holds the view decided (`LandingFacts` over a view whose holds are decided already) is read
/// as every reader reads them (`derive::hold_now`): the own robot of login held its push when the view was derived, and
/// holds it while it runs, not once it ended; a hold of what landed stands while its robot merged it
#[test]
fn a_start_reads_the_holds_of_the_view_as_every_reader_does() {
  use crate::derive::LandingFacts;
  use crate::patronus_view::patronus_view_test::table;

  let table = table(&[("login", None, &[("l1", "vl1")])]);
  let hold_of = |kind: HoldKind, status: PatronusRobotStatus| {
    let hold = HoldView {
      kind,
      by: robot_of("login", "r"),
      carried: None,
    };
    let landed = (kind == HoldKind::Landed).then_some((true, &["l1"][..]));
    let facts = BranchPatronus {
      robot_id: Some("r".to_string()),
      ..held_branch(&robot_of("login", "r"), landed, hold.clone())
    };
    let view = PatronusView {
      branches: BTreeMap::from([("login".to_string(), facts)]),
      ..PatronusView::default()
    };
    let robots = [robot("r", Some("login"), status, 10)];
    let found = LandingFacts::new(&table, &table.remote, &view, &robots, None).hold("login").cloned();
    (found, hold)
  };
  let (running, own) = hold_of(HoldKind::Own, PatronusRobotStatus::Running);
  assert_eq!(running, Some(own));
  assert_eq!(hold_of(HoldKind::Own, PatronusRobotStatus::Failure).0, None, "its robot ended since the view");
  let (merged, landed) = hold_of(HoldKind::Landed, PatronusRobotStatus::Success);
  assert_eq!(merged, Some(landed));
  assert_eq!(hold_of(HoldKind::Landed, PatronusRobotStatus::Canceled).0, None, "its robot merged nothing");
}

/// The push an action needs (`BranchPush::action_blocker`, `action_push_of`) is refused of the table as it is: by the
/// nearest branch of its chain the last sync did not build or whose push drops remote commits, up to the nearest one a
/// running Safe Push holds, whose robot lands it and what it is built on; a robot that ended since the view holds
/// nothing, and the chain goes on below it
#[test]
fn the_push_of_an_action_is_refused_by_its_chain_as_the_table_is() {
  use crate::derive::action_push_of;
  use domain_types::stack::BlockReason;

  let blocker_of = |login_drops: u32, login_robot: Option<PatronusRobotStatus>| {
    let branches = vec![on("oauth", Some("login")), on("login", Some("auth")), on("auth", None)];
    let mut login = remote_status("login", &["copy-login-1"]);
    login.commits_behind = login_drops;
    login.action = login.push_action();
    let mut events = vec![grouping(branches.clone(), &[], &[]), branch_status("auth", BranchSyncStatus::Blocked)];
    events.extend(
      [remote_status("auth", &["copy-auth-1"]), login, remote_status("oauth", &["copy-oauth-1"])]
        .into_iter()
        .map(SyncEvent::RemoteStatusUpdate),
    );
    let mut state = TableState::with_prefix("user");
    run_sync(&mut state, events);
    let mut table = BranchTable {
      prefix: "user".to_string(),
      baseline: "origin/master".to_string(),
      branches,
      ..BranchTable::default()
    };
    table.not_built.insert("auth".to_string(), BlockReason::Conflict);
    let mut facts = synced_facts(&table);
    let robots: Vec<PatronusRobot> = login_robot.map(|status| robot("r", Some("login"), status, 10)).into_iter().collect();
    facts.apply(
      at(2),
      FactResult::Robots {
        read: RobotsRead::List { full: true },
        result: PatronusRobotsResult {
          availability: PatronusAvailability::Available,
          robots,
          github_repository: None,
        },
      },
    );
    // the view derived when the robot of login ran on it: its own hold
    let held = BranchPatronus {
      robot_id: Some("r".to_string()),
      landing: BranchLanding {
        hold: Some(HoldView {
          kind: HoldKind::Own,
          by: robot_of("login", "r"),
          carried: None,
        }),
        ..BranchLanding::default()
      },
      ..BranchPatronus::default()
    };
    let view = PatronusView {
      branches: login_robot.map(|_| ("login".to_string(), held)).into_iter().collect(),
      ..PatronusView::default()
    };
    answer_patronus(&mut facts, 3, view, &table);
    let pushes = View::new(1).diff(&state, &facts).unwrap().changes.pushes;
    let blocker = pushes.get("oauth").and_then(|push| push.action_blocker.clone());
    assert_eq!(blocker, action_push_of(&state, &facts, "oauth").blocker, "the view sends what an action plans by");
    blocker
  };
  let not_built = Some(Reason::CantPush {
    branch: "auth".to_string(),
    block: BlockReason::Conflict,
  });
  assert_eq!(blocker_of(0, None), not_built);
  assert_eq!(
    blocker_of(2, None),
    Some(Reason::DropsRemote {
      branch: "login".to_string(),
      behind: 2,
    }),
    "the nearest first"
  );
  assert_eq!(blocker_of(0, Some(PatronusRobotStatus::Running)), None, "the robot of login lands auth with it");
  assert_eq!(blocker_of(0, Some(PatronusRobotStatus::Failure)), not_built, "the robot of login ended since the view");
}

/// Push Up to Here offers the commits a merged Safe Push did not land, as the view says it while it knows the branch as
/// it is: the one after what landed; of a view of another table (auth has a commit since) every commit the remote lacks,
/// which is the common answer
#[test]
fn push_up_to_here_reads_what_landed_only_of_a_view_that_knows_the_branch() {
  use domain_types::repository_view::LandedByView;

  let up_to_of = |landed: &[&str], after: &[&str]| {
    let mut state = TableState::with_prefix("user");
    state.begin_sync();
    state.apply(grouping(
      vec![GroupedBranchInfo {
        stack: Some(BranchStack::default()),
        ..grouped("auth", &["a2", "a1"])
      }],
      &[],
      &[],
    ));
    for commit in ["a1", "a2"] {
      state.apply(synced("auth", commit, CommitSyncStatus::Created));
    }
    state.apply(branch_status("auth", BranchSyncStatus::Created));
    state.apply(SyncEvent::RemoteStatusUpdate(remote_status("auth", &["copy-a2", "copy-a1"])));
    state.finish_sync(Ok(()));
    let mut facts = FactStore::new();
    let view = PatronusView {
      branches: BTreeMap::from([(
        "auth".to_string(),
        BranchPatronus {
          landed_by: Some(LandedByView {
            by: robot_of("auth", "r"),
            own: true,
            landed: names(landed),
            after: names(after),
          }),
          ..BranchPatronus::default()
        },
      )]),
      ..PatronusView::default()
    };
    answer_patronus(&mut facts, 2, view, &BranchTable::default());
    let pushes = View::new(1).diff(&state, &facts).unwrap().changes.pushes;
    pushes.get("auth").and_then(|push| push.up_to.clone()).and_then(|up_to| up_to.commits)
  };
  assert_eq!(up_to_of(&["a1"], &["a2"]), Some(names(&["copy-a2"])), "only the commit after what landed");
  assert_eq!(up_to_of(&["a1"], &[]), None, "a view of another table: every commit the remote lacks");
}

/// Every push reads the holds of the view against the branches as they are now (`derive::hold_now`): grown has a commit
/// since the view of another table said its robot landed all of it, so its push alone goes, the push an action needs
/// sends it, and the view of the row names no hold; notes, which that view knows as it is, stays held as landed
#[test]
fn a_push_reads_no_hold_of_what_a_view_of_another_table_says_landed() {
  use crate::derive::{action_push_of, alone_push_of, with_push_states};
  use domain_policy::push::Verdict;

  let view = grown_and_notes_landed_whole();
  let unpushed: [(&str, &[&str]); 2] = [("grown", &["copy-grown-2"]), ("notes", &["copy-notes-1"])];
  let (state, facts) = merged_robot_session_with(&[("grown", None), ("notes", None)], &[("grown", "rebuilt"), ("notes", "rebuilt")], view, &unpushed);
  with_session_holds(&facts, |holds| {
    let grown = alone_push_of(&state, &facts, holds, "grown", None).unwrap();
    assert_eq!((grown.verdict, grown.blocker), (Verdict::Goes, None), "a commit since the view");
    let notes = alone_push_of(&state, &facts, holds, "notes", None).unwrap();
    assert_eq!(notes.verdict, Verdict::Landed);
    assert!(matches!(notes.blocker, Some(Reason::Held { .. })), "{:?}", notes.blocker);
    // why a push keeps it out as landed (the fetched baseline has it whole, say): no hold of that view names it
    let refusal = |name: &str| with_push_states(&state, &facts, holds, |rules| rules.refusal(name, Verdict::Landed));
    assert_eq!(refusal("grown"), None);
    assert!(matches!(refusal("notes"), Some(Reason::Held { .. })));
  });
  assert_eq!(action_push_of(&state, &facts, "grown").branches, names(&["grown"]));
  assert_eq!(action_push_of(&state, &facts, "notes").branches, Vec::<String>::new());
  let steps = merged_robot_steps_of(&state, &facts);
  assert_eq!(steps["grown"].hold, None);
  assert_eq!(steps["notes"].hold.as_ref().map(|hold| hold.kind), Some(HoldKind::Landed));
}

/// The whole stack of air: air-child and air-side on air, newest first, the commits of each `<name>-1`, air-child with a
/// second one `air-child-2` while `grown`
const AIR_STACK: [(&str, Option<&str>); 3] = [("air-side", Some("air")), ("air-child", Some("air")), ("air", None)];

fn air_commits(name: &str, grown: bool) -> Vec<String> {
  if name == "air-child" && grown {
    names(&["air-child-2", "air-child-1"])
  } else {
    vec![format!("{name}-1")]
  }
}

/// A session of `AIR_STACK` (air-child grown while `grown`), every branch on its remote as it is, its stack branch on
/// the remote behind its build, the branches of `parked` parked, the robots `robots` listed and `view` the Patronus view
/// it shows, as a view derived for another table (before a commit, a park, the end of a robot) says it
fn air_stack_session(view: PatronusView, robots: Vec<PatronusRobot>, parked: &[(&str, &str)], grown: bool) -> (TableState, FactStore) {
  let branches: Vec<(&str, Option<&str>, Vec<String>)> = AIR_STACK.iter().map(|(name, parent)| (*name, *parent, air_commits(name, grown))).collect();
  stack_session(&branches, view, robots, parked)
}

/// A session of `branches` (newest first, each on the parent it names with its commits, newest first; `air` the root of
/// the whole stack of every one of them), every branch on its remote as it is at its tip `<name>-tip`, its stack branch
/// on the remote behind its build at `stack-tip`, the branches of `parked` parked, the robots `robots` listed and `view`
/// the view a derivation answered, of another table or other robots
fn stack_session(branches: &[(&str, Option<&str>, Vec<String>)], view: PatronusView, robots: Vec<PatronusRobot>, parked: &[(&str, &str)]) -> (TableState, FactStore) {
  let grouped: Vec<GroupedBranchInfo> = branches
    .iter()
    .map(|(name, parent, commits)| on_with(name, *parent, &commits.iter().map(String::as_str).collect::<Vec<_>>()))
    .collect();
  let stack = StackBranchUpdate {
    root: "air".to_string(),
    branch_name: "user/stack/air".to_string(),
    members: branches.iter().rev().map(|(name, _, _)| (*name).to_string()).collect(),
    commit_count: 3,
    my_email: None,
    state: StackBranchState::Built,
    tip: Some("stack-tip".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    // on the remote, behind its build: a push sends it
    remote: Some(RemoteStatusUpdate {
      action: PushAction::Push,
      ..domain_types::fixtures::remote_status("stack:air")
    }),
  };
  let mut events = vec![grouping(grouped.clone(), parked, &["air"])];
  for (name, _, commits) in branches {
    for commit in commits {
      events.push(synced(name, commit, CommitSyncStatus::Created));
    }
    events.push(branch_status(name, BranchSyncStatus::Created));
    events.push(SyncEvent::RemoteStatusUpdate(remote_status(name, &[])));
  }
  events.push(SyncEvent::StackBranchUpdate(Box::new(stack.clone())));
  let mut state = TableState::with_prefix("user");
  run_sync(&mut state, events);
  let table = BranchTable {
    prefix: "user".to_string(),
    baseline: "origin/master".to_string(),
    tips: branches.iter().map(|(name, _, _)| (format!("user/virtual/{name}"), format!("{name}-tip"))).collect(),
    branches: grouped,
    stack_branches: vec![stack],
    parked: parked.iter().map(|(name, note)| ((*name).to_string(), (*note).to_string())).collect(),
    ..BranchTable::default()
  };
  let mut facts = synced_facts(&table);
  list_robots(&mut facts, 2, robots);
  answer_patronus(&mut facts, 3, view, &table);
  (state, facts)
}

/// Patronus listed `robots` at `seq`, every robot of the user
fn list_robots(facts: &mut FactStore, seq: u32, robots: Vec<PatronusRobot>) {
  facts.apply(
    at(seq),
    FactResult::Robots {
      read: RobotsRead::List { full: true },
      result: PatronusRobotsResult {
        availability: PatronusAvailability::Available,
        robots,
        github_repository: Some("acme/monorepo".to_string()),
      },
    },
  );
}

/// A view of the stack of air: what the robot `whole` of the whole stack landed of each of `landed` (its first commit),
/// and `hold`, what it decided holds the push of the stack branch
fn air_stack_view(landed: &[&str], hold: Option<Reason>) -> PatronusView {
  let landed: Vec<(&str, Vec<String>)> = landed.iter().map(|name| (*name, vec![format!("{name}-1")])).collect();
  whole_stack_view("air", "whole", &landed, hold)
}

/// The robot `whole` of the whole stack of air with `status`
fn whole_robot(status: PatronusRobotStatus) -> PatronusRobot {
  PatronusRobot {
    finish_time: (status == PatronusRobotStatus::Success).then_some(20),
    ..stack_robot("whole", "air", status, "stack-tip")
  }
}

/// What Push with Children of air does with its stack branch over the session `state` and `facts`: of the push view
/// (`BranchPush::children`, the push button and the menu) and of the rules of a push (`PushRules::stack_branch_along`,
/// the plan of the push)
fn air_stack_branch_along(state: &TableState, facts: &FactStore) -> (Option<StackBranchAlong>, Option<StackBranchAlong>) {
  use crate::derive::with_push_states;
  let pushes = View::new(1).diff(state, facts).unwrap().changes.pushes;
  let view = pushes.get("air").and_then(|push| push.children.clone()).and_then(|children| children.stack_branch);
  let rules = with_session_holds(facts, |holds| with_push_states(state, facts, holds, |rules| rules.stack_branch_along("air")));
  (view, rules)
}

/// `read` over the holds of the session of `facts`: its Patronus view and its robots
fn with_session_holds<T>(facts: &FactStore, read: impl FnOnce(crate::derive::Holds<'_>) -> T) -> T {
  let robots: Vec<PatronusRobot> = facts.robots.view().robots.values().cloned().collect();
  read(crate::derive::Holds {
    view: &facts.patronus.part().view,
    robots: &robots,
  })
}

/// What holds the push of the stack branch of a whole stack is decided of the table and the robots as they are now
/// (`derive::stack_hold_now`), never as a view derived for another table says it, by the push view of air and the rules of
/// its push alike: every branch of it landed only while each of them is what landed (air-child has a commit since);
/// a member parked since the view keeps it local, and one unparked since keeps it no more; a stack branch the sync built
/// since goes; its own robot only while it runs (it finished since the view)
#[test]
fn the_stack_branch_is_held_of_the_table_as_it_is_never_as_a_view_of_another_one_says() {
  let every_branch = ["air", "air-child", "air-side"];
  let landed = stack_landed_by_own("air", "whole");
  let merged = || vec![whole_robot(PatronusRobotStatus::Success)];
  let (state, facts) = air_stack_session(air_stack_view(&every_branch, Some(landed.clone())), merged(), &[], false);
  let held = |reason: &Reason| Some(StackBranchAlong::Held { reason: reason.clone() });
  assert_eq!(air_stack_branch_along(&state, &facts), (held(&landed), held(&landed)), "the table the view was derived for");
  let (state, facts) = air_stack_session(air_stack_view(&every_branch, Some(landed)), merged(), &[], true);
  let along = Some(StackBranchAlong::Along);
  assert_eq!(air_stack_branch_along(&state, &facts), (along.clone(), along.clone()), "air-child has a commit since");

  // a view from before air-child was parked: nothing held the stack branch then
  let (state, facts) = air_stack_session(air_stack_view(&[], None), Vec::new(), &[("air-child", "waits")], false);
  let parked = Reason::StackParked {
    root: "air".to_string(),
    member: "air-child".to_string(),
  };
  assert_eq!(air_stack_branch_along(&state, &facts), (held(&parked), held(&parked)), "parked since the view");
  // a view from when air-child was parked: unparked since, nothing holds it
  let (state, facts) = air_stack_session(air_stack_view(&[], Some(parked)), Vec::new(), &[], false);
  assert_eq!(air_stack_branch_along(&state, &facts), (along.clone(), along.clone()), "unparked since the view");
  // a view from when the sync could not build the stack branch: built since
  let not_built = Reason::StackNotBuilt {
    root: "air".to_string(),
    state: StackBranchState::Conflict,
    blocked_by: None,
    conflict: None,
    error: None,
  };
  let (state, facts) = air_stack_session(air_stack_view(&[], Some(not_built)), Vec::new(), &[], false);
  assert_eq!(air_stack_branch_along(&state, &facts), (along.clone(), along.clone()), "built since the view");

  let running = stack_held("air", "whole");
  let session = |status| air_stack_session(air_stack_view(&[], Some(running.clone())), vec![whole_robot(status)], &[], false);
  let (state, facts) = session(PatronusRobotStatus::Running);
  assert_eq!(air_stack_branch_along(&state, &facts), (held(&running), held(&running)), "its robot runs");
  let (state, facts) = session(PatronusRobotStatus::Failure);
  assert_eq!(air_stack_branch_along(&state, &facts), (along.clone(), along), "its robot ended since the view");
}

/// A running robot of `branch` on its tip
fn running_on(id: &str, branch: &str) -> PatronusRobot {
  PatronusRobot {
    source_revision: Some(format!("{branch}-tip")),
    ..robot(id, Some(branch), PatronusRobotStatus::Running, 30)
  }
}

/// The view the session shows of `state` and `facts` as it publishes it (`FactStore::publish_patronus`): of the robots,
/// the table and the remote branches as they are, the view a derivation answered while it is of them
fn published(state: &TableState, facts: &mut FactStore) -> PatronusView {
  let _ = facts.publish_patronus(state);
  facts.patronus.part().view.clone()
}

/// A robot started after the view the session shows (the robots as they are list it, the view a derivation answered of
/// the robots before doesn't know it) holds what it holds before a derivation of it answers: the view of the robots as
/// they are holds the push of its branch, and the robot of the whole stack its stack branch
#[test]
fn a_robot_started_since_the_view_holds_its_branch_and_the_stack_branch_before_a_derivation_answers() {
  use crate::derive::alone_push_of;
  use domain_policy::push::Verdict;

  let (state, mut facts) = air_stack_session(PatronusView::default(), Vec::new(), &[], false);
  let verdict = |facts: &FactStore| with_session_holds(facts, |holds| alone_push_of(&state, facts, holds, "air-side", None).map(|push| push.verdict));
  let _ = published(&state, &mut facts);
  assert_eq!(verdict(&facts), Some(Verdict::UpToDate), "nothing runs");
  assert_eq!(air_stack_branch_along(&state, &facts).1, Some(StackBranchAlong::Along));

  list_robots(&mut facts, 5, vec![running_on("side", "air-side"), whole_robot(PatronusRobotStatus::Running)]);
  let view = published(&state, &mut facts);
  assert_eq!(view.branches["air-side"].robot_id.as_deref(), Some("side"));
  assert_eq!(verdict(&facts), Some(Verdict::Held), "its own Safe Push runs");
  let held = Some(StackBranchAlong::Held {
    reason: stack_held("air", "whole"),
  });
  assert_eq!(air_stack_branch_along(&state, &facts), (held.clone(), held), "the robot of the whole stack runs");
}

/// A robot that ended since the view the session shows refuses no start and offers its restart: the robot of air-child
/// that landed air ran when the view was derived; it failed since, and before a derivation of the robots as they are
/// answers, the step of air offers its Safe Push, and the robot its restart
#[test]
fn a_robot_that_ended_since_the_view_refuses_no_start_and_offers_its_restart() {
  use domain_types::next_step::StepBlocker;

  let running = running_on("child", "air-child");
  let (state, mut facts) = air_stack_session(PatronusView::default(), vec![running.clone()], &[], false);
  let of_running = published(&state, &mut facts);
  let refusal = of_running.branches["air"].landing.start.clone();
  assert!(matches!(refusal, Some(Reason::Descendant { .. })), "{refusal:?}");
  let refused = |facts: &FactStore| {
    let step = merged_robot_steps_of(&state, facts).remove("air").map(|view| view.step);
    matches!(
      step.and_then(|step| step.safe_push),
      Some(StepBlocker::Rule {
        reason: Reason::Descendant { .. }
      })
    )
  };
  assert!(refused(&facts), "the robot of air-child runs");
  let table = facts.table.value().map(|table| BranchTable::clone(table)).unwrap();
  // the view a derivation answered while the robot ran
  answer_patronus(&mut facts, 4, of_running, &table);
  list_robots(
    &mut facts,
    5,
    vec![PatronusRobot {
      status: PatronusRobotStatus::Failure,
      finish_time: Some(40),
      ..running
    }],
  );
  let view = published(&state, &mut facts);
  assert_eq!(view.branches["air"].landing.start, None);
  assert!(view.restarts.contains_key("child"), "{:?}", view.restarts);
  assert!(!refused(&facts), "the robot of air-child ended");
}

/// The view the session shows reads the table as it is: a sync added air-top on air-side, whose Safe Push runs, and
/// air-top-child on it since the view of the table before was derived; before a derivation of this table answers, the
/// robot of air-top lands air-side and the branches below it, air-top-child is built on a branch that lands, and the
/// whole stack has a branch that lands a part of it
#[test]
fn the_view_reads_the_branches_of_the_table_as_it_is_before_a_derivation_of_it_answers() {
  let robots = vec![running_on("top", "air-top")];
  let (before, mut facts) = air_stack_session(PatronusView::default(), robots.clone(), &[], false);
  let of_before = published(&before, &mut facts);
  assert!(of_before.branches.get("air-side").and_then(|facts| facts.span.as_ref()).is_none(), "no air-top yet");
  let mut branches: Vec<(&str, Option<&str>, Vec<String>)> = vec![
    ("air-top-child", Some("air-top"), names(&["air-top-child-1"])),
    ("air-top", Some("air-side"), names(&["air-top-1"])),
  ];
  branches.extend(AIR_STACK.iter().map(|(name, parent)| (*name, *parent, air_commits(name, false))));
  let (state, mut facts) = stack_session(&branches, of_before, robots, &[]);
  let view = published(&state, &mut facts);
  let top = robot_of("air-top", "top");
  assert_eq!(view.branches["air-side"].span.as_ref().map(|span| &span.by), Some(&top));
  assert_eq!(view.branches["air"].span.as_ref().map(|span| &span.by), Some(&top));
  assert_eq!(view.branches["air-top-child"].landing_ancestor.as_ref(), Some(&top));
  assert_eq!(view.stacks["air"].landing_member.as_ref(), Some(&top));
}

/// The next step of a whole stack names what holds the push of its stack branch now (`NextStepView::hold`, read by the
/// page: "Open in Patronus" of a refused start, the island's push): its own robot while it runs, nothing once it ended,
/// whatever the view of another table says
#[test]
fn the_step_of_a_whole_stack_names_its_own_robot_only_while_it_holds_the_stack_branch() {
  let running = stack_held("air", "whole");
  let hold_of = |status| {
    let (state, facts) = air_stack_session(air_stack_view(&[], Some(running.clone())), vec![whole_robot(status)], &[], false);
    merged_robot_steps_of(&state, &facts).remove("stack:air").and_then(|step| step.hold)
  };
  assert_eq!(
    hold_of(PatronusRobotStatus::Running),
    Some(HoldView {
      kind: HoldKind::Own,
      by: whole_stack_robot_of("air", "whole"),
      carried: None,
    })
  );
  assert_eq!(hold_of(PatronusRobotStatus::Failure), None);
}
