//! Go to Declaration and Find Usages of Rust on rust-analyzer's tree, one test per class of the names the oracle moved
//! from tree-sitter's (docs/design/symbol-dependencies.md, decision 26)

use crate::highlight::Grammar;
use crate::navigate::{Access, Position, ReferenceKind, reference_at, usages_of};
use crate::test_support::{at, kind_and_local, place, usages};

const RUST: Grammar = Grammar::Rust;

/// A binding of any pattern is a declaration its uses bind to: `if let`, a `for`, a match arm, a closure's parameter
/// without a type, a field of a struct pattern; the nearest one shadows those around it
#[test]
fn a_binding_of_any_pattern_is_a_declaration() {
  let text = "fn f(items: Vec<u8>, before: u8) {\n  if let Some(first) = items.first() { use_it(first); }\n  for item in &items { use_it(item); }\n  match before { before => use_it(before) }\n  items.iter().map(|x| x + 1);\n  let S { field } = s;\n  use_it(field);\n}\n";
  assert_eq!(kind_and_local(RUST, text, "first)", 0), (ReferenceKind::Declaration, None));
  assert_eq!(kind_and_local(RUST, text, "first)", 1), (ReferenceKind::Value, Some((1, 14))));
  assert_eq!(kind_and_local(RUST, text, "item)", 0), (ReferenceKind::Value, Some((2, 6))));
  assert_eq!(
    kind_and_local(RUST, text, "before)", 0),
    (ReferenceKind::Value, Some((3, 17))),
    "the arm's, not the parameter"
  );
  assert_eq!(kind_and_local(RUST, text, "x + 1", 0), (ReferenceKind::Value, Some((4, 20))));
  assert_eq!(kind_and_local(RUST, text, "field)", 0), (ReferenceKind::Value, Some((5, 10))));
  assert_eq!(
    usages(RUST, text, "before", 0),
    (true, vec![(0, Access::Declaration), (3, Access::Read)]),
    "the scrutinee is the parameter, the arm's binding shadows it in the arm"
  );
}

/// A capitalized name alone in a pattern is a variant or a constant, and a path in a pattern is no binding
#[test]
fn a_variant_in_a_pattern_is_no_declaration() {
  let text = "fn f(v: Value) {\n  let Some(Value::Array(items)) = v.get() else { return };\n  match v { None => {} _ => {} }\n}\n";
  assert_eq!(at(RUST, text, "Some", 0).kind, ReferenceKind::Value);
  assert_eq!(at(RUST, text, "Value", 1).kind, ReferenceKind::Value);
  assert_eq!(at(RUST, text, "Array", 0).kind, ReferenceKind::Member);
  assert_eq!(kind_and_local(RUST, text, "None", 0), (ReferenceKind::Value, None));
}

/// The attributes of an item are in its node, and their names in its scopes: `skip(ctx)` names the parameter
#[test]
fn the_names_of_an_attribute_are_in_the_scopes_of_its_item() {
  let text = "#[instrument(skip(ctx))]\nfn run(ctx: &Context) {\n  ctx.go();\n}\n";
  assert_eq!(kind_and_local(RUST, text, "ctx", 0), (ReferenceKind::Value, Some((1, 7))));
  assert_eq!(usages(RUST, text, "ctx", 1).1.len(), 3);
}

/// A lifetime, a label, `Self`, `_`, the predicates of `#[cfg]`, the metavariables and fragments of a macro and the
/// keywords among its tokens are no names
#[test]
fn what_is_no_name() {
  let text = "#[cfg(test)]\nfn f<'a>(x: &'a Self) -> Vec<_> {\n  'outer: loop { break 'outer; }\n  m!(default, union);\n}\nmacro_rules! m { ($x:ident) => { $x } }\n";
  for (needle, occurrence) in [
    ("cfg", 0),
    ("test", 0),
    ("a", 0),
    ("Self", 0),
    ("_", 0),
    ("outer", 0),
    ("default", 0),
    ("union", 0),
    ("x:ident", 0),
    ("ident", 0),
  ] {
    assert_eq!(reference_at(RUST, text, place(text, needle, occurrence)), None, "{needle}");
  }
  assert_eq!(at(RUST, text, "m", 0).kind, ReferenceKind::Value, "a macro's path is a name");
}

/// A method called with type arguments is a call (`error.downcast::<T>()`), a segment with type arguments before `::`
/// a type, and an alias a declaration
#[test]
fn the_kinds_of_paths_and_calls() {
  let text = "use std::io::Error as IoError;\nfn f(error: Box<dyn E>) {\n  error.downcast::<IoError>();\n  Vec::<u8>::new();\n}\n";
  let downcast = at(RUST, text, "downcast", 0);
  assert_eq!((downcast.kind, downcast.qualifier.as_deref()), (ReferenceKind::MemberCall, Some("error")));
  assert_eq!(at(RUST, text, "Vec", 0).kind, ReferenceKind::Type);
  assert_eq!(at(RUST, text, "IoError", 0).kind, ReferenceKind::Declaration);
  assert_eq!(kind_and_local(RUST, text, "IoError", 1), (ReferenceKind::Type, Some((0, 22))));
  let usages = usages_of(RUST, text, "IoError");
  assert_eq!(usages.iter().map(|it| it.access).collect::<Vec<_>>(), [Access::Declaration, Access::Read]);
}

/// A file nested 20 000 deep is navigated on a thread of 2 MiB; one past what any code nests has no answer
#[test]
fn deep_nesting_is_navigated_on_a_small_stack() {
  let thread = std::thread::Builder::new().stack_size(2 << 20).spawn(|| {
    let deep = format!("fn f(x: u8) {}x{}\n", "{ ".repeat(20_000), " }".repeat(20_000));
    let reference = at(RUST, &deep, "x", 1);
    assert_eq!(reference.local, Some(Position { line: 0, column: 5 }));
    assert_eq!(usages_of(RUST, &deep, "x").len(), 0, "a parameter is a local");
    let too_deep = format!("fn g() {}\n", "(".repeat(rust_syntax::nesting::MAX_LEVELS + 1));
    assert_eq!(reference_at(RUST, &too_deep, Position { line: 0, column: 3 }), None);
  });
  thread.unwrap().join().unwrap();
}

/// A binding is seen where the code reads it: the one of a `let` after its statement, of an `if let` and a `while let`
/// in the block they guard, of a `for` in its body; before and after, the name is the one of the scope around
#[test]
fn a_binding_is_seen_after_its_statement_and_in_the_block_it_guards() {
  let text = "fn f(total: u32, items: Vec<u32>, found: Option<u32>) -> u32 {\n  let total = total + 1;\n  if let Some(found) = found { use_it(found); }\n  while let Some(items) = next(&items) { use_it(items); }\n  for items in items.iter() { use_it(items); }\n  let Some(found) = found else { return total };\n  total + found\n}\n";
  let local = |needle: &str, occurrence: usize| kind_and_local(RUST, text, needle, occurrence).1;
  // the value of a `let` reads the binding before it: the parameter
  assert_eq!(local("total + 1", 0), Some((0, 5)));
  // the value an `if let` and a `while let` take apart and the collection of a `for` are read before their bindings
  assert_eq!(
    (local("found {", 0), local("items) {", 0), local("items.iter", 0)),
    (Some((0, 34)), Some((0, 17)), Some((0, 17)))
  );
  // their blocks read the bindings
  assert_eq!(
    (local("found);", 0), local("items);", 0), local("items);", 1)),
    (Some((2, 14)), Some((3, 17)), Some((4, 6)))
  );
  // past the block of the `if let`, `found` is the parameter again, also in the value and the `else` of a `let`
  assert_eq!((local("found else", 0), local("total }", 0)), (Some((0, 34)), Some((1, 6))));
  assert_eq!((local("total + found", 0), local("found\n", 0)), (Some((1, 6)), Some((5, 11))));
  // the usages of the parameter are its declaration and the value read from it
  assert_eq!(usages(RUST, text, "total", 0), (true, vec![(0, Access::Declaration), (1, Access::Read)]));
  assert_eq!(
    usages(RUST, text, "found", 0),
    (true, vec![(0, Access::Declaration), (2, Access::Read), (5, Access::Read)]),
    "the parameter: read by the `if let` and by the `let … else`"
  );
}

/// A name a format string takes from the code around its macro is a use of it: after the `{` of a placeholder, before
/// a `$`; a doubled brace, a code point and a string outside a macro name nothing
#[test]
fn a_format_string_uses_the_names_it_takes_from_the_code_around_it() {
  let text = "fn f(total: u32, width: usize) -> String {\n  let plain = \"{total}\";\n  format!(\"{total} {{total}} {total:>8} {:width$} \\u{61} {0}\", total)\n}\n";
  assert_eq!(
    usages(RUST, text, "total", 0),
    (true, vec![(0, Access::Declaration), (2, Access::Read), (2, Access::Read), (2, Access::Read)]),
    "two placeholders and the argument; not the doubled braces, not the string of line 1"
  );
  assert_eq!(usages(RUST, text, "width", 0), (true, vec![(0, Access::Declaration), (2, Access::Read)]));
  // a name in a string is no place to go from
  assert_eq!(reference_at(RUST, text, place(text, "total}", 1)), None);
}
