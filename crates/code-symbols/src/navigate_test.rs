//! The name at a place and its declaration in the same file, by the scopes of each grammar

use crate::highlight::Grammar;
use crate::navigate::{Access, Position, ReferenceAt, ReferenceKind, Usage, reference_at, usages_at, usages_of};
use pretty_assertions::assert_eq;

/// The reference at the first `needle` of the text, its `skip`-th occurrence on its line counted from 0
fn at(grammar: Grammar, text: &str, needle: &str, occurrence: usize) -> ReferenceAt {
  let (line, column) = text
    .lines()
    .enumerate()
    .flat_map(|(line, content)| content.match_indices(needle).map(move |(column, _)| (line, column)))
    .nth(occurrence)
    .unwrap_or_else(|| panic!("no {needle}"));
  reference_at(
    grammar,
    text,
    Position {
      line: u32::try_from(line).unwrap(),
      column: u32::try_from(column).unwrap(),
    },
  )
  .unwrap_or_else(|| panic!("no name at {needle}"))
}

fn local(reference: &ReferenceAt) -> Option<(u32, u32)> {
  reference.local.map(|it| (it.line, it.column))
}

const KOTLIN: &str = "class Box(val size: Int) {
  private var count = 0
  fun add(item: String, count: Int = 1) {
    this.count += count
    items.add(item)
    val local = size + 1
    run { local + count }
  }
  val items = mutableListOf<String>()
}
";

#[test]
fn finds_a_kotlin_local_a_parameter_that_shadows_a_property_and_a_member_declared_after_its_use() {
  // `count` in the body is the parameter, not the property
  let count = at(Grammar::Kotlin, KOTLIN, "count", 3);
  assert_eq!((count.kind, local(&count)), (ReferenceKind::Value, Some((2, 24))));
  // `items` is a property declared after the function
  let items = at(Grammar::Kotlin, KOTLIN, "items", 0);
  assert_eq!((items.kind, local(&items)), (ReferenceKind::Value, Some((8, 6))));
  // `.add` is a call of a member of `items`: for the review and the index
  let add = at(Grammar::Kotlin, KOTLIN, "add", 1);
  assert_eq!((add.kind, add.qualifier.as_deref(), add.local), (ReferenceKind::MemberCall, Some("items"), None));
  // a local and a property of the primary constructor
  assert_eq!(local(&at(Grammar::Kotlin, KOTLIN, "local", 1)), Some((5, 8)));
  assert_eq!(local(&at(Grammar::Kotlin, KOTLIN, "size", 1)), Some((0, 14)));
  // the name of a declaration has no declaration to go to
  assert_eq!(at(Grammar::Kotlin, KOTLIN, "add", 0).kind, ReferenceKind::Declaration);
}

#[test]
fn finds_a_java_field_a_local_and_tells_a_static_call_by_its_qualifier() {
  let java = "class A {\n  int count;\n  void run(int times) {\n    int left = times;\n    count += left;\n    Math.max(count, 1);\n  }\n}\n";
  assert_eq!(local(&at(Grammar::Java, java, "count", 1)), Some((1, 6)));
  assert_eq!(local(&at(Grammar::Java, java, "left", 1)), Some((3, 8)));
  assert_eq!(local(&at(Grammar::Java, java, "times", 1)), Some((2, 15)));
  let max = at(Grammar::Java, java, "max", 0);
  assert_eq!((max.kind, max.qualifier.as_deref()), (ReferenceKind::MemberCall, Some("Math")));
}

#[test]
fn finds_a_rust_binding_and_tells_a_method_and_a_path() {
  let rust = "fn run(items: Vec<u8>) {\n  let mut total = 0;\n  total += items.len();\n  let made = Vec::new();\n}\n";
  assert_eq!(local(&at(Grammar::Rust, rust, "total", 1)), Some((1, 10)));
  assert_eq!(local(&at(Grammar::Rust, rust, "items", 1)), Some((0, 7)));
  let len = at(Grammar::Rust, rust, "len", 0);
  assert_eq!((len.kind, len.qualifier.as_deref()), (ReferenceKind::MemberCall, Some("items")));
  let new = at(Grammar::Rust, rust, "new", 0);
  assert_eq!((new.kind, new.qualifier.as_deref()), (ReferenceKind::MemberCall, Some("Vec")));
}

#[test]
fn finds_a_typescript_parameter_a_hoisted_function_and_tells_a_property() {
  let typescript = "export function run(items: string[]) {\n  const total = count(items)\n  return this.size + total\n}\nfunction count(list: string[]) { return list.length }\n";
  assert_eq!(local(&at(Grammar::TypeScript, typescript, "items", 1)), Some((0, 20)));
  assert_eq!(local(&at(Grammar::TypeScript, typescript, "total", 1)), Some((1, 8)));
  let count = at(Grammar::TypeScript, typescript, "count", 0);
  assert_eq!((count.kind, local(&count)), (ReferenceKind::Call, Some((4, 9))));
  let size = at(Grammar::TypeScript, typescript, "size", 0);
  assert_eq!((size.kind, size.qualifier.as_deref()), (ReferenceKind::Member, Some("this")));
}

#[test]
fn has_no_reference_outside_a_name() {
  assert_eq!(reference_at(Grammar::Kotlin, "val a = 1\n", Position { line: 0, column: 8 }), None);
  assert_eq!(reference_at(Grammar::Kotlin, "val a = 1\n", Position { line: 5, column: 0 }), None);
}

#[test]
fn finds_a_called_function_declared_later_in_the_file() {
  let text = "fun run() = outsider() + inner()\n\nfun inner() = 2\n";
  let inner = at(Grammar::Kotlin, text, "inner", 0);
  assert_eq!((inner.kind, local(&inner)), (ReferenceKind::Call, Some((2, 4))));
}

/// The line, column and access of each usage
fn places(usages: &[Usage]) -> Vec<(u32, u32, Access)> {
  usages.iter().map(|it| (it.start.line, it.start.column, it.access)).collect()
}

/// Whether the usages at the `occurrence`-th `needle` are of a local, and where they are
fn usages(grammar: Grammar, text: &str, needle: &str, occurrence: usize) -> (bool, Vec<(u32, u32, Access)>) {
  let reference = at(grammar, text, needle, occurrence);
  let found = usages_at(grammar, text, reference.start).unwrap();
  assert_eq!(found.name, needle);
  (found.local, places(&found.usages))
}

#[test]
fn finds_the_usages_of_a_kotlin_parameter_apart_from_the_property_it_shadows() {
  // the parameter `count`: its declaration, its read on the right of `+=` and in the lambda
  assert_eq!(
    usages(Grammar::Kotlin, KOTLIN, "count", 3),
    (true, vec![(2, 24, Access::Declaration), (3, 18, Access::Read), (6, 18, Access::Read)])
  );
  // the property: its declaration and the write through `this`, which other files may use too
  assert_eq!(
    usages(Grammar::Kotlin, KOTLIN, "count", 0),
    (false, vec![(1, 14, Access::Declaration), (3, 9, Access::Write)])
  );
  // a call of a member
  assert_eq!(usages(Grammar::Kotlin, KOTLIN, "add", 0), (false, vec![(2, 6, Access::Declaration), (4, 10, Access::Call)]));
}

#[test]
fn tells_a_java_write_an_increment_and_an_import() {
  let text = "import app.Store;\n\nclass Uses {\n  int total;\n  void run(Store store) {\n    total = 1;\n    total++;\n    store.add(total);\n  }\n}\n";
  assert_eq!(
    usages(Grammar::Java, text, "total", 0),
    (
      false,
      vec![(3, 6, Access::Declaration), (5, 4, Access::Write), (6, 4, Access::Write), (7, 14, Access::Read)]
    )
  );
  assert_eq!(usages(Grammar::Java, text, "Store", 0).1, [(0, 11, Access::Import), (4, 11, Access::Read)]);
  // the parameter is a local
  assert_eq!(usages(Grammar::Java, text, "store", 0), (true, vec![(4, 17, Access::Declaration), (7, 4, Access::Read)]));
}

#[test]
fn finds_the_usages_of_a_rust_binding_and_of_an_imported_function() {
  let text = "use crate::util::helper;\n\nfn run() -> u32 {\n  let mut total = 0;\n  total += helper();\n  total\n}\n";
  assert_eq!(
    usages(Grammar::Rust, text, "total", 0),
    (true, vec![(3, 10, Access::Declaration), (4, 2, Access::Write), (5, 2, Access::Read)])
  );
  assert_eq!(usages(Grammar::Rust, text, "helper", 1), (false, vec![(0, 17, Access::Import), (4, 11, Access::Call)]));
}

#[test]
fn finds_the_usages_of_a_name_in_another_file_without_the_locals_that_shadow_it() {
  let text = "import { helper } from \"./util\"\n\nexport function run(items: string[]) {\n  const helper2 = helper(items)\n  return items.map(helper => helper.length)\n}\n";
  // the arrow function's parameter `helper` and its use are not the imported function
  assert_eq!(places(&usages_of(Grammar::TypeScript, text, "helper")), [(0, 9, Access::Import), (3, 18, Access::Call)]);
  assert_eq!(usages_of(Grammar::TypeScript, text, "absent"), []);
}
