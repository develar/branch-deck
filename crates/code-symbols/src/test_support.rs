//! Helpers of the tests: fixtures in `testdata/<language>/`, their golden files beside them (`<fixture>.summary`, the
//! text of `summary_text::render`), and lookups by name.
//!
//! A golden file is the whole summary of its fixture, one line per import, declaration and line of references, so a
//! rule change shows as a diff. `CODE_SYMBOLS_UPDATE=1 cargo nextest run -p code-symbols` rewrites them; review the diff.

use crate::extract::{PARSE_BUDGET, summarize_with_budget};
use crate::language::{Language, language_of};
use crate::summary::{Declaration, FileSummary};
use crate::summary_text::{constructor_text, modality_text, path_text, reference_text, render, signature_text};
use std::path::PathBuf;

/// No references or declarations, for `assert_eq!` against a list
pub(crate) const NONE: [String; 0] = [];

pub(crate) fn fixture(name: &str) -> (String, Vec<u8>) {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata").join(name);
  let source = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
  (name.to_string(), source)
}

/// The fixtures of `testdata/<directory>/` with this extension, by name
pub(crate) fn fixtures(directory: &str, extension: &str) -> Vec<(String, Vec<u8>)> {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata").join(directory);
  let mut names: Vec<String> = std::fs::read_dir(&path)
    .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    .map(|it| it.unwrap().file_name().to_string_lossy().into_owned())
    .filter(|it| std::path::Path::new(it).extension().is_some_and(|it| it == extension))
    .collect();
  names.sort();
  names.iter().map(|it| fixture(&format!("{directory}/{it}"))).collect()
}

/// Asserts that the leaves of a psi-syntax tree tile its text, at the file's offsets
#[cfg(any(feature = "kotlin", feature = "java", feature = "csharp"))]
pub(crate) fn assert_leaves_tile<L: psi_builder::language::TreeLanguage>(tree: &psi_builder::Tree<L>) {
  let mut end = 0;
  for (_, range) in tree.leaves() {
    assert_eq!(range.start, end);
    end = range.end;
  }
  assert_eq!(end, tree.source().len());
}

pub(crate) fn summary_of(name: &str) -> FileSummary {
  let (path, source) = fixture(name);
  let language = language_of(&path).unwrap_or_else(|| panic!("no language for {path}"));
  summarize_with_budget(language, &path, &source, PARSE_BUDGET)
}

/// The summary of a text with no fixture file, as a file of the language's own name: `summarize` takes a path, which
/// only Starlark reads (a `BUILD` declares targets, a `.bzl` does not)
pub(crate) fn summarize(language: Language, source: &[u8]) -> FileSummary {
  summarize_with_budget(language, path_of(language), source, PARSE_BUDGET)
}

/// `summarize` within a parse budget, for the tests of a parse that runs out of it
pub(crate) fn summarize_in(language: Language, source: &[u8], budget: u32) -> FileSummary {
  summarize_with_budget(language, path_of(language), source, budget)
}

/// A file name of the language, which `language_of` answers with it
pub(crate) const fn path_of(language: Language) -> &'static str {
  match language {
    Language::Kotlin => "Test.kt",
    Language::Java => "Test.java",
    Language::Rust => "test.rs",
    Language::TypeScript => "test.ts",
    Language::CSharp => "Test.cs",
    Language::Starlark => "BUILD",
  }
}

/// Compares the rendering of a fixture's summary with its golden file
pub(crate) fn check_golden(name: &str) {
  let summary = summary_of(name);
  let actual = render(&summary);
  let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata").join(format!("{name}.summary"));
  if std::env::var_os("CODE_SYMBOLS_UPDATE").is_some() {
    std::fs::write(&golden, &actual).unwrap();
    return;
  }
  let expected = std::fs::read_to_string(&golden).unwrap_or_default();
  pretty_assertions::assert_eq!(expected, actual, "{name}: rerun with CODE_SYMBOLS_UPDATE=1 to rewrite the golden file");
}

/// The references to a name as `line text`
pub(crate) fn refs(summary: &FileSummary, name: &str) -> Vec<String> {
  summary.references_to(name).map(|it| format!("{} {}", it.line, reference_text(summary, it))).collect()
}

/// The bare references to a name: locals must have none (a member of the same name is qualified)
pub(crate) fn bare_refs(summary: &FileSummary, name: &str) -> Vec<String> {
  summary
    .references_to(name)
    .filter(|it| it.qualifier.is_none())
    .map(|it| format!("{} {}", it.line, reference_text(summary, it)))
    .collect()
}

pub(crate) fn lines_of(summary: &FileSummary, name: &str) -> Vec<u32> {
  summary.references_to(name).map(|it| it.line).collect()
}

pub(crate) fn declaration<'s>(summary: &'s FileSummary, name: &str) -> &'s Declaration {
  let id = summary.name_id(name).unwrap_or_else(|| panic!("no name {name}"));
  summary.declarations.iter().find(|it| it.name == id).unwrap_or_else(|| panic!("no declaration of {name}"))
}

/// The declarations of a name as `Kind name in container`
pub(crate) fn declarations_of(summary: &FileSummary, name: &str) -> Vec<String> {
  of_name(summary, name, |declaration, container| format!("{:?} {name} in {container}", declaration.kind))
}

/// The declarations of a name as `container (parameters)`, `container -` without a signature
pub(crate) fn signatures(summary: &FileSummary, name: &str) -> Vec<String> {
  of_name(summary, name, |declaration, container| {
    let signature = declaration.signature.as_ref().map_or_else(|| "-".to_string(), |it| signature_text(summary, it));
    format!("{container} {signature}")
  })
}

/// The declarations of a name as `container overrides overridable jvm_static`, without the words that don't hold
pub(crate) fn modalities(summary: &FileSummary, name: &str) -> Vec<String> {
  of_name(summary, name, |declaration, container| format!("{container}{}", modality_text(declaration)))
}

/// The constructors of the type `name` as `new(parameters)` (`constructor_text`); `None` when it has no constructors
pub(crate) fn constructors_of(summary: &FileSummary, name: &str) -> Option<Vec<String>> {
  let constructors = declaration(summary, name).constructors.as_ref()?;
  Some(constructors.iter().map(|it| constructor_text(summary, it)).collect())
}

/// The supertypes of the type `name` as dotted paths
pub(crate) fn supertypes_of(summary: &FileSummary, name: &str) -> Vec<String> {
  declaration(summary, name).supertypes.iter().map(|it| path_text(summary, it)).collect()
}

/// The text of each declaration of a name, given the name of its container (`-` at the top level)
fn of_name(summary: &FileSummary, name: &str, text: impl Fn(&Declaration, &str) -> String) -> Vec<String> {
  let Some(id) = summary.name_id(name) else { return Vec::new() };
  summary
    .declarations
    .iter()
    .filter(|it| it.name == id)
    .map(|it| {
      let container = it.container.and_then(|it| summary.declarations.get(it as usize)).map_or("-", |it| summary.name(it.name));
      text(it, container)
    })
    .collect()
}

/// The style of the text at the `occurrence`-th `needle` of the text (counted from 0) in the highlighting of `grammar`,
/// by the runs of its line
pub(crate) fn style_at(grammar: crate::highlight::Grammar, text: &str, needle: &str, occurrence: usize) -> u32 {
  let place = place(text, needle, occurrence);
  let runs = crate::highlight::highlight(grammar, text).unwrap();
  runs
    .line(place.line as usize)
    .chunks(2)
    .take_while(|pair| pair[0] <= place.column)
    .last()
    .map_or(0, |pair| pair[1])
}

/// The place of the `occurrence`-th `needle` of the text, counted from 0, its column in UTF-16 units
pub(crate) fn place(text: &str, needle: &str, occurrence: usize) -> crate::navigate::Position {
  let offset = text.match_indices(needle).nth(occurrence).unwrap_or_else(|| panic!("no {needle}")).0;
  let line_start = text[..offset].rfind('\n').map_or(0, |it| it + 1);
  crate::navigate::Position {
    line: u32::try_from(text[..offset].matches('\n').count()).unwrap(),
    column: u32::try_from(text[line_start..offset].encode_utf16().count()).unwrap(),
  }
}

/// The name at the `occurrence`-th `needle` of the text and what its syntax says
pub(crate) fn at(grammar: crate::highlight::Grammar, text: &str, needle: &str, occurrence: usize) -> crate::navigate::ReferenceAt {
  crate::navigate::reference_at(grammar, text, place(text, needle, occurrence)).unwrap_or_else(|| panic!("no name at {needle}"))
}

/// The kind of the name at a needle and the line and column of its declaration
pub(crate) fn kind_and_local(grammar: crate::highlight::Grammar, text: &str, needle: &str, occurrence: usize) -> (crate::navigate::ReferenceKind, Option<(u32, u32)>) {
  let reference = at(grammar, text, needle, occurrence);
  (reference.kind, reference.local.map(|it| (it.line, it.column)))
}

/// Whether the usages of the name at a needle are of a local, and their lines and accesses
pub(crate) fn usages(grammar: crate::highlight::Grammar, text: &str, needle: &str, occurrence: usize) -> (bool, Vec<(u32, crate::navigate::Access)>) {
  let found = crate::navigate::usages_at(grammar, text, place(text, needle, occurrence)).unwrap();
  (found.local, found.usages.iter().map(|it| (it.start.line, it.access)).collect())
}

/// The references of a line as `text#container`, sorted
pub(crate) fn line_references(summary: &FileSummary, line: u32) -> Vec<String> {
  let mut texts: Vec<String> = summary
    .references
    .iter()
    .filter(|it| it.line == line)
    .map(|it| {
      let container = it.container.and_then(|id| summary.declarations.get(id as usize)).map_or("-", |it| summary.name(it.name));
      format!("{}#{container}", reference_text(summary, it))
    })
    .collect();
  texts.sort();
  texts
}
