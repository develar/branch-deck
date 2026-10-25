use crate::facts::{Placed, Repository};
use crate::stack::{build_ancestors, build_parent, children, declared_parent, descendants, main_line};
use domain_types::stack::BranchStack;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// A branch of a table: its name, its effective base and its declared parent
pub(crate) type Spec = (&'static str, Option<&'static str>, Option<&'static str>);

pub(crate) fn stacks_of(specs: &[Spec]) -> Vec<(&'static str, BranchStack)> {
  specs
    .iter()
    .map(|&(name, effective_base, declared_parent)| {
      let stack = BranchStack {
        declared_parent: declared_parent.map(str::to_string),
        effective_base: effective_base.map(str::to_string),
        ..BranchStack::default()
      };
      (name, stack)
    })
    .collect()
}

pub(crate) fn repository<'a>(stacks: &'a [(&'static str, BranchStack)], parked: &'a BTreeMap<String, String>) -> Repository<'a> {
  Repository::new(stacks.iter().map(|(name, stack)| Placed { name, stack: Some(stack) }), parked)
}

/// A branch built on its declared parent: `(name, parent)`
const fn on(name: &'static str, parent: Option<&'static str>) -> Spec {
  (name, parent, parent)
}

/// oauth on login on auth, session on auth, docs alone, in the order of the table (newest first)
const TREE: [Spec; 5] = [
  on("oauth", Some("login")),
  on("docs", None),
  on("login", Some("auth")),
  on("session", Some("auth")),
  on("auth", None),
];

/// The children of a branch are the branches that declare it, in the order of the table; the descendants each after its
/// parent, depth-first from the last child
#[test]
fn children_and_descendants_follow_the_declared_parents_in_the_order_of_the_table() {
  let (stacks, parked) = (stacks_of(&TREE), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  assert_eq!(children(&repo, "auth"), ["login", "session"]);
  assert_eq!(children(&repo, "docs"), [] as [&str; 0]);
  assert_eq!(descendants(&repo, "auth"), ["login", "session", "oauth"]);
  assert_eq!(descendants(&repo, "login"), ["oauth"]);
  assert_eq!(descendants(&repo, "oauth"), [] as [&str; 0]);
  assert_eq!(declared_parent(&repo, "oauth"), Some("login"));
  assert_eq!(declared_parent(&repo, "auth"), None);
}

/// D8: a blocked branch (no effective base) whose declared parent has no commits is a root of its own stack, as the user
/// declared it, never a branch of the stack its build detail would name; it is still built on what it carries
#[test]
fn a_declared_parent_the_table_lacks_makes_a_root() {
  // C declares B, which has no commits; the sync built nothing of C
  let (stacks, parked) = (stacks_of(&[("C", None, Some("B")), ("A", None, None)]), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  assert_eq!(declared_parent(&repo, "C"), None);
  assert_eq!(children(&repo, "A"), [] as [&str; 0]);
  assert_eq!(descendants(&repo, "A"), [] as [&str; 0]);
  assert_eq!(build_parent(&repo, "C"), Some("B"));
  // a branch never stacks on itself
  let (stacks, parked) = (stacks_of(&[on("A", Some("A"))]), BTreeMap::new());
  assert_eq!(declared_parent(&repository(&stacks, &parked), "A"), None);
}

/// The ancestors are the chain of the effective bases, root first, of the branches of the table only: a base the table
/// lacks ends it; a cycle ends too
#[test]
fn the_ancestors_are_the_chain_of_the_effective_bases_in_the_table() {
  let specs = [
    on("oauth", Some("login")),
    on("login", Some("auth")),
    on("auth", None),
    on("orphan", Some("gone")),
    on("a", Some("b")),
    on("b", Some("a")),
    // built on auth, declaring login, which has no commits here
    ("sso", Some("auth"), Some("ghost")),
  ];
  let (stacks, parked) = (stacks_of(&specs), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  assert_eq!(build_ancestors(&repo, "oauth"), ["auth", "login"]);
  assert_eq!(build_ancestors(&repo, "auth"), [] as [&str; 0]);
  assert_eq!(build_ancestors(&repo, "orphan"), [] as [&str; 0]);
  assert_eq!(build_ancestors(&repo, "a"), ["b"]);
  assert_eq!(build_ancestors(&repo, "sso"), ["auth"]);
}

/// The main line takes at each fork the child with the longest chain, the last of equal ones, as lane 0 of the stack
/// graph does; below a branch with nothing stacked on it there is none
#[test]
fn the_main_line_takes_the_longest_chain_at_each_fork_and_the_last_of_equal_ones() {
  let (stacks, parked) = (stacks_of(&TREE), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  assert_eq!(main_line(&repo, "auth"), ["login", "oauth"]);
  assert_eq!(main_line(&repo, "login"), ["oauth"]);
  assert_eq!(main_line(&repo, "docs"), [] as [&str; 0]);
  // two children of equal chains: the last in the order of the table continues the line
  let (stacks, parked) = (stacks_of(&[on("b", Some("root")), on("a", Some("root")), on("root", None)]), BTreeMap::new());
  assert_eq!(main_line(&repository(&stacks, &parked), "root"), ["a"]);
}

/// A cycle of declared parents ends the descendants and the main line: every branch once
#[test]
fn a_cycle_of_declared_parents_ends_the_descendants_and_the_main_line() {
  let (stacks, parked) = (stacks_of(&[on("a", Some("b")), on("b", Some("a"))]), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  assert_eq!(descendants(&repo, "a"), ["b"]);
  assert_eq!(main_line(&repo, "a"), ["b"]);
}
