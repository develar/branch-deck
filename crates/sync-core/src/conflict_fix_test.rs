use crate::check::{CheckOutcome, CheckSession};
use crate::conflict_fix::{ConflictFix, FixBudget, FixStep, FixStepKind, SearchState};
use crate::regroup::regroup_test::repo;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicBool;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const PREFIX: &str = "test-prefix";

/// Stacked branches on: `base` adds a file and `feature`, stacked on it, changes its second line, which `other` (a
/// root) changes again in its last commit `(other) Change lib again`
fn other_needs_feature(other_commits: &[(&str, &str, &str)]) -> (TestRepo, String) {
  let repo = repo(true);
  repo.create_commit("(base) Add lib", "lib.txt", "a\nb\nc\n");
  repo.create_commit("(base/feature) Change lib", "lib.txt", "a\nB\nc\n");
  for (subject, file, content) in other_commits {
    repo.create_commit(subject, file, content);
  }
  let again = repo.create_commit("(other) Change lib again", "lib.txt", "a\nBB\nc\n");
  (repo, again)
}

fn session(repo: &TestRepo) -> CheckSession {
  CheckSession::open(repo.git_executor(), repo.path().to_str().unwrap(), PREFIX, &ObjectCache::disabled()).unwrap()
}

fn find_fix(repo: &TestRepo, budget: FixBudget) -> ConflictFix {
  session(repo).find_conflict_fix(None, budget, &AtomicBool::new(false)).unwrap()
}

fn step(kind: FixStepKind, commits: &[&String], from: &[&str], to: &str, fixes: &str) -> FixStep {
  FixStep {
    kind,
    commits: commits.iter().map(|id| (*id).clone()).collect(),
    from: from.iter().map(|name| (*name).to_string()).collect(),
    to: to.to_string(),
    fixes: fixes.to_string(),
  }
}

/// Every branch of a check of the fix's plan applies cleanly
fn assert_applies(repo: &TestRepo, fix: &ConflictFix) {
  assert_eq!(fix.components.len(), 1, "{fix:?}");
  let report = session(repo).check(Some(&fix.components[0].plan)).unwrap();
  for check in &report.branches {
    assert!(matches!(check.outcome, CheckOutcome::Ok { .. }), "{} after the fix: {:?}", check.name, check.outcome);
  }
}

#[test]
fn a_commit_moves_to_the_branch_whose_stack_has_what_it_needs() {
  let (repo, again) = other_needs_feature(&[("(other) Add other", "other.txt", "other\n")]);

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!(fix.search, SearchState::Complete);
  assert_eq!(fix.unresolved, Vec::<String>::new());
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Move, &[&again], &["other"], "feature", "other")]);
  assert_eq!(fix.components[0].fixed, ["other"]);
  assert_applies(&repo, &fix);
}

#[test]
fn a_single_commit_branch_is_restacked_on_the_branch_it_needs() {
  let (repo, _) = other_needs_feature(&[]);

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!(fix.components[0].steps, [step(FixStepKind::Restack, &[], &[], "feature", "other")]);
  assert_eq!(fix.components[0].rewritten, 0, "a restack rewrites no subject");
  assert_applies(&repo, &fix);
}

#[test]
fn an_unassigned_commit_the_branch_needs_is_pulled_in() {
  let repo = repo(true);
  let add = repo.create_commit("Add lib", "lib.txt", "a\nb\nc\n");
  repo.create_commit("(x) Change lib", "lib.txt", "a\nB\nc\n");

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!(fix.components[0].steps, [step(FixStepKind::PullIn, &[&add], &[], "x", "x")]);
  assert_applies(&repo, &fix);
}

#[test]
fn a_parent_that_needs_a_commit_of_its_child_gives_its_own_commit_to_the_child() {
  let repo = repo(true);
  repo.create_commit("(a) Add lib", "lib.txt", "1\n2\n3\n");
  repo.create_commit("(a/b) Change lib", "lib.txt", "one\n2\n3\n");
  let more = repo.create_commit("(a) Change lib more", "lib.txt", "ONE\n2\n3\n");

  let fix = find_fix(&repo, FixBudget::default());

  // moving the parent's commit up keeps both branches; pulling the child's down would empty the child
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Move, &[&more], &["a"], "b", "a")]);
  assert_applies(&repo, &fix);
}

#[test]
fn a_dangling_reference_moves_the_removal_to_the_branch_that_fixes_the_use() {
  let repo = repo(true);
  repo.create_commit_with_files(
    "(a) Add the helper",
    &[("Api.kt", "fun computeHelper() = 1\n"), ("Use.kt", "val usage = computeHelper()\n")],
  );
  repo.create_commit("(c) Stop using the helper", "Use.kt", "val usage = 2\n");
  let remove = repo.create_commit("(a) Remove the helper", "Api.kt", "\n");

  let fix = find_fix(&repo, FixBudget::default());

  // c changes a file of a, so it stacks on a; a removes a name c's change stops using, so the removal goes to c
  assert_eq!(
    fix.components[0].steps,
    [step(FixStepKind::Restack, &[], &[], "a", "c"), step(FixStepKind::Move, &[&remove], &["a"], "c", "a"),]
  );
  assert_eq!(fix.components[0].fixed, ["c", "a"]);
  assert_eq!(fix.components[0].emptied, Vec::<String>::new());
  assert_applies(&repo, &fix);
}

#[test]
fn a_commit_that_needs_two_long_branches_without_stacks_has_no_fix() {
  let repo = repo(false);
  // each branch changes its file in 4 commits: pulling 8 commits in is no fix, and no branch holds both
  for (branch, file) in [("y", "left.txt"), ("z", "right.txt")] {
    for version in 1..=4 {
      repo.create_commit(&format!("({branch}) Change {file} to {version}"), file, &format!("{version}\n2\n3\n"));
    }
  }
  repo.create_commit_with_files("(x) Change both", &[("left.txt", "4\nTWO\n3\n"), ("right.txt", "4\nTWO\n3\n")]);

  let fix = find_fix(&repo, FixBudget::default());

  assert!(fix.components.is_empty(), "{fix:?}");
  assert_eq!(fix.unresolved, ["x"]);
  assert_eq!(fix.search, SearchState::Complete);
}

#[test]
fn a_search_out_of_trials_says_so() {
  let (repo, _) = other_needs_feature(&[]);

  let fix = find_fix(
    &repo,
    FixBudget {
      trials: 0,
      ..FixBudget::default()
    },
  );

  assert_eq!(fix.search, SearchState::Budget);
  assert!(fix.components.is_empty());
  assert_eq!(fix.unresolved, ["other"]);
}

#[test]
fn nothing_to_fix_is_a_complete_search_without_components() {
  let repo = repo(true);
  repo.create_commit("(a) Add lib", "lib.txt", "1\n");

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!((fix.components.len(), fix.unresolved.len(), fix.search, fix.trials), (0, 0, SearchState::Complete, 0));
}
