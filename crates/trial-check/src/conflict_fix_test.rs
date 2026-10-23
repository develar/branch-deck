use crate::check::{CheckOutcome, CheckSession};
use crate::conflict_fix::{ConflictFix, FixBudget, FixRun, FixStep, FixStepKind, SearchState};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::regroup::RegroupPlan;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, baseline_repo};

const PREFIX: &str = "test-prefix";

/// Stacked branches on: `base` adds a file and `feature`, stacked on it, changes its second line, which `other` (a
/// root) changes again in its last commit `(other) Change lib again`
fn other_needs_feature(other_commits: &[(&str, &str, &str)]) -> (TestRepo, String) {
  let repo = baseline_repo(true);
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

fn run(budget: FixBudget) -> FixRun<'static> {
  FixRun {
    budget,
    started: Instant::now(),
    cancel: Box::leak(Box::new(AtomicBool::new(false))),
    progress: &|_| {},
  }
}

fn find_fix(repo: &TestRepo, budget: FixBudget) -> ConflictFix {
  session(repo).find_conflict_fix(None, None, &run(budget)).unwrap()
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
  let repo = baseline_repo(true);
  let add = repo.create_commit("Add lib", "lib.txt", "a\nb\nc\n");
  repo.create_commit("(x) Change lib", "lib.txt", "a\nB\nc\n");

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!(fix.components[0].steps, [step(FixStepKind::PullIn, &[&add], &[], "x", "x")]);
  assert_applies(&repo, &fix);
}

#[test]
fn a_parent_that_needs_a_commit_of_its_child_gives_its_own_commit_to_the_child() {
  let repo = baseline_repo(true);
  repo.create_commit("(a) Add lib", "lib.txt", "1\n2\n3\n");
  repo.create_commit("(a/b) Change lib", "lib.txt", "one\n2\n3\n");
  let more = repo.create_commit("(a) Change lib more", "lib.txt", "ONE\n2\n3\n");

  let fix = find_fix(&repo, FixBudget::default());

  // moving the parent's commit up keeps both branches; pulling the child's down would empty the child
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Move, &[&more], &["a"], "b", "a")]);
  assert_applies(&repo, &fix);
}

/// `a` adds a helper and its use; `c` stops using it; `a` removes it: the commit that removes it
fn helper_removed_after_its_use(repo: &TestRepo) -> String {
  repo.create_commit_with_files(
    "(a) Add the helper",
    &[("Api.kt", "fun computeHelper() = 1\n"), ("Use.kt", "val usage = computeHelper()\n")],
  );
  repo.create_commit("(c) Stop using the helper", "Use.kt", "val usage = 2\n");
  repo.create_commit("(a) Remove the helper", "Api.kt", "\n")
}

/// c changes a file of a, so it stacks on a; a removes a name c's change stops using, so the removal goes to c
fn assert_the_removal_goes_to_c(fix: &ConflictFix, remove: &String) {
  assert_eq!(
    fix.components[0].steps,
    [step(FixStepKind::Restack, &[], &[], "a", "c"), step(FixStepKind::Move, &[remove], &["a"], "c", "a"),]
  );
}

#[test]
fn a_dangling_reference_moves_the_removal_to_the_branch_that_fixes_the_use() {
  let repo = baseline_repo(true);
  let remove = helper_removed_after_its_use(&repo);

  let fix = find_fix(&repo, FixBudget::default());

  assert_the_removal_goes_to_c(&fix, &remove);
  assert_eq!(fix.components[0].fixed, ["c", "a"]);
  assert_eq!(fix.components[0].emptied, Vec::<String>::new());
  assert_applies(&repo, &fix);
}

#[test]
fn a_commit_that_needs_two_long_branches_without_stacks_has_no_fix() {
  let repo = baseline_repo(false);
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
  let repo = baseline_repo(true);
  repo.create_commit("(a) Add lib", "lib.txt", "1\n");

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!((fix.components.len(), fix.unresolved.len(), fix.search, fix.trials), (0, 0, SearchState::Complete, 0));
}

/// With the symbols off, a move still takes what the moved commit's dependents need and the dangling fix is found as
/// with them on: the names come from the lines, not the syntax trees
#[test]
fn the_fixes_are_the_same_with_the_symbols_off() {
  let (stacked, again) = other_needs_feature(&[("(other) Add other", "other.txt", "other\n")]);
  stacked.set_config("branchdeck.symbols", "off").unwrap();
  let fix = find_fix(&stacked, FixBudget::default());
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Move, &[&again], &["other"], "feature", "other")]);
  assert_applies(&stacked, &fix);

  let repo = baseline_repo(true);
  repo.set_config("branchdeck.symbols", "off").unwrap();
  let remove = helper_removed_after_its_use(&repo);
  let fix = find_fix(&repo, FixBudget::default());
  assert_the_removal_goes_to_c(&fix, &remove);
  assert_applies(&repo, &fix);
}

/// The processes `git` counted for `subcommand`
fn processes(git: &GitCommandExecutor, subcommand: &str) -> u32 {
  git.process_stats().unwrap().by_command().get(subcommand).map_or(0, |stats| stats.count)
}

/// The parent `ghost` has no commits but a virtual branch, whose commit left HEAD: before the branches are placed, every
/// check detects whether that branch is integrated. A plan changes which parents have commits, never the tips, so the
/// trials of a search repeat neither the detection, nor the read of the commit the branches start on, nor the read of
/// the commits' changes (`ObjectCache::disabled` reads them again on every ask)
#[test]
fn the_trials_of_a_search_detect_a_parent_without_commits_once() {
  let (repo, again) = other_needs_feature(&[("(other) Add other", "other.txt", "other\n")]);
  let head = repo.head();
  let lost = repo.create_commit("(ghost) Add ghost", "ghost.txt", "ghost\n");
  repo.reset_hard(&head).unwrap();
  repo.create_branch_at(&format!("{PREFIX}/virtual/ghost"), &lost).unwrap();
  repo.create_commit("(ghost/child) Add child", "child.txt", "child\n");
  let path = repo.path().to_str().unwrap();

  let git = repo.git_executor().with_process_stats("test");
  CheckSession::open(&git, path, PREFIX, &ObjectCache::disabled()).unwrap().check(None).unwrap();
  let check = (processes(&git, "rev-parse"), processes(&git, "log"));

  let git = repo.git_executor().with_process_stats("test");
  let session = CheckSession::open(&git, path, PREFIX, &ObjectCache::disabled()).unwrap();
  let fix = session.find_conflict_fix(None, None, &run(FixBudget::default())).unwrap();

  assert_eq!(fix.components[0].steps, [step(FixStepKind::Move, &[&again], &["other"], "feature", "other")]);
  assert!(fix.trials >= 3, "{fix:?}");
  // the baseline of the detection resolved once, its notes read once, and one `log -p` for the changes of the commits
  assert_eq!((processes(&git, "rev-parse"), processes(&git, "log")), (check.0, check.1 + 1));
}

#[test]
fn a_leaf_whose_commit_needs_two_other_leaves_is_restacked_on_their_chain_in_one_trial() {
  let repo = baseline_repo(true);
  repo.create_commit("(root) Add lib", "lib.txt", "1\n2\n3\n4\n5\n");
  repo.create_commit("(root/a) Change the first line", "lib.txt", "one\n2\n3\n4\n5\n");
  repo.create_commit("(root/b) Change the last line", "lib.txt", "one\n2\n3\n4\nfive\n");
  // c's one commit changes the lines of both a and b: no branch holds both, a chain of them does
  repo.create_commit("(root/c) Change both lines", "lib.txt", "ONE\n2\n3\n4\nFIVE\n");

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!(fix.search, SearchState::Complete);
  assert_eq!(fix.trials, 1, "the direct answer verified at once: {fix:?}");
  assert_eq!(
    fix.components[0].steps,
    [step(FixStepKind::Restack, &[], &["root"], "a", "b"), step(FixStepKind::Restack, &[], &["root"], "b", "c")]
  );
  assert_eq!(fix.unresolved, Vec::<String>::new());
  assert_applies(&repo, &fix);
}

#[test]
fn a_search_out_of_budget_ends_with_the_best_plan_so_far() {
  let (repo, _) = other_needs_feature(&[("(other) Add other", "other.txt", "other\n")]);
  // a second conflict, of a branch of one commit: the direct answer restacks it
  repo.create_commit("(base2) Add lib2", "lib2.txt", "a\nb\nc\n");
  repo.create_commit("(base2/feature2) Change lib2", "lib2.txt", "a\nB\nc\n");
  repo.create_commit("(other2) Change lib2 again", "lib2.txt", "a\nBB\nc\n");

  let fix = find_fix(
    &repo,
    FixBudget {
      trials: 1,
      ..FixBudget::default()
    },
  );

  assert_eq!(fix.search, SearchState::Budget);
  assert_eq!(fix.trials, 1);
  assert_eq!(fix.components.len(), 1, "{fix:?}");
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Restack, &[], &[], "feature2", "other2")]);
  assert_eq!(fix.unresolved, ["other"]);
}

#[test]
fn a_search_out_of_time_stops_before_its_first_trial() {
  let (repo, _) = other_needs_feature(&[]);
  let lines = std::sync::Mutex::new(Vec::new());
  let run = FixRun {
    budget: FixBudget {
      time: std::time::Duration::ZERO,
      ..FixBudget::default()
    },
    started: Instant::now(),
    cancel: &AtomicBool::new(false),
    progress: &|line| lines.lock().unwrap().push(line.to_string()),
  };

  let fix = session(&repo).find_conflict_fix(None, None, &run).unwrap();

  assert_eq!((fix.search, fix.trials), (SearchState::Budget, 0));
  assert_eq!(fix.unresolved, ["other"]);
  let lines = lines.into_inner().unwrap();
  assert_eq!(lines.len(), 1, "{lines:?}");
  assert!(lines[0].starts_with("base check: 1 conflicting branch ("), "{lines:?}");
}

#[test]
fn each_trial_is_a_line_of_progress() {
  let (repo, _) = other_needs_feature(&[("(other) Add other", "other.txt", "other\n")]);
  let lines = std::sync::Mutex::new(Vec::new());
  let run = FixRun {
    progress: &|line| lines.lock().unwrap().push(line.to_string()),
    ..run(FixBudget::default())
  };

  let fix = session(&repo).find_conflict_fix(None, None, &run).unwrap();

  let lines = lines.into_inner().unwrap();
  assert_eq!(lines.len(), 1 + fix.trials as usize, "{lines:?}");
  assert!(
    lines.iter().any(|line| line.starts_with("trial 1/24: move 1 commit of other to feature … verified (")),
    "{lines:?}"
  );
}

#[test]
fn a_leaf_on_a_parent_of_its_own_keeps_it_and_the_parent_is_restacked_on_what_the_leaf_needs() {
  let repo = baseline_repo(true);
  repo.create_commit("(root) Add lib", "lib.txt", "1\n2\n3\n4\n5\n");
  repo.create_commit("(root/a) Change the first line", "lib.txt", "one\n2\n3\n4\n5\n");
  repo.create_commit("(root/p) Change the last line", "lib.txt", "one\n2\n3\n4\nfive\n");
  // c, stacked on p, changes the lines of p and of a, a sibling of p: c on a would lose p
  repo.create_commit("(p/c) Change both lines", "lib.txt", "ONE\n2\n3\n4\nFIVE\n");

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!(fix.trials, 1, "{fix:?}");
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Restack, &[], &["root"], "a", "p")]);
  assert_eq!(fix.components[0].fixed, ["c"]);
  assert_applies(&repo, &fix);
}

#[test]
fn a_restack_on_top_of_a_plan_keeps_the_parent_the_plan_gives() {
  let repo = baseline_repo(true);
  repo.create_commit("(root) Add lib", "lib.txt", "1\n2\n3\n4\n5\n");
  repo.create_commit("(root/a) Change the first line", "lib.txt", "one\n2\n3\n4\n5\n");
  repo.create_commit("(root/p) Change the last line", "lib.txt", "one\n2\n3\n4\nfive\n");
  let leaf = repo.create_commit("(c) Change both lines", "lib.txt", "ONE\n2\n3\n4\nFIVE\n");
  // the user's plan stacks c on p, as `bd check --fix --plan` gets it
  let plan = RegroupPlan {
    branches: std::iter::once(("p/c".to_string(), vec![leaf])).collect(),
    ..RegroupPlan::default()
  };

  let fix = session(&repo).find_conflict_fix(Some(&plan), None, &run(FixBudget::default())).unwrap();

  assert_eq!(fix.trials, 1, "{fix:?}");
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Restack, &[], &["root"], "a", "p")]);
  let paths: Vec<&String> = fix.components[0].plan.branches.keys().collect();
  assert_eq!(paths, ["a/p", "p/c"]);
  assert_applies(&repo, &fix);
}
