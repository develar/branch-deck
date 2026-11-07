#[cfg(target_os = "macos")]
use fs_walk::{Walk, walk_bytes};

/// An APFS clone shares its blocks with its template: the walk counts what a delete of it frees, near nothing until
/// one of them is written again
#[cfg(target_os = "macos")]
#[test]
fn a_clone_counts_only_the_blocks_it_shares_with_no_other_file() {
  let dir = tempfile::tempdir().unwrap();
  let template = dir.path().join("template");
  std::fs::create_dir_all(template.join("0/0")).unwrap();
  std::fs::write(template.join("0/0/0.bin"), vec![1u8; 50 * 256 * 1024]).unwrap();
  let clone = dir.path().join("clone");
  lease_pool::clone_dir::clone_directory(&template, &clone, &lease_pool::clone_dir::CloneSkips::default()).unwrap();
  let [template_bytes, clone_bytes] = walk_bytes(&[Walk::of(&template), Walk::of(&clone)])[..] else {
    panic!()
  };
  assert_eq!((template_bytes, clone_bytes), (0, 0), "every block is shared");
  // a file written again in the clone is its own
  std::fs::write(clone.join("0/0/0.bin"), vec![9u8; 50 * 256 * 1024]).unwrap();
  let [_, written] = walk_bytes(&[Walk::of(&template), Walk::of(&clone)])[..] else {
    panic!()
  };
  assert!((50 * 256 * 1024..=2 * 50 * 256 * 1024).contains(&written), "{written}");
}
