use crate::names::{GeneratedName, generated_name, is_lock_file};
use pretty_assertions::assert_eq;

#[test]
fn a_lock_file_a_minified_file_a_source_map_and_a_snapshot_are_generated_by_their_name() {
  for (path, name) in [
    ("Cargo.lock", Some(GeneratedName::LockFile)),
    ("web/pnpm-lock.yaml", Some(GeneratedName::LockFile)),
    ("go.sum", Some(GeneratedName::LockFile)),
    ("dist/app.min.js", Some(GeneratedName::Minified)),
    ("dist/app.min.css", Some(GeneratedName::Minified)),
    ("dist/APP.MIN.JS", Some(GeneratedName::Minified)),
    ("dist/app.js.map", Some(GeneratedName::SourceMap)),
    ("src/__snapshots__/view.test.ts.snap", Some(GeneratedName::Snapshot)),
    // a lock file by name only: an unknown `*.lock` and a path that only holds the name are not
    ("tools/build.lock", None),
    ("Cargo.lock/notes.md", None),
    ("src/app.js", None),
    ("src/minimap.ts", None),
  ] {
    assert_eq!(generated_name(path), name, "{path}");
  }
  assert!(is_lock_file("crates/a/Cargo.lock"));
  assert!(!is_lock_file("dist/app.min.js"));
}
