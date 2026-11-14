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

/// Each unit of `plan` by its name, with its kind and what it says of its renames
fn units(plan: &ReviewPlan) -> Vec<(String, UnitKind, Option<RenameWait>)> {
  plan
    .units
    .iter()
    .map(|unit| (unit.name.clone().unwrap_or_default(), unit.kind.clone(), unit.renamed.as_ref().map(|renamed| renamed.wait)))
    .collect()
}

fn named(name: &str, kind: UnitKind, wait: Option<RenameWait>) -> (String, UnitKind, Option<RenameWait>) {
  (name.to_string(), kind, wait)
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
      named("totalOf", UnitKind::Added, None),
      named("sumOf", UnitKind::Removed, None),
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

/// openspec/specs/branch/review-units/spec.md, "Units: A body changed in a rename the index has not proven says so": the
/// plan sent while the index is asked says the rename is being checked, the plan the index answered without a proof
/// says it is not proven
#[test]
fn a_rename_the_index_is_asked_of_is_checking_until_it_answers() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add the ledger", &[(PATH, LEDGER)]);
  let head = repo.create_commit_with_files("Rename sumOf", &[(PATH, RENAMED)]);
  let wait = |plan: &ReviewPlan| plan.units[0].renamed.as_ref().map(|renamed| renamed.wait);
  let (stages, plan) = staged(&repo, &DeclaredModules, &range(&repo, &base, &head), wait, |_| {});
  assert_ne!(stages, []);
  assert!(stages.iter().all(|stage| *stage == (PlanWait::Index, Some(RenameWait::Checking))), "{stages:?}");
  assert_eq!(wait(&plan), Some(RenameWait::NotProven));
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
}
