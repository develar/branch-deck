use crate::fixtures::view_of_every_field;
use crate::repository_view::{BranchLanding, BranchPatronus, PatronusView, StackLanding, StackPatronus, WholeStackRest};
use crate::session::PatronusChange;
use pretty_assertions::assert_eq;

/// What only Rust reads of the Patronus view goes on no wire: the view, its patches (which the page and the mirror of
/// the IDE apply) leave out what landed a branch, the holds the view decided, what is left of a whole stack and what a
/// merged robot landed, and keep every other field; a view read from the wire has none of them
#[test]
fn what_only_rust_reads_of_the_patronus_view_goes_on_no_wire() {
  let view = view_of_every_field();
  let change = PatronusChange {
    branches: view.branches.clone(),
    stacks: view.stacks.clone(),
    landed_by_robot: Some(view.landed_by_robot.clone()),
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
  let wire_branch = |facts: &BranchPatronus| BranchPatronus {
    landed_by: None,
    landing: BranchLanding {
      hold: None,
      ..facts.landing.clone()
    },
    ..facts.clone()
  };
  let wire_stack = |facts: &StackPatronus| StackPatronus {
    landing: StackLanding {
      hold: None,
      rest: WholeStackRest::default(),
      ..facts.landing.clone()
    },
    ..facts.clone()
  };
  assert_eq!(
    read,
    PatronusView {
      branches: view.branches.iter().map(|(name, facts)| (name.clone(), wire_branch(facts))).collect(),
      stacks: view.stacks.iter().map(|(root, facts)| (root.clone(), wire_stack(facts))).collect(),
      ..PatronusView::default()
    }
  );
}
