use crate::dependencies::{Dependency, compute_dependencies};
use crate::diff_scan::CommitDiff;
use object_cache::commit_diff::{FileChange, Hunk};
use pretty_assertions::assert_eq;

/// A text change of `path` with hunks `(old_start, old_lines, new_start, new_lines)`
fn text(status: char, path: &str, hunks: &[(u32, u32, u32, u32)]) -> FileChange {
  FileChange {
    status,
    path: path.to_string(),
    old_path: None,
    additions: 0,
    deletions: 0,
    binary: false,
    old_blob: None,
    new_blob: None,
    hunks: hunks
      .iter()
      .map(|&(old_start, old_lines, new_start, new_lines)| Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
      })
      .collect(),
  }
}

fn commits(changes: Vec<Vec<FileChange>>) -> Vec<CommitDiff> {
  changes.into_iter().enumerate().map(|(index, files)| CommitDiff { id: index.to_string(), files }).collect()
}

fn edge(from: usize, to: usize, files: &[&str]) -> Dependency {
  Dependency {
    from,
    to,
    files: files.iter().map(|file| (*file).to_string()).collect(),
  }
}

#[test]
fn overlapping_and_separate_hunks() {
  let diffs = commits(vec![
    // 0 inserts lines 11..=15 after line 10
    vec![text('M', "f", &[(10, 0, 11, 5)])],
    // 1 replaces line 13, one of them
    vec![text('M', "f", &[(13, 1, 13, 1)])],
    // 2 changes line 40, far from both
    vec![text('M', "f", &[(40, 1, 40, 1)])],
  ]);
  assert_eq!(compute_dependencies(&diffs), [edge(1, 0, &["f"])]);
}

#[test]
fn lines_move_with_earlier_insertions() {
  let diffs = commits(vec![
    // 0 inserts line 21 after line 20
    vec![text('M', "f", &[(20, 0, 21, 1)])],
    // 1 inserts 10 lines at the top: the line of 0 is now line 31
    vec![text('M', "f", &[(0, 0, 1, 10)])],
    // 2 deletes line 31
    vec![text('M', "f", &[(31, 1, 30, 0)])],
    // 3 changes line 21, which is a baseline line now
    vec![text('M', "f", &[(21, 1, 21, 1)])],
  ]);
  assert_eq!(compute_dependencies(&diffs), [edge(2, 0, &["f"])]);
}

#[test]
fn several_hunks_in_one_change() {
  let diffs = commits(vec![
    // 0 adds lines 11..=12 and, after old line 20, line 23
    vec![text('M', "f", &[(10, 0, 11, 2), (20, 0, 23, 1)])],
    // 1 inserts 3 lines after line 5 (the lines of 0 move to 14..=15 and 26) and replaces line 26
    vec![text('M', "f", &[(5, 0, 6, 3), (23, 1, 26, 1)])],
    // 2 changes line 14 (of 0) and line 27, next to the line of 1
    vec![text('M', "f", &[(14, 1, 14, 1), (27, 1, 27, 1)])],
  ]);
  assert_eq!(compute_dependencies(&diffs), [edge(1, 0, &["f"]), edge(2, 0, &["f"]), edge(2, 1, &["f"])]);
}

#[test]
fn adjacent_changes_depend() {
  let diffs = commits(vec![
    vec![text('M', "f", &[(4, 1, 4, 1)])],
    // replaces line 5, right after the line of 0
    vec![text('M', "f", &[(5, 1, 5, 1)])],
    // inserts after line 7: lines 7 and 8 are baseline lines
    vec![text('M', "f", &[(7, 0, 8, 1)])],
  ]);
  assert_eq!(compute_dependencies(&diffs), [edge(1, 0, &["f"])]);
}

#[test]
fn created_renamed_deleted_and_binary_files() {
  let mut renamed = text('R', "g", &[]);
  renamed.old_path = Some("f".to_string());
  let mut binary = text('M', "image", &[]);
  binary.binary = true;
  let diffs = commits(vec![
    vec![text('A', "f", &[(0, 0, 1, 3)]), text('M', "image", &[])],
    // a pure rename of the file 0 created
    vec![renamed],
    // a change of the renamed file far from the lines of 0 still needs the file
    vec![text('M', "g", &[(100, 0, 101, 1)])],
    vec![binary],
    vec![text('D', "g", &[(1, 4, 0, 0)])],
    vec![text('A', "g", &[(0, 0, 1, 1)])],
  ]);
  assert_eq!(
    compute_dependencies(&diffs),
    [
      edge(1, 0, &["g"]),
      edge(2, 0, &["g"]),
      edge(3, 0, &["image"]),
      edge(4, 0, &["g"]),
      edge(4, 2, &["g"]),
      edge(5, 4, &["g"]),
    ]
  );
}
