use crate::declared_needs::{NeedFacts, declared_refusals};
use domain_types::landing::{LandingFix, LandingRefusal, NeedState};
use domain_types::repository_view::RobotOf;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn declared(lines: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
  lines
    .iter()
    .map(|(branch, needs)| ((*branch).to_string(), needs.iter().map(|need| (*need).to_string()).collect()))
    .collect()
}

fn states(lines: &[(&str, NeedState)]) -> BTreeMap<String, NeedFacts> {
  lines.iter().map(|(need, state)| ((*need).to_string(), NeedFacts::of(state.clone()))).collect()
}

fn chains(lines: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
  lines
    .iter()
    .map(|(branch, chain)| ((*branch).to_string(), chain.iter().map(|parent| (*parent).to_string()).collect()))
    .collect()
}

/// A refusal of a declared need as the table reads it: the branch, what it needs, the state of that branch, the
/// branch it inherits the need from and its fix
type Listed<'a> = (&'a str, &'a str, &'a NeedState, Option<&'a str>, Option<&'a LandingFix>);

/// `Listed` of each refusal, in order
fn listed(refusals: &BTreeMap<String, Vec<LandingRefusal>>) -> Vec<Listed<'_>> {
  refusals
    .values()
    .flatten()
    .map(|refusal| match refusal {
      LandingRefusal::UnlandedNeed {
        branch,
        needs,
        state,
        inherited_from,
        fix,
        ..
      } => (branch.as_str(), needs.as_str(), state, inherited_from.as_deref(), fix.as_ref()),
      other => panic!("not a declared need: {other:?}"),
    })
    .collect()
}

#[test]
fn a_branch_that_needs_an_active_branch_is_refused_its_own_need_without_a_fix() {
  let refusals = declared_refusals(
    &declared(&[("auth", &["docs"])]),
    &states(&[("docs", NeedState::Active)]),
    &chains(&[("auth", &[]), ("docs", &[])]),
  );
  assert_eq!(listed(&refusals), vec![("auth", "docs", &NeedState::Active, None, None)]);
}

#[test]
fn a_need_of_a_branch_that_landed_refuses_nothing() {
  // a sync dropped the line of an integrated need, so no state is known of it
  let refusals = declared_refusals(&declared(&[("auth", &["docs"])]), &states(&[]), &chains(&[("auth", &[])]));
  assert_eq!(listed(&refusals), Vec::new());
}

#[test]
fn every_state_of_a_needed_branch_refuses_the_start_and_names_itself() {
  let unreadable = NeedState::CompanionUnreadable {
    companion: "ci-config".to_string(),
    reason: "no such directory".to_string(),
  };
  let refusals = declared_refusals(
    &declared(&[("auth", &["docs", "ci-config:test-data", "gone"])]),
    &states(&[("docs", NeedState::Unapplied), ("gone", NeedState::Missing), ("ci-config:test-data", unreadable.clone())]),
    &chains(&[("auth", &[])]),
  );
  assert_eq!(
    listed(&refusals),
    vec![
      ("auth", "docs", &NeedState::Unapplied, None, None),
      ("auth", "ci-config:test-data", &unreadable, None, None),
      ("auth", "gone", &NeedState::Missing, None, None),
    ]
  );
}

#[test]
fn a_branch_built_on_one_that_needs_inherits_the_need_and_unstacking_it_is_the_fix() {
  // ui is built on auth, which needs docs: one robot lands both, so a start of ui lands them out of order
  let refusals = declared_refusals(
    &declared(&[("auth", &["docs"])]),
    &states(&[("docs", NeedState::Active)]),
    &chains(&[("auth", &[]), ("ui", &["auth"]), ("docs", &[])]),
  );
  let unstack = LandingFix::Unstack {
    branch: "ui".to_string(),
    from: "auth".to_string(),
  };
  assert_eq!(
    listed(&refusals),
    vec![
      ("auth", "docs", &NeedState::Active, None, None),
      ("ui", "docs", &NeedState::Active, Some("auth"), Some(&unstack)),
    ]
  );
}

#[test]
fn a_need_of_a_branch_further_down_the_chain_is_inherited_once_and_the_nearest_owner_names_the_fix() {
  // top is built on ui, which is built on auth; both auth and ui need docs
  let refusals = declared_refusals(
    &declared(&[("auth", &["docs"]), ("ui", &["docs"])]),
    &states(&[("docs", NeedState::Active)]),
    &chains(&[("auth", &[]), ("ui", &["auth"]), ("top", &["ui", "auth"])]),
  );
  // of top the need is listed once, from the nearest branch that declared it: unstacking it from ui lands it alone
  let unstack = |branch: &str, from: &str| LandingFix::Unstack {
    branch: branch.to_string(),
    from: from.to_string(),
  };
  assert_eq!(
    listed(&refusals),
    vec![
      ("auth", "docs", &NeedState::Active, None, None),
      ("top", "docs", &NeedState::Active, Some("ui"), Some(&unstack("top", "ui"))),
      ("ui", "docs", &NeedState::Active, None, None),
    ]
  );
}

#[test]
fn a_branch_that_is_needed_by_the_branch_it_is_built_on_inherits_no_need_of_itself() {
  // auth needs ui, and ui is built on auth: one robot lands them together, so the order holds
  let refusals = declared_refusals(
    &declared(&[("auth", &["ui"])]),
    &states(&[("ui", NeedState::Active)]),
    &chains(&[("auth", &[]), ("ui", &["auth"])]),
  );
  assert_eq!(listed(&refusals), vec![("auth", "ui", &NeedState::Active, None, None)]);
}

#[test]
fn two_branches_that_need_each_other_are_each_refused_once() {
  let refusals = declared_refusals(
    &declared(&[("auth", &["docs"]), ("docs", &["auth"])]),
    &states(&[("auth", NeedState::Active), ("docs", NeedState::Active)]),
    &chains(&[("auth", &[]), ("docs", &[])]),
  );
  assert_eq!(
    listed(&refusals),
    vec![("auth", "docs", &NeedState::Active, None, None), ("docs", "auth", &NeedState::Active, None, None),]
  );
}

#[test]
fn a_robot_that_lands_the_needed_branch_already_is_named_by_the_refusal() {
  let robot = RobotOf {
    branch: "docs".to_string(),
    robot_id: "robot-1".to_string(),
    whole_stack: false,
  };
  let mut facts = states(&[("docs", NeedState::Active)]);
  facts.get_mut("docs").unwrap().landing_in = Some(robot.clone());
  let refusals = declared_refusals(&declared(&[("auth", &["docs"])]), &facts, &chains(&[("auth", &[])]));
  let landing = refusals["auth"].first().unwrap();
  assert!(matches!(landing, LandingRefusal::UnlandedNeed { landing_in: Some(of), .. } if *of == robot), "{landing:?}");
  assert_eq!(
    landing.reason(),
    "you declared that docs lands before auth; docs is landing in its Safe Push, which tests it without this start"
  );
}

#[test]
fn a_branch_of_another_worktree_keeps_its_declared_need_although_no_chain_names_it() {
  let refusals = declared_refusals(&declared(&[("elsewhere", &["docs"])]), &states(&[("docs", NeedState::Active)]), &chains(&[("auth", &[])]));
  assert_eq!(listed(&refusals), vec![("elsewhere", "docs", &NeedState::Active, None, None)]);
}

#[test]
fn nothing_declared_refuses_nothing() {
  assert_eq!(
    declared_refusals(&BTreeMap::new(), &states(&[("docs", NeedState::Active)]), &chains(&[("auth", &[])])),
    BTreeMap::new()
  );
}
