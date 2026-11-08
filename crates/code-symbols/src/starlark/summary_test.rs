//! The summary of a Starlark file, beside the golden files of `testdata/starlark/` (`summary_test.rs`): each test here
//! is a rule of the Bazel plugin the summary follows, or a line it must not read as a declaration.

use crate::extract::summarize;
use crate::language::Language;
use crate::summary::{DeclKind, FileSummary, ImportTarget, Reach, Role};
use crate::summary_text::path_text;
use crate::test_support::{NONE, declaration, refs, summary_of};

fn starlark(path: &str, source: &str) -> FileSummary {
  summarize(Language::Starlark, path, source.as_bytes())
}

/// The declarations of a summary as `Kind name`, in source order
fn declarations(summary: &FileSummary) -> Vec<String> {
  summary.declarations.iter().map(|it| format!("{:?} {}", it.kind, summary.name(it.name))).collect()
}

#[test]
fn a_top_level_call_with_a_literal_name_is_a_target_of_its_rule() {
  let summary = summary_of("starlark/BUILD");
  let targets: Vec<String> = summary
    .declarations
    .iter()
    .filter(|it| it.kind == DeclKind::Target)
    .map(|it| {
      format!(
        "{} <{}>",
        summary.name(it.name),
        it.value_type.as_ref().map_or_else(String::new, |rule| path_text(&summary, rule))
      )
    })
    .collect();
  assert_eq!(targets, ["monolith <jps_test>", "runtime <kt>", "native_bridge <native.cc_library>"]);
  let monolith = declaration(&summary, "monolith");
  assert_eq!((monolith.start_line, monolith.end_line, monolith.name_line, monolith.name_column), (6, 11, 7, 12));
}

#[test]
fn a_name_that_is_no_literal_or_is_empty_declares_no_target() {
  // `helper(name = "")` is empty, `exports_files([…])` has no `name`, `alias` computes one, `nested` is in a `def`
  let summary = starlark(
    "pkg/BUILD",
    "helper(name = \"\")\nexports_files([\"a.txt\"])\nalias(name = \"gen_\" + SUFFIX)\ndef macro():\n    nested(name = \"inside\")\n",
  );
  assert_eq!(summary.declarations.iter().filter(|it| it.kind == DeclKind::Target).count(), 0);
}

#[test]
fn only_a_build_file_declares_targets_from_its_top_level_calls() {
  let call = "cc_library(name = \"x\")\n";
  for path in ["pkg/BUILD", "pkg/BUILD.bazel", "MODULE.bazel"] {
    assert_eq!(declarations(&starlark(path, call)), ["Target x"], "{path}");
  }
  for path in ["defs.bzl", "WORKSPACE", "third_party/zlib.BUILD"] {
    assert_eq!(declarations(&starlark(path, call)), NONE, "{path}");
  }
}

#[test]
fn a_bzl_file_declares_its_top_level_defs_and_assignments() {
  let summary = summary_of("starlark/defs.bzl");
  assert_eq!(
    declarations(&summary),
    [
      "Constant MAX_HEAP",
      "Constant _PRIVATE_TAGS",
      "Value default_jvm_flags",
      "Value extra_flags",
      "Function jps_test",
      "Function _tagged",
    ]
  );
  // Bazel loads no name that starts with `_`
  assert_eq!(declaration(&summary, "_tagged").reach, Reach::File);
  assert_eq!(declaration(&summary, "jps_test").reach, Reach::Public);
  // a name assigned in a `def` or by a `for` is bound, not declared
  assert!(!summary.declares("tags"));
  assert!(!summary.declares("tag"));
}

#[test]
fn a_def_has_the_shape_of_its_parameters() {
  let summary = summary_of("starlark/defs.bzl");
  let signature = declaration(&summary, "jps_test").signature.as_ref().unwrap();
  let parameters: Vec<String> = signature
    .parameters
    .iter()
    .map(|it| format!("{}{}{}", summary.name(it.name), if it.optional { "?" } else { "" }, if it.vararg { "*" } else { "" }))
    .collect();
  assert_eq!(parameters, ["name", "srcs?", "deps?", "extra*", "kwargs*"]);
}

#[test]
fn a_load_is_an_import_per_name_it_binds_under_the_label_it_names() {
  let summary = summary_of("starlark/BUILD");
  let imports: Vec<String> = summary
    .imports
    .iter()
    .map(|it| {
      let name = match it.target {
        ImportTarget::Name(name) => summary.name(name),
        _ => "*",
      };
      let alias = it.alias.map_or_else(String::new, |alias| format!(" as {}", summary.name(alias)));
      format!("{}{name}{alias} @{}", it.specifier.as_deref().unwrap_or(""), it.line)
    })
    .collect();
  assert_eq!(imports, ["//defs:jvm.bzljps_test @1", "//defs:jvm.bzlkotlin_library as kt @1", ":local.bzlhelper @2"]);
  let lines: Vec<(u32, u32)> = summary.import_lines.iter().map(|it| (it.start, it.end)).collect();
  assert_eq!(lines, [(1, 1), (2, 2)]);
  // the loaded name is an `Import` reference of its own line, which a use of it resolves through
  assert_eq!(refs(&summary, "kotlin_library"), ["1 ^kotlin_library"]);
}

#[test]
fn a_call_is_a_reference_of_the_rule_it_names_and_a_label_in_a_string_is_none() {
  let summary = summary_of("starlark/BUILD");
  assert_eq!(refs(&summary, "jps_test"), ["1 ^jps_test", "6 jps_test(name=Text,srcs=_,deps=_,visibility=_)"]);
  assert_eq!(refs(&summary, "cc_library"), ["19 native.cc_library(name=Text,srcs=_)"]);
  assert_eq!(refs(&summary, "PACKAGE_VISIBILITY"), ["10 PACKAGE_VISIBILITY"]);
  // `":runtime"` and `"//visibility:public"` are labels in strings: the summary reads no reference in a string
  assert_eq!(refs(&summary, "runtime"), NONE);
  assert_eq!(refs(&summary, "visibility"), NONE);
}

#[test]
fn a_name_bound_in_a_def_is_no_reference_of_the_file() {
  let summary = summary_of("starlark/defs.bzl");
  // `name`, `srcs`, `deps`, `tags` and `tag` are its parameters and locals; `java_library` and `extra_flags` are not
  for bound in ["name", "srcs", "deps", "tags", "tag"] {
    assert_eq!(refs(&summary, bound), NONE, "{bound}");
  }
  // the loaded name is an import of the `load` line and a call of its own line
  assert_eq!(refs(&summary, "java_library"), ["1 ^java_library", "9 java_library(name=_,srcs=_,deps=_,tags=_)*"]);
  assert_eq!(refs(&summary, "extra_flags"), ["12 extra_flags"]);
  // a reference inside a `def` is of it
  let container = summary.references_to("java_library").find(|it| it.role == Role::Call).unwrap().container.unwrap();
  assert_eq!(summary.name(summary.declarations[container as usize].name), "jps_test");
}

#[test]
fn a_file_with_a_syntax_error_keeps_the_declarations_outside_it() {
  let summary = starlark("pkg/BUILD", "cc_library(name = \"x\",\n\ndef helper():\n    pass\n");
  assert!(summary.error_bytes > 0, "{} bytes in errors", summary.error_bytes);
  assert!(summary.declares("helper"));
  assert!(!summary.truncated);
}

#[test]
fn a_reference_in_a_build_file_is_a_call_or_a_value() {
  let summary = starlark("pkg/BUILD", "cc_library(name = \"x\", deps = SHARED + other.deps)\n");
  let roles: Vec<(String, Role)> = summary.references.iter().map(|it| (summary.name(it.name).to_string(), it.role)).collect();
  assert_eq!(
    roles,
    [
      ("SHARED".to_string(), Role::Value),
      ("cc_library".to_string(), Role::Call),
      ("deps".to_string(), Role::Value)
    ]
  );
}
