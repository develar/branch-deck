//! Go to Declaration and Find Usages in Kotlin from its PSI, beside `navigate_test.rs`: each reading where it differs
//! from tree-sitter's, by the construct

use crate::highlight::Grammar;
use crate::navigate::{Access, Position, ReferenceAt, ReferenceKind, UsagesAt, position_of, reference_at, usages_at, usages_of};
use pretty_assertions::assert_eq;

/// The place of the `occurrence`-th `needle` of a text, counted from 0
fn place(text: &str, needle: &str, occurrence: usize) -> Position {
  let offset = text.match_indices(needle).nth(occurrence).unwrap_or_else(|| panic!("no {needle}")).0;
  position_of(text, offset)
}

fn at(text: &str, needle: &str, occurrence: usize) -> ReferenceAt {
  reference_at(Grammar::Kotlin, text, place(text, needle, occurrence)).unwrap_or_else(|| panic!("no name at {needle}"))
}

fn usages(text: &str, needle: &str, occurrence: usize) -> UsagesAt {
  usages_at(Grammar::Kotlin, text, place(text, needle, occurrence)).unwrap_or_else(|| panic!("no name at {needle}"))
}

/// The line, column and access of each usage
fn places(usages: &UsagesAt) -> Vec<(u32, u32, Access)> {
  usages.usages.iter().map(|it| (it.start.line, it.start.column, it.access)).collect()
}

#[test]
fn the_name_of_a_parameter_used_in_the_body_goes_to_the_parameter() {
  let text = "fun f(size: Int) = size + 1\n";
  let reference = reference_at(Grammar::Kotlin, text, Position { line: 0, column: 19 }).unwrap();
  assert_eq!((reference.name.as_str(), reference.kind), ("size", ReferenceKind::Value));
  assert_eq!(reference.local, Some(Position { line: 0, column: 6 }));
  assert!(usages_of(Grammar::Kotlin, text, "size").is_empty(), "a parameter is a local");
}

#[test]
fn the_parameter_of_a_catch_is_a_local_declaration() {
  let text = "fun f() {\n  try { g() } catch (e: Exception) { log(e) }\n}\nval e = 1\n";
  assert_eq!(at(text, "e:", 0).kind, ReferenceKind::Declaration);
  assert_eq!(at(text, "e)", 0).local, Some(Position { line: 1, column: 21 }));
  let found = usages(text, "e)", 0);
  assert!(found.local);
  assert_eq!(places(&found), [(1, 21, Access::Declaration), (1, 41, Access::Read)]);
  // the property of the file is another declaration
  assert_eq!(usages_of(Grammar::Kotlin, text, "e").len(), 1);
}

#[test]
fn a_name_after_a_receiver_and_colons_is_its_member() {
  let text = "class Foo(val bar: Int) {\n  fun f(bar: Int) = listOf(Foo::bar, this::bar, ::bar, Foo::class)\n}\n";
  let qualified = at(text, "bar,", 0);
  assert_eq!(
    (qualified.kind, qualified.qualifier.as_deref(), qualified.local),
    (ReferenceKind::Member, Some("Foo"), None)
  );
  let this = at(text, "bar,", 1);
  assert_eq!((this.kind, this.qualifier.as_deref()), (ReferenceKind::Member, Some("this")));
  // without a receiver, the scopes: the parameter
  let bare = at(text, "bar,", 2);
  assert_eq!((bare.kind, bare.local), (ReferenceKind::Value, Some(Position { line: 1, column: 8 })));
  // the receiver is a type
  let class = at(text, "Foo::class", 0);
  assert_eq!((class.kind, class.local), (ReferenceKind::Type, Some(Position { line: 0, column: 6 })));
  // the property's usages are those through a receiver, not the parameter's
  assert_eq!(
    places(&usages(text, "bar", 0)),
    [(0, 14, Access::Declaration), (1, 32, Access::Read), (1, 43, Access::Read)]
  );
}

#[test]
fn the_name_of_a_package_of_one_segment_is_one_usage() {
  let text = "package app\n\nimport app.Store\n\nval app = Store()\n";
  let found = usages(text, "app", 0);
  assert_eq!(places(&found), [(0, 8, Access::Read), (2, 7, Access::Import), (4, 4, Access::Declaration)]);
}

#[test]
fn the_subject_of_a_where_clause_is_the_type_parameter() {
  let text = "fun <T> f(t: T) where T : Comparable<T> = t\n";
  let subject = at(text, "T :", 0);
  assert_eq!((subject.kind, subject.local), (ReferenceKind::Type, Some(Position { line: 0, column: 5 })));
}

#[test]
fn the_qualifier_of_an_assigned_member_is_its_whole_receiver() {
  let text = "fun f(e: Event) {\n  e.presentation.isEnabled = true\n}\n";
  let member = at(text, "isEnabled", 0);
  assert_eq!((member.kind, member.qualifier.as_deref()), (ReferenceKind::Member, Some("e.presentation")));
  assert_eq!(places(&usages(text, "isEnabled", 0)), [(1, 17, Access::Write)]);
}

#[test]
fn a_call_with_type_arguments_is_a_call_and_an_increment_a_write() {
  let text = "fun f() {\n  var n = service<Api>().count()\n  n++\n  --n\n}\n";
  assert_eq!(at(text, "service", 0).kind, ReferenceKind::Call);
  assert_eq!(places(&usages(text, "n =", 0)), [(1, 6, Access::Declaration), (2, 2, Access::Write), (3, 4, Access::Write)]);
}

#[test]
fn a_label_and_a_doc_comment_have_no_names_but_the_label_of_this() {
  let text = "/** See [run] */\nclass Outer {\n  fun run() = list.forEach loop@{ return@loop }\n  val me = this@Outer\n}\n";
  assert_eq!(reference_at(Grammar::Kotlin, text, place(text, "run]", 0)), None);
  assert_eq!(reference_at(Grammar::Kotlin, text, place(text, "loop@", 0)), None);
  assert_eq!(reference_at(Grammar::Kotlin, text, place(text, "loop }", 0)), None);
  let outer = reference_at(Grammar::Kotlin, text, Position { line: 3, column: 16 }).unwrap();
  assert_eq!((outer.kind, outer.local), (ReferenceKind::Type, Some(Position { line: 1, column: 6 })));
}

#[test]
fn a_script_declares_its_top_level_names_for_its_functions() {
  let text = "val v = 1\nfun f() = v\n";
  let reference = reference_at(Grammar::KotlinScript, text, Position { line: 1, column: 10 }).unwrap();
  assert_eq!((reference.kind, reference.local), (ReferenceKind::Value, Some(Position { line: 0, column: 4 })));
}

#[test]
fn a_local_outside_a_function_is_a_local() {
  // a catch of a property's initializer, a local of an `init` block and the subject of a `when` are seen by no other
  // file, as a local of a function
  let text = "class A {\n  val p = try { 1 } catch (t: Throwable) { t.hashCode() }\n  val q = try { 2 } catch (t: Throwable) { t.hashCode() }\n  init { val x = 1; println(x) }\n  val r = when (val s = p) { 1 -> s else -> 0 }\n}\nval s = 1\n";
  let found = usages(text, "t:", 0);
  assert!(found.local, "{found:?}");
  assert_eq!(places(&found), [(1, 27, Access::Declaration), (1, 43, Access::Read)]);
  assert!(usages_of(Grammar::Kotlin, text, "t").is_empty(), "a catch parameter is a local");
  assert!(usages(text, "x)", 0).local);
  let subject = usages(text, "s =", 0);
  assert!(subject.local, "{subject:?}");
  assert_eq!(places(&subject), [(4, 20, Access::Declaration), (4, 34, Access::Read)]);
  // the property of the file is another declaration, which the `when` does not see past its end
  assert_eq!(usages_of(Grammar::Kotlin, text, "s").len(), 1);
}

#[test]
fn a_parameter_of_a_function_type_declares_nothing_the_code_sees() {
  // `(event: String) -> Unit` names its parameter for documentation only: no scope sees it
  let text = "class A(val event: Int) {\n  val listener: (event: String) -> Unit = {}\n  fun f(cb: (event: Int) -> Unit) = event\n}\n";
  let used = at(text, "event\n", 0);
  assert_eq!((used.kind, used.local), (ReferenceKind::Value, Some(Position { line: 0, column: 12 })));
  let declared = at(text, "event: String", 0);
  assert_eq!(declared.kind, ReferenceKind::Declaration);
  let found = usages(text, "event: String", 0);
  assert!(found.local, "{found:?}");
  assert_eq!(places(&found), [(1, 17, Access::Declaration)]);
  assert_eq!(places(&usages(text, "event", 0)), [(0, 12, Access::Declaration), (2, 36, Access::Read)]);
}

#[test]
fn the_usages_of_a_name_are_at_their_places_in_one_pass_over_a_crlf_text() {
  // the places come from one pass over the text (`navigate::Places`): lines by `\r\n`, columns in UTF-16
  let text = "val n = 1\r\nfun f() {\r\n  val s = \"𝔘\" + n\r\n  n.inc(); g(n)\r\n}\r\n";
  let found = usages_of(Grammar::Kotlin, text, "n");
  let expected: Vec<Position> = text.match_indices('n').map(|(offset, _)| position_of(text, offset)).collect();
  let expected = [expected[0], expected[2], expected[3], expected[5]];
  assert_eq!(found.iter().map(|it| it.start).collect::<Vec<_>>(), expected);
  assert_eq!(found[1].start, Position { line: 2, column: 17 });
}

#[test]
fn code_reads_in_its_order_a_variable_after_its_statement_and_a_loop_variable_after_its_range() {
  let text = "fun f(name: String, items: List<Item>) {\n  use(count)\n  val name = name.trim()\n  for (items in items) { use(items) }\n  when (val count = count()) { else -> use(count) }\n  val count = 1\n  use(name, count)\n}\nval count = 0\n";
  let local = |needle: &str, occurrence: usize| at(text, needle, occurrence).local.map(|it| (it.line, it.column));
  // a name before the declaration of its block is the property of the file
  assert_eq!(local("count)", 0), Some((8, 4)));
  // the value of a variable reads the name of a scope around it: the parameter
  assert_eq!(local("name.trim", 0), Some((0, 6)));
  // the range of a `for` is read before its variable is, its body after
  assert_eq!(local("items) {", 0), Some((0, 20)));
  assert_eq!(local("items) }", 0), Some((3, 7)));
  // the subject of a `when` reads the file's `count` and is what its branches read
  assert_eq!(local("count())", 0), Some((8, 4)));
  assert_eq!(local("count) }", 0), Some((4, 12)));
  // after their statements, the variables
  assert_eq!((local("name, count", 0), local("count)\n}", 0)), (Some((2, 6)), Some((5, 6))));
  // so the usages of the parameter are its own and the value read from it, those of the variable what follows it
  assert_eq!(places(&usages(text, "name", 0)), [(0, 6, Access::Declaration), (2, 13, Access::Read)]);
  assert_eq!(places(&usages(text, "name", 1)), [(2, 6, Access::Declaration), (6, 6, Access::Read)]);
}

/// A local is found before a member of an implicit receiver: a property of a local class or an object gives way to a
/// variable of the function around it, as a member of the class of the function gives way to none of the file
#[test]
fn a_member_of_a_class_inside_a_body_gives_way_to_a_local_of_the_body() {
  let text = "val limit = 0\nclass Outer {\n  val limit = 1\n  fun f(): String {\n    val total = \"local\"\n    val sum = \"outer\"\n    val o = object { val sum = \"member\"; val size = 1; fun g() = sum + total + size + limit }\n    class Box(val total: Int) { fun g() = total }\n    return o.g() + total\n  }\n}\n";
  let local = |needle: &str, occurrence: usize| at(text, needle, occurrence).local.map(|it| (it.line, it.column));
  // the variable of the function, though the object declares a property of the name
  assert_eq!(local("sum + total", 0), Some((5, 8)));
  assert_eq!(local("total + size", 0), Some((4, 8)));
  // a member no local hides is the member; a member of the class of the function comes before the file
  assert_eq!((local("size + limit", 0), local("limit }", 0)), (Some((6, 45)), Some((2, 6))));
  // the `val` of a constructor is a member too
  assert_eq!(local("total }", 0), Some((4, 8)));
  assert_eq!(
    places(&usages(text, "total", 0)),
    [(4, 8, Access::Declaration), (6, 71, Access::Read), (7, 42, Access::Read), (8, 19, Access::Read)]
  );
}

/// The condition of a `do … while` sees what the block of its body declares; a name where a type is written is of no
/// value, and the name of a named argument of no local
#[test]
fn a_do_while_condition_sees_its_body_and_a_type_or_a_named_argument_is_no_local() {
  let text = "val more = false\nfun f(total: Int) {\n  var n = 0\n  do { val more = n++ < 3 } while (more)\n  val typed: total = report(total = total)\n  use(more, typed)\n}\n";
  let found = |needle: &str, occurrence: usize| {
    let reference = at(text, needle, occurrence);
    (reference.kind, reference.local.map(|it| (it.line, it.column)))
  };
  // the condition reads the variable of the body, the statement after the loop the property of the file
  assert_eq!(found("more)", 0), (ReferenceKind::Value, Some((3, 11))));
  assert_eq!(found("more, typed", 0), (ReferenceKind::Value, Some((0, 4))));
  assert_eq!(places(&usages(text, "more = n", 0)), [(3, 11, Access::Declaration), (3, 35, Access::Read)]);
  // a type of the spelling of the parameter is no parameter, the name of the argument the parameter of `report`
  assert_eq!(found("total = report", 0), (ReferenceKind::Type, None));
  assert_eq!(found("total = total", 0), (ReferenceKind::Member, None));
  assert_eq!(places(&usages(text, "total:", 0)), [(1, 6, Access::Declaration), (4, 36, Access::Read)]);
}
