use crate::trim::{machine_pools, parse_max_slots, remove_all, remove_all_until, slot_dir};
use git_ops::revs::RepositoryDirs;
use std::path::{Path, PathBuf};

/// A linked worktree `w1` of the repository `<dir>/other` at `at`, as git writes one: its `.git` file and the admin
/// directory that names it back
fn fake_worktree(dir: &Path, at: &Path) -> PathBuf {
  let git_common_dir = dir.join("other/.git");
  let admin = git_common_dir.join("worktrees/w1");
  std::fs::create_dir_all(&admin).unwrap();
  std::fs::create_dir_all(at).unwrap();
  std::fs::write(admin.join("commondir"), "../..\n").unwrap();
  std::fs::write(admin.join("gitdir"), format!("{}\n", at.join(".git").display())).unwrap();
  std::fs::write(at.join(".git"), format!("gitdir: {}\n", admin.display())).unwrap();
  std::fs::canonicalize(git_common_dir).unwrap()
}

/// The notes of the leases beside the slots of a pool directory are no slot: a worktree an agent made in them names no
/// pool of the machine, while the same worktree as a slot (`<pool>/<n>/<name>`) does
#[test]
fn a_worktree_in_the_notes_of_a_pool_directory_names_no_pool() {
  let dir = tempfile::tempdir().unwrap();
  let home = dir.path().join("home");
  let repository = RepositoryDirs {
    top_level: dir.path().join("repo").to_string_lossy().into_owned(),
    git_dir: dir.path().join("repo/.git").to_string_lossy().into_owned(),
    git_common_dir: dir.path().join("repo/.git").to_string_lossy().into_owned(),
  };
  let pool_dir = home.join(".branch-deck/pool/other-12345678");
  let other = fake_worktree(dir.path(), &pool_dir.join("notes/lease-1/repo"));
  let pools: Vec<PathBuf> = machine_pools(&repository, Some(&home)).into_iter().map(|pool| pool.git_common_dir).collect();
  assert!(!pools.contains(&other), "{pools:?}");

  std::fs::rename(pool_dir.join("notes"), pool_dir.join("1")).unwrap();
  std::fs::rename(pool_dir.join("1/lease-1/repo"), pool_dir.join("1/repo")).unwrap();
  fake_worktree(dir.path(), &pool_dir.join("1/repo"));
  let pools: Vec<PathBuf> = machine_pools(&repository, Some(&home)).into_iter().map(|pool| pool.git_common_dir).collect();
  assert!(pools.contains(&other), "{pools:?}");
}

#[test]
fn the_number_directory_of_a_slot_goes_with_it_when_the_slot_is_its_only_directory() {
  let dir = tempfile::tempdir().unwrap();
  let alone = dir.path().join("1/repo");
  std::fs::create_dir_all(&alone).unwrap();
  // a log an agent left beside the slot is pool property too
  std::fs::write(dir.path().join("1/probe.log"), "").unwrap();
  assert_eq!(slot_dir(&alone), dir.path().join("1"));
  let beside = dir.path().join("2/repo");
  std::fs::create_dir_all(&beside).unwrap();
  std::fs::create_dir_all(dir.path().join("2/other")).unwrap();
  assert_eq!(slot_dir(&beside), beside);
  let unnumbered = dir.path().join("named/repo");
  std::fs::create_dir_all(&unnumbered).unwrap();
  assert_eq!(slot_dir(&unnumbered), unnumbered);
}

#[test]
fn every_path_is_deleted_side_by_side_and_its_bytes_counted() {
  let dir = tempfile::tempdir().unwrap();
  let paths: Vec<_> = (0..3).map(|n| dir.path().join(n.to_string())).collect();
  for path in &paths {
    for child in 0..20 {
      let child = path.join(child.to_string());
      std::fs::create_dir_all(&child).unwrap();
      std::fs::write(child.join("file"), vec![1u8; 8192]).unwrap();
    }
  }
  let file = dir.path().join("file");
  std::fs::write(&file, vec![1u8; 8192]).unwrap();
  let mut all: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
  all.push(&file);
  let freed: Vec<u64> = remove_all(&all).into_iter().map(Result::unwrap).collect();
  assert!(freed[..3].iter().all(|&bytes| bytes >= 20 * 8192), "{freed:?}");
  assert!(freed[3] >= 8192, "{freed:?}");
  assert!(all.iter().all(|path| !path.exists()));
  // a path that is gone frees nothing
  assert_eq!(remove_all(&[dir.path().join("gone").as_path()]).into_iter().map(Result::unwrap).collect::<Vec<_>>(), [0]);
}

/// A delete of the eviction that the volume needs no more stops before it begins (the volume got the minimum since the
/// last slot) and deletes nothing; one never stopped deletes everything
#[test]
fn a_delete_side_by_side_stops_when_its_stop_holds_and_leaves_the_paths() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("slot");
  for child in 0..20 {
    std::fs::create_dir_all(path.join(child.to_string())).unwrap();
    std::fs::write(path.join(child.to_string()).join("file"), vec![1u8; 8192]).unwrap();
  }
  let (results, complete) = remove_all_until(&[path.as_path()], &|| true);
  assert_eq!((results.into_iter().map(Result::unwrap).collect::<Vec<_>>(), complete), (vec![0], false));
  assert_eq!(std::fs::read_dir(&path).unwrap().count(), 20);
  let (results, complete) = remove_all_until(&[path.as_path()], &|| false);
  assert!(complete && results[0].as_ref().is_ok_and(|freed| *freed >= 20 * 8192), "{results:?}");
  assert!(!path.exists());
}

#[test]
fn the_limit_is_a_whole_number_of_slots() {
  assert_eq!(parse_max_slots(" 4 "), Ok(4));
  assert_eq!(parse_max_slots("0"), Ok(0));
  assert!(parse_max_slots("10G").unwrap_err().starts_with("not a whole number of slots"));
}
