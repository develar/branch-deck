use crate::landing::Subject;
use crate::repository_view::RobotOf;
use serde_json::json;
use std::path::{Path, PathBuf};

#[test]
fn a_subject_serializes_as_its_kind_and_name() {
  let branch = Subject::Branch { name: "ui".to_string() };
  let stack = Subject::WholeStack { root: "auth".to_string() };
  assert_eq!(serde_json::to_value(&branch).unwrap(), json!({ "kind": "branch", "name": "ui" }));
  assert_eq!(serde_json::to_value(&stack).unwrap(), json!({ "kind": "wholeStack", "root": "auth" }));
  // and back: the page may send a subject it was given
  assert_eq!(serde_json::from_value::<Subject>(json!({ "kind": "branch", "name": "ui" })).unwrap(), branch);
  assert_eq!(serde_json::from_value::<Subject>(json!({ "kind": "wholeStack", "root": "auth" })).unwrap(), stack);
  assert_eq!(branch.name(), "ui");
  assert_eq!(stack.name(), "auth");
  assert!(!branch.is_whole_stack());
  assert!(stack.is_whole_stack());
  // the one rule of "its own robot": the branch itself, never the whole stack of it, never another branch
  assert!(branch.is_branch("ui"));
  assert!(!branch.is_branch("auth"));
  assert!(!stack.is_branch("auth"));
}

#[test]
fn a_subject_names_itself_in_words() {
  assert_eq!(Subject::Branch { name: "auth".to_string() }.words(), "auth");
  assert_eq!(Subject::WholeStack { root: "auth".to_string() }.words(), "the whole auth stack");
}

#[test]
fn the_robot_of_a_subject_names_its_branch_and_whether_it_is_the_whole_stack() {
  let of_branch = RobotOf {
    branch: "ui".to_string(),
    robot_id: "r1".to_string(),
    whole_stack: false,
  };
  let of_stack = RobotOf {
    branch: "auth".to_string(),
    robot_id: "r1".to_string(),
    whole_stack: true,
  };
  assert_eq!(Subject::Branch { name: "ui".to_string() }.robot_of("r1"), of_branch);
  assert_eq!(Subject::WholeStack { root: "auth".to_string() }.robot_of("r1"), of_stack);
  // and back from the wire form, the flag read as the enum
  assert_eq!(of_branch.subject(), Subject::Branch { name: "ui".to_string() });
  assert_eq!(of_stack.subject(), Subject::WholeStack { root: "auth".to_string() });
}

/// Every `.rs` file under `dir`, test files included
fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
  for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
    let path = entry.path();
    if path.is_dir() {
      rust_files(&path, files);
    } else if path.extension().is_some_and(|extension| extension == "rs") {
      files.push(path);
    }
  }
}

#[test]
fn the_words_of_a_whole_stack_come_only_from_subject_words() {
  // one wording of "the whole auth stack" in bd and the app: a sentence embeds `Subject::words`, never the format
  let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
  let mut files = Vec::new();
  rust_files(&workspace.join("crates"), &mut files);
  rust_files(&workspace.join("src-tauri/src"), &mut files);
  assert!(files.len() > 20, "the scan found too few sources: {}", files.len());
  // the needle in two halves, so that this file is not a hit
  let needle = ["the whole ", "{"].concat();
  let formats: Vec<String> = files
    .iter()
    .filter(|file| std::fs::read_to_string(file).unwrap().contains(&needle))
    .map(|file| file.strip_prefix(&workspace).unwrap().to_string_lossy().replace('\\', "/"))
    .collect();
  assert_eq!(
    formats,
    ["crates/domain-types/src/landing.rs"],
    "a sentence formats the whole stack itself: use Subject::words"
  );
}
