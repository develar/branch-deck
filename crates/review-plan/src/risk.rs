//! The risk of each file of a review (spec/branch/review-plan.spec.md, "Risk"): the highest fan-in of the API it
//! removes or changes (`fan_in`), joined by the path of its file after the commits (`ApiChange::file`, not the old
//! path a removal names), and whether it is a source file the reviewed commits change while they change none of
//! its tests.
//!
//! A test, by its path: a directory `test`, `tests`, `__tests__` or `testFixtures`; a name `*Test`, `*Tests` (Kotlin,
//! Java), `*_test` (Rust), `*.test`, `*.spec` (TypeScript) or `test_*`. Its subject is the stem without that
//! mark (`FooTest.kt` and `foo.test.ts` name `Foo` and `foo`), compared without case; a file of a test directory names
//! the stem it has. A source is a file of a language the summaries read (`code_symbols::language::language_of`) that
//! is no test and is still there after the commits; it is `untested` when no changed test names its stem.

use crate::api::{ApiChange, ApiChangeKind};
use code_symbols::language::language_of;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use trial_check::diff_scan::CommitDiff;

/// The risk of a changed file
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FileRisk {
  pub path: String,
  /// The most usages of a declaration it removes or changes; `None` when it has none with a fan-in
  pub max_fan_in: Option<u32>,
  /// A source file changed without a change of any of its tests
  pub untested: bool,
  /// The card raises it: an API with usages removed or changed, or `untested`
  pub risky: bool,
}

/// The directories of tests
const TEST_DIRECTORIES: [&str; 4] = ["test", "tests", "__tests__", "testFixtures"];
/// The marks of a test at the end of a file's stem
const TEST_SUFFIXES: [&str; 5] = ["Tests", "Test", "_test", ".test", ".spec"];

/// The stem of the file of `path`: its name without the last extension; of a `mod.rs`, `lib.rs`, `main.rs` or
/// `index.ts`, the name of its directory
fn stem_of(path: &str) -> &str {
  let (directory, name) = path.rsplit_once('/').unwrap_or(("", path));
  let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
  if matches!(stem, "mod" | "lib" | "main" | "index") && !directory.is_empty() {
    return directory.rsplit('/').next().unwrap_or(directory);
  }
  stem
}

/// The stem of the subject a test file names, lowercase; `None` when `path` is no test
pub fn test_subject(path: &str) -> Option<String> {
  let stem = stem_of(path);
  let marked = TEST_SUFFIXES
    .iter()
    .find_map(|suffix| stem.strip_suffix(suffix).filter(|subject| !subject.is_empty()))
    .or_else(|| stem.strip_prefix("test_").filter(|subject| !subject.is_empty()));
  let in_test_directory = path.split('/').rev().skip(1).any(|directory| TEST_DIRECTORIES.contains(&directory));
  marked.or_else(|| in_test_directory.then_some(stem)).map(str::to_lowercase)
}

/// The stem a test of the source file `path` names, lowercase
pub fn subject_stem(path: &str) -> String {
  stem_of(path).to_lowercase()
}

/// The risk of every file `diffs` change and keep, at its last path (a file they delete is not listed, a renamed one is
/// listed at its new path), the riskiest first: the `risky` ones, then the most usages, then the untested ones, then
/// by path
pub fn file_risks(diffs: &[CommitDiff], api: &[ApiChange]) -> Vec<FileRisk> {
  // path -> still there after the commits
  let mut files: BTreeMap<&str, bool> = BTreeMap::new();
  for change in diffs.iter().flat_map(|diff| &diff.files) {
    if let Some(old_path) = change.old_path.as_deref().filter(|old_path| *old_path != change.path) {
      files.remove(old_path);
    }
    files.insert(&change.path, change.status != 'D');
  }
  // a deleted test is a change of the tests of its subject too
  let tested: HashSet<String> = files.keys().filter_map(|path| test_subject(path)).collect();
  let mut risks: Vec<FileRisk> = files
    .into_iter()
    .filter(|(_, kept)| *kept)
    .map(|(path, _)| {
      let max_fan_in = api
        .iter()
        .filter(|change| change.file == path && change.kind != ApiChangeKind::Added)
        .filter_map(|change| change.fan_in.map(|fan_in| fan_in.usages))
        .max();
      let untested = language_of(path).is_some() && test_subject(path).is_none() && !tested.contains(&subject_stem(path));
      FileRisk {
        path: path.to_string(),
        max_fan_in,
        untested,
        risky: untested || max_fan_in.is_some_and(|usages| usages > 0),
      }
    })
    .collect();
  risks.sort_by(|a, b| {
    b.risky
      .cmp(&a.risky)
      .then(b.max_fan_in.cmp(&a.max_fan_in))
      .then(b.untested.cmp(&a.untested))
      .then_with(|| a.path.cmp(&b.path))
  });
  risks
}
