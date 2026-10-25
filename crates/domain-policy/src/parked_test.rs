use crate::parked::{parked_state, whole_stack_parked};
use crate::stack_test::{repository, stacks_of};
use domain_types::session::{Parked, ParkedMember};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn lines(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
  entries.iter().map(|(name, note)| ((*name).to_string(), (*note).to_string())).collect()
}

fn parked(note: &str, with: Option<&str>) -> Parked {
  Parked {
    note: note.to_string(),
    with: with.map(str::to_string),
  }
}

/// login on auth, oauth on login, sso blocked (no effective base: its declared parent places it), docs alone
const CHAIN: [crate::stack_test::Spec; 5] = [
  ("auth", None, None),
  ("login", Some("auth"), Some("auth")),
  ("oauth", Some("login"), Some("login")),
  ("sso", None, Some("oauth")),
  ("docs", None, None),
];

/// A branch is parked by its own line, else by the nearest branch it is built on that has one, whose commits it carries,
/// a blocked branch by its declared parent; a branch apart is not
#[test]
fn a_branch_built_on_a_parked_one_is_parked_with_it() {
  let stacks = stacks_of(&CHAIN);
  let parked_lines = lines(&[("auth", "IJPL-1")]);
  let repo = repository(&stacks, &parked_lines);
  assert_eq!(parked_state(&repo, "auth"), Some(parked("IJPL-1", None)));
  assert_eq!(parked_state(&repo, "oauth"), Some(parked("IJPL-1", Some("auth"))));
  assert_eq!(parked_state(&repo, "sso"), Some(parked("IJPL-1", Some("auth"))));
  assert_eq!(parked_state(&repo, "docs"), None);
  // the nearest parked ancestor names it
  let both = lines(&[("auth", "IJPL-1"), ("login", "")]);
  assert_eq!(parked_state(&repository(&stacks, &both), "oauth"), Some(parked("", Some("login"))));
  // the line of a branch without commits here parks nothing but itself
  let ghost = lines(&[("ghost", "")]);
  assert_eq!(parked_state(&repository(&stacks, &ghost), "oauth"), None);
}

/// The chain that parks is the one the commits are built on: a branch whose declared parent has no commits is built on
/// the parent's parent and carries its commits, so it is parked with it; a cycle of bases parks nothing
#[test]
fn a_branch_is_parked_by_the_chain_it_is_built_on_not_the_declared_one() {
  let stacks = stacks_of(&[
    ("auth", None, None),
    ("sso", Some("auth"), Some("idle")),
    ("a", Some("b"), Some("b")),
    ("b", Some("a"), Some("a")),
  ]);
  let parked_lines = lines(&[("auth", "waits"), ("c", "")]);
  let repo = repository(&stacks, &parked_lines);
  assert_eq!(parked_state(&repo, "sso"), Some(parked("waits", Some("auth"))));
  assert_eq!(parked_state(&repo, "a"), None);
}

/// The first member of a stack branch that is parked keeps it local, with why; a stack without one is not
#[test]
fn the_first_parked_member_keeps_a_stack_branch_local() {
  let stacks = stacks_of(&CHAIN);
  let parked_lines = lines(&[("login", "IJPL-1")]);
  let repo = repository(&stacks, &parked_lines);
  assert_eq!(
    whole_stack_parked(&repo, ["auth", "login", "oauth"]),
    Some(ParkedMember {
      member: "login".to_string(),
      parked: Parked {
        note: "IJPL-1".to_string(),
        with: None,
      },
    })
  );
  assert_eq!(whole_stack_parked(&repo, ["oauth", "sso"]).map(|parked| parked.member).as_deref(), Some("oauth"));
  assert_eq!(whole_stack_parked(&repo, ["auth", "docs"]), None);
}
