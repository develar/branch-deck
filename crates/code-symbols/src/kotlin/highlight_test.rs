//! The highlighting of Kotlin from its PSI, beside the golden files of `testdata/highlight/` (`highlight_test.rs`): each
//! reading where it differs from tree-sitter's, by the construct

use crate::highlight::{BOLD, ITALIC, ROLE_BITS, Role, STATIC_FIELD, UNDERLINE, highlight};
use code_tokens::Grammar;

/// The pieces of the first line of a text in a style other than the default foreground, as `style "text"`
fn pieces(grammar: Grammar, text: &str) -> Vec<String> {
  let runs = highlight(grammar, text).unwrap_or_else(|| panic!("no highlighting of {text}"));
  let units: Vec<u16> = text.lines().next().unwrap_or_default().encode_utf16().collect();
  let pairs = runs.line(0);
  let mut out = Vec::new();
  for (index, pair) in pairs.chunks(2).enumerate() {
    if pair[1] == 0 {
      continue;
    }
    let end = pairs.get(index * 2 + 2).map_or(units.len(), |it| *it as usize);
    let role = pair[1] & ((1 << ROLE_BITS) - 1);
    let mut name = if role == 0 { "plain".to_string() } else { Role::NAMES[role as usize - 1].to_string() };
    for (bit, font) in [(ITALIC, "italic"), (BOLD, "bold"), (UNDERLINE, "underline")] {
      if pair[1] & bit != 0 {
        name.push('+');
        name.push_str(font);
      }
    }
    out.push(format!("{name} {:?}", String::from_utf16_lossy(&units[pair[0] as usize..end])));
  }
  out
}

fn kotlin(text: &str) -> Vec<String> {
  pieces(Grammar::Kotlin, text)
}

/// The coloured pieces of the first line of a Kotlin text that spell `name`
fn pieces_of(text: &str, name: &str) -> Vec<String> {
  let quoted = format!("{name:?}");
  kotlin(text).into_iter().filter(|it| it.ends_with(&quoted)).collect()
}

#[test]
fn a_keyword_is_a_keyword_in_a_file_and_in_a_script() {
  for grammar in [Grammar::Kotlin, Grammar::KotlinScript] {
    let runs = highlight(grammar, "val x = 1\n").unwrap_or_else(|| panic!("{grammar:?}"));
    assert_eq!(runs.line(0)[..2], [0, Role::Keyword.style()], "{grammar:?}");
  }
}

#[test]
fn the_keywords_are_the_tokens_of_the_parser() {
  // `as?`, `!in` and `!is` are one keyword token each, the `constructor` of a primary constructor and `$this` of a
  // template are keywords, `annotation` after an annotation is a modifier
  assert_eq!(kotlin("fun f() = x as? Int\n"), [r#"keyword "fun""#, r#"functionDeclaration "f""#, r#"keyword "as?""#]);
  assert_eq!(kotlin("val a = x !in y || x !is Int\n")[2..], [r#"keyword "!in""#, r#"keyword "!is""#]);
  assert_eq!(
    kotlin("class A internal constructor()\n"),
    [r#"keyword "class""#, r#"keyword "internal""#, r#"keyword "constructor""#]
  );
  assert_eq!(
    kotlin("fun Any.f() = \"$this\"\n")[2..],
    [r#"string "\"""#, r#"escape "$""#, r#"keyword "this""#, r#"string "\"""#]
  );
  assert_eq!(
    kotlin("@DslMarker annotation class Dsl\n"),
    [r#"annotation "@DslMarker""#, r#"keyword "annotation""#, r#"keyword "class""#]
  );
}

#[test]
fn a_file_annotation_is_an_annotation_with_its_target_and_its_arguments_are_code() {
  assert_eq!(
    kotlin("@file:OptIn(Api::class, \"x\")\n"),
    [r#"annotation "@file:OptIn""#, r#"keyword "class""#, r#"string "\"x\"""#]
  );
}

#[test]
fn the_label_of_this_is_a_label() {
  assert_eq!(kotlin("val a = this@Outer.x\n")[2..], [r#"keyword "this""#, r#"label "@Outer""#, r#"field "x""#]);
}

#[test]
fn a_raw_string_has_no_escapes() {
  assert_eq!(
    kotlin("val r = \"\"\"\\d+\\.$x\"\"\"\n")[2..],
    [r#"string "\"\"\"\\d+\\.""#, r#"escape "$""#, r#"string "\"\"\"""#]
  );
}

#[test]
fn a_name_after_a_receiver_is_no_field_of_the_scopes() {
  // the callee of a qualified call and a reference after `X::` are never the property of that name; `::size` is
  assert_eq!(
    pieces_of("class A(val size: Int) { fun f() = Math.size(1) + B::size.name + ::size.name }\n", "size"),
    [r#"field "size""#, r#"field "size""#]
  );
  // nor a name of the path of an import
  assert_eq!(kotlin("import a.size.b\nval size = 1\n"), [r#"keyword "import""#]);
}

#[test]
fn an_operand_of_an_infix_call_is_read_by_the_scopes() {
  assert_eq!(kotlin("const val MAX = 1; val r = 0 until MAX\n").last().map(String::as_str), Some(r#"field+italic "MAX""#));
  let pieces = kotlin("fun f() { var n = 0; val m = n shl 1; list.map { it to n } }\n");
  assert!(pieces.contains(&r#"plain+bold "it""#.to_string()), "{pieces:?}");
  assert_eq!(pieces.iter().filter(|it| *it == r#"plain+underline "n""#).count(), 3, "{pieces:?}");
}

#[test]
fn the_subject_of_a_where_clause_is_a_type_parameter() {
  assert_eq!(
    kotlin("fun <T> f(t: T) where T : Any {}\n"),
    [
      r#"keyword "fun""#,
      r#"typeParameter "T""#,
      r#"functionDeclaration "f""#,
      r#"typeParameter "T""#,
      r#"keyword "where""#,
      r#"typeParameter "T""#
    ]
  );
}

#[test]
fn a_string_that_a_parse_error_holds_is_still_a_string() {
  // test data with markers the parser reads as errors: their strings and numbers stay coloured
  let pieces = kotlin("fun <!MARKER(\"descr\")!>f<!>() = 42\n");
  assert_eq!(
    pieces.iter().filter(|it| it.starts_with("string") || it.starts_with("number")).collect::<Vec<_>>(),
    [r#"string "\"descr\"""#, r#"number "42""#],
    "{pieces:?}"
  );
}

#[test]
fn a_script_has_its_top_level_properties_as_members() {
  // a property of a script is static as a top-level property of a file, also where a function uses it
  let text = "val v = 1\nfun f() = v\n";
  assert_eq!(pieces(Grammar::KotlinScript, text), [r#"keyword "val""#, r#"field+italic "v""#, r#"number "1""#]);
  let runs = highlight(Grammar::KotlinScript, text).unwrap();
  assert_eq!(runs.line(1)[runs.line(1).len() - 2..], [10, STATIC_FIELD]);
}

#[test]
fn a_construct_past_the_nesting_limit_is_highlighted_as_its_error() {
  // past the parser's nesting limit the rest is one error element, whose leaves keep their colours
  let text = format!("val a = {}1{}\n", "(".repeat(1500), ")".repeat(1500));
  let runs = std::thread::Builder::new()
    .stack_size(64 << 20)
    .spawn(move || highlight(Grammar::Kotlin, &text))
    .unwrap()
    .join()
    .unwrap()
    .unwrap();
  assert_eq!(runs.line(0)[..2], [0, Role::Keyword.style()]);
  assert!(runs.line(0).chunks(2).any(|it| it[1] == Role::Number.style()));
}

#[test]
fn a_parameter_of_a_function_type_binds_nothing() {
  // the name of a function type's parameter is no parameter of the function around it, nor a field itself
  assert_eq!(
    pieces_of("class A(val size: Int) { fun f(cb: (size: Int) -> Unit) = size }\n", "size"),
    [r#"field "size""#, r#"field "size""#]
  );
}
