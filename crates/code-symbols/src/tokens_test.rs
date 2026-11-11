//! The tokens of every grammar and the one rule "equal by tokens": tables of pairs of texts with their answer, the
//! negative ones first (a text that reads alike and is another program), then the lines, the columns and the ranges.

use crate::highlight::Grammar;
use crate::tokens::Equality::{CodeOnly, Different, Same};
use crate::tokens::{
  Equality, MAX_BYTES, RULES_VERSION, Token, TokenClass, Tokens, compare, content, equal_by_tokens, indentation_matters, is_blank, same_after, same_before, same_without_blanks,
  texts_equal_by_tokens, tokens, tokens_version,
};
use pretty_assertions::assert_eq;

#[path = "../examples/shared/token_checks.rs"]
mod token_checks;

/// The value for the parsers and the rules of this build. A change of `RULES_VERSION` (of the tokens or of the
/// summaries) or of the psi-syntax revision changes it: replace it with the value the failure prints
const TOKENS_VERSION: u32 = 1_021_957_210;

const KOTLIN: Grammar = Grammar::Kotlin;
const KTS: Grammar = Grammar::KotlinScript;
const JAVA: Grammar = Grammar::Java;
const CSHARP: Grammar = Grammar::CSharp;
const STARLARK: Grammar = Grammar::Starlark;
const RUST: Grammar = Grammar::Rust;
const TS: Grammar = Grammar::TypeScript;
const TSX: Grammar = Grammar::Tsx;
const JS: Grammar = Grammar::JavaScript;

fn stream(grammar: Grammar, text: &str) -> Tokens {
  tokens(grammar, text).unwrap_or_else(|| panic!("no tokens of {text:?}"))
}

/// The tokens of a text as `class "text"`, one per token
fn render(grammar: Grammar, text: &str) -> Vec<String> {
  stream(grammar, text)
    .all()
    .iter()
    .map(|token| {
      let class = match token.class {
        TokenClass::Comment => "comment",
        TokenClass::Name => "name",
        TokenClass::Keyword => "keyword",
        TokenClass::Literal => "literal",
        TokenClass::Other => "other",
      };
      format!("{class} {:?}", token.text_in(text))
    })
    .collect()
}

/// Each pair of texts is what `expected` says, in both directions; texts that are the same are the same without their
/// blanks, what a reader asks before it reads tokens
fn check(cases: &[(Grammar, &str, &str, Equality)]) {
  let mut failed = Vec::new();
  for &(grammar, old, new, expected) in cases {
    for (old, new) in [(old, new), (new, old)] {
      let actual = texts_equal_by_tokens(grammar, old, new);
      if actual != expected {
        failed.push(format!("{grammar:?}: {old:?} and {new:?} are {actual:?}, not {expected:?}"));
      }
      if actual == Same && !same_without_blanks([old], [new]) {
        failed.push(format!("{grammar:?}: {old:?} and {new:?} are the same and differ without their blanks"));
      }
    }
  }
  assert_eq!(failed, Vec::<String>::new());
}

#[test]
fn the_whitespace_inside_a_literal_is_content() {
  check(&[
    (KOTLIN, "val s = \"a  b\"\n", "val s = \"a b\"\n", Different),
    (JAVA, "class A { String s = \"a  b\"; }\n", "class A { String s = \"a b\"; }\n", Different),
    (CSHARP, "class A { string s = \"a  b\"; }\n", "class A { string s = \"a b\"; }\n", Different),
    (STARLARK, "s = \"a  b\"\n", "s = \"a b\"\n", Different),
    (RUST, "const S: &str = \"a  b\";\n", "const S: &str = \"a b\";\n", Different),
    (TS, "const s = \"a  b\";\n", "const s = \"a b\";\n", Different),
    (JS, "const s = 'a  b';\n", "const s = 'a b';\n", Different),
    (KOTLIN, "val c = ' '\n", "val c = '  '\n", Different),
    (KTS, "val s = \"a  b\"\n", "val s = \"a b\"\n", Different),
    // a blank line inside a raw string, a text block, a template
    (KOTLIN, "val s = \"\"\"\n  a\n  b\n\"\"\"\n", "val s = \"\"\"\n  a\n\n  b\n\"\"\"\n", Different),
    (KOTLIN, "val s = \"\"\"\n  a\n\"\"\"\n", "val s = \"\"\"\n    a\n\"\"\"\n", Different),
    (
      JAVA,
      "class A { String s = \"\"\"\n  a\n  b\n  \"\"\"; }\n",
      "class A { String s = \"\"\"\n  a\n\n  b\n  \"\"\"; }\n",
      Different,
    ),
    (TS, "const s = `a\nb`;\n", "const s = `a\n\nb`;\n", Different),
    (TS, "const s = `a ${b} c`;\n", "const s = `a ${b}  c`;\n", Different),
    (CSHARP, "class A { string s = $\"a {b} c\"; }\n", "class A { string s = $\"a {b}  c\"; }\n", Different),
    (RUST, "const S: &str = r#\"a\nb\"#;\n", "const S: &str = r#\"a\n\nb\"#;\n", Different),
    // a regular expression
    (JS, "const r = /a b/;\n", "const r = /a  b/;\n", Different),
    // the code of a template is code
    (KOTLIN, "val s = \"a ${b + c}\"\n", "val s = \"a ${ b+c }\"\n", Same),
    (TS, "const s = `a ${b + c}`;\n", "const s = `a ${ b+c }`;\n", Same),
    (CSHARP, "class A { string s = $\"a {b + c}\"; }\n", "class A { string s = $\"a {b+c}\"; }\n", Same),
  ]);
}

#[test]
fn operators_that_join_or_split_are_other_tokens() {
  check(&[
    (KOTLIN, "val x = a - -b\n", "val x = a--b\n", Different),
    (JAVA, "class A { int x = a - -b; }\n", "class A { int x = a--b; }\n", Different),
    (CSHARP, "class A { int x = a - -b; }\n", "class A { int x = a--b; }\n", Different),
    (JS, "x = a - -b;\n", "x = a--b;\n", Different),
    (TS, "x = a + +b;\n", "x = a++b;\n", Different),
    // Rust has no `--`: both are `a - (-b)`
    (RUST, "fn f() { a - -b; }\n", "fn f() { a--b; }\n", Same),
    (JAVA, "class A { int x = a >> b; }\n", "class A { int x = a > > b; }\n", Different),
    (JAVA, "class A { int x = a >>> b; }\n", "class A { int x = a >> > b; }\n", Different),
    (JAVA, "class A { void f() { a >>= b; } }\n", "class A { void f() { a >> = b; } }\n", Different),
    (CSHARP, "class A { int x = a >> b; }\n", "class A { int x = a > > b; }\n", Different),
    (RUST, "fn f() { a >> b; }\n", "fn f() { a > > b; }\n", Different),
    (JS, "x = a >> b;\n", "x = a > > b;\n", Different),
    (STARLARK, "x = a >> b\n", "x = a > > b\n", Different),
    // the two `>` that close two lists of type arguments are two tokens with or without a space
    (JAVA, "class A { List<List<String>> x; }\n", "class A { List<List<String> > x; }\n", Same),
    (CSHARP, "class A { List<List<string>> x; }\n", "class A { List<List<string> > x; }\n", Same),
    (RUST, "fn f(x: Vec<Vec<u8>>) {}\n", "fn f(x: Vec<Vec<u8> >) {}\n", Same),
    (TS, "let x: Array<Array<string>>;\n", "let x: Array<Array<string> >;\n", Same),
    (KOTLIN, "val x: List<List<String>> = y\n", "val x: List<List<String> > = y\n", Same),
    // Kotlin joins `?.`, `?:` and `!!` only without a space
    (KOTLIN, "val x = a?.b\n", "val x = a? .b\n", Different),
    (KOTLIN, "val x = a ?: b\n", "val x = a ? : b\n", Different),
    (KOTLIN, "val x = a!!\n", "val x = a! !\n", Different),
    (KOTLIN, "val x = a ?: b\n", "val x = a\n  ?: b\n", Same),
    (TS, "x = a?.b;\n", "x = a ?. b;\n", Same),
    (TS, "x = a ?? b;\n", "x = a ? ? b;\n", Different),
    (JS, "x = a => b;\n", "x = a = > b;\n", Different),
    (RUST, "fn f() { match a { b => c } }\n", "fn f() { match a { b = > c } }\n", Different),
    // a number and a dot
    (KOTLIN, "val x = 1.0\n", "val x = 1 .0\n", Different),
    (RUST, "fn f() { 1..2; }\n", "fn f() { 1. .2; }\n", Different),
  ]);
}

#[test]
fn a_line_break_that_ends_a_statement_is_no_formatting() {
  check(&[
    (KOTLIN, "fun f() {\n  return\n  foo()\n}\n", "fun f() {\n  return foo()\n}\n", Different),
    (KOTLIN, "val x = a\n+ b\n", "val x = a + b\n", Different),
    (KOTLIN, "val x = a +\n  b\n", "val x = a + b\n", Same),
    (KOTLIN, "val x = a\n  .b()\n  .c()\n", "val x = a.b().c()\n", Same),
    // a lambda on the line after a call without brackets is no argument of it; after `)` the parser reads on
    (KOTLIN, "fun f() {\n  foo\n  { bar() }\n}\n", "fun f() {\n  foo { bar() }\n}\n", Different),
    (KOTLIN, "fun f() {\n  foo()\n  { bar() }\n}\n", "fun f() {\n  foo() { bar() }\n}\n", Same),
    (KOTLIN, "fun f() {\n  a\n  (b)\n}\n", "fun f() {\n  a(b)\n}\n", Different),
    (KOTLIN, "fun f() {\n  a\n  [b]\n}\n", "fun f() {\n  a[b]\n}\n", Different),
    (KOTLIN, "fun f() {\n  a; b\n}\n", "fun f() {\n  a\n  b\n}\n", Different),
    (KOTLIN, "fun f() {\n  if (a) b\n  else c\n}\n", "fun f() {\n  if (a) b else c\n}\n", Same),
    (KOTLIN, "fun f() {\n  loop@ for (a in b) {}\n}\n", "fun f() {\n  loop @ for (a in b) {}\n}\n", Different),
    (KOTLIN, "fun f() {\n  return@f x\n}\n", "fun f() {\n  return @f x\n}\n", Different),
    (KOTLIN, "fun f() {\n  return@f x\n}\n", "fun f() {\n  return@f\n  x\n}\n", Different),
    (KOTLIN, "class A\n{\n}\n", "class A {\n}\n", Same),
    // a script: its statements stand at the top level
    (KTS, "println(\"a\")\nfoo()\n", "println(\"a\") foo()\n", Different),
    (KTS, "println(\"a\")\nfoo()\n", "println(\"a\"); foo()\n", Different),
    (KTS, "val x = a\n+ b\n", "val x = a + b\n", Different),
    (KTS, "foo()\nbar()\n", "foo()\n\n\nbar()\n", Same),
    (KTS, "plugins {\n  id(\"a\")\n}\n", "plugins { id(\"a\") }\n", Same),
    (JS, "function f() {\n  return\n  foo()\n}\n", "function f() {\n  return foo()\n}\n", Different),
    (JS, "function f() {\n  return (\n    foo()\n  )\n}\n", "function f() {\n  return (foo())\n}\n", Same),
    (JS, "a\n++b\n", "a++\nb\n", Different),
    (JS, "const x = a\n+ b\n", "const x = a + b\n", Same),
    (JS, "x\n(y)\n", "x(y)\n", Same),
    (JS, "async\nfunction f() {}\n", "async function f() {}\n", Different),
    (JS, "function* g() {\n  yield\n  a\n}\n", "function* g() {\n  yield a\n}\n", Different),
    (JS, "for (;;) {\n  break\n  a\n}\n", "a: for (;;) {\n  break a\n}\n", Different),
    (JS, "let a = 1\nlet b = 2\n", "let a = 1; let b = 2\n", Different),
    (TS, "class A {\n  async\n  x() {}\n}\n", "class A {\n  async x() {}\n}\n", Different),
    (TS, "class A {\n  static\n  x\n}\n", "class A {\n  static x\n}\n", Same),
    (TS, "type A = B\n  | C\n", "type A = B | C\n", Same),
    (TS, "function f() {\n  return\n  foo()\n}\n", "function f() {\n  return foo()\n}\n", Different),
    (TSX, "function f() {\n  return\n  <a/>\n}\n", "function f() {\n  return <a/>\n}\n", Different),
    (STARLARK, "x = 1\ny = 2\n", "x = 1; y = 2\n", Different),
    (STARLARK, "x = (1 +\n  2)\n", "x = (1 + 2)\n", Same),
    // the `\` that continues a line is a token: no blank stands for it; the indent after it counts not
    (STARLARK, "x = 1 + \\\n  2\n", "x = 1 + 2\n", Different),
    (STARLARK, "x = 1 + \\\n  2\n", "x = 1 + \\\n        2\n", Same),
    (STARLARK, "x = [\n  1,\n  2,\n]\n", "x = [1, 2,]\n", Same),
    // a brace language reads no line break
    (
      JAVA,
      "class A {\n  int f() {\n    return\n      foo();\n  }\n}\n",
      "class A { int f() { return foo(); } }\n",
      Same,
    ),
    (
      CSHARP,
      "class A\n{\n  int F()\n  {\n    return\n      Foo();\n  }\n}\n",
      "class A { int F() { return Foo(); } }\n",
      Same,
    ),
    (RUST, "fn f() -> u8 {\n  return\n    foo();\n}\n", "fn f() -> u8 { return foo(); }\n", Same),
  ]);
}

#[test]
fn a_call_wrapped_inside_its_brackets_and_code_indented_again_are_formatting() {
  check(&[
    (KOTLIN, "val x = foo(a, b)\n", "val x = foo(\n  a,\n  b\n)\n", Same),
    (KOTLIN, "val x = foo(a, b)\n", "val x = foo(\n  a,\n  b,\n)\n", Different),
    (
      KTS,
      "dependencies {\n  implementation(\"a\", \"b\")\n}\n",
      "dependencies {\n  implementation(\n    \"a\",\n    \"b\"\n  )\n}\n",
      Same,
    ),
    (JAVA, "class A { int x = foo(a, b); }\n", "class A {\n  int x = foo(\n    a,\n    b);\n}\n", Same),
    (CSHARP, "class A { int x = Foo(a, b); }\n", "class A {\n  int x = Foo(\n    a,\n    b);\n}\n", Same),
    (RUST, "fn f() { foo(a, b); }\n", "fn f() {\n  foo(\n    a,\n    b\n  );\n}\n", Same),
    (TS, "const x = foo(a, b);\n", "const x = foo(\n  a,\n  b\n);\n", Same),
    (JS, "const x = [a, b];\n", "const x = [\n  a,\n  b\n];\n", Same),
    (STARLARK, "x = foo(a, b)\n", "x = foo(\n    a,\n    b\n)\n", Same),
    (
      STARLARK,
      "foo(\n  name = \"a\",\n  deps = [\":b\"],\n)\n",
      "foo(\n    name = \"a\",\n    deps = [\n        \":b\",\n    ],\n)\n",
      Different,
    ),
    (
      STARLARK,
      "foo(\n  name = \"a\",\n  deps = [\":b\"],\n)\n",
      "foo(\n    name = \"a\",\n    deps = [\n        \":b\"\n    ],\n)\n",
      Same,
    ),
    // the indentation of a brace language
    (KOTLIN, "fun f() {\n  a()\n  b()\n}\n", "fun f() {\n\ta()\n\tb()\n}\n", Same),
    (KOTLIN, "fun f() {\n  a()\n  b()\n}\n", "fun f() {\n      a()\n  b()\n}\n", Same),
    (JAVA, "class A {\n  void f() {\n    a();\n  }\n}\n", "class A {\nvoid f() {\na();\n}\n}\n", Same),
    (TS, "function f() {\n  a();\n}\n", "function f() {\n    a();\n}\n", Same),
    (RUST, "fn f() {\n  a();\n}\n", "fn f() {\n    a();\n}\n", Same),
    (
      CSHARP,
      "class A {\n  void F() {\n    a();\n  }\n}\n",
      "class A {\n    void F() {\n        a();\n    }\n}\n",
      Same,
    ),
    // blank lines, spaces at the end of a line, line ends
    (KOTLIN, "fun f() {\n  a()\n  b()\n}\n", "fun f() {\n  a()\n\n\n  b()   \n}\n", Same),
    (KOTLIN, "fun f() {\n  a()\n}\n", "fun f() {\r\n  a()\r\n}\r\n", Same),
    (TS, "a();\nb();\n", "a();\r\nb();\r\n", Same),
    (RUST, "fn f() {\n  a();\n}\n", "fn f() {\r\n  a();\r\n}\r\n", Same),
  ]);
}

#[test]
fn where_the_indentation_is_the_syntax_an_indent_is_no_formatting() {
  check(&[
    // a statement that leaves its block
    (STARLARK, "def f():\n  if x:\n    a()\n    b()\n", "def f():\n  if x:\n    a()\n  b()\n", Different),
    // the same blocks, indented by another width, or by tabs
    (STARLARK, "def f():\n  a()\n  b()\n", "def f():\n    a()\n    b()\n", Different),
    (STARLARK, "def f():\n  a()\n", "def f():\n\ta()\n", Different),
    // what continues a line inside brackets has no indent that counts, nor has a comment or a blank line
    (STARLARK, "def f():\n  a(b,\n    c)\n", "def f():\n  a(b,\n        c)\n", Same),
    (STARLARK, "def f():\n  # why\n  a()\n", "def f():\n      # why\n  a()\n", Same),
    (STARLARK, "def f():\n  a()\n  b()\n", "def f():\n  a()\n\n  b()\n", Same),
    (STARLARK, "x = 1\ny = 2\n", "x = 1\n\n\ny = 2\n", Same),
    (STARLARK, "x = 1\n", " x = 1\n", Different),
  ]);
  assert_eq!(texts_equal_by_tokens(Grammar::StarlarkBuild, "a(name = \"x\")\n", "a(\n    name = \"x\"\n)\n"), Same);
  assert_eq!(
    [
      "a/BUILD",
      "a/BUILD.bazel",
      "defs.bzl",
      "a.py",
      "ci.yml",
      "Makefile",
      "a/b.mk",
      "A.kt",
      "a.rs",
      "a.tsx",
      "a/build.gradle.kts"
    ]
    .map(indentation_matters),
    [true, true, true, true, true, true, true, false, false, false, false]
  );
  // every file the grammar of Starlark reads, whatever its name
  for path in ["WORKSPACE.bzlmod", "third_party/zlib.BUILD", "a/MODULE.bazel", "a/x.star"] {
    assert!(
      crate::highlight::grammar_of(path).is_some_and(|grammar| matches!(grammar, STARLARK | Grammar::StarlarkBuild)),
      "{path}"
    );
    assert!(indentation_matters(path), "{path}");
  }
}

#[test]
fn a_comment_compares_by_its_text_and_is_no_code() {
  check(&[
    (JAVA, "//go:build x\nclass A {}\n", "// go:build x\nclass A {}\n", CodeOnly),
    (RUST, "//go:build x\nfn f() {}\n", "// go:build x\nfn f() {}\n", CodeOnly),
    (KOTLIN, "// a\nval x = 1\n", "// b\nval x = 1\n", CodeOnly),
    (KOTLIN, "val x = 1\n", "// a\nval x = 1\n", CodeOnly),
    (KOTLIN, "val x = 1 // a\nval y = 2\n", "val x = 1\nval y = 2 // a\n", CodeOnly),
    (KOTLIN, "/** A. */\nfun f() {}\n", "/** B. */\nfun f() {}\n", CodeOnly),
    (TS, "/* a */ const x = 1;\n", "/* b */ const x = 1;\n", CodeOnly),
    (JS, "#!/usr/bin/env node\nx();\n", "#!/usr/bin/env deno\nx();\n", CodeOnly),
    (STARLARK, "# a\nx = 1\n", "# b\nx = 1\n", CodeOnly),
    (CSHARP, "/// <summary>A</summary>\nclass A {}\n", "/// <summary>B</summary>\nclass A {}\n", CodeOnly),
    (RUST, "/// A.\nfn f() {}\n", "/// B.\nfn f() {}\n", CodeOnly),
    // a comment and other code is other code
    (KOTLIN, "// a\nval x = 1\n", "// b\nval x = 2\n", Different),
    // the spaces that end a comment's line, and where a comment stands on its line
    (KOTLIN, "val x = 1 // a\n", "val x = 1    // a   \n", Same),
    (KOTLIN, "fun f() {\n  // a\n  b()\n}\n", "fun f() {\n      // a\n      b()\n}\n", Same),
    // a block comment that moved with its code: its other lines without the indent of its first
    (
      KOTLIN,
      "class A {\n  /**\n   * A.\n   */\n  fun f() {}\n}\n",
      "class A {\n    /**\n     * A.\n     */\n    fun f() {}\n}\n",
      Same,
    ),
    (
      JAVA,
      "class A {\n  /*\n   * a\n   *   b\n   */\n  int x;\n}\n",
      "class A {\n\t/*\n\t * a\n\t *   b\n\t */\n\tint x;\n}\n",
      Same,
    ),
    (
      JAVA,
      "class A {\n  /*\n   * a\n   *   b\n   */\n  int x;\n}\n",
      "class A {\n  /*\n   * a\n   * b\n   */\n  int x;\n}\n",
      CodeOnly,
    ),
    (
      CSHARP,
      "class A {\n  /// <summary>\n  /// A\n  /// </summary>\n  int x;\n}\n",
      "class A {\n      /// <summary>\n      /// A\n      /// </summary>\n      int x;\n}\n",
      Same,
    ),
    (
      TS,
      "class A {\n  /**\n   * A.\n   */\n  x = 1;\n}\n",
      "class A {\n      /**\n       * A.\n       */\n      x = 1;\n}\n",
      Same,
    ),
    // only its own lines moved: another comment
    (KOTLIN, "/*\n  a\n*/\nval x = 1\n", "/*\n      a\n*/\nval x = 1\n", CodeOnly),
  ]);
}

#[test]
fn the_text_of_a_jsx_element_compares_as_jsx_reads_it() {
  check(&[
    (TSX, "const a = <p>a  b</p>;\n", "const a = <p>a b</p>;\n", Different),
    (TSX, "const a = <p>a b</p>;\n", "const a = <p> a b</p>;\n", Different),
    (TSX, "const a = <p>a</p>;\n", "const a = <p>\n  a\n</p>;\n", Same),
    // lines are joined by one space
    (TSX, "const a = <p>a b</p>;\n", "const a = <p>\n  a\n  b\n</p>;\n", Same),
    (TSX, "const a = <p>a b c</p>;\n", "const a = <p>\n  a b\n  c\n</p>;\n", Same),
    (TSX, "const a = <p>ab</p>;\n", "const a = <p>\n  a\n  b\n</p>;\n", Different),
    (TSX, "const a = <p>a <b/></p>;\n", "const a = <p>\n  a\n  <b/>\n</p>;\n", Different),
    (TSX, "const a = <p>a <b/></p>;\n", "const a = <p>\n  a <b/>\n</p>;\n", Same),
    // where TypeScript trims a line and Babel does not, the lines stay apart
    (TSX, "const a = <p>a\u{a0} b</p>;\n", "const a = <p>\n  a\u{a0}\n  b\n</p>;\n", Different),
    (TSX, "const a = <p>\n  a\u{a0}\n  b\n</p>;\n", "const a = <p>\n      a\u{a0}\n      b\n</p>;\n", Same),
    (TSX, "const a = <p>\n  a\n  b\n</p>;\n", "const a = <p>\n      a\n      b\n</p>;\n", Same),
    (TSX, "const a = <p>\n  a\n  b\n</p>;\n", "const a = <p>\n  a\n\n  b\n</p>;\n", Same),
    (TSX, "const a = <p><b/><i/></p>;\n", "const a = <p>\n  <b/>\n  <i/>\n</p>;\n", Same),
    (TSX, "const a = <p><b/> <i/></p>;\n", "const a = <p><b/><i/></p>;\n", Different),
    (TSX, "const a = <p><b/>{\" \"}<i/></p>;\n", "const a = <p>\n  <b/>\n  {\" \"}\n  <i/>\n</p>;\n", Same),
    (TSX, "const a = <p x=\"a  b\"/>;\n", "const a = <p x=\"a b\"/>;\n", Different),
    (TSX, "const a = <p x=\"a\" y={b}/>;\n", "const a = <p\n  x=\"a\"\n  y={ b }\n/>;\n", Same),
    (JS, "const a = <p>a  b</p>;\n", "const a = <p>a b</p>;\n", Different),
    (TSX, "const a = <p>\n  a\n</p>;\n", "const a = <p>\n  a\u{a0}\n</p>;\n", Different),
  ]);
}

#[test]
fn a_text_with_a_syntax_error_compares_by_the_tree_the_parser_left() {
  check(&[
    // psi-syntax keeps every token of a broken text in its tree
    (KOTLIN, "fun f( {\n  a()\n}\n", "fun f( {\n      a()\n}\n", Same),
    (KOTLIN, "fun f( {\n  a - -b\n}\n", "fun f( {\n  a--b\n}\n", Different),
    (JAVA, "class A { void f( { a(); } }\n", "class A {\n  void f( {\n    a();\n  }\n}\n", Same),
    (JAVA, "class A { int x = a > > b; }\n", "class A {\n  int x = a > > b;\n}\n", Same),
    (CSHARP, "class A { void F( { a(); } }\n", "class A {\n  void F( {\n    a();\n  }\n}\n", Same),
    (STARLARK, "def f(:\n  a()\n", "def f(:\n  a( )\n", Same),
    (RUST, "fn f( {\n  a();\n}\n", "fn f( {\n      a();\n}\n", Same),
    (RUST, "fn f( {\n  a - b;\n}\n", "fn f( {\n  a + b;\n}\n", Different),
    // an error oxc reports and reads on from
    (TS, "let a = 1;\nlet a = 2;\n", "let a = 1;\n\nlet a = 2;\n", Same),
    // an error that changes no node: oxc and Roslyn keep theirs in a list beside the tree
    (TS, "const f = (a: number) => a + 1;\n", "const f = (a: number)\n  => a + 1;\n", Different),
    (JS, "throw new Error(\"x\");\n", "throw\n  new Error(\"x\");\n", Different),
    (
      CSHARP,
      "#line (1, 2) - (3, 4) \"a.cs\"\nclass A {}\n",
      "#line (1, 2) - (3, 4)\"a.cs\"\nclass A {}\n",
      Different,
    ),
    // the same error in both texts: broken alike
    (TS, "const f = (a: number)\n  => a + 1;\n", "const f = (a: number)\n      => a + 1;\n", Same),
    (JS, "throw\n  new Error(\"x\");\n", "throw\n\n  new Error( \"x\" );\n", Same),
    (CSHARP, "class A { void F() { int x = 1 } }\n", "class A {\n  void F() {\n    int x = 1\n  }\n}\n", Same),
    (RUST, "fn f() { let x = 1 }\n", "fn f() {\n  let x = 1\n}\n", Same),
    // an error on a comment is the comment's: the XML of a documentation comment that does not close
    (CSHARP, "/// <summary>A\nclass A {}\n", "/// <summary>A</summary>\nclass A {}\n", CodeOnly),
    // a file oxc reads only after a recovery has no tokens: never the same, not even as itself
    (
      TS,
      "function f() {}\nclass {{{\nfunction g() {}\n",
      "function f() {}\nclass {{{\nfunction g() {}\n",
      Different,
    ),
    (JS, "let a = (;\n", "let a = ( ;\n", Different),
  ]);
}

#[test]
fn what_a_parser_calls_trivia_and_another_build_compiles_is_code() {
  check(&[
    (CSHARP, "#if DEBUG\nclass A {}\n#endif\n", "#if RELEASE\nclass A {}\n#endif\n", Different),
    (CSHARP, "#if DEBUG\nclass A {}\n#endif\n", "#if   DEBUG\nclass A {}\n#endif\n", Same),
    // the text of a branch the configuration does not take compares by its bytes
    (CSHARP, "#if DEBUG\nclass A { }\n#endif\n", "#if DEBUG\nclass A {}\n#endif\n", Different),
    (
      CSHARP,
      "#if DEBUG\nclass A { int x = 1; }\n#endif\n",
      "#if DEBUG\nclass A { int x = 2; }\n#endif\n",
      Different,
    ),
    (CSHARP, "#region a\nclass A {}\n#endregion\n", "#region b\nclass A {}\n#endregion\n", Different),
    (CSHARP, "#pragma warning disable 1\nclass A {}\n", "class A {}\n", Different),
    (CSHARP, "#nullable enable\nclass A {}\n", "#nullable disable\nclass A {}\n", Different),
    // what a macro of Rust reads of its tokens: which mark follows which
    (RUST, "fn f() { m!(a => b); }\n", "fn f() { m!(a = > b); }\n", Different),
    (RUST, "fn f() { m!(a, b); }\n", "fn f() { m!(a,b); }\n", Same),
    (RUST, "fn f() { m!(a + b); }\n", "fn f() { m!(a+b); }\n", Same),
    (RUST, "fn f() { m!(a - -b); }\n", "fn f() { m!(a--b); }\n", Different),
    (RUST, "fn f() { m!(a, b); }\n", "fn f() {\n  m!(\n    a,\n    b\n  );\n}\n", Same),
  ]);
}

#[test]
fn a_text_over_the_limit_or_nested_too_deeply_has_no_tokens() {
  let long = format!("val x = 1\n// {}\n", "a".repeat(MAX_BYTES));
  assert_eq!(tokens(KOTLIN, &long), None);
  assert_eq!(texts_equal_by_tokens(KOTLIN, &long, &long), Different);
  let fits = format!("// {}\n", "a".repeat(MAX_BYTES - 4));
  assert_eq!(fits.len(), MAX_BYTES);
  assert_eq!(stream(KOTLIN, &fits).all().len(), 1);

  // past the nesting the parser reads, its tree holds the lexer's tokens: `> >` and `>>` would be the same there
  let deep = |open: &str, close: &str, levels: usize| format!("{}a{}", open.repeat(levels), close.repeat(levels));
  let kotlin = format!("val x = {}\n", deep("(", ")", 3000));
  assert_eq!(tokens(KOTLIN, &kotlin), None);
  let java = format!("class A {{ int x = {}; }}\n", deep("(", ")", 3000));
  assert_eq!(tokens(JAVA, &java), None);
  assert_eq!(tokens(STARLARK, &format!("x = {}\n", deep("(", ")", 3000))), None);
  // Roslyn gives the whole text up as one skipped token
  let csharp = format!("class A {{\n  int y = 1;\n  int x = {};\n}}\n", deep("(", ")", 3000));
  assert_eq!(tokens(CSHARP, &csharp), None);
  assert_eq!(texts_equal_by_tokens(CSHARP, &csharp, &csharp), Different);
  assert_eq!(stream(KOTLIN, &format!("val x = {}\n", deep("(", ")", 20))).all().len(), 44);
  assert_eq!(stream(CSHARP, &format!("class A {{ int x = {}; }}\n", deep("(", ")", 20))).all().len(), 49);
  // rust-analyzer and oxc read as deep as the text nests, on a stack of its size
  assert_eq!(stream(RUST, &format!("fn f() {{ {}; }}\n", deep("(", ")", 3000))).all().len(), 6008);
  assert_eq!(stream(TS, &format!("x = {};\n", deep("(", ")", 3000))).all().len(), 6004);
  assert_eq!(stream(JS, &format!("x = {};\n", deep("[", "]", 3000))).all().len(), 6004);
}

#[test]
fn what_stands_between_two_tokens_is_blank() {
  // a byte order mark, and what one parser skips beside the whitespace of Unicode: the same tokens, and the same text
  // without its blanks (`check`)
  check(&[
    (KOTLIN, "\u{feff}val x = 1\n", "val x = 1\n", Same),
    (KTS, "\u{feff}val x = 1\n", "val x = 1\n", Same),
    (JAVA, "\u{feff}class A {}\n", "class A {}\n", Same),
    (CSHARP, "\u{feff}class A {}\n", "class A {}\n", Same),
    (CSHARP, "class A { int x = a\u{1a}+ b; }\n", "class A { int x = a + b; }\n", Same),
    (RUST, "\u{feff}fn f() {}\n", "fn f() {}\n", Same),
    (RUST, "fn f() { a\u{200e}+ b; }\n", "fn f() { a + b; }\n", Same),
    (TS, "\u{feff}x = a + b;\n", "x = a + b;\n", Same),
    (TSX, "x = a\u{a0}+ b;\n", "x = a + b;\n", Same),
    (JS, "x = a\u{feff}+ b;\n", "x = a + b;\n", Same),
    (JS, "x = a\u{2028}+ b;\n", "x = a + b;\n", Same),
    // what a parser does not skip is a token of it
    (KOTLIN, "val x = a\u{a0}+ b\n", "val x = a + b\n", Different),
    (STARLARK, "\u{feff}x = 1\n", "x = 1\n", Different),
    // what oxc skips and ECMAScript calls no whitespace (a zero width space, U+0085) is a syntax error to an engine,
    // to esbuild and to Babel, in a TypeScript file too: an error of the text at its place, wherever it stands
    (JS, "x = a\u{200b}+ b;\n", "x = a + b;\n", Different),
    (JS, "x = a\u{85}+ b;\n", "x = a + b;\n", Different),
    (TS, "x = a\u{200b}+ b;\n", "x = a + b;\n", Different),
    (TSX, "x = a\u{85}+ b;\n", "x = a + b;\n", Different),
    (JS, "\u{200b}x = a + b;\n", "x = a + b;\n", Different),
    (JS, "x = a + b;\u{200b}\n", "x = a + b;\n", Different),
    (JS, "x = a + b;\n// c\n\u{85}", "x = a + b;\n// c\n", Different),
    (JS, "x = a /* c */\u{200b}+ b;\n", "x = a /* c */ + b;\n", Different),
    (JS, "x = a\u{200b}+ b;\n", "x = a +\u{200b}b;\n", Different),
    (JS, "x = a\u{200b}+ b;\n", "x = a\u{85}+ b;\n", Different),
    // broken alike: the same blank at the same place
    (JS, "x = a\u{200b}+ b;\n", "x = a \u{200b}\n  + b;\n", Same),
    // inside a string, a template or a comment it is content, no blank
    (JS, "x = \"a\u{200b}b\";\n", "x =   \"a\u{200b}b\";\n", Same),
    (TSX, "x = <p>a\u{200b}b</p>;\n", "x =   <p>a\u{200b}b</p>;\n", Same),
  ]);
  // such a blank in a text without a token of the code has no place: no tokens
  assert_eq!(tokens(JS, "\u{200b}// c\n"), None);
  assert_eq!(tokens(JS, "\u{85}\n"), None);
  assert_eq!(stream(JS, "// c\n").all().len(), 1);
  assert_eq!(
    [
      '\u{feff}', '\u{200b}', '\u{200e}', '\u{200f}', '\u{1a}', '\u{a0}', ' ', '\t', '\n', '\r', 'a', '\\', '\u{0}', '\u{200d}'
    ]
    .map(is_blank),
    [true, true, true, true, true, true, true, true, true, true, false, false, false, false]
  );
  // the same without their blanks, in whatever pieces, proves nothing: a space moved from one string to another
  assert!(same_without_blanks(["val s = \"a b\" +", "  \"c\""], ["val s = \"ab\" + \" c\""]));
  assert_eq!(texts_equal_by_tokens(KOTLIN, "val s = \"a b\" +\n  \"c\"\n", "val s = \"ab\" + \" c\"\n"), Different);
  assert!(!same_without_blanks(["a b"], ["a c"]));
  assert!(!same_without_blanks(["x = 1 + \\", "  2"], ["x = 1 + 2"]));
  // whitespace written as a Unicode escape of Java is no blank of the text: no tokens
  assert_eq!(tokens(JAVA, "class A {\\u0020int x; }\n"), None);
  assert_eq!(stream(JAVA, "class A { String s = \"\\u0020\"; }\n").all().len(), 9);
  // Roslyn lexes UTF-16 units and cuts a character of two in the symbol of a directive: such tokens are none
  assert_eq!(tokens(CSHARP, "#if \u{1d4b3}\nclass A{}\n#endif\n"), None);
  assert_eq!(stream(CSHARP, "#if X\nclass A{}\n#endif\n").all().len(), 6);
  // Roslyn drops the name before a misplaced `explicit operator`: its tree is of another text, whose offsets it has
  assert_eq!(tokens(CSHARP, "class A { int I explicit operator }\n"), None);
  assert_eq!(stream(CSHARP, "class A { int I; }\n").all().len(), 7);
}

/// Every blank (`is_blank`) a parser skips between two tokens, inside brackets, where no line break counts: what the
/// language of each calls whitespace. A parser that skips one more must not make formatting of it unseen: oxc skips
/// U+0085 and U+200B too, which are errors in its tokens and so not in its list
#[test]
fn each_parser_skips_between_two_tokens_what_its_language_calls_whitespace() {
  let blanks: Vec<char> = (0..=u32::from(char::MAX)).filter_map(char::from_u32).filter(|character| is_blank(*character)).collect();
  assert_eq!(blanks.len(), 30);
  let skipped = |grammar: Grammar, before: &str, after: &str| -> String {
    let plain = format!("{before} {after}");
    let same = |character: &&char| texts_equal_by_tokens(grammar, &format!("{before}{character}{after}"), &plain) == Same;
    let codes: Vec<String> = blanks.iter().filter(same).map(|character| format!("{:04x}", u32::from(*character))).collect();
    codes.join(" ")
  };
  // Kotlin: the space, the tab, the form feed and the line ends
  let kotlin = "0009 000a 000c 000d 0020";
  // Java: the same (JLS 3.6)
  let java = "0009 000a 000c 000d 0020";
  // Starlark: the space, the tab, the carriage return and the newline
  let starlark = "0009 000a 000d 0020";
  // C#, as Roslyn reads it: the class `Zs`, the tab, U+000B, the form feed, the line ends of C# (U+0085, U+2028 and
  // U+2029 among them) and Ctrl-Z; a byte order mark only at the start of the file
  let csharp = "0009 000a 000b 000c 000d 001a 0020 0085 00a0 1680 2000 2001 2002 2003 2004 2005 2006 2007 2008 2009 200a 2028 2029 202f 205f 3000";
  // Rust, as rustc's lexer reads it: `Pattern_White_Space`
  let rust = "0009 000a 000b 000c 000d 0020 0085 200e 200f 2028 2029";
  // ECMAScript: `WhiteSpace` (the tab, U+000B, the form feed, the byte order mark, the class `Zs`) and
  // `LineTerminator`; not U+0085 and U+200B, which oxc skips and an engine refuses
  let ecmascript = "0009 000a 000b 000c 000d 0020 00a0 1680 2000 2001 2002 2003 2004 2005 2006 2007 2008 2009 200a 2028 2029 202f 205f 3000 feff";
  assert_eq!(
    [
      skipped(KOTLIN, "val x = (a", "+ b)\n"),
      skipped(KTS, "val x = (a", "+ b)\n"),
      skipped(JAVA, "class A { int x = (a", "+ b); }\n"),
      skipped(STARLARK, "x = (a", "+ b)\n"),
      skipped(CSHARP, "class A { int x = (a", "+ b); }\n"),
      skipped(RUST, "fn f() { (a", "+ b); }\n"),
      skipped(JS, "x = (a", "+ b);\n"),
      skipped(TS, "x = (a", "+ b);\n"),
      skipped(TSX, "x = (a", "+ b);\n"),
    ],
    [kotlin, kotlin, java, starlark, csharp, rust, ecmascript, ecmascript, ecmascript].map(str::to_string)
  );
}

#[test]
fn the_content_of_a_run_is_the_same_wherever_it_stands() {
  let run = |grammar: Grammar, text: &str, lines: std::ops::Range<u32>| content(stream(grammar, text).in_lines(lines));
  // a function as the last of its file and with another after it: other tokens at its ends, the same content
  let (alone, followed) = ("fun f() {\n  return foo()\n}\n", "val a = 1\n\nfun f() {\n      return foo( )\n}\n\nfun g() {}\n");
  assert_eq!(equal_by_tokens(&stream(KOTLIN, alone), 0..3, &stream(KOTLIN, followed), 2..5), Different);
  assert_eq!(run(KOTLIN, alone, 0..3), run(KOTLIN, followed, 2..5));
  // what changes between its ends is another content: a line break that ends a statement, a space inside a string
  assert_ne!(run(KOTLIN, alone, 0..3), run(KOTLIN, "fun f() {\n  return\n  foo()\n}\n", 0..4));
  assert_ne!(run(KOTLIN, alone, 0..3), run(KOTLIN, "fun f() {\n  return bar()\n}\n", 0..3));
  assert_ne!(run(KOTLIN, "val s = \"a  b\"\n", 0..1), run(KOTLIN, "val s = \"a b\"\n", 0..1));
  // also where the run starts or ends at the changed token: what closes after `return` is between two tokens of it
  assert_ne!(run(KOTLIN, alone, 1..2), run(KOTLIN, "fun f() {\n  return\n  foo()\n}\n", 1..3));
  assert_ne!(
    run(JS, "function f() {\n  return foo()\n}\n", 1..2),
    run(JS, "function f() {\n  return\n  foo()\n}\n", 1..3)
  );
  // an error the parser reported between two tokens
  assert_ne!(run(TS, "const f = (a: number) => a + 1;\n", 0..1), run(TS, "const f = (a: number)\n  => a + 1;\n", 0..2));
  // a comment is part of it
  assert_ne!(run(KOTLIN, "// a\nval x = 1\n", 0..2), run(KOTLIN, "// b\nval x = 1\n", 0..2));
  assert_eq!(
    run(KOTLIN, "// a\nval x = 1\n", 0..2),
    run(KOTLIN, "val y = 2\n    // a   \n    val x = 1\nval z = 3\n", 1..3)
  );
  // the indent of a line of Starlark is of its first token, at an end of the run too
  assert_ne!(
    run(STARLARK, "def f():\n  if x:\n    a()\n    return\n", 3..4),
    run(STARLARK, "def f():\n  if x:\n    a()\n  return\n", 3..4)
  );
  assert_eq!(
    run(STARLARK, "def f():\n  a()\n  return\n", 2..3),
    run(STARLARK, "def f():\n  b()\n  c()\n  return\n\ndef g():\n  pass\n", 3..4)
  );
  // a renamed name
  assert_ne!(run(RUST, "fn f() { let total = 1; }\n", 0..1), run(RUST, "fn f() { let sum = 1; }\n", 0..1));
  assert_eq!(content(&[]), content(stream(KOTLIN, "\n\n").all()));
}

#[test]
fn a_statement_wrapped_into_a_block_keeps_its_tokens() {
  // the body of the `if` is the two calls, indented: the same keys, though the tree around them changed
  let old_text = "fun f() {\n  first()\n  a(1)\n  b(2)\n  last()\n}\n";
  let new_text = "fun f() {\n  first()\n  if (x) {\n    a(1)\n    b(2)\n  }\n  last()\n}\n";
  let (old, new) = (stream(KOTLIN, old_text), stream(KOTLIN, new_text));
  let runs = |old_lines: std::ops::Range<u32>, new_lines: std::ops::Range<u32>| compare(old.in_lines(old_lines), new.in_lines(new_lines));
  assert_eq!(runs(2..4, 3..5), Same);
  assert_eq!(runs(2..4, 2..5), Different);
  // the lines around the block keep their tokens too
  assert_eq!(runs(1..2, 1..2), Same);
  assert_eq!(runs(4..5, 6..7), Same);
  // as a range of lines between lines that did not change they are not the same: the code before them is another,
  // the `{` of the `if` and not the call before it
  assert_eq!(equal_by_tokens(&old, 2..4, &new, 3..5), Different);

  for (grammar, old_text, new_text, old_lines, new_lines) in [
    (
      JAVA,
      "class A {\n  void f() {\n    a(1);\n    b(2);\n  }\n}\n",
      "class A {\n  void f() {\n    try {\n      a(1);\n      b(2);\n    } finally {}\n  }\n}\n",
      2..4,
      3..5,
    ),
    (
      TS,
      "function f() {\n  a(1);\n  b(2);\n}\n",
      "function f() {\n  if (x) {\n    a(1);\n    b(2);\n  }\n}\n",
      1..3,
      2..4,
    ),
    (RUST, "fn f() {\n  a(1);\n  b(2);\n}\n", "fn f() {\n  if x {\n    a(1);\n    b(2);\n  }\n}\n", 1..3, 2..4),
    (
      CSHARP,
      "class A {\n  void F() {\n    a(1);\n    b(2);\n  }\n}\n",
      "class A {\n  void F() {\n    lock (x) {\n      a(1);\n      b(2);\n    }\n  }\n}\n",
      2..4,
      3..5,
    ),
    (
      KOTLIN,
      "val a = 1\nfun f() {}\nfun g() {}\nfun h() {}\n",
      "val a = 1\nclass A {\n  fun f() {}\n  fun g() {}\n}\nfun h() {}\n",
      1..3,
      2..4,
    ),
  ] {
    let (old, new) = (stream(grammar, old_text), stream(grammar, new_text));
    assert_eq!(compare(old.in_lines(old_lines), new.in_lines(new_lines)), Same, "{grammar:?}");
  }

  // without braces the statement is the body itself: the `if` closes after it, another token
  let (old, new) = (stream(KOTLIN, "fun f() {\n  a(1)\n}\n"), stream(KOTLIN, "fun f() {\n  if (x)\n    a(1)\n}\n"));
  assert_eq!(compare(old.in_lines(1..2), new.in_lines(2..3)), Different);
}

#[test]
fn the_tokens_of_lines_are_those_with_a_byte_in_them() {
  // a text block of Java is one token over its lines
  let text = "class A {\n  String s = \"\"\"\n    x\n\n    y\n    \"\"\";\n  int b = 2;\n}\n";
  let all = stream(JAVA, text);
  let texts = |lines: std::ops::Range<u32>| all.in_lines(lines).iter().map(|token| token.text_in(text)).collect::<Vec<_>>();
  let block = "\"\"\"\n    x\n\n    y\n    \"\"\"";
  assert_eq!(all.line_count(), 9);
  assert_eq!(texts(0..1), ["class", "A", "{"]);
  assert_eq!(texts(0..0), [""; 0]);
  assert_eq!(texts(8..12), [""; 0]);
  assert_eq!(texts(1..2), ["String", "s", "=", block]);
  // a line inside the text block is the text block, and so is the place between two of its lines
  assert_eq!(texts(3..4), [block]);
  assert_eq!(texts(3..3), [block]);
  assert_eq!(texts(5..6), [block, ";"]);
  // the place before a line that starts with a token is no token's
  assert_eq!(texts(6..6), [""; 0]);
  assert_eq!(texts(6..7), ["int", "b", "=", "2", ";"]);
  assert_eq!(all.line_bytes(1..3), 10..33);
  assert_eq!(all.line_bytes(9..12), 64..64);
  assert_eq!(all.in_bytes(12..13).iter().map(|token| token.text_in(text)).collect::<Vec<_>>(), ["String"]);

  // so a blank line the text block gains is no formatting, though no token starts in it
  let longer = "class A {\n  String s = \"\"\"\n    x\n\n\n    y\n    \"\"\";\n  int b = 2;\n}\n";
  assert_eq!(equal_by_tokens(&all, 3..4, &stream(JAVA, longer), 3..5), Different);
  assert_eq!(equal_by_tokens(&all, 3..3, &stream(JAVA, longer), 3..4), Different);
  assert_eq!(equal_by_tokens(&all, 6..7, &stream(JAVA, longer), 7..8), Same);
  // Kotlin's lexer writes a raw string line by line: a blank line is one more token of it
  let (old, new) = ("val s = \"\"\"\n  x\n  y\n\"\"\"\n", "val s = \"\"\"\n  x\n\n  y\n\"\"\"\n");
  assert_eq!(equal_by_tokens(&stream(KOTLIN, old), 2..2, &stream(KOTLIN, new), 2..3), Different);
  // a line of a comment that changed is the comment, while the code around it is the same
  let (old, new) = ("/*\n a\n b\n*/\nval x = 1\n", "/*\n a\n c\n*/\nval x = 1\n");
  assert_eq!(equal_by_tokens(&stream(KOTLIN, old), 2..3, &stream(KOTLIN, new), 2..3), CodeOnly);
  assert_eq!(equal_by_tokens(&stream(KOTLIN, old), 4..5, &stream(KOTLIN, new), 4..5), Same);
  // two runs without a token are the same runs; whether lines without a token are the same, the code around them says
  assert_eq!(compare(&[], &[]), Same);
  assert_eq!(compare(all.in_lines(8..9), all.in_lines(0..0)), Same);
  assert_eq!(equal_by_tokens(&all, 8..9, &all, 0..0), Different);
  assert_eq!([all.places_in_lines(1..2), all.places_in_lines(3..3), all.places_in_lines(6..6)], [3..7, 6..7, 8..8]);
}

#[test]
fn lines_without_a_token_are_the_same_only_where_the_code_around_them_reads_the_same() {
  // a text and its lines
  type Side = (&'static str, std::ops::Range<u32>);
  // (grammar, old, new, the answer of the lines, that of the whole texts)
  let cases: [(Grammar, Side, Side, Equality, Equality); 13] = [
    // blank lines between two statements, before the first one and after the last one
    (KOTLIN, ("val a = 1\nval b = 2\n", 1..1), ("val a = 1\n\n\nval b = 2\n", 1..3), Same, Same),
    (KOTLIN, ("val a = 1\n", 0..0), ("\n\nval a = 1\n", 0..2), Same, Same),
    (KOTLIN, ("val a = 1\n", 1..1), ("val a = 1\n\n\n", 1..3), Same, Same),
    (
      JAVA,
      ("class A {\n  int a;\n  int b;\n}\n", 2..2),
      ("class A {\n  int a;\n\n  int b;\n}\n", 2..3),
      Same,
      Same,
    ),
    // Kotlin reads a lambda after a call as its argument across any number of line breaks
    (KOTLIN, ("fun f() {\n  foo()\n  { x }\n}\n", 2..2), ("fun f() {\n  foo()\n\n  { x }\n}\n", 2..3), Same, Same),
    // a blank line after the `\` that continues a line of Starlark ends the statement: the lines around it are others
    (
      STARLARK,
      ("def f(a):\n    x = a + \\\n        2\n    return x\n", 2..2),
      ("def f(a):\n    x = a + \\\n\n        2\n    return x\n", 2..3),
      Different,
      Different,
    ),
    (STARLARK, ("x = (a +\n    2)\n", 1..1), ("x = (a +\n\n    2)\n", 1..2), Same, Same),
    // a blank line before a `#!` makes code of it
    (
      KTS,
      ("#!/usr/bin/env kotlin\nprintln(1)\n", 0..0),
      ("\n#!/usr/bin/env kotlin\nprintln(1)\n", 0..1),
      Different,
      Different,
    ),
    (KTS, ("#!/usr/bin/env kotlin\n", 0..0), ("\n#!/usr/bin/env kotlin\n", 0..1), Different, Different),
    (
      RUST,
      ("#!/usr/bin/env run\nfn main() {}\n", 0..0),
      ("\n#!/usr/bin/env run\nfn main() {}\n", 0..1),
      Different,
      Different,
    ),
    (
      CSHARP,
      ("#!/usr/bin/env dotnet\nclass A {}\n", 0..0),
      ("\n#!/usr/bin/env dotnet\nclass A {}\n", 0..1),
      Different,
      Different,
    ),
    // a line after it changes nothing
    (
      KTS,
      ("#!/usr/bin/env kotlin\nprintln(1)\n", 1..1),
      ("#!/usr/bin/env kotlin\n\nprintln(1)\n", 1..2),
      Same,
      Same,
    ),
    // lines with tokens: a comment that changed before them and one after them are no code around them
    (
      KOTLIN,
      ("// a\nval x = f(1,\n  2)\n// c\nval y = 2\n", 1..3),
      ("// b\nval x = f(1, 2)\n// d\nval y = 2\n", 1..2),
      Same,
      CodeOnly,
    ),
  ];
  let mut failed = Vec::new();
  for (grammar, (old, old_lines), (new, new_lines), expected, whole) in cases {
    let (old_tokens, new_tokens) = (tokens(grammar, old), tokens(grammar, new));
    let (Some(old_tokens), Some(new_tokens)) = (old_tokens, new_tokens) else {
      failed.push(format!("{grammar:?}: {old:?} or {new:?} has no tokens"));
      continue;
    };
    let answers = [
      equal_by_tokens(&old_tokens, old_lines.clone(), &new_tokens, new_lines.clone()),
      equal_by_tokens(&new_tokens, new_lines, &old_tokens, old_lines),
      compare(old_tokens.all(), new_tokens.all()),
    ];
    if answers != [expected, expected, whole] {
      failed.push(format!("{grammar:?}: {old:?} and {new:?} are {answers:?}, not {expected:?} and {whole:?}"));
    }
  }
  assert_eq!(failed, Vec::<String>::new());
  // a script of JavaScript with a line before its `#!` has no tokens at all
  assert_eq!(tokens(JS, "\n#!/usr/bin/env node\nx = 1;\n"), None);

  // the code before a run ends alike when its last token closes the same nodes, whatever that token is; the code
  // after it starts alike when its first token is the same token with the same nodes opening before it
  let text = "fun f() {\n  a(1)\n  b(2)\n  // c\n  return 3\n}\n";
  let all = stream(KOTLIN, text);
  let before = |line: u32| &all.all()[..all.places_in_lines(line..line).start];
  let after = |line: u32| &all.all()[all.places_in_lines(line..line).end..];
  assert!(same_before(before(2), before(3)));
  assert!(!same_before(before(1), before(2)));
  assert!(same_before(&[], &[]));
  assert!(!same_before(&[], before(2)));
  // the comment between is not asked
  assert!(same_after(after(3), after(4)));
  assert!(!same_after(after(1), after(2)));
  assert!(!same_after(after(4), after(5)));
  assert!(same_after(&[], after(6)));
  assert!(!same_after(&[], after(5)));
}

#[test]
fn the_lines_and_the_columns_are_those_of_the_view() {
  // a surrogate pair is two columns, a character of two or three bytes one
  let text = "val a = \"😀é\" + b // ж\r\nval c = 1\rval d = 2\n/* x\n y */ val e = 3";
  let all = stream(KOTLIN, text);
  let places: Vec<String> = all
    .all()
    .iter()
    .map(|token| format!("{:?} {}:{}-{}:{}", token.text_in(text), token.line, token.start, token.end_line, token.end))
    .collect();
  let place = |text: &str, start: (u32, u32), end: (u32, u32)| format!("{text:?} {}:{}-{}:{}", start.0, start.1, end.0, end.1);
  assert_eq!(
    places,
    [
      place("val", (0, 0), (0, 3)),
      place("a", (0, 4), (0, 5)),
      place("=", (0, 6), (0, 7)),
      place("\"", (0, 8), (0, 9)),
      place("😀é", (0, 9), (0, 12)),
      place("\"", (0, 12), (0, 13)),
      place("+", (0, 14), (0, 15)),
      place("b", (0, 16), (0, 17)),
      place("// ж", (0, 18), (0, 22)),
      place("val", (1, 0), (1, 3)),
      place("c", (1, 4), (1, 5)),
      place("=", (1, 6), (1, 7)),
      place("1", (1, 8), (1, 9)),
      place("val", (2, 0), (2, 3)),
      place("d", (2, 4), (2, 5)),
      place("=", (2, 6), (2, 7)),
      place("2", (2, 8), (2, 9)),
      place("/* x\n y */", (3, 0), (4, 5)),
      place("val", (4, 6), (4, 9)),
      place("e", (4, 10), (4, 11)),
      place("=", (4, 12), (4, 13)),
      place("3", (4, 14), (4, 15)),
    ]
  );
  assert_eq!(all.line_count(), 5);
  let comment = all.all()[17];
  assert_eq!(
    [comment.columns_on(3, 4), comment.columns_on(4, 15), comment.columns_on(2, 9)],
    [Some(0..4), Some(0..5), None]
  );
  let plain = stream(KOTLIN, "val a = 1\nval b = 2\n");
  assert_eq!(plain.line_count(), 3);
  assert_eq!(stream(KOTLIN, "").line_count(), 1);
  assert!(plain.heap_bytes() >= 8 * size_of::<Token>());
  assert_eq!(size_of::<Token>(), 64);

  // a token whose bytes end with a line break ends on the line of that break, the last line it has a byte in: a line
  // break of a raw string of Kotlin, the text of a branch of `#if` not taken, the `\` that continues a line of Starlark
  let places = |grammar: Grammar, text: &str| -> Vec<String> {
    let all = stream(grammar, text);
    let broken = all.all().iter().filter(|token| token.text_in(text).ends_with(['\n', '\r']));
    broken
      .map(|token| {
        // it is in every line it names, and its columns on its last line are within that line
        for line in token.line..=token.end_line {
          assert!(
            all.in_lines(line..line + 1).iter().any(|it| it.byte_start == token.byte_start),
            "{:?} in line {line}",
            token.text_in(text)
          );
        }
        format!("{:?} {}:{}-{}:{}", token.text_in(text), token.line, token.start, token.end_line, token.end)
      })
      .collect()
  };
  assert_eq!(
    places(KOTLIN, "val s = \"\"\"\n  x\r\n  y\n\"\"\"\n"),
    [
      place("\n", (0, 11), (0, 11)),
      // the lexer cuts the `\r\n` in two: both halves are of the line break, at the end of its line
      place("  x\r", (1, 0), (1, 3)),
      place("\n", (1, 3), (1, 3)),
      place("\n", (2, 3), (2, 3)),
    ]
  );
  assert_eq!(places(CSHARP, "#if DEBUG\nclass A { }\n\n#endif\nclass B {}\n"), [place("class A { }\n\n", (1, 0), (2, 0))]);
  assert_eq!(places(STARLARK, "x = 1 + \\\n  2\n"), [place("\\\n", (0, 8), (0, 9))]);
  let line_break = stream(KOTLIN, "val s = \"\"\"\n  x\n\"\"\"\n").all()[4];
  assert_eq!([line_break.columns_on(0, 11), line_break.columns_on(1, 3)], [Some(11..11), None]);
}

#[test]
fn a_token_has_its_class_and_a_renamed_name_keeps_its_shape() {
  assert_eq!(
    render(KOTLIN, "/** D. */\nfun f(a: A?) = a?.b ?: \"x $a ${a.c}\" // c\n"),
    [
      "comment \"/** D. */\"",
      "keyword \"fun\"",
      "name \"f\"",
      "other \"(\"",
      "name \"a\"",
      "other \":\"",
      "name \"A\"",
      "other \"?\"",
      "other \")\"",
      "other \"=\"",
      "name \"a\"",
      "other \"?.\"",
      "name \"b\"",
      "other \"?:\"",
      "literal \"\\\"\"",
      "literal \"x \"",
      "other \"$\"",
      "name \"a\"",
      "literal \" \"",
      "other \"${\"",
      "name \"a\"",
      "other \".\"",
      "name \"c\"",
      "other \"}\"",
      "literal \"\\\"\"",
      "comment \"// c\"",
    ]
  );
  assert_eq!(
    render(TSX, "const type = <p x=\"1\">\n  a  b\n  <b/>\n</p>; // c\nlet r = a / b + /x y/g.test(`a ${b}`);\n"),
    [
      "keyword \"const\"",
      "name \"type\"",
      "other \"=\"",
      "other \"<\"",
      "name \"p\"",
      "name \"x\"",
      "other \"=\"",
      "literal \"\\\"1\\\"\"",
      "other \">\"",
      "literal \"\\n  a  b\\n  \"",
      "other \"<\"",
      "name \"b\"",
      "other \"/\"",
      "other \">\"",
      "other \"<\"",
      "other \"/\"",
      "name \"p\"",
      "other \">\"",
      "other \";\"",
      "comment \"// c\"",
      "keyword \"let\"",
      "name \"r\"",
      "other \"=\"",
      "name \"a\"",
      "other \"/\"",
      "name \"b\"",
      "other \"+\"",
      "literal \"/x y/g\"",
      "other \".\"",
      "name \"test\"",
      "other \"(\"",
      "literal \"`a ${\"",
      "name \"b\"",
      "literal \"}`\"",
      "other \")\"",
      "other \";\"",
    ]
  );
  assert_eq!(
    render(JAVA, "class A { /** D. */ int f() { return a >> 1; } String s = \"x\"; }\n"),
    [
      "keyword \"class\"",
      "name \"A\"",
      "other \"{\"",
      "comment \"/** D. */\"",
      "keyword \"int\"",
      "name \"f\"",
      "other \"(\"",
      "other \")\"",
      "other \"{\"",
      "keyword \"return\"",
      "name \"a\"",
      "other \">>\"",
      "other \"1\"",
      "other \";\"",
      "other \"}\"",
      "name \"String\"",
      "name \"s\"",
      "other \"=\"",
      "literal \"\\\"x\\\"\"",
      "other \";\"",
      "other \"}\"",
    ]
  );
  assert_eq!(
    render(CSHARP, "#if DEBUG\nclass A { }\n#endif\nclass B { /// <summary>D</summary>\n  string s = $\"a {b}\"; }\n"),
    [
      "other \"#\"",
      "keyword \"if\"",
      "name \"DEBUG\"",
      "other \"class A { }\\n\"",
      "other \"#\"",
      "keyword \"endif\"",
      "keyword \"class\"",
      "name \"B\"",
      "other \"{\"",
      "comment \"/// <summary>D</summary>\"",
      "keyword \"string\"",
      "name \"s\"",
      "other \"=\"",
      "literal \"$\\\"\"",
      "literal \"a \"",
      "other \"{\"",
      "name \"b\"",
      "other \"}\"",
      "literal \"\\\"\"",
      "other \";\"",
      "other \"}\"",
    ]
  );
  assert_eq!(
    render(STARLARK, "# c\ndef f(x):\n  return \"a\" if x else None\n"),
    [
      "comment \"# c\"",
      "keyword \"def\"",
      "name \"f\"",
      "other \"(\"",
      "name \"x\"",
      "other \")\"",
      "other \":\"",
      "keyword \"return\"",
      "literal \"\\\"a\\\"\"",
      "keyword \"if\"",
      "name \"x\"",
      "keyword \"else\"",
      "keyword \"None\"",
    ]
  );
  assert_eq!(
    render(RUST, "/// D.\nfn f<'a>(x: &'a str) -> u8 { m!(x); b'a' }\n"),
    [
      "comment \"/// D.\"",
      "keyword \"fn\"",
      "name \"f\"",
      "other \"<\"",
      "name \"'a\"",
      "other \">\"",
      "other \"(\"",
      "name \"x\"",
      "other \":\"",
      "other \"&\"",
      "name \"'a\"",
      "name \"str\"",
      "other \")\"",
      "other \"->\"",
      "name \"u8\"",
      "other \"{\"",
      "name \"m\"",
      "other \"!\"",
      "other \"(\"",
      "name \"x\"",
      "other \")\"",
      "other \";\"",
      "literal \"b'a'\"",
      "other \"}\"",
    ]
  );

  // a renamed name: the same shape, another key; any other token of the two texts is the same token
  for (grammar, old, new) in [
    (KOTLIN, "fun f() {\n  val total = 1\n  print(total)\n}\n", "fun f() {\n  val sum = 1\n  print(sum)\n}\n"),
    (
      JAVA,
      "class A { int f() { int total = 1; return total; } }\n",
      "class A { int f() { int sum = 1; return sum; } }\n",
    ),
    (TS, "function f() { const total = 1; return total; }\n", "function f() { const sum = 1; return sum; }\n"),
    (RUST, "fn f() -> u8 { let total = 1; total }\n", "fn f() -> u8 { let sum = 1; sum }\n"),
    (
      CSHARP,
      "class A { int F() { int total = 1; return total; } }\n",
      "class A { int F() { int sum = 1; return sum; } }\n",
    ),
    (STARLARK, "def f():\n  total = 1\n  return total\n", "def f():\n  sum = 1\n  return sum\n"),
  ] {
    let (old_tokens, new_tokens) = (stream(grammar, old), stream(grammar, new));
    assert_eq!(old_tokens.all().len(), new_tokens.all().len(), "{grammar:?}");
    let mut renamed = 0;
    for (before, after) in old_tokens.all().iter().zip(new_tokens.all()) {
      assert_eq!(before.shape(), after.shape(), "{grammar:?}: {}", before.text_in(old));
      if before.key() != after.key() {
        assert_eq!((before.class, before.text_in(old), after.text_in(new)), (TokenClass::Name, "total", "sum"), "{grammar:?}");
        renamed += 1;
      }
    }
    assert_eq!(renamed, 2, "{grammar:?}");
  }
}

#[test]
fn the_keys_are_the_same_in_every_run() {
  // a key is a hash of names and bytes, the same across runs, machines and compilers: a stored fingerprint reads the
  // content of the tokens, a stored diff the keys. Another value here is another rule: bump `RULES_VERSION`
  let digest = |grammar: Grammar, text: &str| {
    let all = stream(grammar, text);
    let fold = |of: fn(&Token) -> u64| all.all().iter().fold(0u64, |hash, token| hash.rotate_left(7) ^ of(token));
    format!("{:016x} {:016x}", fold(Token::key), fold(|token| token.text))
  };
  assert_eq!(
    [
      digest(KOTLIN, "/** D. */\nfun f(a: A?) = a?.b ?: \"x $a\" // c\n"),
      digest(JAVA, "class A { int f() { return a >> 1; } }\n"),
      digest(CSHARP, "#if DEBUG\nclass A { }\n#endif\nclass B { string s = $\"a {b}\"; }\n"),
      digest(STARLARK, "def f(x):\n  return \"a\" if x else None\n"),
      digest(RUST, "fn f<'a>(x: &'a str) -> u8 { m!(x => 1); b'a' }\n"),
      digest(TSX, "const a = <p x=\"1\">\n  a  b\n</p>; // c\nlet r = a / b + /x y/g.test(`a ${b}`);\n"),
    ],
    [
      "4bf7df29e8960d75 b4a392a8fa194b65",
      "be5fb3839b630615 18d5a08c272c8e5b",
      "d55108ef23201050 cf0428d983418350",
      "d6e411e585db0eab 2a71b38caf3a8bee",
      "3e20d1703d6fb94a 7db667bb188936b9",
      "50617b7297164d8b 59bf59805977bb9e",
    ]
  );
}

#[test]
fn tokens_version_is_stable() {
  // stored tokens, and what was derived from them, are made again when the rules or the parsers change
  let version = tokens_version();
  assert_eq!(version, tokens_version());
  assert_ne!(version, crate::extract::extractor_version());
  assert_ne!(version, crate::highlight::highlighter_version());
  if cfg!(all(feature = "kotlin", feature = "java", feature = "rust", feature = "typescript", feature = "starlark")) {
    assert_eq!(
      version, TOKENS_VERSION,
      "RULES_VERSION {RULES_VERSION} of the tokens, that of the summaries or a grammar changed: set TOKENS_VERSION to {version}"
    );
  }
}

/// Every fixture of the crate and its own sources: the checks of `examples/shared/token_checks.rs`
#[test]
fn every_fixture_formatted_again_has_the_same_tokens() {
  fn walk(path: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if path.is_dir() {
      for entry in std::fs::read_dir(path).unwrap() {
        walk(&entry.unwrap().path(), out);
      }
    } else {
      out.push(path.to_path_buf());
    }
  }
  let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let mut files = Vec::new();
  for directory in ["testdata", "src", "examples"] {
    walk(&root.join(directory), &mut files);
  }
  files.sort();
  let (mut checked, mut lines, mut failed) = (0, 0, Vec::new());
  for path in files {
    let name = path.strip_prefix(&root).unwrap().to_string_lossy().into_owned();
    let Some(grammar) = crate::highlight::grammar_of(&name) else { continue };
    let text = std::fs::read_to_string(&path).unwrap();
    match token_checks::check(grammar, &name, &text) {
      Ok(Some(found)) => {
        assert!(found.tokens > 0, "{name}");
        checked += 1;
        lines += found.lines;
      }
      Ok(None) => failed.push(format!("{name}: no tokens")),
      Err(what) => failed.push(format!("{name}: {what}")),
    }
  }
  assert_eq!(failed, Vec::<String>::new());
  assert!(checked >= 100 && lines >= 20_000, "{checked} files, {lines} lines");
}
