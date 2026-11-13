//! The highlighting of TypeScript, TSX and JavaScript by oxc's tokens and tree: the classes of colours that moved from
//! the tree-sitter rules, each by a text of its own (the whole fixtures are golden files of `highlight_test.rs`)

use crate::highlight::{ITALIC, Role, highlight};
use crate::test_support::style_at;
use code_tokens::Grammar;

fn style(grammar: Grammar, text: &str, needle: &str, occurrence: usize) -> u32 {
  style_at(grammar, text, needle, occurrence)
}

/// `export declare` exports: a declared function and a constant of a declaration file are global
#[test]
fn an_exported_ambient_declaration_is_global() {
  let text = "export declare function load(): void\nexport declare const LIMIT: number\ndeclare const hidden: number\nload(LIMIT, hidden)\n";
  assert_eq!(style(Grammar::TypeScript, text, "load", 0), Role::GlobalFunction.style());
  assert_eq!(style(Grammar::TypeScript, text, "LIMIT", 1), Role::GlobalVariable.style());
  assert_eq!(style(Grammar::TypeScript, text, "hidden", 1), Role::LocalVariable.style());
}

/// A decorator is in its colour from its `@` to its arguments, which are code; the text before it is not
#[test]
fn a_decorator_is_coloured_to_its_arguments() {
  let text = "class Box {\n  @Input({ alias: NAME }) name = \"\"\n  @module.Getter(\"x\") value = 1\n}\n";
  for (needle, occurrence) in [("@", 0), ("Input", 0), ("module", 0), ("Getter", 0)] {
    assert_eq!(style(Grammar::TypeScript, text, needle, occurrence), Role::Annotation.style(), "{needle}");
  }
  assert_eq!(style(Grammar::TypeScript, text, "NAME", 0), Role::GlobalVariable.style());
  assert_eq!(style(Grammar::TypeScript, text, "  @", 0), 0, "the indentation before it");
}

/// A template literal type is a string; the `${` and `}` of a template are code
#[test]
fn a_template_literal_type_is_a_string() {
  let text = "type Route = `/${string}`\nconst path = `a${b}c`\n";
  assert_eq!(style(Grammar::TypeScript, text, "`/", 0), Role::String.style());
  assert_eq!(style(Grammar::TypeScript, text, "${", 1), 0);
  assert_eq!(style(Grammar::TypeScript, text, "c`", 0), Role::String.style());
}

/// The escapes of a string are those of JavaScript: `\x1f`, `\u{1F600}` and an octal one whole
#[test]
fn a_string_has_the_escapes_of_javascript() {
  let text = "const s = \"a\\x1fb\\u{1F600}c\\012d\"\n";
  assert_eq!(style(Grammar::JavaScript, text, "1f", 0), Role::Escape.style());
  assert_eq!(style(Grammar::JavaScript, text, "1F600", 0), Role::Escape.style());
  assert_eq!(style(Grammar::JavaScript, text, "012", 0), Role::Escape.style());
  assert_eq!(style(Grammar::JavaScript, text, "b\\u", 0), Role::String.style());
}

/// Each part of a member tag is a component, and the brackets of a fragment are those of an element
#[test]
fn a_member_tag_is_a_component_and_a_fragment_has_brackets() {
  let text = "export const view = <><Menu.Item /></>\n";
  assert_eq!(style(Grammar::Tsx, text, "Menu", 0), Role::ComponentTag.style());
  assert_eq!(style(Grammar::Tsx, text, "Item", 0), Role::ComponentTag.style());
  assert_eq!(style(Grammar::Tsx, text, "<>", 0), Role::TagBracket.style());
  assert_eq!(style(Grammar::Tsx, text, "</>", 0), Role::TagBracket.style());
}

/// The name of an index signature and the label of a named tuple member are plain, as a contextual word that names
/// something (`{ as: x }`, `z.infer`); the parameter of a type predicate and of a function type is a parameter
#[test]
fn names_by_what_they_name() {
  let text = "interface A { [key: string]: unknown; pair: [label: string]; is(value: unknown): value is A; on(cb: (event: E, ...rest: R) => void): this }\nconst { as: alias } = q\ntype T = z.infer<X>\n";
  for (needle, occurrence) in [("key", 0), ("label", 0), ("as", 0), ("infer", 0)] {
    assert_eq!(style(Grammar::TypeScript, text, needle, occurrence), 0, "{needle}");
  }
  for (needle, occurrence) in [("value", 1), ("event", 0), ("rest", 0)] {
    assert_eq!(style(Grammar::TypeScript, text, needle, occurrence), Role::Parameter.style(), "{needle}");
  }
  assert_eq!(style(Grammar::TypeScript, text, "this", 0), Role::Keyword.style(), "the type `this`");
}

/// A call with type arguments is a call: an import it calls is a function, a member a method
#[test]
fn a_call_with_type_arguments_is_a_call() {
  let text = "import { read } from \"./io\"\nconst a = read<Config>(path)\nconst b = store.load<Data>()\n";
  assert_eq!(style(Grammar::TypeScript, text, "read", 0), Role::GlobalFunction.style());
  assert_eq!(style(Grammar::TypeScript, text, "load", 0), Role::Method.style());
}

/// The key of a mapped type and what `infer` declares are type parameters
#[test]
fn a_mapped_key_and_infer_are_type_parameters() {
  let text = "type Keys<T> = { [K in keyof T]: K }\ntype Head<T> = T extends [infer H] ? H : never\n";
  assert_eq!(style(Grammar::TypeScript, text, "K", 1), Role::TypeParameter.style());
  assert_eq!(style(Grammar::TypeScript, text, "H", 1), Role::TypeParameter.style());
}

/// A `.ts` file reads `<T>x` as an assertion; a `.js` file reads JSX
#[test]
fn the_grammar_reads_its_dialect() {
  let assertion = "const size = <number>count\n";
  assert_eq!(style(Grammar::TypeScript, assertion, "number", 0), Role::Keyword.style());
  assert_eq!(style(Grammar::JavaScript, "export const v = <div className=\"a\" />\n", "div", 0), Role::Tag.style());
  assert_eq!(style(Grammar::Tsx, "class A { static make() {} }\n", "make", 0), Role::FunctionDeclaration.style() | ITALIC);
}

/// Any nesting the parse reads is highlighted on a 2 MiB thread; a file without a tree, nested deeper than the parse
/// reads, has no highlighting, so the view colours it otherwise
#[test]
fn deep_nesting_is_highlighted_on_a_small_stack() {
  let thread = std::thread::Builder::new().stack_size(2 << 20).spawn(|| {
    for (open, close) in crate::js::summary_test::NESTINGS {
      let text = crate::js::summary_test::nested_source(20_000, open, close);
      assert!(highlight(Grammar::TypeScript, &text).is_some(), "{open}");
    }
    assert_eq!(highlight(Grammar::TypeScript, &crate::js::summary_test::nested_source(300_000, "[", "]")), None);
  });
  thread.unwrap().join().unwrap();
}
