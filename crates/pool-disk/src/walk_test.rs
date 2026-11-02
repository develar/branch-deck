use crate::walk::{Walk, walk_bytes};
use std::path::Path;

/// A tree of `count` files of `size` bytes each under `root`, in nested directories
fn tree(root: &Path, count: usize, size: usize) {
  for n in 0..count {
    let dir = root.join(format!("{}/{}", n % 7, n % 3));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{n}.bin")), vec![u8::try_from(n % 251).unwrap(); size]).unwrap();
  }
}

#[test]
fn a_walk_counts_each_root_side_by_side_skips_what_counts_apart_and_a_hard_linked_file_once() {
  let dir = tempfile::tempdir().unwrap();
  let roots: Vec<_> = (0..3).map(|n| dir.path().join(n.to_string())).collect();
  for (n, root) in roots.iter().enumerate() {
    tree(root, 300, 4096 * (n + 1));
  }
  let walks: Vec<Walk> = roots.iter().map(Walk::of).collect();
  let alone = walk_bytes(&walks);
  assert!(alone[0] >= 300 * 4096 && alone[1] >= 300 * 2 * 4096 && alone[2] >= 300 * 3 * 4096, "{alone:?}");
  // cargo links the same file into `deps/` and the profile dir: one file
  #[cfg(unix)]
  std::fs::hard_link(roots[0].join("0/0/0.bin"), roots[0].join("0/linked")).unwrap();
  assert_eq!(walk_bytes(&walks), alone);
  // a directory measured apart counts nothing in its parent's walk
  let skipping = Walk {
    root: roots[0].clone(),
    skip: vec![roots[0].join("0")],
  };
  let [skipped] = walk_bytes(&[skipping])[..] else { panic!() };
  assert!(skipped < alone[0] && skipped > 0, "{skipped} of {}", alone[0]);
  // a file as a root, and a root that is gone
  let file = walk_bytes(&[Walk::of(roots[2].join("0/0/0.bin")), Walk::of(dir.path().join("gone"))]);
  assert!(file[0] >= 3 * 4096 && file[1] == 0, "{file:?}");
}

/// An APFS clone shares its blocks with its template: the walk counts what a delete of it frees, near nothing until
/// one of them is written again
#[cfg(target_os = "macos")]
#[test]
fn a_clone_counts_only_the_blocks_it_shares_with_no_other_file() {
  let dir = tempfile::tempdir().unwrap();
  let template = dir.path().join("template");
  tree(&template, 50, 256 * 1024);
  let clone = dir.path().join("clone");
  lease_pool::clone_dir::clone_directory(&template, &clone, &lease_pool::clone_dir::CloneSkips::default()).unwrap();
  let [template_bytes, clone_bytes] = walk_bytes(&[Walk::of(&template), Walk::of(&clone)])[..] else {
    panic!()
  };
  assert_eq!((template_bytes, clone_bytes), (0, 0), "every block is shared");
  // a file written again in the clone is its own
  std::fs::write(clone.join("0/0/0.bin"), vec![9u8; 256 * 1024]).unwrap();
  let [_, written] = walk_bytes(&[Walk::of(&template), Walk::of(&clone)])[..] else {
    panic!()
  };
  assert!((256 * 1024..=2 * 256 * 1024).contains(&written), "{written}");
}
