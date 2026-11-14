//! The bodies that changed in renamed names alone (`renamed_uses`): told by their renames while the index has not
//! proven them, a body that changed otherwise, and no word once the index proved them

use crate::stages_test::staged;
use crate::test_support::{DeclaredModules, plan_by_accio, plan_of, range};
use crate::units::{RenameWait, RenamedName, RenamedUses, UnitKind};
use crate::{PlanWait, ReviewPlan};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const PATH: &str = "src/main/kotlin/probe/Ledger.kt";
const LEDGER: &str = include_str!("../../file-diffs/testdata/split-reformat/Ledger.new.kt");
/// `LEDGER` with its private `sumOf` renamed to `totalOf`, at its declaration and its three calls
const RENAMED: &str = include_str!("../../file-diffs/testdata/split-reformat/Ledger.renamed.kt");

/// A unit as a test reads it: its name, its kind, what its renamed uses wait on and the old name of its declaration
/// renamed with what that waits on
type Read = (String, UnitKind, Option<RenameWait>, Option<(String, RenameWait)>);

/// Each unit of `plan` by its name, with its kind and what it says of its renames
fn units(plan: &ReviewPlan) -> Vec<Read> {
  plan
    .units
    .iter()
    .map(|unit| {
      (
        unit.name.clone().unwrap_or_default(),
        unit.kind.clone(),
        unit.renamed.as_ref().map(|renamed| renamed.wait),
        unit.renamed_from.as_ref().map(|from| (from.old.clone(), from.wait)),
      )
    })
    .collect()
}

fn named(name: &str, kind: UnitKind, wait: Option<RenameWait>) -> Read {
  (name.to_string(), kind, wait, None)
}

/// The unit of the declaration `name` renamed from `old`, waiting on `wait`
fn renamed_from(name: &str, old: &str, wait: RenameWait) -> Read {
  (name.to_string(), UnitKind::Behaviour, None, Some((old.to_string(), wait)))
}

/// openspec/specs/branch/review-units/spec.md, "Units: A body changed in a rename the index has not proven says so": the
/// calls of a renamed private function, where no index proves the rename, are told by it, not proven; a body that
/// changed beside the rename is a body that changed
#[test]
fn a_body_changed_in_a_rename_no_index_proved_is_told_by_the_rename() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the ledger", &[(PATH, LEDGER)]);
  let head = repo.create_commit_with_files("Rename sumOf", &[(PATH, RENAMED)]);
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  let not_proven = Some(RenameWait::NotProven);
  assert_eq!(
    units(&plan),
    [
      named("balance", UnitKind::Behaviour, not_proven),
      named("balanceOn", UnitKind::Behaviour, not_proven),
      renamed_from("totalOf", "sumOf", RenameWait::NotProven),
      named("averageAmount", UnitKind::Behaviour, not_proven),
    ],
  );
  assert_eq!(
    plan.units[0].renamed,
    Some(RenamedUses {
      names: vec![RenamedName {
        old: "sumOf".to_string(),
        new: "totalOf".to_string(),
      }],
      wait: RenameWait::NotProven,
    }),
  );

  // the filter of `balanceOn` changed too: its body changed in more than the rename
  let changed = repo.create_commit_with_files(
    "Rename sumOf and count the day itself",
    &[(PATH, &RENAMED.replace("!it.date.isAfter(date)", "it.date.isBefore(date)"))],
  );
  let plan = plan_of(&repo, None, &range(&repo, &base, &changed)).unwrap();
  let balance_on = plan.units.iter().find(|unit| unit.name.as_deref() == Some("balanceOn")).unwrap();
  assert_eq!((&balance_on.kind, &balance_on.renamed), (&UnitKind::Behaviour, &None));
}

/// openspec/specs/branch/review-units/spec.md, "Units: A body changed in a rename the index has not proven says so"
/// and "Units: A declaration renamed where the index has not proven it is one unit": the plan sent while the index is
/// asked says the rename of the calls and of the declaration is being checked, the plan the index answered without a
/// proof says it is not proven, the same units
#[test]
fn a_rename_the_index_is_asked_of_is_checking_until_it_answers() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the ledger", &[(PATH, LEDGER)]);
  let head = repo.create_commit_with_files("Rename sumOf", &[(PATH, RENAMED)]);
  let waits = |plan: &ReviewPlan| {
    let declaration = plan.units.iter().find_map(|unit| unit.renamed_from.as_ref().map(|from| from.wait));
    (plan.units.len(), plan.units[0].renamed.as_ref().map(|renamed| renamed.wait), declaration)
  };
  let (stages, plan) = staged(&repo, &DeclaredModules, &range(&repo, &base, &head), waits, |_| {});
  assert_ne!(stages, []);
  let checking = Some(RenameWait::Checking);
  assert!(stages.iter().all(|stage| *stage == (PlanWait::Index, (4, checking, checking))), "{stages:?}");
  let not_proven = Some(RenameWait::NotProven);
  assert_eq!(waits(&plan), (4, not_proven, not_proven));
}

/// openspec/specs/branch/review-units/spec.md, "Units: A declaration renamed where the index has not proven it is one
/// unit": a function renamed with another body is the removal of the old one and the addition of the new one, as the
/// diff says it is more than a rename
#[test]
fn a_declaration_renamed_with_more_than_its_name_stays_removed_and_added() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the ledger", &[(PATH, LEDGER)]);
  let rounded = repo.create_commit_with_files(
    "Rename sumOf and round down",
    &[(
      PATH,
      &RENAMED.replace(
        "RoundingMode.HALF_UP)\n  }\n\n  private fun formatEntry",
        "RoundingMode.DOWN)\n  }\n\n  private fun formatEntry",
      ),
    )],
  );
  let plan = plan_of(&repo, None, &range(&repo, &base, &rounded)).unwrap();
  let declarations: Vec<Read> = units(&plan).into_iter().filter(|(name, ..)| name == "totalOf" || name == "sumOf").collect();
  assert_eq!(
    declarations,
    [named("totalOf", UnitKind::Added, None), named("sumOf", UnitKind::Removed, None)],
    "{:?}",
    units(&plan)
  );
}

/// openspec/specs/branch/review-units/spec.md, "Units: A body changed in a rename the index has not proven says so":
/// where the accio of this machine proves the rename, the calls say nothing of it, as the diffs pass them
#[test]
fn a_rename_the_index_proved_leaves_no_word_on_the_body() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the ledger", &[(PATH, LEDGER)]);
  let head = repo.create_commit_with_files("Rename sumOf", &[(PATH, RENAMED)]);
  let Some(plan) = plan_by_accio(&repo, &base, &head) else {
    return;
  };
  assert!(plan.reading.unproven.is_empty(), "{:?}", plan.reading);
  assert!(plan.units.iter().all(|unit| unit.renamed.is_none()), "{:?}", units(&plan));
  // the declaration of each name as without a rename: the index proved it, no unit says it
  let declarations: Vec<Read> = units(&plan).into_iter().filter(|(name, ..)| name == "totalOf" || name == "sumOf").collect();
  assert_eq!(declarations, [named("totalOf", UnitKind::Added, None), named("sumOf", UnitKind::Removed, None)]);
}

/// openspec/specs/branch/review-units/spec.md, "Units: A declaration renamed where the index has not proven it is one
/// unit": a class renamed is one unit, its members under their new owner paired with themselves; renamed with a member
/// that changed too it is the removal of the old one and the addition of the new one
#[test]
fn a_class_renamed_is_one_unit_with_its_members() {
  let store = |name: &str, opened: u8| format!("package a\n\nclass {name} {{\n  fun open(): Int = {opened}\n\n  fun close(): Int = 0\n}}\n");
  let user = |name: &str| format!("package a\n\nfun use(): Int = {name}().open()\n");
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the store", &[("src/a/Store.kt", &store("Store", 1)), ("src/a/Use.kt", &user("Store"))]);
  let head = repo.create_commit_with_files("Rename the store", &[("src/a/Store.kt", &store("Shop", 1)), ("src/a/Use.kt", &user("Shop"))]);
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  let use_renamed = named("use", UnitKind::Behaviour, Some(RenameWait::NotProven));
  assert_eq!(units(&plan), [renamed_from("Shop", "Store", RenameWait::NotProven), use_renamed]);

  let opened = repo.create_commit_with_files("Rename the store and open two", &[("src/a/Store.kt", &store("Shop", 2)), ("src/a/Use.kt", &user("Shop"))]);
  let plan = plan_of(&repo, None, &range(&repo, &base, &opened)).unwrap();
  let declarations: Vec<Read> = units(&plan).into_iter().filter(|(name, ..)| name != "use").collect();
  assert_eq!(
    declarations,
    [named("Shop", UnitKind::Added, None), named("Store", UnitKind::Removed, None)],
    "{:?}",
    units(&plan)
  );
}
