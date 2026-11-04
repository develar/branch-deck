//! Go to Declaration and Find Usages in C# from Roslyn's tree, beside `navigate_test.rs`: the scopes and the forms of
//! C# a name is read in

use crate::highlight::Grammar;
use crate::navigate::{Access, ReferenceKind, reference_at, usages_at, usages_of};
use crate::test_support::{at, kind_and_local, place, usages};
use pretty_assertions::assert_eq;

const SCOPES: &str = "class A<T> {
  int count;
  public int Count => count;
  void F(int size, out int result) {
    result = size + count;
    int Local() => size;
    var total = Local();
    foreach (var item in Items()) item.Go();
    if (Parse(out var parsed)) parsed++;
    goto done;
    done: return;
  }
  T Make() => default;
}
";

#[test]
fn a_name_goes_to_its_declaration_by_the_scopes() {
  // a parameter, a field, a local function declared after the use's statement, a pattern variable
  assert_eq!(kind_and_local(Grammar::CSharp, SCOPES, "size", 1), (ReferenceKind::Value, Some((3, 13))));
  assert_eq!(kind_and_local(Grammar::CSharp, SCOPES, "count", 2), (ReferenceKind::Value, Some((1, 6))));
  assert_eq!(kind_and_local(Grammar::CSharp, SCOPES, "Local", 1), (ReferenceKind::Call, Some((5, 8))));
  assert_eq!(kind_and_local(Grammar::CSharp, SCOPES, "parsed", 1), (ReferenceKind::Value, Some((8, 22))));
  assert_eq!(kind_and_local(Grammar::CSharp, SCOPES, "item", 1).1, Some((7, 17)));
  // a type parameter as a type, a label
  assert_eq!(kind_and_local(Grammar::CSharp, SCOPES, "T Make", 0), (ReferenceKind::Type, Some((0, 8))));
  assert_eq!(kind_and_local(Grammar::CSharp, SCOPES, "done;", 0), (ReferenceKind::Value, Some((10, 4))));
}

#[test]
fn a_member_after_a_qualifier_has_its_qualifier_and_no_declaration() {
  let text = "class A { int count; void F(A other) { other.count = 1; this.count++; other?.F(null); System.Console.WriteLine(); } }\n";
  let member = at(Grammar::CSharp, text, "count = 1", 0);
  assert_eq!((member.kind, member.qualifier.as_deref(), member.local), (ReferenceKind::Member, Some("other"), None));
  assert_eq!(at(Grammar::CSharp, text, "F(null)", 0).kind, ReferenceKind::MemberCall);
  assert_eq!(at(Grammar::CSharp, text, "F(null)", 0).qualifier.as_deref(), Some("other"));
  assert_eq!(at(Grammar::CSharp, text, "WriteLine", 0).qualifier.as_deref(), Some("System.Console"));
}

#[test]
fn the_names_of_a_namespace_and_a_using_are_values_and_members() {
  let text = "using System.Text;\nnamespace Example.App;\nclass A { System.Text.StringBuilder b; }\n";
  assert_eq!(at(Grammar::CSharp, text, "System", 0).kind, ReferenceKind::Value);
  assert_eq!(at(Grammar::CSharp, text, "Text", 0).kind, ReferenceKind::Member);
  assert_eq!(at(Grammar::CSharp, text, "App", 0).kind, ReferenceKind::Member);
  // in a type, every part of the name is a type
  assert_eq!(at(Grammar::CSharp, text, "Text.StringBuilder", 0).kind, ReferenceKind::Type);
  assert_eq!(at(Grammar::CSharp, text, "StringBuilder", 0).kind, ReferenceKind::Type);
}

#[test]
fn the_usages_of_a_local_are_its_own_and_of_a_member_all_unbound() {
  // a parameter written by an assignment and read: only its own
  assert_eq!(
    usages(Grammar::CSharp, SCOPES, "size", 0),
    (true, vec![(3, Access::Declaration), (4, Access::Read), (5, Access::Read)])
  );
  // `out var` declares, `++` writes, an `out` argument of a declared name writes
  assert_eq!(usages(Grammar::CSharp, SCOPES, "parsed", 0), (true, vec![(8, Access::Declaration), (8, Access::Write)]));
  assert_eq!(usages(Grammar::CSharp, SCOPES, "result", 0), (true, vec![(3, Access::Declaration), (4, Access::Write)]));
  // a field: its declaration and its uses
  assert_eq!(
    usages(Grammar::CSharp, SCOPES, "count", 0),
    (false, vec![(1, Access::Declaration), (2, Access::Read), (4, Access::Read)])
  );
}

#[test]
fn the_usages_in_another_file_are_the_names_no_local_binds() {
  let text = "class B { void G(int Count) { Count++; } void H(A a) { var n = a.Count; } }\n";
  let lines: Vec<(u32, Access)> = usages_of(Grammar::CSharp, text, "Count").iter().map(|it| (it.start.line, it.access)).collect();
  assert_eq!(lines, [(0, Access::Read)]);
}

#[test]
fn a_verbatim_identifier_is_found_by_its_value() {
  let text = "class A { int @event; void F() { @event = 1; } }\n";
  let found = usages_at(Grammar::CSharp, text, place(text, "event = 1", 0)).unwrap();
  assert_eq!(found.name, "event");
  assert_eq!(
    found.usages.iter().map(|it| (it.start.column, it.access)).collect::<Vec<_>>(),
    [(15, Access::Declaration), (34, Access::Write)]
  );
}

#[test]
fn a_name_in_a_documentation_comment_or_a_directive_is_none() {
  let text = "/// <see cref=\"A\"/>\n#if DEBUG\nclass A {}\n#endif\n";
  assert_eq!(reference_at(Grammar::CSharp, text, place(text, "A\"", 0)), None);
  assert_eq!(reference_at(Grammar::CSharp, text, place(text, "DEBUG", 0)), None);
}

#[test]
fn a_goto_goes_to_the_label_of_its_name_in_the_blocks_around_it() {
  let text = "class A {\n  void F(bool a) {\n    if (a) {\n      goto again;\n      again: return;\n    } else {\n      again: goto again;\n    }\n  }\n}\n";
  // the label of the `if`'s block for the `goto` of that block, the `else`'s for the `goto` of the `else`
  assert_eq!(kind_and_local(Grammar::CSharp, text, "again;", 0).1, Some((4, 6)));
  assert_eq!(kind_and_local(Grammar::CSharp, text, "again;", 1).1, Some((6, 6)));
}
