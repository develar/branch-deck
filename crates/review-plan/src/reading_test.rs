use crate::PlanSources;
use crate::reading::{READING_POLICY, ahead_readings, layer_readings};
use crate::test_support::plan_of_range;
use domain_types::range_reading::{FileRole, QuietProof, RangeReading, RoleReading, Unproven, UnprovenFile};
use file_diffs::model::{DiffContent, DiffSource};
use file_diffs::reading::DiffReading;
use file_diffs::service::FileDiffs;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{assert_processes, counting};

const REPORT: &str = "package a\n\nclass Report {\n  fun total(a: Int, b: Int): Int {\n    return a+b\n  }\n\n  fun name(): String {\n    return \"report\"\n  }\n}\n";
/// `REPORT` with the spaces around its `+`: formatting alone
const REPORT_FORMATTED: &str = "package a\n\nclass Report {\n  fun total(a: Int, b: Int): Int {\n    return a + b\n  }\n\n  fun name(): String {\n    return \"report\"\n  }\n}\n";
/// `REPORT_FORMATTED` with another body of `total`
const REPORT_CHANGED: &str = "package a\n\nclass Report {\n  fun total(a: Int, b: Int): Int {\n    return a * b\n  }\n\n  fun name(): String {\n    return \"report\"\n  }\n}\n";
const TEST: &str = "package a\n\nclass ReportTest {\n  fun total() {\n    check(Report().total(1, 2) == 3)\n  }\n}\n";

fn role(role: FileRole, files: u32, files_to_read: u32, changes: u32, to_read: u32) -> RoleReading {
  RoleReading {
    role,
    files,
    files_to_read,
    changes,
    to_read,
  }
}

/// The sum of the badges of the files of `base..head` as the review window counts them with its plan
fn badges(repo: &TestRepo, base: &str, head: &str) -> u32 {
  let files = FileDiffs::default()
    .read(
      repo.git_executor(),
      repo.path_str(),
      &ObjectCache::disabled(),
      &DiffSource::Range {
        base: base.to_string(),
        head: head.to_string(),
      },
      READING_POLICY,
    )
    .unwrap();
  files
    .iter()
    .map(|file| match &file.content {
      DiffContent::Text(diff) => DiffReading::of(diff).stops(true),
      _ => 0,
    })
    .sum()
}

/// openspec/specs/review/range-reading/spec.md, "A reading sums the badges of the files"
#[test]
fn a_reading_sums_what_the_badge_of_each_file_counts_by_its_role_and_names_what_cannot_be_proven() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Initial",
    &[
      ("src/a/Report.kt", REPORT),
      ("test/a/ReportTest.kt", TEST),
      ("docs/guide.md", "# Guide\n\nRead the report.\n"),
      ("notes.cfg", "a=1\n"),
    ],
  );
  let formatted = repo.create_commit_with_files("Format the report", &[("src/a/Report.kt", REPORT_FORMATTED)]);
  assert_eq!(
    plan_of_range(&repo, &base, &formatted).reading,
    RangeReading {
      roles: vec![role(FileRole::Code, 1, 0, 1, 0)],
      unproven: vec![],
      proofs: vec![QuietProof::Formatting],
    },
    "formatting alone: nothing to read",
  );

  std::fs::write(repo.path().join("icon.png"), b"\x89PNG\r\n\x1a\n\0\0\0").unwrap();
  repo.git(&["add", "icon.png"]);
  let head = repo.create_commit_with_files(
    "Multiply",
    &[
      ("src/a/Report.kt", REPORT_CHANGED),
      ("test/a/ReportTest.kt", &TEST.replace("== 3", "== 2")),
      ("docs/guide.md", "# Guide\n\nRead the report twice.\n"),
      ("notes.cfg", "a=2\n"),
    ],
  );
  let reading = plan_of_range(&repo, &formatted, &head).reading;
  assert_eq!(
    reading,
    RangeReading {
      roles: vec![
        role(FileRole::Code, 1, 1, 1, 1),
        role(FileRole::Tests, 1, 1, 1, 1),
        role(FileRole::Prose, 1, 1, 1, 1),
        role(FileRole::Resources, 2, 1, 1, 1),
      ],
      unproven: vec![
        UnprovenFile {
          path: "icon.png".to_string(),
          role: FileRole::Resources,
          why: Unproven::Binary,
        },
        UnprovenFile {
          path: "notes.cfg".to_string(),
          role: FileRole::Resources,
          why: Unproven::NoGrammar,
        },
      ],
      proofs: vec![],
    },
  );
  assert_eq!(reading.to_read(), badges(&repo, &formatted, &head), "the sum is the sum of the badges");
}

/// openspec/specs/commit/quiet-changes/spec.md, "Runs: Blocks the comparison cut out of one change are read as one": a
/// Kotlin file reformatted whole, whose changes of two members the comparison cut in two, leaves nothing to read, and
/// its plan folds every block into the one unit of whitespace: the reading and the units say the same of the diff
#[test]
fn a_reformat_the_comparison_cut_in_two_leaves_nothing_to_read_and_one_unit_of_whitespace() {
  let path = "src/main/kotlin/probe/Ledger.kt";
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Initial", &[(path, include_str!("../../file-diffs/testdata/split-reformat/Ledger.old.kt"))]);
  let head = repo.create_commit_with_files("Reformat", &[(path, include_str!("../../file-diffs/testdata/split-reformat/Ledger.new.kt"))]);
  let plan = plan_of_range(&repo, &base, &head);
  assert_eq!(
    plan.reading,
    RangeReading {
      roles: vec![role(FileRole::Code, 1, 0, 12, 0)],
      unproven: vec![],
      proofs: vec![QuietProof::Formatting],
    },
  );
  let units: Vec<&str> = plan.units.iter().map(|unit| unit.id.as_str()).collect();
  assert_eq!(units, ["mechanical:whitespace"]);
  assert_eq!((plan.steps.coverage.blocks, plan.steps.coverage.mechanical), (12, 12));
}

/// openspec/specs/review/range-reading/spec.md, "The layers of a stack are read at once"
#[test]
fn the_layers_of_a_stack_are_read_in_one_process_a_phase() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Initial", &[("src/a/Report.kt", REPORT), ("test/a/ReportTest.kt", TEST)]);
  let formatted = repo.create_commit_with_files("Format the report", &[("src/a/Report.kt", REPORT_FORMATTED)]);
  let changed = repo.create_commit_with_files("Multiply", &[("src/a/Report.kt", REPORT_CHANGED)]);
  let tested = repo.create_commit_with_files("Test the product", &[("test/a/ReportTest.kt", &TEST.replace("== 3", "== 2"))]);
  let ranges = vec![(base, formatted.clone()), (formatted, changed.clone()), (changed, tested)];

  let git = counting();
  let (cache, file_diffs) = (ObjectCache::disabled(), FileDiffs::default());
  let sources = PlanSources {
    cache: &cache,
    file_diffs: &file_diffs,
    index: None,
  };
  let mut readings: Vec<Option<RangeReading>> = vec![None; ranges.len()];
  layer_readings(&git, sources, repo.path_str(), &ranges, |index, reading| {
    assert!(readings[index].is_none(), "a layer with no rename is told once");
    readings[index] = Some(reading);
  })
  .unwrap();
  let to_read: Vec<u32> = readings.iter().map(|reading| reading.as_ref().unwrap().to_read()).collect();
  assert_eq!(to_read, [0, 1, 1]);
  assert_eq!(readings[2].as_ref().unwrap().roles, [role(FileRole::Tests, 1, 1, 1, 1)]);
  // the lists of every layer in one `diff-tree --stdin`, the blobs of every pair in one `cat-file --batch`
  assert_processes(&git, &[("cat-file", 1), ("diff-tree", 1)]);
}

/// openspec/specs/review/range-reading/spec.md, "Reading: The read ahead asks no index": a rename across files waits for
/// the index while the machine has one, and is not proven without one; the read asks nothing
#[test]
fn the_read_ahead_leaves_a_rename_across_files_to_the_index_and_asks_none() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Initial", &[("src/a/Report.kt", REPORT), ("test/a/ReportTest.kt", TEST)]);
  let renamed = repo.create_commit_with_files(
    "Rename total to sum",
    &[
      ("src/a/Report.kt", &REPORT.replace("fun total", "fun sum")),
      ("test/a/ReportTest.kt", &TEST.replace(".total(", ".sum(")),
    ],
  );
  let ranges = vec![(base, renamed)];
  let (cache, file_diffs) = (ObjectCache::disabled(), FileDiffs::default());
  let sources = PlanSources {
    cache: &cache,
    file_diffs: &file_diffs,
    index: None,
  };
  let why = |indexed: bool| {
    let mut whys = Vec::new();
    ahead_readings(repo.git_executor(), sources, repo.path_str(), &ranges, indexed, |_, reading| {
      whys = reading.unproven.iter().map(|file| file.why).collect();
    })
    .unwrap();
    whys.sort();
    whys.dedup();
    whys
  };
  assert_eq!(why(true), [Unproven::RenamesPending], "checking renames until a window asks the index");
  assert_eq!(why(false), [Unproven::RenamesUnanswered], "no index on the machine: not proven");
}
