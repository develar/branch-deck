//! The highlighting of Starlark from its PSI, by the Bazel plugin's rules: `StarlarkSyntaxHighlighter` for the tokens
//! and `starlarkSemanticHighlightingColor` for the names a parent gives a role (its `FunctionAnnotatorTestData.bzl`)

use crate::highlight::{COMMENT, Role};
use code_tokens::{Grammar, grammar_of};
use pretty_assertions::assert_eq;

fn style_at(grammar: Grammar, text: &str, needle: &str, occurrence: usize) -> u32 {
  crate::test_support::style_at(grammar, text, needle, occurrence)
}

const KEYWORD: u32 = Role::Keyword.style();
const DECLARATION: u32 = Role::FunctionDeclaration.style();
const NAMED: u32 = Role::NamedArgument.style();

/// `FunctionAnnotatorTestData.bzl`: the name of a `def`, a named argument's name and its `=`; a positional argument and
/// a call in a `.bzl` file stay plain
#[test]
fn a_def_s_name_and_a_named_argument_are_painted_as_the_plugin_annotates_them() {
  let text = "def square(x):\n    return x * x\n\nsquare(x = 3)\n\nsquare(3)\n";
  assert_eq!(
    [("square", 0), ("x =", 0), ("= 3", 0), ("square", 1), ("3)", 1)].map(|(needle, at)| style_at(Grammar::Starlark, text, needle, at)),
    [DECLARATION, NAMED, NAMED, 0, Role::Number.style()]
  );
  assert_eq!(
    (style_at(Grammar::Starlark, text, "def", 0), style_at(Grammar::Starlark, text, "return", 0)),
    (KEYWORD, KEYWORD)
  );
}

/// A top-level call of a BUILD or MODULE file declares: its callee is a declaration (`isBazelFileTopLevelCall`), the
/// receiver of a dotted one (`native`) and a nested call (`glob`) are not
#[test]
fn a_build_file_s_top_level_call_is_a_declaration() {
  let text = "load(\"//defs:jvm.bzl\", \"jvm_library\")\n\njvm_library(\n    name = \"monolith\",\n    srcs = glob([\"*.kt\"]),  # sources\n)\nnative.cc_library(name = \"n\")\n";
  let build = |needle: &str| style_at(Grammar::StarlarkBuild, text, needle, 0);
  assert_eq!(
    [
      build("load"),
      build("\"//defs"),
      build("jvm_library("),
      build("name"),
      build("glob"),
      build("native"),
      build("cc_library"),
      build("# sources")
    ],
    [KEYWORD, Role::String.style(), DECLARATION, NAMED, 0, 0, DECLARATION, COMMENT]
  );
  // the same text as a `.bzl` file calls what it calls
  assert_eq!(style_at(Grammar::Starlark, text, "jvm_library(", 0), 0);
}

/// The plugin's keywords only: `class` and `while` are words the lexer reserves and the highlighter leaves plain
#[test]
fn the_keywords_are_the_plugin_s() {
  let text = "x = None if not y and True else lambda: False\nclass\n";
  assert_eq!(
    ["None", "if", "not", "and", "True", "else", "lambda", "False"].map(|it| style_at(Grammar::Starlark, text, it, 0)),
    [KEYWORD; 8]
  );
  assert_eq!(style_at(Grammar::Starlark, text, "class", 0), 0);
}

/// Bazel's files are named, not suffixed: a BUILD file has no extension, and a `.bazel` that is no BUILD or MODULE file
/// declares nothing by its calls
#[test]
fn bazel_files_are_starlark_by_their_names() {
  for path in ["a/BUILD", "a/BUILD.bazel", "MODULE.bazel"] {
    assert_eq!(grammar_of(path), Some(Grammar::StarlarkBuild), "{path}");
  }
  for path in ["WORKSPACE", "WORKSPACE.bzlmod", "REPO.bazel", "defs.bzl", "rules.star", "zlib.BUILD", "x.bazel"] {
    assert_eq!(grammar_of(path), Some(Grammar::Starlark), "{path}");
  }
  assert_eq!(grammar_of("Build.java"), Some(Grammar::Java));
}
