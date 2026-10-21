use crate::agent_workspace::{subdir, unwritable_dir};
use crate::lease::canonical_path;
use pretty_assertions::assert_eq;
use std::path::Path;

#[test]
fn the_subdir_is_the_path_below_the_top_level() {
  let root = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(root.path().join("crates/x")).unwrap();
  let top_level = canonical_path(root.path());
  assert_eq!(subdir(root.path().join("crates/x").to_str().unwrap(), &top_level), "crates/x");
  assert_eq!(subdir(root.path().to_str().unwrap(), &top_level), "");
  // outside the top level, as a `.git` directory: nothing to continue in
  assert_eq!(subdir("/", &top_level), "");
}

/// The probe of a pool that does not exist yet is made in its first existing parent, and leaves nothing behind
#[test]
fn the_probe_of_the_pool_writes_its_first_existing_parent_and_removes_its_file() {
  let home = tempfile::tempdir().unwrap();
  assert_eq!(unwritable_dir(&home.path().join(".branch-deck/pool/repo-1234")), None);
  assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
  assert!(!Path::new(&home.path().join(".branch-deck")).exists());
}
