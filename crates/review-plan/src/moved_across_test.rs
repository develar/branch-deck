//! The declarations moved whole to another file (`moved_across`): a removed and an added one of the same code in two
//! files say where each went and came from, a file taken whole names its place without a word of its own, and a move
//! with a changed line, a code of two places on a side and a move inside one file say nothing

use crate::ReviewPlan;
use crate::test_support::{plan_of, range};
use crate::units::{MovedPlace, UnitKind};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const ORDERS: &str = "src/a/Orders.kt";
const REPORT: &str = "src/a/Report.kt";
const PARSING: &str = "src/a/Parsing.kt";
const LEGACY: &str = "src/a/Legacy.kt";

const CLASS_ORDERS: &str = "class Orders(private val items: List<Int>) {
  fun total(): Int = items.sum()
}
";

const FORMAT_TOTAL: &str = "/** The total as the report prints it */
fun formatTotal(total: Int): String {
  val text = total.toString()
  return \"Total: $text\"
}
";

const PARSE_TOTAL: &str = "fun parseTotal(text: String): Int {
  return text.removePrefix(\"Total: \").toInt()
}
";

const LEGACY_TOTAL: &str = "fun legacyTotal(items: List<Int>): Int {
  return items.fold(0) { sum, item -> sum + item }
}
";

const CLASS_REPORT: &str = "class Report(private val orders: Orders) {
  fun render(): String = formatTotal(orders.total())
}
";

/// The text of a file of the package `a` with `declarations`, a blank line between them
fn source(declarations: &[&str]) -> String {
  format!("package a\n\n{}", declarations.join("\n"))
}

/// The plan of `base..head` of `repo`, without an index: the words are of git alone
fn plan(repo: &TestRepo, base: &str, head: &str) -> ReviewPlan {
  plan_of(repo, None, &range(repo, base, head)).unwrap()
}

/// A unit as a test reads it: its kind, its name, its file, and where it moved from and to as (path, line)
type Read = (UnitKind, Option<String>, String, Option<(String, u32)>, Option<(String, u32)>);

/// Each unit of `plan` (`Read`)
fn units(plan: &ReviewPlan) -> Vec<Read> {
  let place = |place: &Option<MovedPlace>| place.as_ref().map(|place| (place.path.clone(), place.line));
  plan
    .units
    .iter()
    .map(|unit| {
      (
        unit.kind.clone(),
        unit.name.clone(),
        unit.slices[0].path.clone(),
        place(&unit.moved_from),
        place(&unit.moved_to),
      )
    })
    .collect()
}

fn unit(kind: UnitKind, name: &str, path: &str, moved_from: Option<(&str, u32)>, moved_to: Option<(&str, u32)>) -> Read {
  let place = |place: Option<(&str, u32)>| place.map(|(path, line)| (path.to_string(), line));
  (kind, Some(name.to_string()), path.to_string(), place(moved_from), place(moved_to))
}

/// The line of `needle` in `text`, from 1
fn line_of(text: &str, needle: &str) -> u32 {
  u32::try_from(text.lines().position(|line| line.contains(needle)).unwrap() + 1).unwrap()
}

/// openspec/specs/branch/review-units/spec.md, "Units: A declaration moved whole to another file says where": a function
/// moved to a file that stays, one extracted into a new file and one moved out of a file the commits delete, each the
/// same code
#[test]
fn a_declaration_moved_whole_to_another_file_says_where_it_went_and_came_from() {
  let repo = TestRepo::new();
  let orders_before = source(&[CLASS_ORDERS, FORMAT_TOTAL, PARSE_TOTAL]);
  let report_before = source(&[CLASS_REPORT]);
  let legacy = source(&[LEGACY_TOTAL]);
  let base = repo.create_commit_with_files("Add the orders", &[(ORDERS, &orders_before), (REPORT, &report_before), (LEGACY, &legacy)]);
  let orders_after = source(&[CLASS_ORDERS, LEGACY_TOTAL]);
  let report_after = source(&[CLASS_REPORT, FORMAT_TOTAL]);
  let parsing = source(&[PARSE_TOTAL]);
  repo.git(&["rm", "-q", LEGACY]);
  let head = repo.create_commit_with_files("Move the totals", &[(ORDERS, &orders_after), (REPORT, &report_after), (PARSING, &parsing)]);
  let format_before = line_of(&orders_before, "fun formatTotal");
  let format_after = line_of(&report_after, "fun formatTotal");
  let parse_after = line_of(&parsing, "fun parseTotal");
  let legacy_before = line_of(&legacy, "fun legacyTotal");
  let mut read = units(&plan(&repo, &base, &head));
  read.sort_by(|a, b| (&a.2, &a.1).cmp(&(&b.2, &b.1)));
  assert_eq!(
    read,
    [
      (UnitKind::DeletedFile, Some("Legacy.kt".to_string()), LEGACY.to_string(), None, None),
      unit(UnitKind::Removed, "formatTotal", ORDERS, None, Some((REPORT, format_after))),
      unit(UnitKind::Added, "legacyTotal", ORDERS, Some((LEGACY, legacy_before)), None),
      unit(UnitKind::Removed, "parseTotal", ORDERS, None, Some((PARSING, parse_after))),
      (UnitKind::NewFile, Some("Parsing.kt".to_string()), PARSING.to_string(), None, None),
      unit(UnitKind::Added, "formatTotal", REPORT, Some((ORDERS, format_before)), None),
    ],
  );
}

/// openspec/specs/branch/review-units/spec.md, "Units: A declaration moved whole to another file says where": a function
/// moved into a class that was there, indented again, is the same code
#[test]
fn a_function_moved_into_a_class_of_another_file_indented_again_is_a_move() {
  let repo = TestRepo::new();
  let orders_before = source(&[CLASS_ORDERS, PARSE_TOTAL]);
  let base = repo.create_commit_with_files("Add the orders", &[(ORDERS, &orders_before), (REPORT, &source(&[CLASS_REPORT]))]);
  let indented = PARSE_TOTAL.replace('\n', "\n  ");
  let indented = format!("  {}\n", indented.trim_end());
  let report_after = source(&[&CLASS_REPORT.replace("\n}\n", &format!("\n\n{indented}}}\n"))]);
  let head = repo.create_commit_with_files("Parse in the report", &[(ORDERS, &source(&[CLASS_ORDERS])), (REPORT, &report_after)]);
  let parse_before = line_of(&orders_before, "fun parseTotal");
  let parse_after = line_of(&report_after, "fun parseTotal");
  assert_eq!(
    units(&plan(&repo, &base, &head)),
    [
      unit(UnitKind::Removed, "parseTotal", ORDERS, None, Some((REPORT, parse_after))),
      unit(UnitKind::Added, "parseTotal", REPORT, Some((ORDERS, parse_before)), None),
    ],
  );
}

/// openspec/specs/branch/review-units/spec.md, "Units: A declaration moved whole to another file says where": a move
/// with a changed line, a code added to two places or removed from two, and a move to another class of its own file say
/// nothing
#[test]
fn a_move_with_a_changed_line_or_of_two_places_on_a_side_says_nothing() {
  let repo = TestRepo::new();
  let orders_before = source(&[CLASS_ORDERS, FORMAT_TOTAL, PARSE_TOTAL]);
  let base = repo.create_commit_with_files("Add the orders", &[(ORDERS, &orders_before), (REPORT, &source(&[CLASS_REPORT]))]);
  let moved_words = |read: &[Read]| read.iter().filter(|unit| unit.3.is_some() || unit.4.is_some()).count();

  // moved with a changed line
  let changed = repo.create_commit_with_files(
    "Move the format and say Sum",
    &[
      (ORDERS, &source(&[CLASS_ORDERS, PARSE_TOTAL])),
      (REPORT, &source(&[CLASS_REPORT, &FORMAT_TOTAL.replace("Total: ", "Sum: ")])),
    ],
  );
  let read = units(&plan(&repo, &base, &changed));
  assert_eq!((read.len(), moved_words(&read)), (2, 0), "{read:?}");

  // copied to two files: the file that stays and a new file
  let copied = repo.create_commit_with_files(
    "Parse in two places",
    &[
      (ORDERS, &source(&[CLASS_ORDERS, FORMAT_TOTAL])),
      (REPORT, &source(&[CLASS_REPORT, PARSE_TOTAL])),
      (PARSING, &source(&[PARSE_TOTAL])),
    ],
  );
  let read = units(&plan(&repo, &base, &copied));
  assert_eq!((read.len(), moved_words(&read)), (3, 0), "{read:?}");

  // removed from two files, added to one
  let other = "src/b/Orders.kt";
  repo.git(&["rm", "-q", PARSING]);
  let two = repo.create_commit_with_files(
    "Parse in b too",
    &[
      (ORDERS, &orders_before),
      (REPORT, &source(&[CLASS_REPORT])),
      (other, &format!("package b\n\n{PARSE_TOTAL}")),
    ],
  );
  let joined = repo.create_commit_with_files(
    "Parse in one place",
    &[(ORDERS, &source(&[CLASS_ORDERS, FORMAT_TOTAL])), (other, "package b\n"), (PARSING, &source(&[PARSE_TOTAL]))],
  );
  let read = units(&plan(&repo, &two, &joined));
  assert_eq!((read.len(), moved_words(&read)), (3, 0), "{read:?}");

  // moved from a class to another class of its own file
  let summary = "class Summary(private val count: Int) {\n  fun count(): Int = count\n}\n";
  let in_file = repo.create_commit_with_files("Add the summary", &[(ORDERS, &source(&[CLASS_ORDERS, summary]))]);
  let moved_inside = repo.create_commit_with_files(
    "Total in the summary",
    &[(
      ORDERS,
      &source(&[
        "class Orders(private val items: List<Int>) {\n}\n",
        &summary.replace("\n}\n", "\n  fun total(): Int = items.sum()\n}\n"),
      ]),
    )],
  );
  let read = units(&plan(&repo, &in_file, &moved_inside));
  assert_eq!(moved_words(&read), 0, "{read:?}");
  let total: Vec<&UnitKind> = read.iter().filter(|unit| unit.1.as_deref() == Some("total")).map(|unit| &unit.0).collect();
  assert_eq!(total, [&UnitKind::Removed, &UnitKind::Added], "{read:?}");
}
