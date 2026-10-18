use crate::check::{CheckOutcome, CheckReport, CheckSession};
use crate::dangling_references::{DanglingKind, DanglingReference};
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const PREFIX: &str = "test-prefix";

const LEDGER: &str = "bridge/src/AcpOwnershipLedger.kt";
const LIFECYCLE_TEST: &str = "contract/test/AcpJourneyLifecycleTest.kt";
const OTHER_TEST: &str = "contract/test/OtherCleanupTest.kt";

fn ledger(cleanup: &str) -> String {
  format!(
    "package bridge

class AcpOwnershipLedger {{
  private val resources = mutableListOf<String>()

  {cleanup}
}}
"
  )
}

const PLAIN: &str = "fun cleanup() {\n    resources.clear()\n  }";
const SUSPENDING: &str = "suspend fun cleanup() {\n    resources.clear()\n  }";

const LIFECYCLE: &str = "package contract

import bridge.AcpOwnershipLedger

class AcpJourneyLifecycleTest {
  fun cleansInReverse() {
    val ledger = AcpOwnershipLedger()
    ledger.cleanup()
  }
}
";

/// A commit that deletes `paths`
fn delete(repo: &TestRepo, message: &str, paths: &[&str]) -> String {
  let mut args = vec!["rm", "-q", "--"];
  args.extend_from_slice(paths);
  repo.git(&args);
  repo.git(&["commit", "-q", "-m", message]);
  repo.head()
}

/// A check of the grouping, as `bd check` without a plan
fn check(repo: &TestRepo) -> CheckReport {
  let session = CheckSession::open(repo.git_executor(), repo.path_str(), PREFIX, &ObjectCache::disabled()).unwrap();
  session.check(None).unwrap()
}

fn references(report: &CheckReport, branch: &str) -> Vec<DanglingReference> {
  report.dangling.get(branch).cloned().unwrap_or_default()
}

/// The Safe Push of IJAI-4185 (2026-10-17): `(journey)` deletes a test, `(suspend)` makes the `cleanup` the test calls
/// suspend. HEAD compiles, the tree of `(suspend)` has the test with the call outside a coroutine
fn suspended(cleanup: &str) -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial commit", &[(LEDGER, &ledger(PLAIN)), (LIFECYCLE_TEST, LIFECYCLE)]);
  let deleted_by = delete(&repo, "(journey) IJAI-4182 The journey support goes with its tests", &[LIFECYCLE_TEST]);
  let changed_by = repo.create_commit_with_files("(suspend) IJAI-4185 The cleanups may suspend", &[(LEDGER, &ledger(cleanup))]);
  (repo, deleted_by, changed_by)
}

#[test]
fn a_function_made_suspend_dangles_where_another_branch_removed_its_call() {
  let (repo, deleted_by, changed_by) = suspended(SUSPENDING);
  let report = check(&repo);
  assert!(report.branches.iter().all(|branch| matches!(branch.outcome, CheckOutcome::Ok { .. })), "{report:?}");
  assert_eq!(
    references(&report, "suspend"),
    [DanglingReference {
      kind: DanglingKind::Changed,
      identifier: "cleanup".to_string(),
      file: LIFECYCLE_TEST.to_string(),
      line: 8,
      removed_by: changed_by,
      fixed_by: Some(deleted_by),
      on_baseline: false,
      mutual: false,
      by_syntax: true,
    }]
  );
  assert!(!report.builds_cleanly("suspend"));
}

/// A change of the body alone keeps the header: the call fits
#[test]
fn a_change_of_the_body_alone_changes_no_form() {
  let (repo, ..) = suspended("fun cleanup() {\n    resources.removeAll { true }\n  }");
  assert_eq!(references(&check(&repo), "suspend"), []);
}

/// A parameter with a default takes the call without the argument; one without a default doesn't
#[test]
fn a_new_parameter_dangles_only_where_the_call_does_not_fit_it() {
  let (repo, _, changed_by) = suspended("fun cleanup(force: Boolean) {\n    resources.clear()\n  }");
  let found: Vec<(DanglingKind, String)> = references(&check(&repo), "suspend").into_iter().map(|it| (it.kind, it.removed_by)).collect();
  assert_eq!(found, [(DanglingKind::Changed, changed_by)]);

  let (repo, ..) = suspended("fun cleanup(force: Boolean = false) {\n    resources.clear()\n  }");
  assert_eq!(references(&check(&repo), "suspend"), []);
}

/// `other.cleanup()` on a receiver of another type is no use of the ledger's `cleanup`, though the name is the same
#[test]
fn a_call_on_a_receiver_of_another_type_is_no_use() {
  let repo = TestRepo::new();
  let other = "package contract\n\nclass Other {\n  fun cleanup() {}\n}\n\nfun cleansOther(other: Other) {\n  other.cleanup()\n}\n";
  repo.create_commit_with_files("Initial commit", &[(LEDGER, &ledger(PLAIN)), (LIFECYCLE_TEST, LIFECYCLE), (OTHER_TEST, other)]);
  let deleted_by = delete(&repo, "(journey) IJAI-4182 The tests go", &[LIFECYCLE_TEST, OTHER_TEST]);
  repo.create_commit_with_files("(suspend) IJAI-4185 The cleanups may suspend", &[(LEDGER, &ledger(SUSPENDING))]);
  let found: Vec<(String, Option<String>)> = references(&check(&repo), "suspend").into_iter().map(|it| (it.file, it.fixed_by)).collect();
  assert_eq!(found, [(LIFECYCLE_TEST.to_string(), Some(deleted_by))]);
}

/// A use that a commit of the branch writes is written against the form the branch gives the declaration: a later
/// commit of another branch that deletes it leaves nothing that dangles
#[test]
fn a_use_the_branch_writes_fits_its_form() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial commit", &[(LEDGER, &ledger(PLAIN))]);
  let written = "package contract\n\nimport bridge.AcpOwnershipLedger\n\nsuspend fun cleans(ledger: AcpOwnershipLedger) {\n  ledger.cleanup()\n}\n";
  repo.create_commit_with_files("(suspend) IJAI-4185 The cleanups may suspend", &[(LEDGER, &ledger(SUSPENDING)), (OTHER_TEST, written)]);
  delete(&repo, "(journey) IJAI-4182 The test goes", &[OTHER_TEST]);
  assert_eq!(references(&check(&repo), "suspend"), []);
}

/// An instance member removed with its overloads, called on a receiver of its type in a test another branch deletes;
/// a bare `cleanup()` of another class in a deleted file is no use of it
#[test]
fn a_removed_instance_member_dangles_where_its_receiver_calls_it() {
  let repo = TestRepo::new();
  let bare = "package contract\n\nclass Unrelated {\n  fun run() {\n    cleanup()\n  }\n\n  private fun cleanup() {}\n}\n";
  repo.create_commit_with_files("Initial commit", &[(LEDGER, &ledger(PLAIN)), (LIFECYCLE_TEST, LIFECYCLE), (OTHER_TEST, bare)]);
  let deleted_by = delete(&repo, "(journey) IJAI-4182 The tests go", &[LIFECYCLE_TEST, OTHER_TEST]);
  let removed_by = repo.create_commit_with_files("(ledger) IJAI-4185 The ledger cleans nothing", &[(LEDGER, &ledger("fun isEmpty() = resources.isEmpty()"))]);
  assert_eq!(
    references(&check(&repo), "ledger"),
    [DanglingReference {
      kind: DanglingKind::Removed,
      identifier: "cleanup".to_string(),
      file: LIFECYCLE_TEST.to_string(),
      line: 8,
      removed_by,
      fixed_by: Some(deleted_by),
      on_baseline: false,
      mutual: false,
      by_syntax: true,
    }]
  );
}

const OLD_PANE: &str = "air/src/old/LanePane.kt";
const NEW_PANE: &str = "air/src/LanePane.kt";

/// A move split across two branches: `(add)` adds the type in its new file, `(drop)` deletes the old one; the tree of
/// `(add)` declares it twice in one package and module
#[test]
fn a_type_declared_again_in_the_same_package_is_a_duplicate() {
  let repo = TestRepo::new();
  let pane = "package air\n\nclass LanePane(val id: String)\n";
  repo.create_commit_with_files("Initial commit", &[("air/BUILD.bazel", "kt_jvm_library(name = \"air\")\n"), (OLD_PANE, pane)]);
  let added_by = repo.create_commit_with_files("(add) IJAI-1 The pane moves up", &[(NEW_PANE, pane)]);
  let deleted_by = delete(&repo, "(drop) IJAI-2 The old pane goes", &[OLD_PANE]);
  assert_eq!(
    references(&check(&repo), "add"),
    [DanglingReference {
      kind: DanglingKind::Duplicate,
      identifier: "LanePane".to_string(),
      file: OLD_PANE.to_string(),
      line: 3,
      removed_by: added_by,
      fixed_by: Some(deleted_by),
      on_baseline: false,
      mutual: false,
      by_syntax: true,
    }]
  );
}

/// The same name in another package is another key: no duplicate
#[test]
fn a_type_of_the_same_name_in_another_package_is_no_duplicate() {
  let repo = TestRepo::new();
  repo.create_commit_with_files(
    "Initial commit",
    &[
      ("air/BUILD.bazel", "kt_jvm_library(name = \"air\")\n"),
      (OLD_PANE, "package air.old\n\nclass LanePane(val id: String)\n"),
    ],
  );
  repo.create_commit_with_files("(add) IJAI-1 A new pane", &[(NEW_PANE, "package air\n\nclass LanePane(val id: String)\n")]);
  delete(&repo, "(drop) IJAI-2 The old pane goes", &[OLD_PANE]);
  assert_eq!(references(&check(&repo), "add"), []);
}

/// The only declaration of the key, in a file another branch deletes later: the tree of the branch declares it once
#[test]
fn a_declaration_another_branch_deletes_is_no_duplicate() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial commit", &[("air/BUILD.bazel", "kt_jvm_library(name = \"air\")\n")]);
  repo.create_commit_with_files("(add) IJAI-1 A pane", &[(NEW_PANE, "package air\n\nclass LanePane(val id: String)\n")]);
  delete(&repo, "(drop) IJAI-2 The pane goes", &[NEW_PANE]);
  assert_eq!(references(&check(&repo), "add"), []);
}
