use crate::output_bases::{OutputBase, orphans, output_bases, output_user_roots};
use std::path::{Path, PathBuf};
use test_utils::links::directory_link;

/// An output base `name` under `root` that names `workspace`
fn base(root: &Path, name: &str, workspace: &Path) -> PathBuf {
  let base = root.join(name);
  std::fs::create_dir_all(base.join("execroot/_main/bazel-out")).unwrap();
  std::fs::write(base.join("DO_NOT_BUILD_HERE"), workspace.to_string_lossy().as_bytes()).unwrap();
  base
}

#[test]
fn the_roots_are_found_by_the_bazel_out_links_of_the_workspaces_of_a_checkout() {
  let dir = tempfile::tempdir().unwrap();
  let dir = std::fs::canonicalize(dir.path()).unwrap();
  let (first, second) = (dir.join("root-1"), dir.join("root-2"));
  let checkout = dir.join("checkout");
  // the checkout links through `out/` (`--symlink_prefix=out/bazel-`), its `community/` at its root
  let main = base(&first, "main", &checkout);
  let community = base(&second, "community", &checkout.join("community"));
  std::fs::create_dir_all(checkout.join("out")).unwrap();
  std::fs::create_dir_all(checkout.join("community")).unwrap();
  std::fs::create_dir_all(checkout.join("docs")).unwrap();
  std::fs::write(checkout.join("MODULE.bazel"), "").unwrap();
  std::fs::write(checkout.join("community/WORKSPACE"), "").unwrap();
  directory_link(&main.join("execroot/_main/bazel-out"), &checkout.join("out/bazel-out"));
  directory_link(&community.join("execroot/_main/bazel-out"), &checkout.join("community/bazel-out"));
  // a directory without a workspace file has no link that counts
  directory_link(&community.join("execroot/_main/bazel-out"), &checkout.join("docs/bazel-out"));
  assert_eq!(output_user_roots([checkout.as_path(), dir.join("gone").as_path()]), [first, second]);
}

#[test]
fn the_output_bases_of_a_root_name_their_workspace_and_the_gone_ones_of_a_pool_are_orphans() {
  let dir = tempfile::tempdir().unwrap();
  let dir = std::fs::canonicalize(dir.path()).unwrap();
  let root = dir.join("root");
  let pool = dir.join("pool");
  let slot = pool.join("1/repo");
  std::fs::create_dir_all(&slot).unwrap();
  let kept = base(&root, "a", &slot);
  let gone = base(&root, "b", &pool.join("2/repo/community"));
  // a workspace that the commit of its slot lacks: the slot is there
  let lacking = base(&root, "ba", &slot.join("community"));
  let outside = base(&root, "c", &dir.join("elsewhere"));
  // no `DO_NOT_BUILD_HERE`, or a relative workspace: no output base
  std::fs::create_dir_all(root.join("install")).unwrap();
  base(&root, "d", Path::new("relative"));
  let bases = output_bases(std::slice::from_ref(&root));
  assert_eq!(
    bases,
    [
      OutputBase {
        path: kept,
        workspace: slot.clone(),
      },
      OutputBase {
        path: gone.clone(),
        workspace: pool.join("2/repo/community"),
      },
      OutputBase {
        path: lacking,
        workspace: slot.join("community"),
      },
      OutputBase {
        path: outside,
        workspace: dir.join("elsewhere"),
      },
    ]
  );
  assert!(bases[0].is_in(&slot) && !bases[1].is_in(&slot));
  let found: Vec<&Path> = orphans(&bases, std::slice::from_ref(&pool)).into_iter().map(|base| base.path.as_path()).collect();
  assert_eq!(found, [gone.as_path()]);
  assert!(!bases[0].has_server());
}
