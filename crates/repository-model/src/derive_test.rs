use crate::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use crate::derive::derive_budget_test::{answer_patronus, at, synced_facts};
use crate::derive::{LandSources, attention_of, blocking_parent, decide_lands, land_decision, open_pull_requests_view, repository};
use crate::facts::{FactResult, FactStore, RobotsRead};
use crate::patronus_view::patronus_view_test::chain;
use crate::state::View;
use crate::table::TableState;
use crate::table::table_test::{branch_status, grouped, remote_status, run_sync, synced};
use domain_policy::parked::parked_state;
use domain_types::actions::{ActionId, Availability};
use domain_types::error::ApiError;
use domain_types::fixtures::{pull_request, robot, robot_of, stack_robot};
use domain_types::landed::LandedBranch;
use domain_types::landing::{Confirmation, FactWait, Land, LandFact, LandProfiles, PlannedEffect, ProfileChoice, StartRoute, Subject};
use domain_types::next_step::{LandingNodeKind, StepKind, StepRow};
use domain_types::patronus::{
  LandingAllowances, MissingApprovalsReason, MissingReviewGroup, PatronusAvailability, PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotStatus, PatronusRobotsResult,
  PatronusStartOptions, Person, ReviewReadiness, ReviewReadinessBranch,
};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestBase, PullRequestState, PullRequestView, ReviewCandidate};
use domain_types::reason::Reason;
use domain_types::repository_view::{BranchLanding, BranchPatronus, HoldKind, HoldView, PatronusView, StackPatronus};
use domain_types::session::{
  BranchPush, BranchSection, BranchStatus, BranchStructure, ChildrenPush, Fact, FactState, LandedSection, Parked, ParkedMember, PullRequestsView, PushUpTo, StackBranchAlong,
  SubjectReadiness, WholeStackView,
};
use domain_types::stack::{BranchStack, StackBranchState, StackBranchUpdate};
use domain_types::{BranchSyncStatus, CommitSyncStatus, GroupedBranchInfo, RemoteStatusUpdate, SyncEvent};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn on(name: &str, parent: Option<&str>) -> GroupedBranchInfo {
  GroupedBranchInfo {
    stack: Some(BranchStack {
      declared_parent: parent.map(str::to_string),
      effective_base: parent.map(str::to_string),
      ..BranchStack::default()
    }),
    ..grouped(name, &[&format!("{name}-1")])
  }
}

fn grouping(branches: Vec<GroupedBranchInfo>, parked: &[(&str, &str)], stack_branches: &[&str]) -> SyncEvent {
  SyncEvent::BranchesGrouped {
    branches,
    baseline_branch: "origin/master".to_string(),
    stacks: true,
    issue_groups: BTreeMap::new(),
    stack_parents: BTreeMap::new(),
    parked: parked.iter().map(|(name, note)| ((*name).to_string(), (*note).to_string())).collect(),
    stack_branches: stack_branches.iter().map(ToString::to_string).collect(),
    target_branch: None,
  }
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
  let mut table = TableState::new();
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
  let mut view = View::new(1, "user");
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
  let mut table = TableState::new();
  table.begin_sync();
  table.apply(grouping(tree(), &[], &["auth"]));
  let mut view = View::new(1, "user");
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
  let mut table = TableState::new();
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
  let mut table = TableState::new();
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
  let mut table = TableState::new();
  table.begin_sync();
  table.apply(grouping(tree(), &[], &["auth"]));
  for (branch, unpushed) in [("auth", &["a"][..]), ("login", &["l"]), ("oauth", &["o"]), ("session", &[]), ("docs", &["d"])] {
    table.apply(SyncEvent::RemoteStatusUpdate(remote_status(branch, unpushed)));
  }
  let mut view = View::new(1, "user");
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
  let mut table = TableState::new();
  table.begin_sync();
  table.apply(grouping(tree(), &[], &[]));
  let rewritten = RemoteStatusUpdate {
    remote_exists: true,
    fast_forward: false,
    commits_behind: 1,
    ..remote_status("login", &["l"])
  };
  let rewritten = RemoteStatusUpdate {
    action: domain_policy::push::action(&rewritten),
    ..rewritten
  };
  table.apply(SyncEvent::RemoteStatusUpdate(rewritten));
  for (branch, unpushed) in [("auth", &["a"][..]), ("oauth", &["o"]), ("session", &["s"])] {
    table.apply(SyncEvent::RemoteStatusUpdate(remote_status(branch, unpushed)));
  }
  table.finish_sync(Ok(()));
  let mut view = View::new(1, "user");
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
      "user"
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
    stack_pull_requests: BTreeMap::new(),
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
  let sources = LandSources {
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
    target_conflict: None,
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
/// blockers of the decision and the gate of the app's start (`pushForAction`: `landing.push`, the view's push of an
/// action, derived by the rule). `by_robot`: a merged Safe Push landed auth whole, else the fetched baseline has every
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
  auth.action = domain_policy::push::action(&auth);
  let mut events = vec![grouping(branches.clone(), &[], &[])];
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
  let mut state = TableState::new();
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
  decide_landing(&mut view, &table, &session_remote(&table, &state), &robots, &HashMap::new(), &BTreeMap::new());
  answer_patronus(&mut facts, 3, view, &table);

  let patch = View::new(1, "user").diff(&state, &facts).unwrap();
  let decision = &patch.changes.lands["login"].safe_push;
  let decided = decision.effects.iter().find_map(|effect| match effect {
    PlannedEffect::Push { branches, .. } => Some(branches.clone()),
    _ => None,
  });
  let push = patch.changes.pushes.get("login").cloned().unwrap_or_default();
  let mut row = push.action.or(push.stack).unwrap_or_default();
  row.push("login".to_string());
  let gate = patch
    .changes
    .patronus
    .as_ref()
    .and_then(|patronus| patronus.branches.get("login"))
    .and_then(|login| login.landing.push.clone());
  (decided, row, decision.blockers.clone(), gate)
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

/// The view carries the next step and the node of every row and of every whole stack that forks
/// (`domain_policy::next_step`, `domain_policy::landing_node`), over the table and the facts of the session; a patch
/// carries only the subjects whose step or node changed
#[test]
fn the_view_carries_the_next_step_and_the_node_of_each_subject_and_a_patch_only_those_that_changed() {
  let (state, mut facts) = crate::derive::derive_budget_test::session();
  let mut view = View::new(1, "user");
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
  let mut view = View::new(1, "user");
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
/// fixed since, a parked one, one the fetched baseline has whole or a stack branch not rebuilt. A patch carries only the
/// subjects whose answer changed
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
  let mut state = TableState::new();
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
  let mut window = View::new(1, "user");
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
