use crate::check::{CheckOutcome, CheckReport, CheckSession};
use crate::dangling_references::{DanglingKind, DanglingReference, DeclaredIn};
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
      possible: false,
      mutual: false,
      by_syntax: true,
      declared_in: DeclaredIn {
        file: LEDGER.to_string(),
        line: Some(6),
      },
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
      possible: false,
      mutual: false,
      by_syntax: true,
      declared_in: DeclaredIn {
        file: LEDGER.to_string(),
        line: Some(6),
      },
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
      possible: false,
      mutual: false,
      by_syntax: true,
      declared_in: DeclaredIn {
        file: NEW_PANE.to_string(),
        line: Some(3),
      },
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

/// `(suspend)` changes the members of the ledger from `base` to `new`, after `(journey)` deleted `uses`, a test of
/// the ledger
fn changed(base: &str, new: &str, uses: &str) -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial commit", &[(LEDGER, &ledger(base)), (LIFECYCLE_TEST, uses)]);
  let deleted_by = delete(&repo, "(journey) IJAI-4182 The test goes", &[LIFECYCLE_TEST]);
  let changed_by = repo.create_commit_with_files("(suspend) IJAI-4185 The ledger changes", &[(LEDGER, &ledger(new))]);
  (repo, deleted_by, changed_by)
}

/// A test of the ledger whose function `cleans` holds `body`, in a file that imports what it needs
fn ledger_test(body: &str) -> String {
  format!("package contract\n\nimport bridge.AcpOwnershipLedger\nimport kotlinx.coroutines.runBlocking\n\nclass AcpJourneyLifecycleTest {{\n{body}\n}}\n")
}

/// `(kind, possible, fixed_by)` of the references of `(suspend)`
fn kinds(repo: &TestRepo) -> Vec<(DanglingKind, bool, Option<String>)> {
  references(&check(repo), "suspend").into_iter().map(|it| (it.kind, it.possible, it.fixed_by)).collect()
}

/// A suspend call compiles in a suspend function and in a coroutine builder's lambda; in the lambda of an unknown
/// function it may (its parameter may be a suspend function type): only possible
#[test]
fn a_function_made_suspend_fits_a_coroutine() {
  let (repo, deleted_by, _) = changed(PLAIN, SUSPENDING, &ledger_test("  fun cleans(ledger: AcpOwnershipLedger) {\n    ledger.cleanup()\n  }"));
  assert_eq!(kinds(&repo), [(DanglingKind::Changed, false, Some(deleted_by))]);

  let (repo, ..) = changed(
    PLAIN,
    SUSPENDING,
    &ledger_test("  suspend fun cleans(ledger: AcpOwnershipLedger) {\n    ledger.cleanup()\n  }"),
  );
  assert_eq!(kinds(&repo), []);
  let (repo, ..) = changed(
    PLAIN,
    SUSPENDING,
    &ledger_test("  fun cleans(ledger: AcpOwnershipLedger) = runBlocking {\n    ledger.cleanup()\n  }"),
  );
  assert_eq!(kinds(&repo), []);

  let (repo, deleted_by, _) = changed(
    PLAIN,
    SUSPENDING,
    &ledger_test("  fun cleans(ledger: AcpOwnershipLedger) = retrying {\n    ledger.cleanup()\n  }"),
  );
  let report = check(&repo);
  assert_eq!(kinds(&repo), [(DanglingKind::Changed, true, Some(deleted_by))]);
  assert!(report.builds_cleanly("suspend"), "a possible one fails no build");
}

/// A changed return type may break a use the summary has no types for: only possible. A `var` for a `val`, an explicit
/// type for an inferred one and `operator` break none
#[test]
fn a_header_change_the_summary_cannot_judge_is_only_possible() {
  let call = ledger_test("  fun cleans(ledger: AcpOwnershipLedger) {\n    ledger.cleanup()\n  }");
  let (repo, deleted_by, _) = changed(PLAIN, "fun cleanup(): Int {\n    return 1\n  }", &call);
  assert_eq!(kinds(&repo), [(DanglingKind::Changed, true, Some(deleted_by))]);

  let (repo, ..) = changed(PLAIN, "operator fun cleanup() {\n  }", &call);
  assert_eq!(kinds(&repo), []);
  let read = ledger_test("  fun cleans(ledger: AcpOwnershipLedger) = ledger.cleanup");
  let (repo, ..) = changed("val cleanup = 1", "var cleanup: Int = 1", &read);
  assert_eq!(kinds(&repo), []);
}

/// A new type of a parameter rejects a literal of another family: `cleanup(1)` of `cleanup(force: String)`; an argument
/// the summary has no type for fits
#[test]
fn a_changed_parameter_type_rejects_a_literal() {
  let (repo, deleted_by, _) = changed(
    "fun cleanup(force: Int) {\n  }",
    "fun cleanup(force: String) {\n  }",
    &ledger_test("  fun cleans(ledger: AcpOwnershipLedger) {\n    ledger.cleanup(1)\n  }"),
  );
  assert_eq!(kinds(&repo), [(DanglingKind::Changed, false, Some(deleted_by))]);

  let (repo, ..) = changed(
    "fun cleanup(force: Int) {\n  }",
    "fun cleanup(force: String) {\n  }",
    &ledger_test("  fun cleans(ledger: AcpOwnershipLedger, force: Int) {\n    ledger.cleanup(force)\n  }"),
  );
  assert_eq!(kinds(&repo), []);
}

/// An overload removed while another stays: a call only the removed one takes dangles, one the other takes doesn't
#[test]
fn a_removed_overload_dangles_where_the_one_left_rejects_the_call() {
  let both = "fun log(msg: String) {\n  }\n\n  fun log(msg: String, t: Throwable) {\n  }";
  let one = "fun log(msg: String) {\n  }";
  let (repo, deleted_by, _) = changed(
    both,
    one,
    &ledger_test("  fun cleans(ledger: AcpOwnershipLedger, e: Throwable) {\n    ledger.log(\"x\", e)\n  }"),
  );
  assert_eq!(kinds(&repo), [(DanglingKind::Changed, false, Some(deleted_by))]);

  let (repo, ..) = changed(both, one, &ledger_test("  fun cleans(ledger: AcpOwnershipLedger) {\n    ledger.log(\"x\")\n  }"));
  assert_eq!(kinds(&repo), []);
}

/// A changed extension receiver breaks a call that its old receiver let in, by a bare name in an extension of it
#[test]
fn a_changed_extension_receiver_breaks_its_call() {
  let repo = TestRepo::new();
  let extensions = |receiver: &str| format!("package bridge\n\nfun {receiver}.cleanup() {{\n}}\n");
  let uses = "package contract\n\nimport bridge.AcpOwnershipLedger\nimport bridge.cleanup\n\nfun AcpOwnershipLedger.cleans() {\n  cleanup()\n}\n";
  repo.create_commit_with_files("Initial commit", &[(LEDGER, &extensions("AcpOwnershipLedger")), (LIFECYCLE_TEST, uses)]);
  let deleted_by = delete(&repo, "(journey) IJAI-4182 The test goes", &[LIFECYCLE_TEST]);
  repo.create_commit_with_files("(suspend) IJAI-4185 The cleanup is of the scope", &[(LEDGER, &extensions("AcpScope"))]);
  assert_eq!(kinds(&repo), [(DanglingKind::Changed, false, Some(deleted_by))]);
}

/// Rust and TypeScript keep the parameters in the header, defaults included, so their forms are not judged; the same
/// change in Kotlin is
#[test]
fn a_changed_form_is_judged_in_the_jvm_languages_only() {
  let (repo, ..) = changed(
    "fun cleanup(force: Int) {\n  }",
    "fun cleanup(force: Int, deep: Int) {\n  }",
    &ledger_test("  fun cleans(ledger: AcpOwnershipLedger) {\n    ledger.cleanup(1)\n  }"),
  );
  assert_eq!(kinds(&repo).len(), 1);

  let repo = TestRepo::new();
  let library = "crates/ledger/src/lib.rs";
  let user = "crates/ledger/src/user.rs";
  repo.create_commit_with_files(
    "Initial commit",
    &[
      ("crates/ledger/Cargo.toml", "[package]\nname = \"ledger\"\n"),
      (library, "mod user;\n\npub fn cleanup(force: i32) {}\n"),
      (user, "use crate::cleanup;\n\npub fn cleans() {\n  cleanup(1);\n}\n"),
    ],
  );
  delete(&repo, "(journey) IJAI-4182 The user goes", &[user]);
  repo.create_commit_with_files(
    "(suspend) IJAI-4185 The cleanup goes deep",
    &[(library, "mod user;\n\npub fn cleanup(force: i32, deep: bool) {}\n")],
  );
  assert_eq!(kinds(&repo), []);
}

/// A member removed from its type while another type of the branch adds one of its name is still removed; one that
/// moves with its type to another file of the package is not
#[test]
fn a_removed_member_is_kept_only_by_its_own_key() {
  let other = "bridge/src/Other.kt";
  let (repo, deleted_by, _) = {
    let repo = TestRepo::new();
    repo.create_commit_with_files("Initial commit", &[(LEDGER, &ledger(PLAIN)), (LIFECYCLE_TEST, LIFECYCLE)]);
    let deleted_by = delete(&repo, "(journey) IJAI-4182 The test goes", &[LIFECYCLE_TEST]);
    let removed_by = repo.create_commit_with_files(
      "(suspend) IJAI-4185 The cleanup moves to another class",
      &[
        (LEDGER, &ledger("fun isEmpty() = resources.isEmpty()")),
        (other, "package bridge\n\nclass Other {\n  fun cleanup() {}\n}\n"),
      ],
    );
    (repo, deleted_by, removed_by)
  };
  assert_eq!(kinds(&repo), [(DanglingKind::Removed, false, Some(deleted_by))]);

  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial commit", &[(LEDGER, &ledger(PLAIN)), (LIFECYCLE_TEST, LIFECYCLE)]);
  delete(&repo, "(journey) IJAI-4182 The test goes", &[LIFECYCLE_TEST]);
  repo.git(&["rm", "-q", "--", LEDGER]);
  repo.create_commit_with_files("(suspend) IJAI-4185 The ledger moves", &[("bridge/src/Ledger.kt", &ledger(PLAIN))]);
  assert_eq!(kinds(&repo), []);
}

/// A use the branch writes with the text of an old one: the line numbers of the branch's hunks tell them apart, so the
/// old one still dangles
#[test]
fn a_use_the_branch_writes_is_told_by_its_line_not_its_text() {
  let repo = TestRepo::new();
  let old = "  fun cleans(ledger: AcpOwnershipLedger) {\n    ledger.cleanup()\n  }";
  let written = "  suspend fun suspends(ledger: AcpOwnershipLedger) {\n    ledger.cleanup()\n  }";
  repo.create_commit_with_files("Initial commit", &[(LEDGER, &ledger(PLAIN)), (LIFECYCLE_TEST, &ledger_test(old))]);
  repo.create_commit_with_files(
    "(suspend) IJAI-4185 The cleanups may suspend",
    &[(LEDGER, &ledger(SUSPENDING)), (LIFECYCLE_TEST, &ledger_test(&format!("{old}\n\n{written}")))],
  );
  let fixed_by = repo.create_commit_with_files("(journey) IJAI-4182 The old test goes", &[(LIFECYCLE_TEST, &ledger_test(written))]);
  let found: Vec<(u32, Option<String>)> = references(&check(&repo), "suspend").into_iter().map(|it| (it.line, it.fixed_by)).collect();
  assert_eq!(found, [(8, Some(fixed_by))]);
}

/// A member extension moved to the top level of another file, which the file of the use imports now: the use means the
/// import (the measurement's `dynamicAgentOptions` of 2026-10-18). Without the import it is a removed member
#[test]
fn a_removed_member_whose_name_the_file_imports_is_no_use() {
  let moved = |import: &str| {
    let repo = TestRepo::new();
    repo.create_commit_with_files("Initial commit", &[(LEDGER, &ledger(PLAIN)), (LIFECYCLE_TEST, LIFECYCLE)]);
    let uses = LIFECYCLE.replace("import bridge.AcpOwnershipLedger\n", &format!("import bridge.AcpOwnershipLedger\n{import}"));
    repo.create_commit_with_files(
      "(suspend) IJAI-4185 The cleanup is an extension",
      &[
        (LEDGER, &ledger("fun isEmpty() = resources.isEmpty()")),
        ("bridge/src/Cleanup.kt", "package bridge\n\nfun AcpOwnershipLedger.cleanup() {}\n"),
        (LIFECYCLE_TEST, &uses),
      ],
    );
    delete(&repo, "(journey) IJAI-4182 The test goes", &[LIFECYCLE_TEST]);
    repo
  };
  assert_eq!(kinds(&moved("")).into_iter().map(|(kind, ..)| kind).collect::<Vec<_>>(), [DanglingKind::Removed]);
  assert_eq!(kinds(&moved("import bridge.cleanup\n")), []);
}
