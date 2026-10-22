use crate::cache_memo::CacheMemo;
use crate::check::{CheckOutcome, CheckReport, CheckSession, Planned, StackIndependence, StackUse, check_plan, run_builds, stack_independence};
use crate::regroup_test;
use git_ops::merge_memo::{MergeMemo, MergeOutcome, MergeTrees};
use history_rewrite::regroup::{RegroupError, RegroupPlan};
use object_cache::cache::{CacheConfig, ObjectCache};
use object_cache::merge_result::{MergeKey, MergeResult};
use pretty_assertions::assert_eq;
use std::sync::{Mutex, mpsc};
use std::time::Duration;
use sync_types::landing::StackCommit;
use sync_types::stack::BlockReason;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const PREFIX: &str = "test-prefix";

/// `(a)` creates a file and changes it again; `(c)` is unrelated. Returns the commits
fn repo(stacks: bool) -> (TestRepo, Vec<String>) {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  if stacks {
    repo.set_config("branchdeck.stacks", "true").unwrap();
  }
  let ids = vec![
    repo.create_commit("(a) Add shared", "shared.txt", "one\n"),
    repo.create_commit("(a) Change shared", "shared.txt", "two\n"),
    repo.create_commit("(c) Add other", "other.txt", "other\n"),
  ];
  (repo, ids)
}

fn plan(branches: &[(&str, &[&String])]) -> RegroupPlan {
  regroup_test::plan(branches, &[])
}

fn cache(dir: &tempfile::TempDir) -> ObjectCache {
  ObjectCache::open_dir(dir.path(), &CacheConfig::default())
}

fn check(repo: &TestRepo, plan: Option<&RegroupPlan>, cache: &ObjectCache) -> Result<CheckReport, RegroupError> {
  check_plan(repo.git_executor(), repo.path().to_str().unwrap(), PREFIX, plan, cache)
}

fn outcome<'a>(report: &'a CheckReport, branch: &str) -> &'a CheckOutcome {
  &report
    .branches
    .iter()
    .find(|check| check.name == branch)
    .unwrap_or_else(|| panic!("no branch {branch}"))
    .outcome
}

/// Refs with their targets, the notes and HEAD: everything a check must leave alone
fn repository_state(repo: &TestRepo) -> (String, String, String) {
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  (
    git.execute_command(&["for-each-ref", "--format=%(refname) %(objectname)"], path).unwrap(),
    git.execute_command_with_status(&["notes", "list"], path).unwrap().0,
    repo.head(),
  )
}

#[test]
fn the_current_grouping_and_a_clean_plan_build() {
  let (repo, ids) = repo(false);
  let cache = ObjectCache::disabled();
  let report = check(&repo, None, &cache).unwrap();
  assert_eq!(report.branches.iter().map(|check| check.name.as_str()).collect::<Vec<_>>(), ["a", "c"]);
  assert!(report.branches.iter().all(|check| matches!(check.outcome, CheckOutcome::Ok { .. })), "{report:?}");
  assert_eq!(report.base.as_deref(), Some(repo.rev_parse("HEAD~3").unwrap().as_str()));

  let report = check(&repo, Some(&plan(&[("other", &[&ids[2]])])), &cache).unwrap();
  assert!(matches!(outcome(&report, "other"), CheckOutcome::Ok { .. }), "{report:?}");
  assert_eq!(report.regroup.changed_count(), 1);
}

#[test]
fn a_commit_moved_away_from_its_dependency_conflicts() {
  let (repo, ids) = repo(false);
  let before = repository_state(&repo);
  let report = check(&repo, Some(&plan(&[("b", &[&ids[1]])])), &ObjectCache::disabled()).unwrap();
  assert_eq!(
    outcome(&report, "b"),
    &CheckOutcome::Conflict {
      commit: ids[1].clone(),
      files: vec!["shared.txt".to_string()],
    }
  );
  assert!(matches!(outcome(&report, "a"), CheckOutcome::Ok { .. }));
  // a check writes objects only
  assert_eq!(repository_state(&repo), before);
}

#[test]
fn stacking_resolves_the_conflict() {
  let (repo, ids) = repo(true);
  let before = repository_state(&repo);
  let report = check(&repo, Some(&plan(&[("a/b", &[&ids[1]])])), &ObjectCache::disabled()).unwrap();
  let b = report.branches.iter().find(|check| check.name == "b").unwrap();
  assert!(matches!(b.outcome, CheckOutcome::Ok { .. }), "{report:?}");
  assert_eq!((b.built_on.as_deref(), b.declared_parent.as_deref()), (Some("a"), Some("a")));
  // the parent is checked before the child
  let names: Vec<&str> = report.branches.iter().map(|check| check.name.as_str()).collect();
  assert!(names.iter().position(|name| *name == "a") < names.iter().position(|name| *name == "b"), "{names:?}");
  assert_eq!(repository_state(&repo), before);
}

#[test]
fn children_of_a_conflicting_branch_are_blocked() {
  let (repo, ids) = repo(true);
  // b conflicts on the baseline; d is stacked on it
  let report = check(&repo, Some(&plan(&[("b", &[&ids[1]]), ("b/d", &[&ids[2]])])), &ObjectCache::disabled()).unwrap();
  assert!(matches!(outcome(&report, "b"), CheckOutcome::Conflict { .. }), "{report:?}");
  assert_eq!(
    outcome(&report, "d"),
    &CheckOutcome::Blocked {
      parent: "b".to_string(),
      reason: BlockReason::Conflict,
    }
  );
}

#[test]
fn an_invalid_plan_is_refused() {
  let (repo, _) = repo(false);
  let unknown = "0123456789".to_string();
  let error = check(&repo, Some(&plan(&[("b", &[&unknown])])), &ObjectCache::disabled()).unwrap_err();
  assert!(matches!(error, RegroupError::NotAheadOfBaseline { .. }), "{error}");
}

#[test]
fn the_cache_gives_the_same_report() {
  let (repo, ids) = repo(true);
  let dir = tempfile::tempdir().unwrap();
  let cache = cache(&dir);
  let conflicting = plan(&[("b", &[&ids[1]]), ("c", &[&ids[2]])]);
  let outcomes = |report: &CheckReport| report.branches.iter().map(|check| (check.name.clone(), check.outcome.clone())).collect::<Vec<_>>();

  let uncached = outcomes(&check(&repo, Some(&conflicting), &ObjectCache::disabled()).unwrap());
  let cold = outcomes(&check(&repo, Some(&conflicting), &cache).unwrap());
  let warm = outcomes(&check(&repo, Some(&conflicting), &cache).unwrap());
  assert_eq!(cold, uncached);
  assert_eq!(warm, uncached);
  // the conflict of b is a remembered merge now: its parent's tree, its tree and the baseline's tree
  let tree = |rev: &str| repo.rev_parse(&format!("{rev}^{{tree}}")).unwrap();
  let key = MergeKey {
    base_tree: tree(&ids[0]),
    commit_tree: tree(&ids[1]),
    target_tree: tree("HEAD~3"),
  };
  assert_eq!(
    cache.get_merge_results(&[key]),
    [Some(MergeResult::Conflict {
      files: vec!["shared.txt".to_string()],
    })]
  );
}

/// The summary of a check of `plan` and the `merge-tree` processes it ran
fn check_counting_merges(repo: &TestRepo, plan: &RegroupPlan, cache: &ObjectCache) -> ((Vec<(String, CheckOutcome)>, usize), u32) {
  let git = repo.git_executor().with_process_stats("test");
  let report = check_plan(&git, repo.path().to_str().unwrap(), PREFIX, Some(plan), cache).unwrap();
  let merges = git.process_stats().unwrap().by_command().get("merge-tree").map_or(0, |stats| stats.count);
  (summary(&report), merges)
}

/// `gc` prunes the merged trees of a trial build, which no ref holds, while their `merge_result` entries stay: a
/// later check merges them again instead of building on a missing tree, and reuses a tree that is still there
#[test]
fn a_remembered_tree_that_gc_pruned_is_merged_again() {
  let (repo, ids) = repo(true);
  let dir = tempfile::tempdir().unwrap();
  let cache = cache(&dir);
  // c alone on the baseline: its parent's tree is not the baseline's, so the trial merges a tree no ref holds
  let moved = plan(&[("a", &[&ids[0], &ids[1]]), ("c", &[&ids[2]])]);
  let uncached = summary(&check(&repo, Some(&moved), &ObjectCache::disabled()).unwrap());
  assert_eq!(check_counting_merges(&repo, &moved, &cache), (uncached.clone(), 1), "the cold check merges c");
  assert_eq!(check_counting_merges(&repo, &moved, &cache), (uncached.clone(), 0), "a tree still there is reused");
  let tree = |rev: &str| repo.rev_parse(&format!("{rev}^{{tree}}")).unwrap();
  let key = MergeKey {
    base_tree: tree(&ids[1]),
    commit_tree: tree(&ids[2]),
    target_tree: tree("HEAD~3"),
  };
  let Some(MergeResult::Clean { tree: merged }) = cache.get_merge_results(std::slice::from_ref(&key)).pop().flatten() else {
    panic!("the merge of c is not remembered");
  };

  repo
    .git_executor()
    .execute_command(&["gc", "--prune=now", "--quiet"], repo.path().to_str().unwrap())
    .unwrap();
  assert!(repo.rev_parse(&format!("{merged}^{{tree}}")).is_err(), "gc kept the merged tree");

  assert_eq!(check_counting_merges(&repo, &moved, &cache), (uncached, 1), "a pruned tree is merged again");
  assert!(repo.rev_parse(&format!("{merged}^{{tree}}")).is_ok(), "the merge wrote the tree again");
}

/// A merge the check remembers is read back before it is written, and the last handle writes what no flush wrote
#[test]
fn a_remembered_merge_is_read_before_the_memo_writes_it() {
  let (repo, _) = repo(false);
  let dir = tempfile::tempdir().unwrap();
  let memo = CacheMemo::new(cache(&dir), repo.git_executor().clone(), repo.path().to_str().unwrap().to_string());
  let trees = MergeTrees {
    base: "base".to_string(),
    commit: "commit".to_string(),
    target: "target".to_string(),
  };
  let clean = MergeOutcome::Clean { tree: "result".to_string() };
  assert_eq!(memo.merge(&trees), None);
  memo.remember_merge(&trees, &clean);
  memo.remember_commit_tree("c1", "t1");
  assert_eq!(memo.merge(&trees), Some(clean));

  drop(memo);
  let key = MergeKey {
    base_tree: "base".to_string(),
    commit_tree: "commit".to_string(),
    target_tree: "target".to_string(),
  };
  let reopened = cache(&dir);
  assert_eq!(reopened.get_merge_results(&[key]), [Some(MergeResult::Clean { tree: "result".to_string() })]);
  assert_eq!(reopened.get_commit_trees(&["c1".to_string()]).remove("c1").as_deref(), Some("t1"));
}

/// The outcome of every branch, by name, and the changed count: what two checks of the same plan must share
fn summary(report: &CheckReport) -> (Vec<(String, CheckOutcome)>, usize) {
  let outcomes = report.branches.iter().map(|check| (check.name.clone(), check.outcome.clone())).collect();
  (outcomes, report.regroup.changed_count())
}

#[test]
fn a_session_checks_many_plans_as_check_plan_does() {
  let (repo, ids) = repo(false);
  let cache = ObjectCache::disabled();
  let before = repository_state(&repo);
  let session = CheckSession::open(repo.git_executor(), repo.path().to_str().unwrap(), PREFIX, &cache).unwrap();
  assert_eq!(session.snapshot().commits.len(), 3);
  assert_eq!(session.snapshot().baseline, "master");
  let plans = [None, Some(plan(&[("b", &[&ids[1]])])), Some(plan(&[("other", &[&ids[2]])]))];
  for plan in &plans {
    let from_session = session.check(plan.as_ref()).unwrap();
    assert_eq!(summary(&from_session), summary(&check(&repo, plan.as_ref(), &cache).unwrap()), "{plan:?}");
  }
  assert!(matches!(
    session.check(Some(&plan(&[("b", &[&ids[1]])]))).unwrap().branches[1].outcome,
    CheckOutcome::Conflict { .. }
  ));
  session.check(Some(&regroup_test::plan(&[], &[&ids[0]]))).unwrap();
  assert_eq!(repository_state(&repo), before, "a session writes objects only");
}

/// Stacked branches on, `shared.txt` of eight lines added by `(a)`, its first line changed by `(a/b)`; `extra` adds
/// commits after them. Returns the commits
fn stacked_repo(extra: &[(&str, &str, &str)]) -> (TestRepo, Vec<String>) {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let mut ids = vec![
    repo.create_commit("(a) Add other", "other.txt", "other\n"),
    repo.create_commit("(a) Add shared", "shared.txt", "1\n2\n3\n4\n5\n6\n7\n8\n"),
    repo.create_commit("(a/b) Change the first line", "shared.txt", "one\n2\n3\n4\n5\n6\n7\n8\n"),
  ];
  for (subject, file, content) in extra {
    ids.push(repo.create_commit(subject, file, content));
  }
  (repo, ids)
}

fn independence(repo: &TestRepo, branch: &str) -> Result<StackIndependence, RegroupError> {
  stack_independence(repo.git_executor(), repo.path().to_str().unwrap(), PREFIX, branch, &ObjectCache::disabled())
}

fn commit(id: &str, subject: &str, branch: &str) -> StackCommit {
  StackCommit {
    id: id.to_string(),
    subject: subject.to_string(),
    branch: branch.to_string(),
  }
}

/// It uses no name of its parent, and says that the names were checked; with the symbols on report only, they weren't
#[test]
fn a_stacked_branch_that_applies_on_the_baseline_builds_alone() {
  let (repo, _) = stacked_repo(&[("(a/c) Add notes", "notes.txt", "notes\n")]);
  let before = repository_state(&repo);
  let builds = |symbols| StackIndependence::Builds {
    parent: "a".to_string(),
    uses: Vec::new(),
    symbols,
  };
  assert_eq!(independence(&repo, "c").unwrap(), builds(true));
  assert_eq!(repository_state(&repo), before, "a check writes objects only");
  repo.set_config("branchdeck.symbols", "report").unwrap();
  assert_eq!(independence(&repo, "c").unwrap(), builds(false));
}

/// A branch that applies alone but uses a function its parent adds names the use
#[test]
fn a_stacked_branch_that_uses_a_name_its_parent_adds_names_the_use() {
  let (repo, _) = stacked_repo(&[]);
  let declares = repo.create_commit("(a) Add controlUrl", "src/a/Util.kt", "package a\n\nfun controlUrl(): String = \"\"\n");
  let uses = repo.create_commit(
    "(a/c) Start with controlUrl",
    "src/b/Use.kt",
    "package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n",
  );
  assert_eq!(
    independence(&repo, "c").unwrap(),
    StackIndependence::Builds {
      parent: "a".to_string(),
      uses: vec![StackUse {
        commit: commit(&uses, "Start with controlUrl", "c"),
        name: "controlUrl".to_string(),
        declared_by: commit(&declares, "Add controlUrl", "a"),
      }],
      symbols: true,
    }
  );
}

#[test]
fn a_stacked_branch_that_conflicts_alone_names_the_commit_of_its_parent_it_needs() {
  let (repo, ids) = stacked_repo(&[]);
  assert_eq!(
    independence(&repo, "b").unwrap(),
    StackIndependence::Needs {
      parent: "a".to_string(),
      child_commit: commit(&ids[2], "Change the first line", "b"),
      files: vec!["shared.txt".to_string()],
      parent_commits: vec![commit(&ids[1], "Add shared", "a")],
      movable: true,
    }
  );
}

/// The subjects of the commits `branch` needs, and whether they can move; panics when it builds alone
fn needs(repo: &TestRepo, branch: &str) -> (Vec<String>, bool) {
  let StackIndependence::Needs { parent_commits, movable, .. } = independence(repo, branch).unwrap() else {
    panic!("{branch} builds alone");
  };
  (parent_commits.into_iter().map(|commit| commit.subject).collect(), movable)
}

/// `(subject, file, content)` of a commit that changes the last line of `shared.txt`
fn change_the_last_line(subject: &str) -> (&str, &'static str, &'static str) {
  (subject, "shared.txt", "one\n2\n3\n4\n5\n6\n7\neight\n")
}

#[test]
fn a_needed_commit_is_not_movable_when_a_commit_left_in_the_parent_depends_on_it() {
  // a later commit of the parent changes the file the needed commit creates
  let (repo, _) = stacked_repo(&[change_the_last_line("(a) Change the last line")]);
  assert_eq!(needs(&repo, "b"), (vec!["Add shared".to_string()], false));
}

#[test]
fn a_commit_of_another_stack_that_depends_on_a_needed_commit_keeps_it_movable() {
  // the root x changes the file the needed commit creates: a move into b changes nothing x is built on
  let (repo, _) = stacked_repo(&[change_the_last_line("(x) Change the last line")]);
  assert_eq!(needs(&repo, "b"), (vec!["Add shared".to_string()], true));
}

#[test]
fn a_needed_commit_brings_its_fixups_and_a_needed_fixup_its_target() {
  let unit = || vec!["Add shared".to_string(), "fixup! Add shared".to_string()];
  // the fixup changes another file: no line ties it to b, but a move takes it with its target
  let (repo, ids) = stacked_repo(&[("fixup! (a) Add shared", "fix.txt", "fix\n")]);
  assert_eq!(needs(&repo, "b"), (unit(), true));
  let StackIndependence::Needs { parent_commits, .. } = independence(&repo, "b").unwrap() else {
    panic!("b builds alone");
  };
  assert_eq!(parent_commits, [commit(&ids[1], "Add shared", "a"), commit(&ids[3], "fixup! Add shared", "a")]);

  // c needs only the fixup by its lines; the fixup can't leave its target, so the target moves too. b, built on a as
  // well, needs that target: a move into c would make b need c
  let (repo, _) = stacked_repo(&[("fixup! (a) Add shared", "fix.txt", "fix\n"), ("(a/c) Use the fix", "fix.txt", "fixed\n")]);
  assert_eq!(needs(&repo, "c"), (unit(), false));
}

#[test]
fn a_root_is_not_stacked() {
  let (repo, _) = stacked_repo(&[]);
  let error = independence(&repo, "a").unwrap_err();
  assert!(matches!(error, RegroupError::InvalidPlan { .. }), "{error}");
}

/// The trial build checks the mapping notes as a sync does: a note copied to a changed commit doesn't make the check
/// build the old copy
#[test]
fn check_does_not_reuse_a_stale_copy() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.create_commit("(a) Add shared", "shared.txt", "one\n");
  let path = repo.path().to_str().unwrap();
  let git = |args: &[&str]| repo.git_executor().execute_command(args, path).unwrap();
  let dir = tempfile::tempdir().unwrap();
  let report = check(&repo, None, &cache(&dir)).unwrap();
  let CheckOutcome::Ok { head } = outcome(&report, "a") else { panic!("{report:?}") };
  git(&["update-ref", &format!("refs/heads/{PREFIX}/virtual/a"), head]);
  repo
    .add_note(
      "HEAD",
      &git_ops::notes::MappingNote {
        original: String::new(),
        mapped: head.clone(),
        source: git_ops::notes::NoteSource {
          parent_tree: git(&["rev-parse", "HEAD~1^{tree}"]),
          tree: git(&["rev-parse", "HEAD^{tree}"]),
        },
        resolved: None,
      }
      .content(),
    )
    .unwrap();

  let old = repo.head();
  std::fs::write(repo.path().join("shared.txt"), "one\nmoved hunk\n").unwrap();
  git(&["add", "-A"]);
  git(&["commit", "--amend", "--no-edit"]);
  git(&["notes", "copy", &old, "HEAD"]);

  let report = check(&repo, None, &cache(&dir)).unwrap();
  let CheckOutcome::Ok { head: rebuilt } = outcome(&report, "a") else {
    panic!("{report:?}")
  };
  assert_ne!(rebuilt, head);
  assert_eq!(git(&["show", &format!("{rebuilt}:shared.txt")]), "one\nmoved hunk");
}

/// Branches `a` (root), `b` on `a`, `c` (root), `d` on `b`, `e` (root, placed with a problem) and `f` on `e`
fn planned() -> Vec<Planned> {
  let branch = |name: &str, parent: Option<usize>| Planned {
    name: name.to_string(),
    parent,
    immediate: None,
  };
  vec![
    branch("a", None),
    branch("b", Some(0)),
    branch("c", None),
    branch("d", Some(1)),
    Planned {
      immediate: Some(CheckOutcome::Problems(Vec::new())),
      ..branch("e", None)
    },
    branch("f", Some(4)),
  ]
}

/// The branches of a check build at the same time when they don't wait for each other: the two roots each wait for the
/// other to start, which a build one by one never does
#[test]
fn independent_branches_build_at_the_same_time() {
  let (started_a, wait_a) = mpsc::channel::<()>();
  let (started_c, wait_c) = mpsc::channel::<()>();
  let (started_a, started_c) = (Mutex::new(started_a), Mutex::new(started_c));
  let (wait_a, wait_c) = (Mutex::new(wait_a), Mutex::new(wait_c));
  let outcomes = run_builds(&planned(), Some("base"), 4, |index, base, _| {
    let both_started = match index {
      0 => {
        started_a.lock().unwrap().send(()).unwrap();
        wait_c.lock().unwrap().recv_timeout(Duration::from_secs(10)).is_ok()
      }
      2 => {
        started_c.lock().unwrap().send(()).unwrap();
        wait_a.lock().unwrap().recv_timeout(Duration::from_secs(10)).is_ok()
      }
      _ => true,
    };
    if both_started {
      CheckOutcome::Ok { head: format!("{base}+{index}") }
    } else {
      CheckOutcome::Error {
        message: "built alone".to_string(),
      }
    }
  });
  assert_eq!(outcomes[0], CheckOutcome::Ok { head: "base+0".to_string() });
  assert_eq!(outcomes[2], CheckOutcome::Ok { head: "base+2".to_string() });
}

/// A branch builds on the head of its parent, a root on the base; a branch on a conflicting parent is blocked by the
/// conflict, one on a blocked branch is blocked in turn, and one on a branch placed with a problem is blocked by an
/// error, as a check one by one decides
#[test]
fn a_branch_waits_for_its_parent_and_is_blocked_by_its_failure() {
  let built = run_builds(&planned(), Some("base"), 4, |index, base, on_parent| CheckOutcome::Ok {
    head: format!("{base}+{index}{}", if on_parent { "^" } else { "" }),
  });
  assert_eq!(
    built,
    [
      CheckOutcome::Ok { head: "base+0".to_string() },
      CheckOutcome::Ok { head: "base+0+1^".to_string() },
      CheckOutcome::Ok { head: "base+2".to_string() },
      CheckOutcome::Ok { head: "base+0+1^+3^".to_string() },
      CheckOutcome::Problems(Vec::new()),
      CheckOutcome::Blocked {
        parent: "e".to_string(),
        reason: BlockReason::Problem,
      },
    ]
  );

  let conflict = CheckOutcome::Conflict {
    commit: "x".to_string(),
    files: vec!["file.txt".to_string()],
  };
  let failed = run_builds(&planned(), Some("base"), 1, |index, base, _| match index {
    0 => conflict.clone(),
    _ => CheckOutcome::Ok { head: format!("{base}+{index}") },
  });
  assert_eq!(failed[0], conflict);
  assert_eq!(
    failed[1],
    CheckOutcome::Blocked {
      parent: "a".to_string(),
      reason: BlockReason::Conflict,
    }
  );
  assert_eq!(
    failed[3],
    CheckOutcome::Blocked {
      parent: "b".to_string(),
      reason: BlockReason::Blocked,
    }
  );

  let without_base = run_builds(&planned(), None, 2, |_, _, _| CheckOutcome::Ok { head: String::new() });
  assert!(
    matches!(&without_base[0], CheckOutcome::Error { message } if message.contains("no parent")),
    "{without_base:?}"
  );
}
