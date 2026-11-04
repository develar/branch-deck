//! The highlighting of C# from Roslyn's tree, beside the golden file of `testdata/highlight/` (`highlight_test.rs`):
//! what the tree tells that a reading of the tokens alone would not

use crate::highlight::{FIELD, Grammar, ITALIC, Role, STATIC_FIELD};
use pretty_assertions::assert_eq;

/// The style of the text at the `occurrence`-th `needle` of the text (counted from 0)
fn style_at(text: &str, needle: &str, occurrence: usize) -> u32 {
  crate::test_support::style_at(Grammar::CSharp, text, needle, occurrence)
}

const KEYWORD: u32 = Role::Keyword.style();
const STRING: u32 = Role::String.style();
const ESCAPE: u32 = Role::Escape.style();
const STATIC_METHOD: u32 = Role::FunctionDeclaration.style() | ITALIC;

#[test]
fn the_contextual_keywords_the_parser_reads_as_keywords_are_keywords_and_names_elsewhere() {
  let text = "partial record R {\n  async void F() { await G(); var var = 1; int value = nameof(var); }\n  int P { get; set; }\n}\n";
  assert_eq!(
    ["partial", "record", "async", "await", "var", "nameof", "get", "set"].map(|it| style_at(text, it, 0)),
    [KEYWORD; 8]
  );
  // the variable `var` and a local `value` outside an accessor are names
  assert_eq!((style_at(text, "var = 1", 0), style_at(text, "value", 0), style_at(text, "var)", 0)), (0, 0, 0));
}

#[test]
fn strings_have_the_escapes_of_their_kind() {
  let text = "class A {\n  string a = \"\\x41z\\u0042\";\n  string b = @\"\"\"q\"\"\";\n  string c = $\"{{x}}\\n{a}\";\n  string d = \"\"\"\\n\"\"\";\n}\n";
  assert_eq!((style_at(text, "\\x41", 0), style_at(text, "z", 0), style_at(text, "\\u0042", 0)), (ESCAPE, STRING, ESCAPE));
  // the opening `@"` is no doubled quote; the `""` inside is
  assert_eq!((style_at(text, "@\"", 0), style_at(text, "\"\"q", 0), style_at(text, "q", 0)), (STRING, ESCAPE, STRING));
  // `{{` is an escape of an interpolated string, `{a}` a hole of code
  assert_eq!((style_at(text, "{{x", 0), style_at(text, "\\n{", 0), style_at(text, "a}", 0)), (ESCAPE, ESCAPE, FIELD));
  // a raw string has no escapes
  assert_eq!(style_at(text, "\\n\"\"\"", 0), STRING);
}

#[test]
fn members_are_fields_by_their_declarations_and_static_ones_are_italic() {
  let text = "class A {\n  const int Limit = 1;\n  static int created;\n  int count, other;\n  int Count => count;\n  static void Make() {}\n  void F(int other) { created = Limit + count + other; this.other = 0; }\n}\n";
  assert_eq!(
    [
      style_at(text, "Limit", 0),
      style_at(text, "created", 0),
      style_at(text, "count", 0),
      style_at(text, "other", 0)
    ],
    [STATIC_FIELD, STATIC_FIELD, FIELD, FIELD]
  );
  assert_eq!((style_at(text, "Count", 0), style_at(text, "Make", 0)), (FIELD, STATIC_METHOD));
  // uses: a parameter hides the field of its name, `this.other` is the field
  assert_eq!(
    [
      style_at(text, "created =", 0),
      style_at(text, "Limit +", 0),
      style_at(text, "count +", 0),
      style_at(text, "other;", 1),
      style_at(text, "other = 0", 0)
    ],
    [STATIC_FIELD, STATIC_FIELD, FIELD, 0, FIELD]
  );
}

#[test]
fn a_name_in_a_type_position_is_a_type_not_the_member_of_its_name() {
  let text = "class A {\n  Color Color;\n  void F(object o) { var c = (Color)o; if (o is Color) {} foreach (Color item in Color.All) {} }\n}\n";
  assert_eq!(
    [
      style_at(text, "Color Color", 0),
      style_at(text, "Color;", 0),
      style_at(text, "Color)o", 0),
      style_at(text, "Color) {}", 0),
      style_at(text, "Color item", 0)
    ],
    [0, FIELD, 0, 0, 0]
  );
  // the collection of a `foreach` is an expression: the field
  assert_eq!(style_at(text, "Color.All", 0), FIELD);
}

#[test]
fn a_member_of_a_value_is_a_field_and_one_of_a_type_name_is_unknown() {
  let text = "class A {\n  void F(string text) { int n = text.Length + Console.Out.Width + text?.Length ?? 0; text.Trim(); }\n}\n";
  assert_eq!(
    (
      style_at(text, "Length", 0),
      style_at(text, "Out", 0),
      style_at(text, "Length", 1),
      style_at(text, "Trim", 0)
    ),
    (FIELD, 0, FIELD, 0)
  );
}

#[test]
fn a_type_parameter_is_one_where_declared_and_used() {
  let text = "class A<T> where T : new() {\n  T Make<U>(U u) => new T();\n}\n";
  let parameter = Role::TypeParameter.style();
  assert_eq!(
    [
      style_at(text, "T>", 0),
      style_at(text, "T :", 0),
      style_at(text, "T Make", 0),
      style_at(text, "U>", 0),
      style_at(text, "U u", 0),
      style_at(text, "T()", 0)
    ],
    [parameter; 6]
  );
}

#[test]
fn a_documentation_comment_has_its_tags_and_their_values() {
  let text = "/// <summary>See <see cref=\"B.F\"/> and <paramref name=\"x\"/>.</summary>\nclass A {}\n";
  let doc = Role::DocComment.style();
  let tag = Role::DocTag.style();
  let value = Role::DocTagValue.style();
  assert_eq!(
    [
      style_at(text, "///", 0),
      style_at(text, "<summary>", 0),
      style_at(text, "See", 0),
      style_at(text, "<see", 0),
      style_at(text, "B.F", 0),
      style_at(text, "\"x\"", 0)
    ],
    [doc, tag, doc, tag, value, value]
  );
}

#[test]
fn a_directive_has_its_keywords_and_a_branch_not_taken_is_a_comment() {
  let text = "#nullable enable\nclass A {\n#if DEBUG\n  int x;\n#else\n  int y;\n#endif\n  #region Members\n  #endregion\n}\n";
  assert_eq!(
    [
      style_at(text, "#nullable", 0),
      style_at(text, "enable", 0),
      style_at(text, "#if", 0),
      style_at(text, "#region", 0)
    ],
    [KEYWORD; 4]
  );
  assert_eq!((style_at(text, "DEBUG", 0), style_at(text, "Members", 0)), (0, 0));
  assert_eq!((style_at(text, "int x", 0), style_at(text, "int y", 0)), (Role::Comment.style(), KEYWORD));
}

#[test]
fn an_attribute_is_one_and_its_arguments_are_code() {
  let text = "[Obsolete(\"old\", error: false)]\nclass A {}\n";
  let attribute = Role::Annotation.style();
  assert_eq!(
    [
      style_at(text, "Obsolete", 0),
      style_at(text, "\"old\"", 0),
      style_at(text, "error", 0),
      style_at(text, "false", 0)
    ],
    [attribute, STRING, Role::NamedArgument.style(), KEYWORD]
  );
}

#[test]
fn a_label_and_the_goto_to_it_are_labels() {
  let text = "class A { void F() { again: goto again; } }\n";
  let label = Role::Label.style();
  assert_eq!((style_at(text, "again:", 0), style_at(text, "again;", 0)), (label, label));
}

#[test]
fn a_verbatim_identifier_binds_its_value_text() {
  let text = "class A {\n  int count;\n  void F() { @count = 1; }\n}\n";
  assert_eq!(style_at(text, "@count", 0), FIELD);
}

#[test]
fn the_parameters_of_a_record_are_its_properties() {
  let text = "record Point(int X, int Y) {\n  public int Sum() => X + Y;\n}\nclass Box(int size) {\n  int Twice() => size * 2;\n}\n";
  assert_eq!(
    [style_at(text, "X,", 0), style_at(text, "Y)", 0), style_at(text, "X +", 0), style_at(text, "Y;", 0)],
    [FIELD; 4]
  );
  // a class's primary constructor parameter is a parameter, not a field
  assert_eq!((style_at(text, "size)", 0), style_at(text, "size *", 0)), (0, 0));
}
