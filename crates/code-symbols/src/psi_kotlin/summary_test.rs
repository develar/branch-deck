//! The summaries of Kotlin files from their PSI, beside the golden files of `testdata/kotlin/` (`summary_test.rs`) and
//! the rules of `kotlin_test.rs`: each test here is a construct whose reading moved from tree-sitter's to the
//! compiler's, the reason in its name.

use crate::extract::summarize;
use crate::language::Language;
use crate::summary::{DeclKind, FileSummary};
use crate::test_support::{NONE, constructors_of, declaration, fixtures, refs};

fn summary(source: &str) -> FileSummary {
  summarize(Language::Kotlin, source.as_bytes())
}

#[test]
fn every_kotlin_fixture_is_read() {
  for (name, source) in fixtures("kotlin", "kt") {
    assert!(!summarize(Language::Kotlin, &source).truncated, "{name}");
  }
}

#[test]
fn the_target_of_an_assignment_is_a_member_of_its_receiver() {
  let summary = summary("fun f(e: Event) {\n  e.presentation.isEnabled = false\n  Settings.instance.limit = 1\n  rows[0].value += 2\n}\n");
  assert_eq!(refs(&summary, "isEnabled"), ["2 ?.isEnabled"]);
  assert_eq!(refs(&summary, "limit"), ["3 Settings.instance.limit"]);
  assert_eq!(refs(&summary, "value"), ["4 ?.value"]);
}

#[test]
fn this_in_a_string_template_is_no_name() {
  let summary = summary("fun String.f() = \"was: $this, ${this.length}\"\n");
  assert_eq!(refs(&summary, "this"), NONE);
  assert_eq!(refs(&summary, "length"), ["1 this.length"]);
}

#[test]
fn a_context_parameter_is_a_local() {
  let summary = summary("context(scope: CoroutineScope)\nfun f() = scope.launch {}\n");
  assert_eq!(refs(&summary, "scope"), NONE);
  assert_eq!(refs(&summary, "launch"), ["2 ?.launch(Lambda)"]);
  assert_eq!(refs(&summary, "CoroutineScope"), ["1 :CoroutineScope"]);
}

#[test]
fn a_member_of_a_callable_reference_is_one_of_the_reference() {
  let summary = summary("val a = Request::depth.name\nval b = Random()::nextInt\n");
  assert_eq!(refs(&summary, "name"), ["1 ?.name"]);
  assert_eq!(refs(&summary, "depth"), ["1 Request.depth"]);
  // the receiver of `::` is a call here, not a type
  assert_eq!(refs(&summary, "Random"), ["2 Random()"]);
  assert_eq!(refs(&summary, "nextInt"), ["2 ?.nextInt"]);
}

#[test]
fn a_name_before_colons_is_a_type() {
  let summary = summary("val a = Foo::class\nval b = String::length\nval c = Array<String>::class.java\n");
  assert_eq!(refs(&summary, "Foo"), ["1 :Foo"]);
  assert_eq!(refs(&summary, "String"), ["2 :String", "3 :String"]);
  assert_eq!(refs(&summary, "length"), ["2 String.length"]);
  assert_eq!(refs(&summary, "Array"), ["3 :Array"]);
  assert_eq!(refs(&summary, "java"), ["3 ?.java"]);
}

#[test]
fn a_call_with_type_arguments_is_a_call() {
  let summary = summary("fun f(x: Any) = x.asSafely<Foo>()?.let { arrayOfNulls<String>(3) }\n");
  assert_eq!(refs(&summary, "asSafely"), ["1 <Any>.asSafely()"]);
  assert_eq!(refs(&summary, "arrayOfNulls"), ["1 arrayOfNulls(Number){}"]);
}

#[test]
fn an_infix_call_between_comments_is_a_call() {
  let summary = summary("fun f() { for (i in 0 /* from */ until /* to */ 10) {} }\n");
  assert_eq!(refs(&summary, "until"), ["1 ?.until()"]);
}

#[test]
fn a_lambda_on_the_line_after_a_call_is_its_argument() {
  // the compiler's reading, which it then rejects for more than one lambda: `foo {}` and `{}` are one call
  let summary = summary("fun f() {\n  foo {}\n  {}\n  bar(null)\n  { baz() }\n}\n");
  assert_eq!(refs(&summary, "foo"), ["2 foo(Lambda,Lambda)"]);
  assert_eq!(refs(&summary, "bar"), ["4 bar(_,Lambda)"]);
  assert_eq!(refs(&summary, "baz"), ["5 baz(){}"]);
}

#[test]
fn the_variable_of_a_loop_without_braces_is_bound_in_the_statement_below() {
  let summary = summary("fun f(rows: List<Row>) {\n  for (row in rows)\n    put(row.key)\n  for ((i, width) in rows.withIndex())\n    set(i, width)\n}\n");
  assert_eq!(refs(&summary, "row"), NONE);
  assert_eq!(refs(&summary, "key"), ["3 ?.key"]);
  assert_eq!(refs(&summary, "i"), NONE);
  assert_eq!(refs(&summary, "width"), NONE);
}

#[test]
fn a_type_after_is_and_as_is_a_type() {
  let source = "fun Scope.f(x: Any) =\n  this == Scope.Ready ||\n    this is Scope.Selection\nfun g(xs: List<Any>) {\n  val item = xs[0] as Tree.Element<String>\n  (item)\n}\n";
  let summary = summary(source);
  assert_eq!(refs(&summary, "Selection"), ["3 :Scope.Selection"]);
  assert_eq!(refs(&summary, "Element"), ["5 :Tree.Element"]);
  assert_eq!(refs(&summary, "item"), NONE);
}

#[test]
fn an_accessor_sees_the_names_around_its_property_not_the_property() {
  // a local property (of an object expression) is visible after it, not in its own getter
  let source = "fun f(file: Path) {\n  val path = file\n  object : Paths {\n    override val path: Location\n      get() = path.location\n  }\n}\n";
  let summary = summary(source);
  assert_eq!(refs(&summary, "location"), ["5 ?.location"]);
}

#[test]
fn an_enum_class_without_a_body_has_no_constructors() {
  let summary = summary("enum class Category\nclass Plain\n");
  assert_eq!(constructors_of(&summary, "Category"), None);
  assert_eq!(constructors_of(&summary, "Plain"), Some(vec!["new()".to_string()]));
}

#[test]
fn a_modifier_keyword_as_a_name_stays_in_the_header() {
  let summary = summary("class A {\n  val external: Boolean? = null\n  fun String.external(): Int = 0\n}\n");
  let headers: Vec<&str> = summary.declarations.iter().filter_map(|it| it.header.as_deref()).collect();
  assert_eq!(headers, ["val external : Boolean ?", "fun String . external : Int"]);
}

#[test]
fn a_type_constraint_names_a_type() {
  let summary = summary("val <T> T.lifetime: Lifetime where T : Holder, T : Disposable by lazy { error(\"\") }\n");
  assert_eq!(refs(&summary, "T"), ["1 :T"]);
  assert_eq!(refs(&summary, "Holder"), ["1 :Holder"]);
}

#[test]
fn a_build_script_reads_as_a_script_and_a_file_with_context_receivers_as_a_file() {
  // a summary is asked without the file's name: the script parse wins only where the file parse fails at the top
  let script = summary("plugins {\n  id(\"java\")\n}\nval (major, minor) = 1 to 2\ndependencies { implementation(project(\":p\")) }\n");
  assert_eq!(script.error_bytes, 0);
  assert_eq!(refs(&script, "plugins"), ["1 plugins(Lambda)"]);
  assert_eq!(refs(&script, "implementation"), ["5 implementation(_){}"]);
  assert_eq!(declaration(&script, "major").kind, DeclKind::Value);
  let file = summary("context(c: Context)\nfun f() = c.run()\n");
  assert_eq!(file.error_bytes, 0);
  assert_eq!(declaration(&file, "f").kind, DeclKind::Function);
  assert_eq!(refs(&file, "context"), NONE);
}

#[test]
fn the_tokens_a_parse_error_skipped_are_no_references() {
  let summary = summary("class A {\n  fun first() = 1\n  @@@ stray tokens (((\n  fun second() = first()\n}\n");
  assert!(summary.error_bytes > 0);
  assert_eq!(refs(&summary, "stray"), NONE);
  assert!(summary.declarations.iter().all(|it| !it.in_error));
  assert_eq!(constructors_of(&summary, "A"), None);
}
