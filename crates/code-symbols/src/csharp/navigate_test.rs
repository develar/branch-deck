//! Go to Declaration and Find Usages in C# from Roslyn's tree, beside `navigate_test.rs`: the scopes and the forms of
//! C# a name is read in

use crate::navigate::{Access, ReferenceKind, reference_at, usages_at, usages_of};
use crate::test_support::{at, kind_and_local, place, readings_unlike_the_compiler, usages};
use code_tokens::Grammar;
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

#[test]
fn the_variable_of_a_foreach_is_seen_in_its_statement_alone() {
  let text = "class A {\n  List<Item> item;\n  void F() {\n    foreach (var item in item) { Use(item); }\n    Use(item);\n  }\n}\n";
  // the collection is read before the variable is, and the statement after the loop reads the field again
  assert_eq!(kind_and_local(Grammar::CSharp, text, "item) {", 0).1, Some((1, 13)));
  assert_eq!(kind_and_local(Grammar::CSharp, text, "item); }", 0).1, Some((3, 17)));
  assert_eq!(kind_and_local(Grammar::CSharp, text, "item);\n", 0).1, Some((1, 13)));
  assert_eq!(usages(Grammar::CSharp, text, "item in", 0), (true, vec![(3, Access::Declaration), (3, Access::Read)]));
}

/// The statements of the sections of a `switch` share its block, a section alone has the variables of its labels, and
/// what the governing expression declares is of the statements around the `switch`
#[test]
fn the_sections_of_a_switch_share_its_block() {
  let text = "class A {\n  int total, n;\n  int F(int k) {\n    switch (Next(out var seen)) {\n      case 1: int total = 1; Use(total); break;\n      case int n when n > 0: total = 2; Use(Twice(n)); break;\n      default: Use(n); int Twice(int x) => x * 2; break;\n    }\n    return total + seen;\n  }\n}\n";
  let local = |needle: &str, occurrence: usize| kind_and_local(Grammar::CSharp, text, needle, occurrence).1;
  // the local of the first section is the one the second assigns; after the `switch` the field is read
  assert_eq!(
    (local("total); break", 0), local("total = 2", 0), local("total + seen", 0)),
    (Some((4, 18)), Some((4, 18)), Some((1, 6)))
  );
  // the variable of a label is of its section alone; the local function of a section is called from another
  assert_eq!(
    (local("n > 0", 0), local("n)); break", 0), local("n); int", 0)),
    (Some((5, 15)), Some((5, 15)), Some((1, 13)))
  );
  assert_eq!(local("Twice(n)", 0), Some((6, 27)));
  // the variable of the governing expression is seen after the `switch`
  assert_eq!(local("seen;", 0), Some((3, 25)));
  assert_eq!(
    usages(Grammar::CSharp, text, "total = 1", 0),
    (true, vec![(4, Access::Declaration), (4, Access::Read), (5, Access::Write)])
  );
}

/// A loop and a statement that is the body of another without a block end the variables their expressions declare;
/// the variable of an `if` and of a `lock` is of the statements around it
#[test]
fn a_loop_and_an_embedded_statement_end_the_variables_of_their_expressions() {
  let text = "class A {\n  int x, y, z, w, v;\n  void F(object o, bool c) {\n    while (o is int x) { Use(x); }\n    Use(x);\n    if (c) Use(out var y);\n    Use(y);\n    do { } while (Next(out var z));\n    Use(z);\n    lock (Get(out var w)) { Use(w); }\n    Use(w);\n    if (o is string s) { }\n    Use(s);\n    lock (o) Use(out var v);\n    Use(v);\n  }\n}\n";
  let local = |needle: &str, occurrence: usize| kind_and_local(Grammar::CSharp, text, needle, occurrence).1;
  assert_eq!((local("x); }", 0), local("x);\n", 0)), (Some((3, 20)), Some((1, 6))));
  assert_eq!(local("y);\n    do", 0), Some((1, 9)));
  assert_eq!(local("z);\n    lock", 0), Some((1, 12)));
  // what the expression of a `lock` declares lives on after it, as what the condition of an `if` does
  assert_eq!((local("w); }", 0), local("w);\n    if", 0)), (Some((9, 22)), Some((9, 22))));
  assert_eq!(local("s);", 0), Some((11, 20)));
  // but not what its body without a block declares
  assert_eq!(local("v);\n  }", 0), Some((1, 18)));
  assert_eq!(usages(Grammar::CSharp, text, "x)", 0), (true, vec![(3, Access::Declaration), (3, Access::Read)]));
  assert_eq!(
    usages(Grammar::CSharp, text, "w)", 0),
    (true, vec![(9, Access::Declaration), (9, Access::Read), (10, Access::Read)])
  );
}

/// The top-level statements of a file share one scope: a local of one is read by the next and by a local function
#[test]
fn the_top_level_statements_of_a_file_share_a_scope() {
  let text = "var total = 1;\nSystem.Console.WriteLine(total);\nint Twice(int x) => x * total;\nSystem.Console.WriteLine(Twice(total));\n";
  let local = |needle: &str, occurrence: usize| kind_and_local(Grammar::CSharp, text, needle, occurrence).1;
  assert_eq!(
    (local("total);", 0), local("total;", 0), local("Twice(total)", 0)),
    (Some((0, 4)), Some((0, 4)), Some((2, 4)))
  );
  assert_eq!(
    usages(Grammar::CSharp, text, "total", 0),
    (true, vec![(0, Access::Declaration), (1, Access::Read), (2, Access::Read), (3, Access::Read)])
  );
}

/// A query reads its clauses in their order: the source of a `from`, the value of a `let` and the left key of a `join`
/// before their own variable, the source of a `join` before every variable of the query, the right key the variable
/// of the `join` alone; `into` ends the variables before it. The collection of a `foreach` is read before the
/// variables of its deconstruction
#[test]
fn a_query_reads_its_clauses_in_their_order() {
  let text = "class A {\n  Box item, part;\n  object F(int[] xs) {\n    var a = from item in item.Parts let part = part + item select part;\n    var b = from x in xs join item in item.Parts on item equals item into g select g;\n    var c = from x in xs group x by x into g select item;\n    foreach (var (item, n) in item.Pairs) { Use(item, n); }\n    var d = from item in xs join y in item.Parts on item equals y select item;\n    return from x in xs group x by x into g select x;\n  }\n}\n";
  let local = |needle: &str, occurrence: usize| kind_and_local(Grammar::CSharp, text, needle, occurrence).1;
  // the source of the `from` and the value of the `let` read the fields, what follows the range variables
  assert_eq!(
    (local("item.Parts let", 0), local("part + item", 0), local("item select", 0)),
    (Some((1, 6)), Some((1, 12)), Some((3, 17)))
  );
  assert_eq!(local("part;", 1), Some((3, 40)));
  // the source and the left key of the `join` read the field, its right key the variable of the `join`
  assert_eq!(
    (local("item.Parts on", 0), local("item equals", 0), local("item into", 0)),
    (Some((1, 6)), Some((1, 6)), Some((4, 30)))
  );
  assert_eq!(local("g;", 0), Some((4, 74)));
  // after an `into` the variables before it are gone: the field, and no declaration at all
  assert_eq!((local("item;", 0), local("x;", 0)), (Some((1, 6)), None));
  // the collection of the `foreach` is the field's, its body reads the variable
  assert_eq!((local("item.Pairs", 0), local("item, n);", 0)), (Some((1, 6)), Some((6, 18))));
  // the source of a `join` sees no variable of its query, the `from` before it either: the field; its keys and
  // what follows read the variables
  assert_eq!(
    (local("item.Parts on", 1), local("item equals", 1), local("item;", 1)),
    (Some((1, 6)), Some((7, 17)), Some((7, 17)))
  );
}

/// A name where C# reads a type is of a type, never of a variable; the member an initializer names and the name of
/// a `Name =` are members, which no scope binds
#[test]
fn a_type_and_a_member_an_initializer_names_are_of_no_variable() {
  let text = "class total {}\nclass A {\n  object F(int Size) {\n    var total = 1;\n    total typed = null;\n    var box = new Box { Size = Size, total = total };\n    return new { Size = total, typed, box };\n  }\n}\n";
  let found = |needle: &str, occurrence: usize| kind_and_local(Grammar::CSharp, text, needle, occurrence);
  assert_eq!(found("total typed", 0), (ReferenceKind::Type, Some((0, 6))));
  assert_eq!(
    (found("Size = Size", 0), found("Size,", 0)),
    ((ReferenceKind::Member, None), (ReferenceKind::Value, Some((2, 15))))
  );
  assert_eq!(
    (found("total = total }", 0), found("total }", 0)),
    ((ReferenceKind::Member, None), (ReferenceKind::Value, Some((3, 8))))
  );
  assert_eq!(found("Size = total", 0), (ReferenceKind::Member, None));
  assert_eq!(
    usages(Grammar::CSharp, text, "total = 1", 0),
    (true, vec![(3, Access::Declaration), (5, Access::Read), (6, Access::Read)])
  );
}

/// The right side of an `is` is a type where one of the name is declared, else a constant, a constant pattern: a
/// constant of the class, a local constant
#[test]
fn the_right_side_of_an_is_is_a_constant_where_no_type_of_the_name_is_declared() {
  let text = "class Max {}\nclass A {\n  const int Top = 5;\n  bool F(object x) {\n    const int Limit = 1, Max = 2;\n    return x is Top || x is Limit || x is Max || x is not Limit || x is Other;\n  }\n}\n";
  let found = |needle: &str, occurrence: usize| kind_and_local(Grammar::CSharp, text, needle, occurrence);
  assert_eq!(found("Top ||", 0), (ReferenceKind::Type, Some((2, 12))));
  assert_eq!(found("Limit ||", 0), (ReferenceKind::Type, Some((4, 14))));
  // a type of the name comes before a constant of it, a local one too
  assert_eq!(found("Max ||", 0), (ReferenceKind::Type, Some((0, 6))));
  // a pattern after `not` is a constant by its syntax; a name nothing declares has no declaration
  assert_eq!(found("Limit ||", 1), (ReferenceKind::Value, Some((4, 14))));
  assert_eq!(found("Other", 0), (ReferenceKind::Type, None));
  assert_eq!(
    usages(Grammar::CSharp, text, "Limit", 0),
    (true, vec![(4, Access::Declaration), (5, Access::Read), (5, Access::Read)])
  );
}

/// A variable an expression declares, and any other local, is seen where C# scopes it, in every statement, member
/// and clause that may hold one. The readings of `testdata/navigate/Scopes.cs` are the compiler's (csc 3.9)
#[test]
fn a_variable_of_an_expression_is_seen_where_the_compiler_sees_it() {
  let (wrong, reads) = readings_unlike_the_compiler(Grammar::CSharp, "Scopes.cs", (3, 13));
  assert!(wrong.is_empty(), "{} of {reads} reads go wrong:\n{}", wrong.len(), wrong.join("\n"));
  assert_eq!(reads, 141);
}
