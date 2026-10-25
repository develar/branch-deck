use crate::derive::repository;
use crate::facts::FactStore;
use crate::state::View;
use crate::table::TableState;
use crate::table::table_test::grouped;
use domain_policy::parked::parked_state;
use domain_types::session::{BranchSection, BranchStructure, Parked, ParkedMember, RowsView};
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
  let mut view = View::new(1);
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
  let mut view = View::new(1);
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
