use crate::extract::{MAX_AVERAGE_LINE, MAX_BYTES, MAX_LINES, RULES_VERSION, extractor_version, spec_of, summarize, summarize_with_budget};
use crate::language::{Family, Language, language_of};
use crate::summary::{DeclKind, FileSummary, Role};
use crate::test_support::{check_golden, declaration, fixture, render, summary_of};

/// The value for the grammars and rules of this build. A change of `RULES_VERSION` or of a grammar version changes it:
/// replace it with the value the failure prints.
const EXTRACTOR_VERSION: u32 = 2_013_825_544;

#[test]
fn every_spec_kind_exists_in_its_grammar() {
  for language in Language::ALL {
    let Some(spec) = spec_of(language) else { continue };
    let grammar = (spec.grammar)();
    let (rule_kinds, rule_fields) = spec.rule_names();
    let kinds = spec
      .rules
      .iter()
      .map(|it| it.0)
      .chain(rule_kinds)
      .chain(spec.kinds.iter().copied())
      .chain(spec.binding_names.iter().copied());
    for kind in kinds {
      assert_ne!(grammar.id_for_node_kind(kind, true), 0, "{language:?}: no named node kind {kind}");
    }
    for token in spec.tokens {
      assert_ne!(grammar.id_for_node_kind(token, false), 0, "{language:?}: no token {token}");
    }
    for field in rule_fields.iter().chain(spec.fields) {
      assert!(grammar.field_id_for_name(field).is_some(), "{language:?}: no field {field}");
    }
  }
}

#[test]
fn every_fixture_matches_its_golden_file() {
  let testdata = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
  let mut checked = 0;
  for directory in std::fs::read_dir(&testdata).unwrap() {
    let directory = directory.unwrap().path();
    let mut files: Vec<String> = std::fs::read_dir(&directory)
      .unwrap()
      .map(|it| it.unwrap().file_name().to_string_lossy().into_owned())
      .collect();
    files.sort();
    for file in files.iter().filter(|it| !it.ends_with(".summary")) {
      let name = format!("{}/{file}", directory.file_name().unwrap().to_string_lossy());
      if language_of(&name).is_some() {
        check_golden(&name);
        checked += 1;
      }
    }
  }
  assert_ne!(checked, 0);
}

#[test]
fn no_kotlin_ng_in_the_lock_file() {
  // tree-sitter-kotlin-ng exports the same C symbol as tree-sitter-kotlin-sg: linking both picks one grammar at random
  let lock = include_str!("../../../Cargo.lock");
  assert!(lock.contains("name = \"tree-sitter-kotlin-sg\""));
  assert!(!lock.contains("name = \"tree-sitter-kotlin-ng\""));
}

#[test]
fn extractor_version_is_stable() {
  let version = extractor_version();
  assert_eq!(version, extractor_version());
  if cfg!(all(feature = "kotlin", feature = "java", feature = "rust", feature = "typescript")) {
    assert_eq!(
      version, EXTRACTOR_VERSION,
      "RULES_VERSION {RULES_VERSION} or a grammar changed: set EXTRACTOR_VERSION to {version}"
    );
  }
}

#[test]
#[cfg(all(feature = "kotlin", feature = "java", feature = "rust", feature = "typescript"))]
fn language_by_extension() {
  assert_eq!(language_of("a/b/Foo.kt"), Some(Language::Kotlin));
  assert_eq!(language_of("build.gradle.kts"), Some(Language::Kotlin));
  assert_eq!(language_of("src\\Main.java"), Some(Language::Java));
  assert_eq!(language_of("crates/x/src/lib.rs"), Some(Language::Rust));
  for path in ["a.ts", "a.tsx", "a.mts", "a.cts", "a.js", "a.jsx", "a.mjs", "a.cjs"] {
    assert_eq!(language_of(path), Some(Language::TypeScript), "{path}");
  }
  assert_eq!(language_of("README.md"), None);
  assert_eq!(language_of("Makefile"), None);
  assert_eq!(language_of("dir.kt/file"), None);
  assert_eq!(Language::Kotlin.family(), Family::Jvm);
  assert_eq!(Language::Java.family(), Family::Jvm);
  assert_eq!(Language::Rust.family(), Family::Rust);
  assert_eq!(Language::TypeScript.family(), Family::Script);
  let ids: Vec<u8> = Language::ALL.iter().map(|it| it.id()).collect();
  assert_eq!(ids, [0, 1, 2, 3]);
}

#[cfg(any(feature = "kotlin", feature = "rust", feature = "typescript"))]
fn assert_skipped(summary: &FileSummary, source: &[u8]) {
  assert!(summary.truncated);
  assert_eq!((summary.declarations.len(), summary.references.len(), summary.imports.len()), (0, 0, 0));
  assert_eq!(summary.total_bytes as usize, source.len());
}

#[test]
#[cfg(feature = "kotlin")]
fn an_oversized_file_is_not_parsed() {
  let line = "val x = 1\n";
  let source = line.repeat(MAX_BYTES / line.len() + 1);
  let summary = summarize(Language::Kotlin, source.as_bytes());
  assert_skipped(&summary, source.as_bytes());
}

#[test]
#[cfg(feature = "rust")]
fn a_file_of_too_many_lines_is_not_parsed() {
  let source = "\n".repeat(MAX_LINES as usize + 1);
  let summary = summarize(Language::Rust, source.as_bytes());
  assert_skipped(&summary, source.as_bytes());
  assert_eq!(summary.lines, MAX_LINES + 1);
}

#[test]
#[cfg(feature = "typescript")]
fn a_minified_file_is_not_parsed() {
  let statement = "var a=b(c);";
  let line = statement.repeat(MAX_AVERAGE_LINE as usize / statement.len() + 10);
  let source = format!("{line}\n{line}\n");
  let summary = summarize(Language::TypeScript, source.as_bytes());
  assert_skipped(&summary, source.as_bytes());
  // the same statements on their own lines are parsed
  let readable = statement.repeat(200).replace(';', ";\n");
  assert!(!summarize(Language::TypeScript, readable.as_bytes()).truncated);
}

#[test]
#[cfg(feature = "kotlin")]
fn the_parse_budget_truncates_deterministically() {
  let (_, source) = fixture("kotlin/references.kt");
  let full = summarize_with_budget(Language::Kotlin, &source, u32::MAX);
  assert!(!full.truncated);
  assert_ne!(full.references.len(), 0);
  let cut = summarize_with_budget(Language::Kotlin, &source, 1);
  assert!(cut.truncated);
  assert_eq!((cut.declarations.len(), cut.references.len()), (0, 0));
  assert_eq!(summarize_with_budget(Language::Kotlin, &source, 1), cut);
  // the parser starts afresh after a cancelled parse
  assert_eq!(summarize_with_budget(Language::Kotlin, &source, u32::MAX), full);
}

#[test]
#[cfg(feature = "kotlin")]
fn a_degraded_file_marks_what_it_misreads() {
  let summary = summary_of("kotlin/broken.kt");
  assert!(summary.error_bytes > 0);
  assert!(summary.is_degraded(), "{} of {} bytes in errors", summary.error_bytes, summary.total_bytes);
  assert!(!declaration(&summary, "ok").in_error);
  assert!(declaration(&summary, "call").in_error);
  let clean = summary_of("kotlin/declarations.kt");
  assert_eq!(clean.error_bytes, 0);
  assert!(!clean.is_degraded());
  assert!(!summarize(Language::Kotlin, b"").is_degraded());
}

#[test]
#[cfg(feature = "kotlin")]
fn deep_nesting_does_not_overflow_the_stack() {
  let depth = 9_000;
  let source = format!("fun f() =\n{}x{}\n", "(\n".repeat(depth), ")\n".repeat(depth));
  let summary = summarize(Language::Kotlin, source.as_bytes());
  assert!(!summary.truncated);
  assert_eq!(summary.references_to("x").count(), 1);
}

#[test]
#[cfg(feature = "kotlin")]
fn names_are_sorted_and_lookups_use_them() {
  let summary = summary_of("kotlin/references.kt");
  assert!(summary.names.windows(2).all(|it| it[0] < it[1]));
  assert!(summary.references.windows(2).all(|it| it[0] < it[1]), "sorted without duplicates");
  let id = summary.name_id("canvas").unwrap();
  assert_eq!(summary.name(id), "canvas");
  assert_eq!(summary.name_id("nothing"), None);
  assert_eq!(summary.name(u32::MAX), "");
  assert!(summary.references_to("canvas").all(|it| it.name == id));
  assert_eq!(summary.references_to("nothing").count(), 0);
  assert!(summary.declares("Renderer"));
  assert!(!summary.declares("canvas.draw"));
}

#[test]
#[cfg(feature = "typescript")]
fn the_same_bytes_give_the_same_summary() {
  let first = summary_of("typescript/references.tsx");
  let second = summary_of("typescript/references.tsx");
  assert_eq!(first, second);
  assert_eq!(render(&first), render(&second));
}

#[test]
#[cfg(feature = "rust")]
fn a_summary_survives_serialization() {
  let summary = summary_of("rust/use_binding.rs");
  let json = serde_json::to_string(&summary).unwrap();
  let back: FileSummary = serde_json::from_str(&json).unwrap();
  assert_eq!(back, summary);
}

#[test]
#[cfg(all(feature = "kotlin", feature = "java", feature = "rust", feature = "typescript"))]
fn every_language_gives_declarations_and_references() {
  for (name, declared, used) in [
    ("kotlin/declarations.kt", "Circle", "Shape"),
    ("java/Declarations.java", "Visitor", "Comparable"),
    ("rust/declarations.rs", "Circle", "Debug"),
    ("typescript/declarations.ts", "Circle", "Base"),
  ] {
    let summary = summary_of(name);
    assert!(!summary.truncated, "{name}");
    assert_eq!(declaration(&summary, declared).kind, DeclKind::Type, "{name}");
    assert!(summary.references_to(used).any(|it| it.role == Role::Type || it.role == Role::Value), "{name}");
  }
}
