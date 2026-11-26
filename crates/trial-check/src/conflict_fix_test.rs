use crate::check::{CheckOutcome, CheckSession};
use crate::conflict_fix::{ConflictFix, FixBudget, FixRun, FixStep, FixStepKind, SearchState};
use domain_types::rewrite::RegroupPlan;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, baseline_repo};

const PREFIX: &str = "test-prefix";

/// `base` adds a file and `feature`, stacked on it, changes its second line, which `other` (a
/// root) changes again in its last commit `(other) Change lib again`
fn other_needs_feature(other_commits: &[(&str, &str, &str)]) -> (TestRepo, String) {
  let repo = baseline_repo();
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

/// The fix of a search that ran to its end and fixed every branch
fn find_complete_fix(repo: &TestRepo) -> ConflictFix {
  let fix = find_fix(repo, FixBudget::default());
  assert_eq!((fix.search, fix.unresolved.as_slice()), (SearchState::Complete, &[] as &[String]), "{fix:?}");
  fix
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

/// `other` keeps a commit that needs nothing of `feature`: moving its last commit to `feature` would split the group the
/// user made, so the branch is stacked on `feature` whole, the order of landing its commit needs
#[test]
fn a_commit_whose_branch_keeps_other_commits_restacks_its_branch_on_what_it_needs() {
  let (repo, _) = other_needs_feature(&[("(other) Add other", "other.txt", "other\n")]);

  let fix = find_complete_fix(&repo);

  assert_eq!(fix.trials, 1, "the direct answer verified at once: {fix:?}");
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Restack, &[], &[], "feature", "other")]);
  assert_eq!(fix.components[0].fixed, ["other"]);
  assert_eq!(fix.components[0].rewritten, 0, "the commits keep their group");
  assert_eq!(fix.components[0].holders, ["feature"], "what the commit needs is in feature");
  assert_applies(&repo, &fix);
}

/// `b`, stacked on `a`, changes a line that the last commit of `a` changes again: no restack fixes a parent that needs its
/// child, so the commit moves; the other conflict of the repository is still a restack
#[test]
fn a_commit_moves_only_where_no_restack_fixes_the_conflict() {
  let (repo, _) = other_needs_feature(&[("(other) Add other", "other.txt", "other\n")]);
  let more = parent_needs_its_child(&repo);

  let fix = find_complete_fix(&repo);

  let steps: Vec<FixStep> = fix.components.iter().flat_map(|component| component.steps.clone()).collect();
  assert_eq!(
    steps,
    [
      step(FixStepKind::Restack, &[], &[], "feature", "other"),
      step(FixStepKind::Move, &[&more], &["a"], "b", "a")
    ]
  );
}

#[test]
fn a_single_commit_branch_is_restacked_on_the_branch_it_needs() {
  let (repo, _) = other_needs_feature(&[]);

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!(fix.components[0].steps, [step(FixStepKind::Restack, &[], &[], "feature", "other")]);
  assert_eq!(fix.components[0].rewritten, 0, "a restack rewrites no subject");
  assert_eq!(fix.components[0].holders, ["feature"], "the new parent holds what it needs");
  assert_applies(&repo, &fix);
}

/// Each of `b`, `c` and `d` changes the line the branch before it wrote: each round restacks one, and a candidate of a
/// later round that carries a restack of an earlier one adds no step twice
#[test]
fn a_chain_of_branches_each_needing_the_one_before_is_restacked_once_each() {
  let repo = baseline_repo();
  repo.create_commit("(a) Add lib", "lib.txt", "1\n");
  repo.create_commit("(b) Change lib", "lib.txt", "2\n");
  repo.create_commit("(c) Change lib again", "lib.txt", "3\n");
  repo.create_commit("(d) Change lib once more", "lib.txt", "4\n");

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!(
    fix.components[0].steps,
    [
      step(FixStepKind::Restack, &[], &[], "a", "b"),
      step(FixStepKind::Restack, &[], &[], "b", "c"),
      step(FixStepKind::Restack, &[], &[], "c", "d"),
    ]
  );
  assert_applies(&repo, &fix);
}

#[test]
fn an_unassigned_commit_the_branch_needs_is_pulled_in() {
  let repo = baseline_repo();
  let add = repo.create_commit("Add lib", "lib.txt", "a\nb\nc\n");
  repo.create_commit("(x) Change lib", "lib.txt", "a\nB\nc\n");

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!(fix.components[0].steps, [step(FixStepKind::PullIn, &[&add], &[], "x", "x")]);
  assert_eq!(fix.components[0].holders, Vec::<String>::new(), "no branch holds an unassigned commit");
  assert_applies(&repo, &fix);
}

/// `a` adds a file, `b` stacked on it changes its first line, and the last commit of `a` changes that line again: the
/// commit that conflicts
fn parent_needs_its_child(repo: &TestRepo) -> String {
  repo.create_commit("(a) Add list", "list.txt", "1\n2\n3\n");
  repo.create_commit("(a/b) Change list", "list.txt", "one\n2\n3\n");
  repo.create_commit("(a) Change list more", "list.txt", "ONE\n2\n3\n")
}

#[test]
fn a_parent_that_needs_a_commit_of_its_child_gives_its_own_commit_to_the_child() {
  let repo = baseline_repo();
  let more = parent_needs_its_child(&repo);

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
  let repo = baseline_repo();
  let remove = helper_removed_after_its_use(&repo);

  let fix = find_fix(&repo, FixBudget::default());

  assert_the_removal_goes_to_c(&fix, &remove);
  assert_eq!(fix.components[0].fixed, ["c", "a"]);
  assert_eq!(fix.components[0].emptied, Vec::<String>::new());
  assert_applies(&repo, &fix);
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
  let repo = baseline_repo();
  repo.create_commit("(a) Add lib", "lib.txt", "1\n");

  let fix = find_fix(&repo, FixBudget::default());

  assert_eq!((fix.components.len(), fix.unresolved.len(), fix.search, fix.trials), (0, 0, SearchState::Complete, 0));
}

/// With the symbols off, a move still takes what the moved commit's dependents need and the dangling fix is found as
/// with them on: the names come from the lines, not the syntax trees
#[test]
fn the_fixes_are_the_same_with_the_symbols_off() {
  let stacked = baseline_repo();
  stacked.set_config("branchdeck.symbols", "off").unwrap();
  let more = parent_needs_its_child(&stacked);
  let fix = find_fix(&stacked, FixBudget::default());
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Move, &[&more], &["a"], "b", "a")]);
  assert_applies(&stacked, &fix);

  let repo = baseline_repo();
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
  let repo = baseline_repo();
  let more = parent_needs_its_child(&repo);
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

  assert_eq!(fix.components[0].steps, [step(FixStepKind::Move, &[&more], &["a"], "b", "a")]);
  assert!(fix.trials >= 2, "{fix:?}");
  // the baseline of the detection resolved once, its notes read once, and one `log -p` for the changes of the commits
  assert_eq!((processes(&git, "rev-parse"), processes(&git, "log")), (check.0, check.1 + 1));
}

#[test]
fn a_leaf_whose_commit_needs_two_other_leaves_is_restacked_on_their_chain_in_one_trial() {
  let repo = baseline_repo();
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
  // a second conflict, that no restack fixes: the direct answer restacks only other, the rounds are out of trials
  parent_needs_its_child(&repo);

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
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Restack, &[], &[], "feature", "other")]);
  assert_eq!(fix.unresolved, ["a"]);
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
  assert!(lines.iter().any(|line| line.starts_with("trial 1/24: restack other on feature … verified (")), "{lines:?}");
}

#[test]
fn a_leaf_on_a_parent_of_its_own_keeps_it_and_the_parent_is_restacked_on_what_the_leaf_needs() {
  let repo = baseline_repo();
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
  let repo = baseline_repo();
  repo.create_commit("(root) Add lib", "lib.txt", "1\n2\n3\n4\n5\n");
  repo.create_commit("(root/a) Change the first line", "lib.txt", "one\n2\n3\n4\n5\n");
  repo.create_commit("(root/p) Change the last line", "lib.txt", "one\n2\n3\n4\nfive\n");
  let leaf = repo.create_commit("(c) Change both lines", "lib.txt", "ONE\n2\n3\n4\nFIVE\n");
  // the user's plan stacks c on p, as `brd check --fix --plan` gets it
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

/// The user's plan makes `feature` a root, where its commit misses the one of `base`: the fix stacks it on `base` again,
/// in place of the unstack, never both
#[test]
fn a_restack_on_top_of_a_plan_that_unstacks_the_branch_replaces_the_unstack() {
  let repo = baseline_repo();
  repo.create_commit("(base) Add lib", "lib.txt", "a\nb\nc\n");
  repo.create_commit("(base/feature) Change lib", "lib.txt", "a\nB\nc\n");
  let plan = RegroupPlan {
    unstack: vec!["feature".to_string()],
    ..RegroupPlan::default()
  };

  let fix = session(&repo).find_conflict_fix(Some(&plan), None, &run(FixBudget::default())).unwrap();

  assert_eq!(fix.unresolved, Vec::<String>::new(), "{fix:?}");
  assert_eq!(fix.components[0].steps, [step(FixStepKind::Restack, &[], &[], "base", "feature")]);
  assert_eq!(fix.components[0].plan.unstack, Vec::<String>::new());
  assert_applies(&repo, &fix);
}

/// The apply of a fix is a regroup (`regroup_planned`, as a move): the snapshot, its fingerprint, the plan and the new
/// commits before the repository lock; under it only the witness of the refs it read and the compare-and-swap
/// transaction (docs/design/git-operations.md, "Lock spans")
#[cfg(unix)]
#[test]
fn a_conflict_fix_locks_only_the_witness_and_the_write() {
  let (repo, _) = other_needs_feature(&[("(other) Add other", "other.txt", "other\n")]);
  let fix = find_fix(&repo, FixBudget::default());
  let span = test_utils::lock_span::LockSpan::new(&repo.path().join(".git"));
  let params = crate::conflict_fix::ApplyConflictFixParams {
    repository_path: repo.path_str().to_string(),
    fingerprint: fix.fingerprint.clone(),
    plan: fix.components[0].plan.clone(),
  };
  let applied = crate::conflict_fix::apply_conflict_fix_core(span.executor(), &params, history_rewrite::repository_lock::LockWait::default()).unwrap();
  assert!(matches!(applied, domain_types::rewrite::AppliedConflictFix::Applied { .. }), "{applied:?}");
  assert_eq!(span.commands_under_the_lock(), ["for-each-ref", "update-ref"], "{:#?}", span.commands());
  span.assert_before_the_lock(&["fast-import"]);
}
