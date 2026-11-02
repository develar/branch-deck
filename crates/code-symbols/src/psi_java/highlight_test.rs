//! The highlighting of Java from its PSI, beside the golden files of `testdata/highlight/` (`highlight_test.rs`): what
//! the PSI reads apart from tree-sitter's grammar, each a construct the corpus moved

use crate::highlight::{FIELD, Grammar, ITALIC, Role, STATIC_FIELD, highlight};
use pretty_assertions::assert_eq;

/// The style of the text at the `occurrence`-th `needle` of the text (counted from 0), by the runs of its line
fn style_at(text: &str, needle: &str, occurrence: usize) -> u32 {
  let offset = text.match_indices(needle).nth(occurrence).unwrap_or_else(|| panic!("no {needle}")).0;
  let line = text[..offset].matches('\n').count();
  let line_start = text[..offset].rfind('\n').map_or(0, |it| it + 1);
  let column = u32::try_from(text[line_start..offset].encode_utf16().count()).unwrap();
  let runs = highlight(Grammar::Java, text).unwrap();
  runs.line(line).chunks(2).take_while(|pair| pair[0] <= column).last().map_or(0, |pair| pair[1])
}

const KEYWORD: u32 = Role::Keyword.style();
const STATIC_METHOD: u32 = Role::FunctionDeclaration.style() | ITALIC;

#[test]
fn a_keyword_is_a_keyword() {
  let runs = highlight(Grammar::Java, "class A {}\n").unwrap();
  assert_eq!(runs.line(0)[..2], [0, KEYWORD]);
}

#[test]
fn the_keywords_the_parser_reads_in_context_are_keywords_and_names_elsewhere() {
  let text = "public sealed @interface A permits B {}\nnon-sealed class B {\n  void f(Object o) {\n    var var = 1;\n    record R(int x) {}\n    int y = switch (o) { case R r when r.x() > 0 -> { yield 1; } default -> 0; };\n  }\n}\n";
  assert_eq!(
    ["sealed", "interface", "permits", "non-sealed", "var", "record", "when", "yield"].map(|it| style_at(text, it, 0)),
    [KEYWORD; 8]
  );
  // the `@` of `@interface` is no part of the keyword, and the variable `var` is a name
  assert_eq!((style_at(text, "@interface", 0), style_at(text, "var", 1)), (0, 0));
}

#[test]
fn a_character_a_string_and_a_text_block_have_their_escapes() {
  let text = "class A {\n  char c = '\\n';\n  String s = \"\\770\";\n  String t = \"\"\"\n    a \\\n    b\\tc\"\"\";\n}\n";
  let escape = Role::Escape.style();
  let string = Role::String.style();
  assert_eq!(style_at(text, "\\n'", 0), escape);
  // an octal escape of a digit above 3 has two digits: `\77`, then `0`
  assert_eq!((style_at(text, "\\770", 0), style_at(text, "0\"", 0)), (escape, string));
  // the line continuation of a text block and its escape
  assert_eq!(
    (style_at(text, "\\\n", 0), style_at(text, "\\tc", 0), style_at(text, "c\"\"\"", 0)),
    (escape, escape, string)
  );
}

#[test]
fn a_field_of_an_anonymous_class_and_of_an_enum_constant_is_a_field() {
  let text = "class A {\n  Object o = new Object() {\n    int count;\n    void f() { count++; }\n  };\n  enum E { ONE { int size; int g() { return size; } } }\n}\n";
  assert_eq!((style_at(text, "count", 1), style_at(text, "size", 1)), (FIELD, FIELD));
}

#[test]
fn a_pattern_variable_and_a_vararg_hide_the_field_of_their_name() {
  let text = "class A {\n  String name;\n  void f(Object o) {\n    if (o instanceof String name) name.length();\n  }\n  void g(String... name) { name.clone(); }\n}\n";
  assert_eq!((style_at(text, "name.length", 0), style_at(text, "name.clone", 0)), (0, 0));
}

#[test]
fn a_qualified_this_is_a_keyword_and_the_fields_through_it_are_the_instance_s() {
  let text = "class Outer {\n  int size;\n  class Inner {\n    int f() { return Outer.this.size + Base.super.hashCode(); }\n  }\n}\n";
  assert_eq!(style_at(text, "this", 0), KEYWORD);
  assert_eq!(style_at(text, "size +", 0), FIELD);
  // a call of the default method of an interface through `super` is no static call
  assert_eq!(style_at(text, "hashCode", 0), 0);
}

#[test]
fn a_field_is_static_by_its_modifier_and_in_an_interface_only() {
  let text = "interface I {\n  int LIMIT = 1;\n  class Nested {\n    int count;\n    public/*a*/static/*b*/int total;\n    int f() { return count + total + LIMIT; }\n  }\n}\n";
  assert_eq!(
    (
      style_at(text, "LIMIT", 0),
      style_at(text, "count +", 0),
      style_at(text, "total;", 0),
      style_at(text, "total +", 0)
    ),
    (STATIC_FIELD, FIELD, STATIC_FIELD, STATIC_FIELD)
  );
}

#[test]
fn an_element_of_an_annotation_type_is_a_method_declaration_and_an_argument_name_no_field() {
  let text = "@interface Range {\n  int value() default 0;\n}\nclass A {\n  int value;\n  @Range(value = 1) int f() { return 0; }\n}\n";
  assert_eq!(style_at(text, "value()", 0), Role::FunctionDeclaration.style());
  assert_eq!(style_at(text, "value = 1", 0), 0);
}

#[test]
fn an_annotated_type_parameter_is_a_type_parameter() {
  let text = "class Box<@Nullable T> {\n  T item;\n  static <@A K> K first(K k) { return k; }\n}\n";
  let parameter = Role::TypeParameter.style();
  assert_eq!(
    [("T>", 0), ("T item", 0), ("K>", 0), ("K first", 0), ("K k", 0)].map(|(it, n)| style_at(text, it, n)),
    [parameter; 5]
  );
  assert_eq!(style_at(text, "first", 0), STATIC_METHOD);
}

#[test]
fn a_markdown_doc_comment_is_a_doc_comment_from_the_slashes_of_each_line() {
  let text = "class A {\n  /// Sums.\n  ///   @param x the x\n  int f(int x) { return x; }\n}\n";
  let runs = highlight(Grammar::Java, text).unwrap();
  let doc = Role::DocComment.style();
  assert_eq!(runs.line(1), [2, doc]);
  assert_eq!(runs.line(2)[..2], [2, doc]);
}

#[test]
fn the_name_of_a_method_reference_is_no_field() {
  let text = "class A {\n  Runnable run;\n  void f() { Runnable r = this::run; }\n  void run() {}\n}\n";
  assert_eq!(style_at(text, "run;", 1), 0);
}

#[test]
fn a_comment_inside_a_text_block_is_its_text() {
  let text = "class A {\n  String s = \"\"\"\n    \\t/* not a comment */\n    \"\"\";\n}\n";
  assert_eq!(style_at(text, "/*", 0), Role::String.style());
}
