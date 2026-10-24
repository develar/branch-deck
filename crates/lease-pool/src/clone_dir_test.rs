use crate::clone_dir::{CloneSkips, clone_directory};
use std::path::Path;

/// A template with build output under `out/`, a kept cache in it, a link into it and a `.git`
fn template(root: &Path) {
  for dir in ["src", "out/dev-run/jar-cache", "out/dev-run/idea", "out/deploy", "out/notes", ".git"] {
    std::fs::create_dir_all(root.join(dir)).unwrap();
  }
  for file in [
    "src/a.txt",
    "out/dev-run/jar-cache/j",
    "out/dev-run/idea/lib",
    "out/deploy/x",
    "out/notes/handoff.md",
    ".git/HEAD",
  ] {
    std::fs::write(root.join(file), file).unwrap();
  }
  #[cfg(unix)]
  std::os::unix::fs::symlink("/elsewhere/bazel-out", root.join("out/bazel-bin")).unwrap();
}

/// What a tree holds, as sorted relative paths
fn listing(root: &Path) -> Vec<String> {
  let mut paths = Vec::new();
  let mut stack = vec![root.to_path_buf()];
  while let Some(dir) = stack.pop() {
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
      let path = entry.path();
      paths.push(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
      if entry.file_type().unwrap().is_dir() {
        stack.push(path);
      }
    }
  }
  paths.sort();
  paths
}

/// A clone with skips leaves out the skipped paths but for their `except` entries, keeps everything else and a link
/// as the link, and leaves out the `.git` of the template; without skips it is the whole tree. Where no clone is, the
/// call is unsupported with or without skips, and the caller checks out instead
#[test]
fn a_clone_leaves_out_the_skipped_paths_but_for_their_kept_entries() {
  let dir = tempfile::tempdir().unwrap();
  let from = dir.path().join("template");
  template(&from);
  let skips = CloneSkips::new(&[
    ("out/dev-run".to_string(), vec!["jar-cache".to_string()]),
    ("out/deploy".to_string(), Vec::new()),
    ("out/bazel-bin".to_string(), Vec::new()),
  ]);
  let cloned = clone_directory(&from, &dir.path().join("clone"), &skips);
  let whole = clone_directory(&from, &dir.path().join("whole"), &CloneSkips::default());
  if cfg!(target_os = "macos") {
    cloned.unwrap();
    whole.unwrap();
    assert_eq!(
      listing(&dir.path().join("clone")),
      [
        "out",
        "out/dev-run",
        "out/dev-run/jar-cache",
        "out/dev-run/jar-cache/j",
        "out/notes",
        "out/notes/handoff.md",
        "src",
        "src/a.txt"
      ]
    );
    assert_eq!(listing(&dir.path().join("whole")), listing(&from));
  } else {
    for result in [cloned, whole] {
      assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Unsupported);
    }
  }
}

/// A rule below a kept entry of another rule applies too: `out` but for `dev-run`, and `out/dev-run` but for
/// `jar-cache`, leave only the jar cache of `out`
#[test]
fn a_rule_below_a_kept_entry_applies_too() {
  let dir = tempfile::tempdir().unwrap();
  let from = dir.path().join("template");
  template(&from);
  let skips = CloneSkips::new(&[("out".to_string(), vec!["dev-run".to_string()]), ("out/dev-run".to_string(), vec!["jar-cache".to_string()])]);
  let cloned = clone_directory(&from, &dir.path().join("clone"), &skips);
  if cfg!(target_os = "macos") {
    cloned.unwrap();
    assert_eq!(
      listing(&dir.path().join("clone")),
      ["out", "out/dev-run", "out/dev-run/jar-cache", "out/dev-run/jar-cache/j", "src", "src/a.txt"]
    );
  } else {
    assert_eq!(cloned.unwrap_err().kind(), std::io::ErrorKind::Unsupported);
  }
}
