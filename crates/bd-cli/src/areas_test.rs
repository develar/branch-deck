use crate::areas::{area_of, top_areas};
use crate::generated::is_hub_file;
use object_cache::commit_diff::FileChange;
use pretty_assertions::assert_eq;
use serde_json::json;

fn change(path: &str, additions: u32, deletions: u32) -> FileChange {
  FileChange {
    status: 'M',
    path: path.to_string(),
    old_path: None,
    additions,
    deletions,
    binary: false,
    hunks: Vec::new(),
  }
}

#[test]
fn an_area_keeps_at_most_depth_directories() {
  assert_eq!(area_of("plugins/air/shared/src/com/Foo.kt", 3), "plugins/air/shared");
  assert_eq!(area_of("plugins/air/shared/src/com/Foo.kt", 1), "plugins");
  assert_eq!(area_of("plugins/air/Foo.kt", 3), "plugins/air");
  assert_eq!(area_of("docs/a.md", 3), "docs");
  assert_eq!(area_of("Cargo.toml", 3), ".");
}

#[test]
fn hub_files_match_by_name() {
  for path in [
    ".idea/modules.xml",
    "community/.idea/modules.xml",
    "MODULE.bazel",
    "lib/MODULE.bazel",
    "a/b/x-file-list.txt",
    "README.md",
    "docs/CHANGELOG",
  ] {
    assert!(is_hub_file(path), "{path}");
  }
  for path in [
    "modules.xml",
    "x.idea/modules.xml",
    "MODULE.bazel.lock",
    "file-list.txt.bak",
    "docs/readme.md",
    "src/lib.rs",
  ] {
    assert!(!is_hub_file(path), "{path}");
  }
}

#[test]
fn top_areas_by_lines_without_hub_files() {
  let mut binary = change("icons/a.png", 0, 0);
  binary.binary = true;
  let changes = [
    change("src/api/a.rs", 5, 1),
    change("src/api/b.rs", 2, 0),
    change("src/ui/c.rs", 10, 0),
    change("README.md", 100, 0),
    change(".idea/modules.xml", 50, 0),
    change("build.rs", 1, 0),
    binary,
  ];
  let areas = serde_json::to_value(top_areas(&changes, 2)).unwrap();
  assert_eq!(
    areas,
    json!([{ "dir": "src/ui", "lines": 10 }, { "dir": "src/api", "lines": 8 }, { "dir": ".", "lines": 1 }])
  );
  // an area under 5 % of the lines is left out, unless it is the first
  let peripheral = [change("src/api/a.rs", 99, 0), change("docs/a.md", 4, 0), change("tests/a.rs", 6, 0)];
  let areas = serde_json::to_value(top_areas(&peripheral, 3)).unwrap();
  assert_eq!(areas, json!([{ "dir": "src/api", "lines": 99 }, { "dir": "tests", "lines": 6 }]));
  // an area of binary files only has 0 lines, which count only without other lines
  let areas = serde_json::to_value(top_areas(&changes[5..], 3)).unwrap();
  assert_eq!(areas, json!([{ "dir": ".", "lines": 1 }]));
  let areas = serde_json::to_value(top_areas(&changes[6..], 3)).unwrap();
  assert_eq!(areas, json!([{ "dir": "icons", "lines": 0 }]));
}
