use crate::branch_table::{RemoteFact, RemoteFacts};
use crate::derive::{LandSources, decide_lands, land_answer, open_pull_requests_view, repository};
use crate::facts::FactStore;
use crate::patronus_view::patronus_view_test::chain;
use crate::state::View;
use crate::table::TableState;
use crate::table::table_test::{grouped, remote_status};
use domain_policy::parked::parked_state;
use domain_types::error::ApiError;
use domain_types::fixtures::pull_request;
use domain_types::landing::{Confirmation, FactWait, Land, LandFact, LandProfiles, PlannedEffect, ProfileChoice, StartRoute, Subject};
use domain_types::next_step::{LandingNodeKind, StepKind, StepRow};
use domain_types::patronus::{
  LandingAllowances, MissingApprovalsReason, MissingReviewGroup, PatronusProfile, PatronusPushMode, PatronusStartOptions, Person, ReviewReadiness, ReviewReadinessBranch,
};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestBase, PullRequestState, PullRequestView, ReviewCandidate};
use domain_types::repository_view::PatronusView;
use domain_types::session::{
  BranchPush, BranchSection, BranchStructure, ChildrenPush, Fact, FactState, LandedSection, Parked, ParkedMember, PullRequestsView, RowsView, StackBranchAlong, SubjectReadiness,
  WholeStackView,
};
use domain_types::stack::{BranchStack, StackBranchState, StackBranchUpdate};
use domain_types::{GroupedBranchInfo, SyncEvent};
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
  }
}

fn stack_branch(root: &str, members: &[&str]) -> SyncEvent {
  SyncEvent::StackBranchUpdate(Box::new(StackBranchUpdate {
    root: root.to_string(),
    branch_name: format!("test/stack/{root}"),
    members: members.iter().map(ToString::to_string).collect(),
    commit_count: 3,
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
/// parked and its section, and the rows of each section, so the page lays them out without a rule of its own
#[test]
fn a_grouping_places_every_branch_and_the_rows_of_each_section() {
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
  let rows = RowsView {
    main: names(&["docs", "session", "auth"]),
    parked: names(&["oauth", "login"]),
  };
  let mut view = View::new(1, "user");
  let patch = view.diff(&table, &FactStore::new()).unwrap();
  assert_eq!(patch.changes.rows.as_ref(), Some(&rows));
  assert_eq!(patch.changes.branches["auth"].structure, Some(structure("auth")));
  // `bd` reads the same rule over the same input
  assert_eq!(parked_state(&repository(&table), "oauth"), structure("oauth").parked);
}

/// A grouping that changes nothing of the structure sends none; one that parks a branch sends the structures that
/// changed and the rows; the stack branch of a root reports its parked member into the root's structure
#[test]
fn a_patch_carries_only_the_structures_that_changed() {
  let mut table = TableState::new();
  table.begin_sync();
  table.apply(grouping(tree(), &[], &["auth"]));
  let mut view = View::new(1, "user");
  let _ = view.diff(&table, &FactStore::new());
  table.apply(grouping(tree(), &[], &["auth"]));
  assert_eq!(view.diff(&table, &FactStore::new()).map(|patch| patch.changes.rows), None);

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
  assert_eq!(patch.changes.rows.map(|rows| rows.parked), Some(names(&["session"])));

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
  let answer = land_answer(&sources, &land).unwrap();
  assert_eq!(answer.decision.start_route(), Some(&StartRoute::Api { profile: None }));
  assert_eq!(
    answer.profiles,
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
  assert_eq!(land_answer(&sources, &gone), None);
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
  let robot = facts.robots.view().robots["r0"].clone();
  let branch = robot.branch_name.clone().unwrap();
  facts.apply(
    crate::derive::derive_budget_test::at(9),
    crate::facts::FactResult::Robots {
      read: crate::facts::RobotsRead::Poll,
      result: domain_types::patronus::PatronusRobotsResult {
        availability: domain_types::patronus::PatronusAvailability::Available,
        robots: vec![domain_types::patronus::PatronusRobot {
          status: domain_types::patronus::PatronusRobotStatus::Success,
          ..robot
        }],
        github_repository: None,
      },
    },
  );
  let second = view.diff(&state, &facts).unwrap();
  assert_eq!(second.changes.next_steps.keys().collect::<Vec<_>>(), [&branch]);
  assert_eq!(second.changes.next_steps[&branch].node.kind, LandingNodeKind::InMaster, "merged by its robot");
}
