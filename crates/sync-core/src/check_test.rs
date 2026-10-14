use crate::check::{CheckOutcome, CheckReport, check_plan};
use crate::regroup::regroup_test;
use crate::regroup::{RegroupError, RegroupPlan};
use object_cache::cache::{CacheConfig, ObjectCache};
use object_cache::merge_result::{MergeKey, MergeResult};
use pretty_assertions::assert_eq;
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
