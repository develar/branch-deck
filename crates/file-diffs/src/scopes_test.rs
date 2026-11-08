use crate::scopes::scopes_of;
use code_symbols::language::Language;
use pretty_assertions::assert_eq;
use text_diff::model::Scope;

fn scope(start_line: u32, end_line: u32, label: &str) -> Scope {
  Scope {
    start_line,
    end_line,
    label: label.to_string(),
  }
}

#[test]
fn a_member_is_labelled_with_its_containers() {
  let text = "package a\n\nclass CommitPatch {\n  fun readPatch() {\n    val x = 1\n  }\n}\n\nfun top() = 2\n";
  assert_eq!(
    scopes_of(Language::Kotlin, "CommitPatch.kt", text),
    [scope(2, 7, "CommitPatch"), scope(3, 6, "CommitPatch › readPatch"), scope(8, 9, "top")]
  );
}

#[test]
fn a_rust_impl_member_is_labelled_with_its_type() {
  let text = "struct Patch;\n\nimpl Patch {\n  fn read(&self) {}\n}\n";
  let labels: Vec<String> = scopes_of(Language::Rust, "patch.rs", text).into_iter().map(|scope| scope.label).collect();
  assert!(labels.contains(&"Patch › read".to_string()), "{labels:?}");
}

#[test]
fn a_bazel_target_is_a_scope_so_that_a_fold_inside_its_call_names_it() {
  let text = "jps_test(\n    name = \"monolith\",\n    deps = [\n        \":runtime\",\n    ],\n)\n";
  assert_eq!(scopes_of(Language::Starlark, "pkg/BUILD", text), [scope(0, 6, "monolith")]);
  // the same text in a `.bzl` declares no target, so its lines fold under no name
  assert_eq!(scopes_of(Language::Starlark, "pkg/defs.bzl", text), []);
}
