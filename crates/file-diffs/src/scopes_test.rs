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
    scopes_of(Language::Kotlin, text),
    [scope(2, 7, "CommitPatch"), scope(3, 6, "CommitPatch › readPatch"), scope(8, 9, "top")]
  );
}

#[test]
fn a_rust_impl_member_is_labelled_with_its_type() {
  let text = "struct Patch;\n\nimpl Patch {\n  fn read(&self) {}\n}\n";
  let labels: Vec<String> = scopes_of(Language::Rust, text).into_iter().map(|scope| scope.label).collect();
  assert!(labels.contains(&"Patch › read".to_string()), "{labels:?}");
}
