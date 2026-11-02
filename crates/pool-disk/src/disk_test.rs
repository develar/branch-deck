use crate::{build_dirs, walk_bytes};
use std::path::Path;

#[test]
fn the_build_dirs_of_a_slot_are_the_outermost_dirs_its_clean_rules_name() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(
    dir.path().join(".branchdeck.toml"),
    r#"[lease]
clean = [
  { cargo = "target", keep = 8, age = "1h" },
  { delete = "out", except = ["dev-run"] },
  { delete = "out/dev-run", except = ["jar-cache"] },
  { delete = "tests/results" },
]
"#,
  )
  .unwrap();
  assert_eq!(build_dirs(dir.path()), ["out", "target", "tests/results"]);
  assert_eq!(build_dirs(&dir.path().join("none")), Vec::<String>::new());
}

#[test]
fn a_walk_counts_each_root_side_by_side_and_a_hard_linked_file_once() {
  let dir = tempfile::tempdir().unwrap();
  let roots: Vec<_> = (0..3).map(|n| dir.path().join(n.to_string())).collect();
  for (n, root) in roots.iter().enumerate() {
    for child in 0..300 {
      let child = root.join(format!("{}/{child}", child % 7));
      std::fs::create_dir_all(&child).unwrap();
      std::fs::write(child.join("file"), vec![1u8; 4096 * (n + 1)]).unwrap();
    }
  }
  let all: Vec<&Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
  let alone = walk_bytes(&all);
  // cargo links the same file into `deps/` and the profile dir
  #[cfg(unix)]
  std::fs::hard_link(roots[0].join("0/0/file"), roots[0].join("0/linked")).unwrap();
  let linked = walk_bytes(&all);
  assert_eq!(alone, linked);
  assert!(alone[0] >= 300 * 4096 && alone[1] >= 300 * 2 * 4096 && alone[2] >= 300 * 3 * 4096, "{alone:?}");
  assert_eq!(walk_bytes(&[dir.path().join("gone").as_path()]), [0]);
}
