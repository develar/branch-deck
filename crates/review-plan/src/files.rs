//! The facts of each file of a review (spec/branch/review-plan.spec.md, "Files", "Mechanical changes", "Repeated
//! edits"): its links to the other changed files, whether its change is mechanical, the repeated edit it shares, the
//! usages of the API it removes or changes, and whether it changed without its tests.
//!
//! - **The files**: every path the reviewed commits change, once, at its last path: a renamed file at its new path with
//!   the path before the first commit, a deleted one at its path, none that the commits add and delete again.
//! - **Mechanical**, by name or by the text of every hunk of every commit that changes the file, as `ImportChangeFilter`
//!   of `CoRT` and `detectReformatting` of `SmartCommit` do: `Deleted` when the commits delete it, `BuildModel`
//!   for a file of the build model (`*.iml`, `BUILD`, `BUILD.bazel`, `*.lock`, `pnpm-lock.yaml`, `.idea/modules.xml`),
//!   else each change a pure rename (status `R`, no line added or removed), or each of its hunks the same text without
//!   its whitespace (`Whitespace`) or, in a source file, only blank lines and at least one `import`, `use` or `package`
//!   line (`ImportsOnly`). One import hunk makes the file `ImportsOnly`, else one whitespace hunk `Whitespace`. A file
//!   without text (binary, too large, an added one) is only mechanical by name or deletion.
//! - **Repeated edits**: the key of a hunk is its removed and its added lines without whitespace, blank lines left out;
//!   a hunk of whitespace alone has none, nor one without a line of 10 or more characters. A key of 3 or more files
//!   makes a group, the key of the most files first; a file is in one group at most, and a group keeps at least 3 files
//!   that no larger group took.
//! - **Tests**, by the path: a directory `test`, `tests`, `__tests__` or `testFixtures`; a name `*Test`, `*Tests`
//!   (Kotlin, Java), `*_test` (Rust), `*.test`, `*.spec` (TypeScript) or `test_*`. Its subject is the stem without that
//!   mark (`FooTest.kt` and `foo.test.ts` name `Foo` and `foo`), compared without case; a file of a test directory names
//!   the stem it has. A source file (`code_symbols::language::language_of`), no test, still there and not mechanical,
//!   is `untested` when no changed test names its stem.
//! - **The text of the hunks** comes from the blobs the diffs read (`FileDiffs::blob_texts`): from memory, else one
//!   `cat-file --batch`, only of the modified and the renamed files that are not mechanical by name.

use crate::api::{ApiChange, ApiChangeKind};
use crate::links::{FileLink, LinkState};
use anyhow::Result;
use code_symbols::language::language_of;
use file_diffs::model::BlobRequest;
use file_diffs::service::{FileDiffs, MAX_BLOB_REQUESTS, compare_tree_paths};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::commit_diff::{FileChange, Hunk};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use trial_check::diff_scan::CommitDiff;

/// Why a change of a file is mechanical: the review folds it into one group at the end
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum MechanicalKind {
  /// A file of the build model: `*.iml`, `BUILD`, `BUILD.bazel`, `*.lock`, `pnpm-lock.yaml`, `.idea/modules.xml`
  BuildModel,
  /// Only `import`, `use` and `package` lines and blank lines changed
  ImportsOnly,
  /// Only whitespace changed
  Whitespace,
  /// Moved, no line changed
  Renamed,
  /// Deleted
  Deleted,
}

/// Files that get the same edit: one hunk with the same removed and added lines, without whitespace
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RepeatedEditGroup {
  /// At their last paths, in tree order; 3 or more
  pub files: Vec<String>,
  /// The first lines the edit removes and adds (at most `LABEL_LINES` of each, trimmed, no blank one), for its label
  pub removed: Vec<String>,
  pub added: Vec<String>,
}

/// The facts of a changed file
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FileFacts {
  /// Its last path
  pub path: String,
  /// Its path before the first commit that changes it, when the commits rename it
  pub old_path: Option<String>,
  /// Why its change is mechanical; `None` for real work
  pub mechanical: Option<MechanicalKind>,
  /// The changed files it uses, and those that use it, after the commits (the links `Added` or `Kept`)
  pub uses: u32,
  pub used_by: u32,
  /// The links it uses that the commits added
  pub new_links: u32,
  /// The usages at the head of the API it removes or changes (`ApiChange::fan_in`)
  pub breaks: u32,
  /// A source file changed while none of its tests changed
  pub untested: bool,
  /// Its group of `ReviewPlan::repeated_edits`
  pub repeated_edit: Option<u32>,
}

/// A file the reviewed commits change, once, at its last path
pub(crate) struct ChangedFile<'a> {
  /// The path after the commits
  pub path: &'a str,
  /// The path before the first commit that changes it
  pub first_path: &'a str,
  /// Its blob before the first commit that changes it and after the last one; `None` where it is not, or is no file
  pub before: Option<&'a str>,
  pub after: Option<&'a str>,
  /// The last commit that changes it deletes it
  pub deleted: bool,
  /// Its changes, oldest first
  pub changes: Vec<&'a FileChange>,
}

/// The files `diffs` (oldest first) change, each once at its last path (none added and deleted again), in tree order
pub(crate) fn changed_files(diffs: &[CommitDiff]) -> Vec<ChangedFile<'_>> {
  // the last path -> the file, and whether it was there before the commits
  let mut files: BTreeMap<&str, (ChangedFile<'_>, bool)> = BTreeMap::new();
  for change in diffs.iter().flat_map(|diff| &diff.files) {
    let from = change.old_path.as_deref().filter(|old_path| *old_path != change.path).unwrap_or(&change.path);
    let (mut file, existed) = files.remove(from).unwrap_or_else(|| {
      let file = ChangedFile {
        path: &change.path,
        first_path: from,
        before: change.old_blob.as_deref(),
        after: None,
        deleted: false,
        changes: Vec::new(),
      };
      (file, change.status != 'A')
    });
    file.path = &change.path;
    file.deleted = change.status == 'D';
    file.after = if file.deleted { None } else { change.new_blob.as_deref() };
    file.changes.push(change);
    files.insert(&change.path, (file, existed));
  }
  let mut files: Vec<ChangedFile<'_>> = files.into_values().filter(|(file, existed)| *existed || !file.deleted).map(|(file, _)| file).collect();
  files.sort_by(|a, b| compare_tree_paths(a.path, b.path));
  files
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

/// The names of the files of the build model, beside `*.iml` and `*.lock`
const BUILD_MODEL_NAMES: [&str; 3] = ["BUILD", "BUILD.bazel", "pnpm-lock.yaml"];

/// Whether `path` is a file of the build model by its name
pub fn is_build_model(path: &str) -> bool {
  let name = path.rsplit('/').next().unwrap_or(path);
  let extension = name.rsplit_once('.').map(|(_, extension)| extension);
  extension.is_some_and(|extension| extension.eq_ignore_ascii_case("iml") || extension.eq_ignore_ascii_case("lock"))
    || BUILD_MODEL_NAMES.contains(&name)
    || path == ".idea/modules.xml"
    || path.ends_with("/.idea/modules.xml")
}

/// The changes whose hunks are read as text: a modification or a rename with lines, of a file not mechanical by name
fn has_hunk_text(file: &ChangedFile<'_>, change: &FileChange) -> bool {
  !file.deleted && !is_build_model(file.path) && matches!(change.status, 'M' | 'R') && !change.binary && !change.hunks.is_empty()
}

/// The texts of the blobs whose hunks the facts read (`has_hunk_text`), by id: from the memory of `file_diffs`, else
/// one `cat-file --batch` (`FileDiffs::texts_by_id`); none for a binary, a too large or a missing blob, nor past
/// `MAX_BLOB_REQUESTS` blobs
pub(crate) fn hunk_texts(git: &GitCommandExecutor, repository_path: &str, file_diffs: &FileDiffs, files: &[ChangedFile<'_>]) -> Result<HashMap<String, String>> {
  let mut seen: HashSet<&str> = HashSet::new();
  let requests: Vec<BlobRequest> = files
    .iter()
    .flat_map(|file| file.changes.iter().filter(|change| has_hunk_text(file, change)))
    .flat_map(|change| [change.old_blob.as_deref(), change.new_blob.as_deref()])
    .flatten()
    .filter(|id| seen.insert(id))
    .take(MAX_BLOB_REQUESTS)
    .map(|id| BlobRequest { id: id.to_string(), path: None })
    .collect();
  if requests.is_empty() {
    return Ok(HashMap::new());
  }
  file_diffs.texts_by_id(git, repository_path, &requests)
}

/// The lines of the hunks of one change: (removed, added) per hunk
type HunkLines<'t> = Vec<(Vec<&'t str>, Vec<&'t str>)>;

/// The removed and the added lines of each hunk of `change`; `None` without the text of a side
fn hunk_lines<'t>(change: &FileChange, texts: &'t HashMap<String, String>) -> Option<HunkLines<'t>> {
  let lines = |blob: Option<&str>| -> Option<Vec<&'t str>> { Some(texts.get(blob?)?.lines().collect()) };
  let (old, new) = (lines(change.old_blob.as_deref())?, lines(change.new_blob.as_deref())?);
  let slice = |lines: &[&'t str], start: u32, count: u32| -> Vec<&'t str> {
    if count == 0 {
      return Vec::new();
    }
    let start = start.saturating_sub(1) as usize;
    lines.iter().skip(start).take(count as usize).copied().collect()
  };
  Some(
    change
      .hunks
      .iter()
      .map(|hunk: &Hunk| (slice(&old, hunk.old_start, hunk.old_lines), slice(&new, hunk.new_start, hunk.new_lines)))
      .collect(),
  )
}

/// `lines` without any whitespace, blank lines left out
fn without_whitespace(lines: &[&str]) -> Vec<String> {
  lines
    .iter()
    .map(|line| line.chars().filter(|char| !char.is_whitespace()).collect::<String>())
    .filter(|line| !line.is_empty())
    .collect()
}

/// An `import`, `use` or `package` line (Kotlin, Java, Rust, TypeScript); a `pub use` re-exports, so it is no import
fn is_import(line: &str) -> bool {
  let line = line.trim_start();
  ["import ", "import{", "use ", "package "].iter().any(|keyword| line.starts_with(keyword))
}

/// What a hunk is, when mechanical
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum HunkKind {
  Whitespace,
  Imports,
}

/// The kind of a hunk of `removed` and `added` lines; `source`: of a file of a known language
fn hunk_kind(removed: &[&str], added: &[&str], source: bool) -> Option<HunkKind> {
  if without_whitespace(removed) == without_whitespace(added) {
    return Some(HunkKind::Whitespace);
  }
  let lines = || removed.iter().chain(added);
  let imports_only = lines().all(|line| line.trim().is_empty() || is_import(line)) && lines().any(|line| is_import(line));
  (source && imports_only).then_some(HunkKind::Imports)
}

/// Why the change of `file` is mechanical, `None` when it is not: by its deletion, its name, or the text of each hunk
pub(crate) fn mechanical_kind(file: &ChangedFile<'_>, texts: &HashMap<String, String>) -> Option<MechanicalKind> {
  if file.deleted {
    return Some(MechanicalKind::Deleted);
  }
  if is_build_model(file.path) {
    return Some(MechanicalKind::BuildModel);
  }
  let source = language_of(file.path).is_some();
  let mut kinds: HashSet<HunkKind> = HashSet::new();
  for change in &file.changes {
    if change.status == 'R' && change.additions == 0 && change.deletions == 0 {
      continue;
    }
    if !has_hunk_text(file, change) {
      return None;
    }
    for (removed, added) in hunk_lines(change, texts)? {
      kinds.insert(hunk_kind(&removed, &added, source)?);
    }
  }
  Some(if kinds.contains(&HunkKind::Imports) {
    MechanicalKind::ImportsOnly
  } else if kinds.contains(&HunkKind::Whitespace) {
    MechanicalKind::Whitespace
  } else {
    MechanicalKind::Renamed
  })
}

/// The fewest files of a repeated edit
const MIN_REPEATED_FILES: usize = 3;
/// A repeated edit needs a line of at least this many characters, so that a `}` alone makes no group
const MIN_REPEATED_LINE: usize = 10;
/// The lines of each side a group keeps for its label
const LABEL_LINES: usize = 3;

/// The key of the edit of one hunk: its removed and its added lines without whitespace
type EditKey = (Vec<String>, Vec<String>);

/// The key of the edit of a hunk, `None` for one of whitespace alone or without a long line
fn edit_key(removed: &[&str], added: &[&str]) -> Option<EditKey> {
  let key = (without_whitespace(removed), without_whitespace(added));
  let long = removed.iter().chain(added).any(|line| line.trim().chars().count() >= MIN_REPEATED_LINE);
  (key.0 != key.1 && long).then_some(key)
}

/// The files that make one edit, by their place in the files, and the label of its first hunk
struct Edit {
  files: BTreeSet<usize>,
  removed: Vec<String>,
  added: Vec<String>,
}

/// The first `LABEL_LINES` lines of `lines` that are not blank, trimmed
fn label_lines(lines: &[&str]) -> Vec<String> {
  lines
    .iter()
    .map(|line| line.trim())
    .filter(|line| !line.is_empty())
    .take(LABEL_LINES)
    .map(str::to_string)
    .collect()
}

/// The groups of files (by their place in `files`) that share an edit, the most files first, then by their first file;
/// each file in one group at most
pub(crate) fn repeated_edits(files: &[ChangedFile<'_>], texts: &HashMap<String, String>, mechanical: &[Option<MechanicalKind>]) -> Vec<(Vec<usize>, RepeatedEditGroup)> {
  let lines: Vec<Vec<HunkLines<'_>>> = files
    .iter()
    .zip(mechanical)
    .map(|(file, kind)| {
      let edited = !matches!(kind, Some(MechanicalKind::Deleted | MechanicalKind::BuildModel));
      let changes = file.changes.iter().filter(|change| edited && has_hunk_text(file, change));
      changes.filter_map(|change| hunk_lines(change, texts)).collect()
    })
    .collect();
  let mut edits: BTreeMap<EditKey, Edit> = BTreeMap::new();
  for (index, changes) in lines.iter().enumerate() {
    for (removed, added) in changes.iter().flatten() {
      let Some(key) = edit_key(removed, added) else { continue };
      let edit = edits.entry(key).or_insert_with(|| Edit {
        files: BTreeSet::new(),
        removed: label_lines(removed),
        added: label_lines(added),
      });
      edit.files.insert(index);
    }
  }
  let mut candidates: Vec<Edit> = edits.into_values().filter(|edit| edit.files.len() >= MIN_REPEATED_FILES).collect();
  candidates.sort_by_key(|edit| (Reverse(edit.files.len()), edit.files.first().copied()));
  let mut taken: HashSet<usize> = HashSet::new();
  let mut groups: Vec<(Vec<usize>, RepeatedEditGroup)> = Vec::new();
  for Edit { files: members, removed, added } in candidates {
    let left: Vec<usize> = members.into_iter().filter(|index| !taken.contains(index)).collect();
    if left.len() < MIN_REPEATED_FILES {
      continue;
    }
    taken.extend(&left);
    let group = RepeatedEditGroup {
      files: left.iter().map(|&index| files[index].path.to_string()).collect(),
      removed,
      added,
    };
    groups.push((left, group));
  }
  groups.sort_by_key(|(members, _)| (Reverse(members.len()), members.first().copied()));
  groups
}

/// The facts of `files` (`changed_files`) and the groups of their repeated edits: by `links` and the usages of `api`,
/// the most `breaks` first, then the `untested` ones, then in tree order
pub(crate) fn file_facts(files: &[ChangedFile<'_>], texts: &HashMap<String, String>, links: &[FileLink], api: &[ApiChange]) -> (Vec<FileFacts>, Vec<RepeatedEditGroup>) {
  let mechanical: Vec<Option<MechanicalKind>> = files.iter().map(|file| mechanical_kind(file, texts)).collect();
  let groups = repeated_edits(files, texts, &mechanical);
  let mut group_of: HashMap<usize, u32> = HashMap::new();
  for (group, (members, _)) in groups.iter().enumerate() {
    for &member in members {
      group_of.insert(member, u32::try_from(group).unwrap_or(u32::MAX));
    }
  }
  // a deleted test is a change of the tests of its subject too
  let tested: HashSet<String> = files.iter().filter_map(|file| test_subject(file.path)).collect();
  let count = |matches: &dyn Fn(&FileLink) -> bool| u32::try_from(links.iter().filter(|link| matches(link)).count()).unwrap_or(u32::MAX);
  let mut facts: Vec<FileFacts> = files
    .iter()
    .zip(mechanical)
    .enumerate()
    .map(|(index, (file, mechanical))| {
      let path = file.path;
      let breaks = api
        .iter()
        .filter(|change| change.file == path && change.kind != ApiChangeKind::Added)
        .filter_map(|change| change.fan_in.map(|fan_in| fan_in.usages))
        .sum();
      let source = language_of(path).is_some() && test_subject(path).is_none();
      FileFacts {
        path: path.to_string(),
        old_path: (file.first_path != path).then(|| file.first_path.to_string()),
        mechanical,
        uses: count(&|link| link.from == path && link.state != LinkState::Removed),
        used_by: count(&|link| link.to == path && link.state != LinkState::Removed),
        new_links: count(&|link| link.from == path && link.state == LinkState::Added),
        breaks,
        untested: mechanical.is_none() && source && !tested.contains(&subject_stem(path)),
        repeated_edit: group_of.get(&index).copied(),
      }
    })
    .collect();
  facts.sort_by(|a, b| b.breaks.cmp(&a.breaks).then(b.untested.cmp(&a.untested)));
  (facts, groups.into_iter().map(|(_, group)| group).collect())
}
