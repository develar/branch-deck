use crate::derive::{open_pull_requests_view, repository};
use crate::facts::FactStore;
use crate::state::View;
use crate::table::TableState;
use crate::table::table_test::{grouped, remote_status};
use domain_policy::parked::parked_state;
use domain_types::fixtures::pull_request;
use domain_types::patronus::{MissingApprovalsReason, MissingReviewGroup, Person, ReviewReadiness};
use domain_types::pull_request::{PullRequest, PullRequestBase, PullRequestState, PullRequestView, ReviewCandidate};
use domain_types::session::{BranchPush, BranchSection, BranchStructure, ChildrenPush, Parked, ParkedMember, PullRequestsView, RowsView, StackBranchAlong, WholeStackView};
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
