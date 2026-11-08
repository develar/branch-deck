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
