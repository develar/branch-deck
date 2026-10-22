use crate::branch_table::stack_ancestors;
use domain_types::GroupedBranchInfo;
use pretty_assertions::assert_eq;
use test_utils::sync_fixtures::placed_branch;

pub(crate) fn placed(name: &str, effective_base: Option<&str>) -> GroupedBranchInfo {
  placed_branch(name, effective_base, effective_base)
}

/// The chain of effective bases, root first, of the branches of the table only: a parent the table lacks ends it, as
/// the page's `stackAncestors` does; a cycle ends too
#[test]
fn the_ancestors_of_a_branch_are_the_chain_of_its_bases_in_the_table() {
  let branches = [
    placed("oauth", Some("login")),
    placed("login", Some("auth")),
    placed("auth", None),
    placed("orphan", Some("gone")),
    placed("a", Some("b")),
    placed("b", Some("a")),
  ];
  assert_eq!(stack_ancestors(&branches, "oauth"), ["auth", "login"]);
  assert_eq!(stack_ancestors(&branches, "auth"), Vec::<String>::new());
  assert_eq!(stack_ancestors(&branches, "orphan"), Vec::<String>::new());
  assert_eq!(stack_ancestors(&branches, "a"), ["b"]);
}
