use crate::{Walk, tree_size, walk_bytes, walk_bytes_with_callback, walk_tree};
use std::path::Path;
use std::sync::Mutex;

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
  #[cfg(unix)]
  std::fs::hard_link(roots[0].join("0/0/0.bin"), roots[0].join("0/linked")).unwrap();
  assert_eq!(walk_bytes(&walks), alone);
  #[cfg(unix)]
  {
    let trees = walk_tree(&walks, 3);
    assert!(trees[0].has_hardlinks);
    assert!(trees[0].nodes.iter().any(|node| node.has_hardlinks));
  }
  let skipping = Walk {
    root: roots[0].clone(),
    skip: vec![roots[0].join("0")],
  };
  let [skipped] = walk_bytes(&[skipping])[..] else { panic!() };
  assert!(skipped < alone[0] && skipped > 0, "{skipped} of {}", alone[0]);
  let file = walk_bytes(&[Walk::of(roots[2].join("0/0/0.bin")), Walk::of(dir.path().join("gone"))]);
  assert!(file[0] >= 3 * 4096 && file[1] == 0, "{file:?}");
}

#[test]
fn tree_size_uses_the_walk_and_counts_directory_bytes() {
  let dir = tempfile::tempdir().unwrap();
  tree(dir.path(), 30, 4096);
  let walked = walk_bytes(&[Walk::of(dir.path())])[0];

  assert!(tree_size(dir.path()) >= walked);
  assert_eq!(tree_size(&dir.path().join("gone")), 0);
  assert_eq!(tree_size(&dir.path().join("0/0/0.bin")), walk_bytes(&[Walk::of(dir.path().join("0/0/0.bin"))])[0]);
}

#[test]
fn walk_callback_reports_each_root_once_when_it_is_complete() {
  let dir = tempfile::tempdir().unwrap();
  let first = dir.path().join("first");
  let second = dir.path().join("second");
  std::fs::create_dir_all(first.join("nested")).unwrap();
  std::fs::write(first.join("nested/output"), vec![1u8; 8192]).unwrap();
  std::fs::write(&second, vec![2u8; 4096]).unwrap();
  let walks = [Walk::of(&first), Walk::of(&second), Walk::of(dir.path().join("gone"))];
  let reports = Mutex::new(Vec::new());

  let bytes = walk_bytes_with_callback(&walks, |index, bytes| reports.lock().unwrap().push((index, bytes)));
  let mut reports = reports.into_inner().unwrap();
  reports.sort_unstable_by_key(|(index, _)| *index);

  assert_eq!(reports, bytes.into_iter().enumerate().collect::<Vec<_>>());
}

#[test]
fn walk_tree_nodes_and_root_remainder_add_up_to_the_walk() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let deep = root.join("deep/branch/node");
  let shallow = root.join("shallow");
  std::fs::create_dir_all(&deep).unwrap();
  std::fs::create_dir_all(&shallow).unwrap();
  std::fs::write(root.join("root.bin"), vec![1u8; 8192]).unwrap();
  std::fs::write(deep.join("deep.bin"), vec![2u8; 4096]).unwrap();
  std::fs::write(shallow.join("shallow.bin"), vec![3u8; 4096]).unwrap();

  let [walked] = walk_bytes(&[Walk::of(root)])[..] else { panic!() };
  let trees = walk_tree(&[Walk::of(root)], 3);
  let [tree] = trees.as_slice() else { panic!() };
  let node_bytes: u64 = tree.nodes.iter().map(|node| node.bytes).sum();
  let remainder = tree.bytes - node_bytes;
  let paths: Vec<&str> = tree.nodes.iter().map(|node| node.path.as_str()).collect();

  assert_eq!(tree.bytes, walked);
  assert_eq!(remainder, walk_bytes(&[Walk::of(root.join("root.bin"))])[0]);
  assert_eq!(paths, ["deep/branch/node", "shallow"]);
  let shallow_trees = walk_tree(&[Walk::of(root)], 1);
  let [shallow_tree] = shallow_trees.as_slice() else { panic!() };
  assert_eq!(shallow_tree.nodes.iter().map(|node| node.path.as_str()).collect::<Vec<_>>(), ["deep", "shallow"]);
  let root_trees = walk_tree(&[Walk::of(root)], 0);
  let [root_only] = root_trees.as_slice() else { panic!() };
  assert_eq!(root_only.nodes.len(), 0);
  assert_eq!(node_bytes + remainder, tree.bytes);
}
