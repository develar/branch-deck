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

/// The unit that shows the other place of each unit that says one (`MovedPlace::unit`), by the id of the unit: of its
/// `moved_from`, else of its `moved_to`
fn shown_by(plan: &ReviewPlan) -> Vec<(String, String)> {
  let mut shown: Vec<(String, String)> = plan
    .units
    .iter()
    .filter_map(|unit| unit.moved_from.as_ref().or(unit.moved_to.as_ref()).map(|place| (unit.id.clone(), place.unit.clone())))
    .collect();
  shown.sort();
  shown
}

/// The slices of `plan` that are the move alone (`UnitSlice::moved_out`), each by the kind and the name of its unit
/// and its path, in the order of the units
fn moved_out(plan: &ReviewPlan) -> Vec<(UnitKind, Option<String>, String)> {
  plan
    .units
    .iter()
    .flat_map(|unit| {
      unit
        .slices
        .iter()
        .filter(|slice| slice.moved_out)
        .map(|slice| (unit.kind.clone(), unit.name.clone(), slice.path.clone()))
    })
    .collect()
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
  let plan = plan(&repo, &base, &head);
  // the unit that shows the other place: the unit of its declaration, or the unit that takes its file whole
  let id = |path: &str, name: &str| {
    let unit = plan.units.iter().find(|unit| unit.slices[0].path == path && unit.name.as_deref() == Some(name));
    unit.map(|unit| unit.id.clone()).unwrap()
  };
  assert_eq!(
    shown_by(&plan),
    [
      (id(ORDERS, "formatTotal"), id(REPORT, "formatTotal")),
      (id(ORDERS, "legacyTotal"), format!("deleted:{LEGACY}")),
      (id(ORDERS, "parseTotal"), format!("new:{PARSING}")),
      (id(REPORT, "formatTotal"), id(ORDERS, "formatTotal")),
    ],
  );
  // the deleted file holds its package line and the moved function alone; the block of the orders adds the legacy
  // total in the place of the two it removes, and is read
  assert_eq!(moved_out(&plan), [(UnitKind::DeletedFile, Some("Legacy.kt".to_string()), LEGACY.to_string())]);
  let mut read = units(&plan);
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

const COUNT: &str = "fun count(items: List<Int>): Int {\n  return items.size\n}\n";

const PARSE_SUM: &str = "fun parseSum(text: String): Int {\n  return text.removePrefix(\"Sum: \").toInt()\n}\n";

/// openspec/specs/branch/review-units/spec.md, "Units: The place a move left is nothing to read where it holds the move
/// alone": a function moved to a file that stays, two beside each other extracted into a new file, one block, and a
/// file deleted whose license, package, import and function are all it holds; then a block that removes a declaration
/// that did not move beside one that did, and a deleted file with a declaration of its own
#[test]
fn the_place_a_move_left_is_nothing_to_read_where_it_holds_the_move_alone() {
  let repo = TestRepo::new();
  let license = "// Copyright the authors\n";
  let legacy = format!("{license}package a\n\nimport kotlin.math.max\n\n{LEGACY_TOTAL}");
  let orders_before = source(&[CLASS_ORDERS, FORMAT_TOTAL, COUNT, PARSE_TOTAL, PARSE_SUM]);
  let base = repo.create_commit_with_files("Add the orders", &[(ORDERS, &orders_before), (REPORT, &source(&[CLASS_REPORT])), (LEGACY, &legacy)]);
  repo.git(&["rm", "-q", LEGACY]);
  let head = repo.create_commit_with_files(
    "Move the totals",
    &[
      (ORDERS, &source(&[CLASS_ORDERS, COUNT])),
      (REPORT, &source(&[CLASS_REPORT, FORMAT_TOTAL, LEGACY_TOTAL])),
      (PARSING, &source(&[PARSE_TOTAL, PARSE_SUM])),
    ],
  );
  let read = plan(&repo, &base, &head);
  assert_eq!(
    moved_out(&read),
    [
      (UnitKind::DeletedFile, Some("Legacy.kt".to_string()), LEGACY.to_string()),
      (UnitKind::Removed, Some("formatTotal".to_string()), ORDERS.to_string()),
      // the block of both: the unit of the first of the two holds it, the other one joins it
      (UnitKind::Removed, Some("parseTotal".to_string()), ORDERS.to_string()),
    ],
    "{:#?}",
    read.units
  );
  // the words and the units are those of the moves
  let said = units(&read).iter().filter(|unit| unit.3.is_some() || unit.4.is_some()).count();
  assert_eq!(said, 5, "{:#?}", read.units);

  // a constant removed beside the moved function, which moved nowhere; a deleted file whose other function goes
  let rate = "const val RATE = 3\n";
  let old = "fun oldTotal(): Int = 0\n";
  let base = repo.create_commit_with_files(
    "Add the rate",
    &[
      (ORDERS, &source(&[CLASS_ORDERS, &format!("{rate}{FORMAT_TOTAL}"), COUNT])),
      (REPORT, &source(&[CLASS_REPORT])),
      (LEGACY, &source(&[LEGACY_TOTAL, old])),
    ],
  );
  repo.git(&["rm", "-q", LEGACY]);
  let head = repo.create_commit_with_files(
    "Move the format, drop the rate",
    &[(ORDERS, &source(&[CLASS_ORDERS, COUNT])), (REPORT, &source(&[CLASS_REPORT, FORMAT_TOTAL, LEGACY_TOTAL]))],
  );
  let read = plan(&repo, &base, &head);
  assert_eq!(moved_out(&read), [], "{:#?}", read.units);
  let said = units(&read).iter().filter(|unit| unit.3.is_some() || unit.4.is_some()).count();
  assert_eq!(said, 3, "the format moved and the legacy total came from the deleted file: {:#?}", read.units);
}

/// openspec/specs/branch/review-units/spec.md, "Units: The place a move left is nothing to read where it holds the move
/// alone": a function between two others without a blank line, its closing brace the line before the next one, and one
/// whose annotation the next one has too: lines the comparison could take from the code beside the function, which
/// is the move alone however it takes them
#[test]
fn a_function_moved_from_between_two_others_without_a_blank_line_is_the_move_alone() {
  let repo = TestRepo::new();
  for annotation in ["", "@Deprecated(\"old\")\n"] {
    let function = |name: &str, value: u32| format!("{annotation}fun {name}(): Int {{\n  val value = {value}\n  return value\n}}\n");
    let (first, moved, last) = (function("first", 1), function("moved", 2), function("last", 3));
    let base = repo.create_commit_with_files(
      "Add the functions",
      &[(ORDERS, &format!("package a\n\n{first}{moved}{last}")), (REPORT, &source(&[CLASS_REPORT]))],
    );
    let head = repo.create_commit_with_files(
      "Move the middle one",
      &[(ORDERS, &format!("package a\n\n{first}{last}")), (REPORT, &source(&[CLASS_REPORT, &moved]))],
    );
    let plan = plan(&repo, &base, &head);
    assert_eq!(
      moved_out(&plan),
      [(UnitKind::Removed, Some("moved".to_string()), ORDERS.to_string())],
      "{annotation}{:#?}",
      plan.units
    );
  }
}

/// openspec/specs/branch/review-units/spec.md, "In the Review Window: A declaration moved whole is read at its new
/// place": a function copied into the report by one commit and dropped from the orders by the next is a move of the
/// range of both, and the range of either holds one place of it: no move, a new and a removed function, read
#[test]
fn a_range_that_holds_one_place_of_a_move_says_nothing() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add the orders",
    &[(ORDERS, &source(&[CLASS_ORDERS, FORMAT_TOTAL, COUNT])), (REPORT, &source(&[CLASS_REPORT]))],
  );
  let copied = repo.create_commit_with_files("Format in the report", &[(REPORT, &source(&[CLASS_REPORT, FORMAT_TOTAL]))]);
  let dropped = repo.create_commit_with_files("Drop the old format", &[(ORDERS, &source(&[CLASS_ORDERS, COUNT]))]);
  let moved = |base: &str, head: &str| {
    let plan = plan(&repo, base, head);
    let read = units(&plan).into_iter().map(|unit| (unit.0, unit.3.is_some() || unit.4.is_some())).collect::<Vec<_>>();
    (read, moved_out(&plan).len())
  };
  assert_eq!(moved(&base, &dropped), (vec![(UnitKind::Removed, true), (UnitKind::Added, true)], 1));
  assert_eq!(moved(&base, &copied), (vec![(UnitKind::Added, false)], 0));
  assert_eq!(moved(&copied, &dropped), (vec![(UnitKind::Removed, false)], 0));
}
