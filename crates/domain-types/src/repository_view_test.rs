use crate::fixtures::{robot_of, stack_held, whole_stack_robot_of};
use crate::repository_view::{BranchLanding, BranchPatronus, HoldKind, HoldView, LandedByView, PatronusView, RobotLandedBranch, StackLanding, StackPatronus, WholeStackRest};
use crate::session::PatronusChange;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// What only Rust reads of the Patronus view, with every other field beside it
fn every_field() -> (BranchPatronus, StackPatronus, BTreeMap<String, Vec<RobotLandedBranch>>) {
  let branch = BranchPatronus {
    robot_id: Some("r1".to_owned()),
    landed_by: Some(LandedByView {
      by: robot_of("auth", "r1"),
      own: true,
      landed: vec!["a1".to_owned()],
      after: Vec::new(),
    }),
    landing: BranchLanding {
      hold: Some(HoldView {
        kind: HoldKind::Landed,
        by: robot_of("auth", "r1"),
        carried: None,
      }),
      replaces: Some("r0".to_owned()),
      ..BranchLanding::default()
    },
    ..BranchPatronus::default()
  };
  let stack = StackPatronus {
    robot_id: Some("whole".to_owned()),
    landing: StackLanding {
      hold: Some(stack_held("auth", "whole")),
      rest: WholeStackRest {
        members: vec!["ui".to_owned()],
        commit_count: 1,
        landed: vec!["auth".to_owned()],
        by: vec![whole_stack_robot_of("auth", "whole")],
        by_own_robot: false,
      },
      ..StackLanding::default()
    },
    ..StackPatronus::default()
  };
  let landed = BTreeMap::from([(
    "r1".to_owned(),
    vec![RobotLandedBranch {
      branch: "auth".to_owned(),
      own: true,
      after: Vec::new(),
    }],
  )]);
  (branch, stack, landed)
}

/// What only Rust reads of the Patronus view goes on no wire: the view, its patches (which the page and the mirror of
/// the IDE apply) leave out what landed a branch, the holds the view decided, what is left of a whole stack and what a
/// merged robot landed, and keep every other field; a view read from the wire has none of them
#[test]
fn what_only_rust_reads_of_the_patronus_view_goes_on_no_wire() {
  let (branch, stack, landed) = every_field();
  let view = PatronusView {
    branches: BTreeMap::from([("auth".to_owned(), branch.clone())]),
    stacks: BTreeMap::from([("auth".to_owned(), stack.clone())]),
    landed_by_robot: landed.clone(),
    ..PatronusView::default()
  };
  let change = PatronusChange {
    branches: view.branches.clone(),
    stacks: view.stacks.clone(),
    landed_by_robot: Some(landed),
    ..PatronusChange::default()
  };
  for wire in [serde_json::to_string(&view).unwrap(), serde_json::to_string(&change).unwrap()] {
    for field in ["landedBy", "landedByRobot", "\"hold\"", "\"rest\""] {
      assert!(!wire.contains(field), "{field} on the wire: {wire}");
    }
    for field in ["\"robotId\":\"r1\"", "\"replaces\":\"r0\"", "\"robotId\":\"whole\""] {
      assert!(wire.contains(field), "{field} off the wire: {wire}");
    }
  }
  let read: PatronusView = serde_json::from_str(&serde_json::to_string(&view).unwrap()).unwrap();
  let wire_branch = BranchPatronus {
    landed_by: None,
    landing: BranchLanding { hold: None, ..branch.landing },
    ..branch
  };
  let wire_stack = StackPatronus {
    landing: StackLanding {
      hold: None,
      rest: WholeStackRest::default(),
      ..stack.landing
    },
    ..stack
  };
  assert_eq!(
    read,
    PatronusView {
      branches: BTreeMap::from([("auth".to_owned(), wire_branch)]),
      stacks: BTreeMap::from([("auth".to_owned(), wire_stack)]),
      ..PatronusView::default()
    }
  );
}
