//! The plan in stages (`review_plan_of`, openspec/specs/branch/review-plan/spec.md, "In the App: A fact of the plan
//! shows when its query answered"): the plan of git alone before the index is asked, then the plan with every answer
//! but the tests of the steps while that query still runs, then the plan whole

use crate::api::BreakVerdict;
use crate::fan_in_test::{Counts, reordered};
use crate::files::MechanicalKind;
use crate::test_support::{PREFIX, range};
use crate::units::UnitKind;
use crate::{PlanSources, PlanWait, ReviewPlan, ReviewPlanParams, resolve_subject, review_plan_of};
use diff_marks::index::Order;
use file_diffs::accio::{DeclarationAt, ReachingTest, References, SymbolHit, SymbolIndex, TestClass, TestReach};
use file_diffs::service::FileDiffs;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::sync::mpsc;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// An index that answers as `inner`, its answer of the tests held until `release` says so (or is dropped)
pub(crate) struct HeldTests<'a> {
  inner: &'a Counts,
  release: Mutex<mpsc::Receiver<()>>,
}

impl<'a> HeldTests<'a> {
  /// The index over `inner`, and what releases its answer of the tests
  pub(crate) fn of(inner: &'a Counts) -> (Self, mpsc::Sender<()>) {
    let (release, held) = mpsc::channel();
    let index = Self { inner, release: Mutex::new(held) };
    (index, release)
  }
}

impl SymbolIndex for HeldTests<'_> {
  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    self.inner.symbols(repository_root, query, revision)
  }

  fn references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> anyhow::Result<Option<References>> {
    self.inner.references(repository_root, name, revision)
  }

  fn references_at(&self, repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> anyhow::Result<Option<Vec<References>>> {
    self.inner.references_at(repository_root, declarations, revision)
  }

  fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], given: &[String], revision: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    // a guard against a hang of a broken staging, never the wait of a passing run: the stage of the tests releases it
    let released = self.release.lock().unwrap_or_else(PoisonError::into_inner).recv_timeout(Duration::from_secs(60));
    anyhow::ensure!(released != Err(mpsc::RecvTimeoutError::Timeout), "the tests were never released");
    self.inner.tests(repository_root, declarations, given, revision)
  }
}

/// What a test reads of a plan: its verdict, the uses left outside of each entry of its API that has usages, and of its
/// steps whether one waits for its tests, whether one has them, and the files of the tests that reach their code
#[derive(Debug, PartialEq, Eq)]
struct Shown {
  verdict: BreakVerdict,
  outside: Vec<u32>,
  awaited: bool,
  answered: bool,
  proof: Vec<String>,
}

fn shown(plan: &ReviewPlan) -> Shown {
  let tests = || plan.steps.steps.iter().map(|step| &step.tests);
  Shown {
    verdict: plan.verdict,
    outside: plan.api.iter().filter_map(|change| change.usages.as_ref()).map(|usages| usages.outside).collect(),
    awaited: tests().any(|tests| tests.awaited),
    answered: tests().any(|tests| tests.answered),
    proof: tests().flat_map(|tests| tests.proof.iter().map(|file| file.path.clone())).collect(),
  }
}

/// The stages `review_plan_of` sends of `params` with `index`, each as `read` reads it, and the plan it answers;
/// `on_stage` runs after each
pub(crate) fn staged<T>(
  repo: &TestRepo,
  index: &dyn SymbolIndex,
  params: &ReviewPlanParams,
  read: impl Fn(&ReviewPlan) -> T,
  mut on_stage: impl FnMut(PlanWait),
) -> (Vec<(PlanWait, T)>, ReviewPlan) {
  let (cache, file_diffs) = (ObjectCache::disabled(), FileDiffs::for_scans());
  let sources = PlanSources {
    cache: &cache,
    file_diffs: &file_diffs,
    index: Some(index),
  };
  let subject = resolve_subject(repo.git_executor(), PREFIX, params).unwrap();
  let mut stages = Vec::new();
  let plan = review_plan_of(repo.git_executor(), sources, params, subject, |plan, waits| {
    stages.push((waits, read(plan)));
    on_stage(waits);
  })
  .unwrap();
  (stages, plan)
}

const MANAGER: &str = "package a\n\nclass Manager {\n  fun open(): Int = 1\n\n  fun isEmpty(): Boolean = true\n}\n";

fn manager_test(expected: u8) -> String {
  format!("package a\n\nimport org.junit.Test\n\nclass ManagerTest {{\n  @Test fun `opens`() = check(Manager().open() == {expected})\n}}\n")
}

/// The manager whose `open` answers two and whose `isEmpty` takes a parameter: a change of behaviour and a change of the
/// API in one commit
fn changed_manager() -> String {
  MANAGER
    .replace("fun open(): Int = 1", "fun open(): Int = 2")
    .replace("fun isEmpty(): Boolean = true", "fun isEmpty(strict: Boolean): Boolean = strict")
}

/// The usages show while the tests of the steps are asked: the plan before the index is asked has neither, the plan
/// sent once the usages answered has them and the verdict with every step still waiting for its tests, and the plan
/// the reading answers has the tests that reach its code too
#[test]
fn the_plan_shows_the_usages_while_the_tests_of_its_steps_are_asked() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("src/a/Manager.kt", MANAGER), ("test/a/ManagerTest.kt", &manager_test(1))]);
  let head = repo.create_commit_with_files(
    "The manager opens two and is empty only strictly\n\nThe open answers two, and the caller says whether it asks strictly.",
    &[("src/a/Manager.kt", &changed_manager()), ("test/a/ManagerTest.kt", &manager_test(2))],
  );
  let changed = format!("{head}:src/a/Manager.kt:6");
  let opens = ReachingTest {
    path: "test/a/ManagerTest.kt".to_string(),
    line: 6,
    declaration: "ManagerTest.opens".to_string(),
    reads_as: "opens".to_string(),
    framework: "junit4".to_string(),
    depth: 1,
    via: Vec::new(),
    class: TestClass::Direct,
    through_hub: false,
  };
  let counts = Counts::of(&[(changed.as_str(), 2)]).with_tests([(
    "src/a/Manager.kt:4",
    TestReach {
      tests: vec![opens],
      complete: true,
      unavailable: false,
    },
  )]);
  let (index, release) = HeldTests::of(&counts);

  let (stages, plan) = staged(&repo, &index, &range(&repo, &base, &head), shown, |waits| {
    if waits == PlanWait::Tests {
      release.send(()).unwrap();
    }
  });
  let waiting = |verdict, outside: &[u32]| Shown {
    verdict,
    outside: outside.to_vec(),
    awaited: true,
    answered: false,
    proof: Vec::new(),
  };
  assert_eq!(
    stages,
    [
      (PlanWait::Index, waiting(BreakVerdict::Pending, &[])),
      (PlanWait::Tests, waiting(BreakVerdict::Breaks, &[2])),
    ]
  );
  assert_eq!(
    shown(&plan),
    Shown {
      verdict: BreakVerdict::Breaks,
      outside: vec![2],
      awaited: false,
      answered: true,
      proof: vec!["test/a/ManagerTest.kt".to_string()],
    }
  );
  assert_eq!((plan.index_note.as_deref(), plan.is_final()), (None, true));
  assert_eq!(counts.tested(), ["src/a/Manager.kt:4", "src/a/Manager.kt:6"]);
}

/// A range without tests asks the index nothing of them, so its plan has no stage of its tests, however fast the
/// usages answer: one stage before the index is asked, no step waiting, then the plan whole
#[test]
fn a_range_without_tests_has_no_stage_of_its_tests() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("src/a/Manager.kt", MANAGER)]);
  let head = repo.create_commit_with_files(
    "The manager opens two and is empty only strictly\n\nThe open answers two, and the caller says whether it asks strictly.",
    &[("src/a/Manager.kt", &changed_manager())],
  );
  let changed = format!("{head}:src/a/Manager.kt:6");
  let index = Counts::of(&[(changed.as_str(), 2)]);

  let (stages, plan) = staged(&repo, &index, &range(&repo, &base, &head), shown, |_| {});
  assert_eq!(
    stages,
    [(
      PlanWait::Index,
      Shown {
        verdict: BreakVerdict::Pending,
        outside: Vec::new(),
        awaited: false,
        answered: false,
        proof: Vec::new(),
      }
    )]
  );
  assert_eq!(index.tested(), Vec::<String>::new());
  let after = shown(&plan);
  assert_eq!((after.verdict, after.outside, after.awaited, after.answered), (BreakVerdict::Breaks, vec![2], false, true));
}

/// A function moved to the end of its file is read in the plan of git alone, before the index is asked, and folded in
/// the plan the index answered, which says its order is free: the moves are asked once, the reading of the pair asking
/// them of the same trees after the plan finds them in memory. A change where nothing moved asks no moves
#[test]
fn a_moved_declaration_is_read_in_the_plan_of_git_and_folded_once_the_index_says_its_order_is_free() {
  let function = |name: &str| format!("fun {name}(x: Int): Int {{\n  return compute(x, \"{name}\")\n}}\n");
  let file = |names: &[&str]| format!("package a\n\n{}", names.iter().map(|name| function(name)).collect::<Vec<_>>().join("\n"));
  let repo = TestRepo::new();
  let base = repo.create_commit("Add", "src/a/Store.kt", &file(&["one", "two", "three"]));
  let head = repo.create_commit("Move one last", "src/a/Store.kt", &file(&["two", "three", "one"]));
  let index = Counts::of(&[]).with_reordered(vec![reordered("src/a/Store.kt", "one", [3, 5], [11, 13], Order::Free)]);
  let kinds = |plan: &ReviewPlan| plan.units.iter().map(|unit| unit.kind.clone()).collect::<Vec<_>>();
  let (stages, plan) = staged(&repo, &index, &range(&repo, &base, &head), kinds, |_| {});
  assert_eq!(stages, [(PlanWait::Index, vec![UnitKind::Behaviour])]);
  assert_eq!(
    kinds(&plan),
    [UnitKind::Mechanical {
      mechanical: MechanicalKind::Moved
    }]
  );
  assert_eq!(index.moves_asked(), 1);

  // nothing moved: no moves asked by the plan, nor by the reading, whose files only add a line
  let added = repo.create_commit("Add four", "src/a/Store.kt", &file(&["two", "three", "one", "four"]));
  let (_, plan) = staged(&repo, &index, &range(&repo, &head, &added), kinds, |_| {});
  assert_eq!(kinds(&plan), [UnitKind::Added]);
  assert_eq!(index.moves_asked(), 1);
}
