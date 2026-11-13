//! The highlighting of Rust on rust-analyzer's tree, one test per class of the pieces the oracle moved from
//! tree-sitter's (docs/design/diff-view.md, "Syntax highlighting"; docs/design/symbol-dependencies.md, decision 26)

use crate::highlight::{FIELD, ITALIC, Role, UNDERLINE, highlight};
use crate::test_support::style_at;
use code_tokens::Grammar;

const RUST: Grammar = Grammar::Rust;
const KEYWORD: u32 = Role::Keyword.style();
const MUTABLE: u32 = UNDERLINE;

/// `Self` is a keyword, as `RustRover`'s lexer has it; tree-sitter's grammar read it as a type name
#[test]
fn self_type_is_a_keyword() {
  let text = "impl S {\n  fn new() -> Self { Self(1) }\n}\n";
  assert_eq!(style_at(RUST, text, "Self", 0), KEYWORD);
  assert_eq!(style_at(RUST, text, "Self", 1), KEYWORD);
}

/// An attribute is one colour, a literal in it painted over it; its words (`default`, `self`, `true`) are the
/// attribute's
#[test]
fn an_attribute_is_one_colour_but_its_literals() {
  let text = "#[serde(default, rename = \"b\")]\n#[instrument(skip(self), ret = true)]\nfn f() {}\n";
  let annotation = Role::Annotation.style();
  for (needle, occurrence) in [("serde", 0), ("default", 0), ("rename", 0), ("self", 0), ("true", 0)] {
    assert_eq!(style_at(RUST, text, needle, occurrence), annotation, "{needle}");
  }
  assert_eq!(style_at(RUST, text, "\"b\"", 0), Role::String.style());
}

/// The tokens of a macro are lexed: a keyword is a keyword (`else`, `in`), a primitive type too, a metavariable
/// (`$x`, `$crate`, `$type`) is plain and the fragment of one in a pattern (`expr`, `ty`) a keyword
#[test]
fn the_tokens_of_a_macro_are_lexed() {
  let text = "macro_rules! m { ($x:expr, $type:ty) => { $crate::f($x as $type) }; }\nfn f() { json!(if a { 1 } else { 2 }); check!(x as u8); }\n";
  assert_eq!(style_at(RUST, text, "expr", 0), KEYWORD);
  assert_eq!(style_at(RUST, text, "ty)", 0), KEYWORD);
  assert_eq!(style_at(RUST, text, "type", 0), 0, "the metavariable `$type`");
  assert_eq!(style_at(RUST, text, "crate", 0), 0, "`$crate`");
  assert_eq!(style_at(RUST, text, "else", 0), KEYWORD);
  assert_eq!(style_at(RUST, text, "u8", 0), KEYWORD);
}

/// A raw string has no escapes; the `\` that continues a string on the next line is one
#[test]
fn the_escapes_of_a_string() {
  let text = "fn f() {\n  let a = r\"\\\\?\\\";\n  let b = \"x\\n\\\n  y\";\n}\n";
  let string = Role::String.style();
  assert_eq!(style_at(RUST, text, "\\\\?", 0), string, "raw");
  assert_eq!(style_at(RUST, text, "\\n", 0), Role::Escape.style());
  assert_eq!(style_at(RUST, text, "\\\n", 0), Role::Escape.style(), "a continuation");
}

/// The last binding of a name is the one its uses see, after its `let`: `let x = x;` reads the binding before it
#[test]
fn a_binding_shadows_the_ones_before_it() {
  let text = "fn f() {\n  let mut x = 1;\n  let x = x + 1;\n  use_it(x);\n}\n";
  assert_eq!(style_at(RUST, text, "x", 1), MUTABLE, "the declaration of the `mut` one");
  assert_eq!(style_at(RUST, text, "x + 1", 0), MUTABLE, "the initializer sees the `mut` one");
  assert_eq!(style_at(RUST, text, "x);", 0), 0, "the use after the second `let`");
}

/// A `mut` shorthand of a struct pattern binds a mutable local; a field with a pattern is no shorthand: its pattern is
/// a name, a variant after a type (`Either::One(_t)`) a constant, a type's name no constant whatever its case
#[test]
fn the_names_of_patterns_and_declarations() {
  let text = "struct S2 { a: u8 }\nfn f(p: P, e: Either) {\n  let P { mut groups, reason: None } = p;\n  groups.push(1);\n  if let Either::One(_t) = e {}\n}\n";
  assert_eq!(style_at(RUST, text, "groups", 1), MUTABLE);
  assert_eq!(style_at(RUST, text, "reason", 0), FIELD);
  assert_eq!(style_at(RUST, text, "None", 0), 0);
  assert_eq!(style_at(RUST, text, "One", 0), FIELD | ITALIC);
  assert_eq!(style_at(RUST, text, "S2", 0), 0);
}

/// A file nested 20 000 deep is highlighted on a thread of 2 MiB; one past what any code nests has none (Shiki then
/// colours it in the view)
#[test]
fn deep_nesting_is_highlighted_on_a_small_stack() {
  let thread = std::thread::Builder::new().stack_size(2 << 20).spawn(|| {
    let deep = format!("fn f() {}1{}\n", "{ ".repeat(20_000), " }".repeat(20_000));
    assert_eq!(style_at(RUST, &deep, "fn", 0), KEYWORD);
    let too_deep = "(".repeat(rust_syntax::nesting::MAX_LEVELS + 1);
    assert_eq!(highlight(RUST, &too_deep), None);
  });
  thread.unwrap().join().unwrap();
}
