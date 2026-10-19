//! The name at a place and its declaration in the same file, by the scopes of each grammar

use crate::highlight::Grammar;
use crate::navigate::{Position, ReferenceAt, ReferenceKind, reference_at};
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
