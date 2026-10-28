//! The oracle of the page's words of a typed branch name (`src/test-support/branch-names.cases.json`): each state of
//! `domain_types::branch_names::check_branch_name` with the rules that decide it, as Rust answers it; the words are the
//! page's (`src/presenters/branch-name.test.ts`)

use crate::catalog_test::assert_catalog;
use domain_types::branch_names::{BranchNameCheck, BranchNameRules, check_branch_name};
use serde::Serialize;

/// A typed name, the rules it is checked by and what the check answers: a case of the oracle of the page
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NameCase {
  name: &'static str,
  rules: BranchNameRules,
  #[serde(flatten)]
  check: BranchNameCheck,
}

const FLAT: BranchNameRules = BranchNameRules { stacks: false, leaf_only: false };
const STACKS: BranchNameRules = BranchNameRules { stacks: true, leaf_only: false };
const LEAF: BranchNameRules = BranchNameRules { stacks: true, leaf_only: true };

/// Each state of a typed name, with the rules that decide it
const NAME_CASES: &[(&str, BranchNameRules)] = &[
  ("", FLAT),
  ("feature-auth", FLAT),
  ("  feature   auth  ", FLAT),
  ("a", FLAT),
  (" a ", FLAT),
  ("invalid@branch#name", FLAT),
  ("invalid@branch", STACKS),
  (".hidden", FLAT),
  ("auth/login", FLAT),
  ("auth/login", STACKS),
  ("auth/login", LEAF),
  ("login", LEAF),
  ("feature-", FLAT),
  ("-feature", FLAT),
  ("feature--auth", FLAT),
  ("feature..auth", FLAT),
  ("auth//login", STACKS),
  ("auth/", STACKS),
  ("v1.2", FLAT),
  ("snake_case", FLAT),
];

/// `UPDATE_BRANCH_NAME_CASES=1 cargo nextest run -p domain-policy -E 'test(/branch_name_cases/)'` writes it; without
/// the variable a stale file fails
#[test]
fn the_branch_name_cases_are_what_the_check_answers() {
  let cases: Vec<NameCase> = NAME_CASES
    .iter()
    .map(|&(name, rules)| NameCase {
      name,
      rules,
      check: check_branch_name(name, rules),
    })
    .collect();
  assert_catalog("../../src/test-support/branch-names.cases.json", "UPDATE_BRANCH_NAME_CASES", "branch_name_cases", &cases);
}
