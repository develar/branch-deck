//! Go to Declaration and Find Usages in Java from its PSI, beside `navigate_test.rs`: what the PSI reads apart from
//! tree-sitter's grammar, each a construct the corpus moved

use crate::highlight::Grammar;
use crate::navigate::{Access, Position, ReferenceAt, ReferenceKind, reference_at, usages_at, usages_of};
use pretty_assertions::assert_eq;

/// The place of the `occurrence`-th `needle` of the text, counted from 0
fn place(text: &str, needle: &str, occurrence: usize) -> Position {
  let offset = text.match_indices(needle).nth(occurrence).unwrap_or_else(|| panic!("no {needle}")).0;
  let line_start = text[..offset].rfind('\n').map_or(0, |it| it + 1);
  Position {
    line: u32::try_from(text[..offset].matches('\n').count()).unwrap(),
    column: u32::try_from(text[line_start..offset].encode_utf16().count()).unwrap(),
  }
}

fn at(text: &str, needle: &str, occurrence: usize) -> ReferenceAt {
  reference_at(Grammar::Java, text, place(text, needle, occurrence)).unwrap_or_else(|| panic!("no name at {needle}"))
}

/// The kind of the name at a needle and the line and column of its declaration
fn kind_and_local(text: &str, needle: &str, occurrence: usize) -> (ReferenceKind, Option<(u32, u32)>) {
  let reference = at(text, needle, occurrence);
  (reference.kind, reference.local.map(|it| (it.line, it.column)))
}

/// Whether the usages of the name at a needle are of a local, and their lines and accesses
fn usages(text: &str, needle: &str, occurrence: usize) -> (bool, Vec<(u32, Access)>) {
  let found = usages_at(Grammar::Java, text, place(text, needle, occurrence)).unwrap();
  (found.local, found.usages.iter().map(|it| (it.start.line, it.access)).collect())
}

#[test]
fn the_name_of_a_parameter_used_in_the_body_goes_to_the_parameter() {
  let text = "class A {\n  int f(int size) { return size + 1; }\n}\n";
  let reference = reference_at(Grammar::Java, text, Position { line: 1, column: 29 }).unwrap();
  assert_eq!((reference.name.as_str(), reference.kind), ("size", ReferenceKind::Value));
  assert_eq!(reference.local, Some(Position { line: 1, column: 12 }));
}

const VARIABLES: &str = "class A {
  String item;
  void f(Object o, java.util.List<String> list, String... rest) {
    for (String item : list) item.length();
    if (o instanceof String item) item.length();
    try (var item = open()) { item.close(); }
    list.forEach(item -> item.length());
    list.forEach((item, other) -> item.length());
    rest.clone();
  }
}
";

#[test]
fn a_loop_variable_a_pattern_variable_a_resource_and_a_lambda_parameter_declare_their_name() {
  // the name of each, after the field's
  for occurrence in [1, 3, 5, 7, 9] {
    assert_eq!(kind_and_local(VARIABLES, "item", occurrence), (ReferenceKind::Declaration, None), "item {occurrence}");
  }
  // each use goes to its own declaration, not to the field
  assert_eq!(kind_and_local(VARIABLES, "item.length", 0), (ReferenceKind::Value, Some((3, 16))));
  assert_eq!(kind_and_local(VARIABLES, "item.length", 1), (ReferenceKind::Value, Some((4, 28))));
  assert_eq!(kind_and_local(VARIABLES, "item.close", 0), (ReferenceKind::Value, Some((5, 13))));
  assert_eq!(kind_and_local(VARIABLES, "item.length", 2), (ReferenceKind::Value, Some((6, 17))));
  assert_eq!(kind_and_local(VARIABLES, "item.length", 3), (ReferenceKind::Value, Some((7, 18))));
  assert_eq!(kind_and_local(VARIABLES, "rest.clone", 0), (ReferenceKind::Value, Some((2, 58))));
  // the field has no usage in the file but its declaration
  assert_eq!(usages(VARIABLES, "item", 0), (false, vec![(1, Access::Declaration)]));
  assert_eq!(usages_of(Grammar::Java, VARIABLES, "item").len(), 1);
}

#[test]
fn a_use_before_the_declaration_of_a_local_is_the_field() {
  let text = "class Parser {\n  Reader reader;\n  void f() {\n    skip(reader);\n    Reader reader = wrap(this.reader);\n    reader.close();\n  }\n}\n";
  assert_eq!(kind_and_local(text, "reader", 1).1, Some((1, 9)));
  assert_eq!(kind_and_local(text, "reader.close", 0).1, Some((4, 11)));
}

#[test]
fn a_local_of_an_initializer_is_a_local() {
  let text = "class Client {\n  {\n    Runnable test = foo();\n    run(test);\n  }\n  static { int test = 1; }\n}\n";
  assert_eq!(usages(text, "test", 0), (true, vec![(2, Access::Declaration), (3, Access::Read)]));
}

#[test]
fn the_members_of_an_anonymous_class_are_its_own() {
  let text = "class A {\n  Object o = new Object() {\n    void save() {}\n  };\n  void f() { save(); }\n  static void save(int x) {}\n}\n";
  assert_eq!(kind_and_local(text, "save()", 1), (ReferenceKind::Call, Some((5, 14))));
}

#[test]
fn a_break_goes_to_its_label() {
  let text = "class A {\n  int outer;\n  void f() {\n    outer:\n    for (;;) { break outer; }\n  }\n}\n";
  assert_eq!(kind_and_local(text, "outer", 1), (ReferenceKind::Declaration, None));
  assert_eq!(kind_and_local(text, "outer;", 1), (ReferenceKind::Value, Some((3, 4))));
  assert_eq!(usages(text, "outer", 1), (true, vec![(3, Access::Declaration), (4, Access::Read)]));
}

#[test]
fn an_argument_of_an_annotation_goes_to_the_element_of_its_type_in_the_file() {
  let text = "@interface Range {\n  int value();\n}\nclass A {\n  int value;\n  @Range(value = 1) void f() {}\n  @Other(value = 2) void g() {}\n}\n";
  assert_eq!(kind_and_local(text, "value = 1", 0), (ReferenceKind::Value, Some((1, 6))));
  assert_eq!(kind_and_local(text, "value = 2", 0), (ReferenceKind::Value, None));
}

#[test]
fn var_and_this_are_no_names() {
  let text = "class A {\n  void f(@Nullable A this) { var x = 1; }\n}\n";
  assert_eq!(reference_at(Grammar::Java, text, place(text, "var", 0)), None);
  assert_eq!(reference_at(Grammar::Java, text, place(text, "this", 0)), None);
}

#[test]
fn a_call_through_super_of_an_interface_has_that_interface_for_its_qualifier() {
  let text = "class A implements Base {\n  public String name() { return Base.super.name() + super.toString(); }\n}\n";
  let name = at(text, "name()", 1);
  assert_eq!((name.kind, name.qualifier.as_deref()), (ReferenceKind::MemberCall, Some("Base")));
  let to_string = at(text, "toString", 0);
  assert_eq!((to_string.kind, to_string.qualifier.as_deref()), (ReferenceKind::MemberCall, Some("super")));
}

#[test]
fn a_compact_constructor_reads_the_record_component_and_a_record_pattern_names_a_type() {
  let text = "record User(String email) {\n  User {\n    check(email);\n  }\n  public String email() { return email; }\n  static boolean f(Object o) { return o instanceof User(String e) && e.isEmpty(); }\n}\n";
  assert_eq!(kind_and_local(text, "email", 1), (ReferenceKind::Value, Some((0, 19))));
  assert_eq!(kind_and_local(text, "User(", 1).0, ReferenceKind::Type);
}

#[test]
fn the_dotted_names_of_an_import_and_a_module_are_a_value_and_members_without_a_qualifier() {
  let text = "import static java.lang.Math.max;\nimport app.Store;\nclass A {}\n";
  let java = at(text, "java", 0);
  assert_eq!((java.kind, java.qualifier), (ReferenceKind::Value, None));
  for (needle, occurrence) in [("lang", 0), ("Math", 0), ("max", 0), ("Store", 0)] {
    let member = at(text, needle, occurrence);
    assert_eq!((member.kind, member.qualifier), (ReferenceKind::Member, None), "{needle}");
  }
  assert_eq!(usages(text, "Store", 0).1, [(1, Access::Import)]);
  let module = "module app.core {\n  requires java.sql;\n}\n";
  assert_eq!(at(module, "core", 0).kind, ReferenceKind::Member);
  assert_eq!(at("module app {\n}\n", "app", 0).kind, ReferenceKind::Declaration);
}

#[test]
fn a_write_is_the_left_side_of_an_assignment_and_the_operand_of_an_increment() {
  let text = "class A {\n  int total;\n  int[] all;\n  void f() {\n    this.total = 1;\n    --total;\n    all[total] = total += 2;\n  }\n}\n";
  assert_eq!(
    usages(text, "total", 0).1,
    [(1, Access::Declaration), (4, Access::Write), (5, Access::Write), (6, Access::Read), (6, Access::Write)]
  );
  assert_eq!(usages(text, "all", 1).1, [(2, Access::Declaration), (6, Access::Read)]);
}
