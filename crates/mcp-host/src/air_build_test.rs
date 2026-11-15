use std::path::Path;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::air_build::{copy_build, rerun_paths};

/// A build of `files` files, each of which says the build it is of
fn write_build(dir: &Path, id: usize, files: usize) {
  std::fs::create_dir_all(dir.join("assets")).unwrap();
  std::fs::write(dir.join("index.html"), format!("build {id}")).unwrap();
  for file in 1..files {
    std::fs::write(dir.join("assets").join(format!("chunk-{id}-{file}.js")), format!("build {id}")).unwrap();
  }
}

/// The builds a copy holds: one id per file
fn ids_of(files: &[(String, std::path::PathBuf)]) -> std::collections::BTreeSet<String> {
  files.iter().map(|(_, copy)| std::fs::read_to_string(copy).unwrap()).collect()
}

/// The stamp is always a path the script reruns on, the build too where there is one: no rerun of the script on every
/// build while no build of the page is there, and the next publish (which touches the stamp) reads it again
#[test]
fn the_script_reruns_on_the_stamp_always_and_on_the_build_where_there_is_one() {
  let dir = tempfile::tempdir().unwrap();
  let (dist, stamp) = (dir.path().join("dist-air"), dir.path().join("air-build.stamp"));
  assert_eq!(rerun_paths(&dist, &stamp), vec![stamp.clone()]);
  write_build(&dist, 1, 3);
  assert_eq!(rerun_paths(&dist, &stamp), vec![stamp, dist]);
}

/// No build of the page: no files, the stub page of `brd mcp`, and no wait
#[test]
fn no_build_copies_no_file() {
  let dir = tempfile::tempdir().unwrap();
  let copies = dir.path().join("copies");
  let files = copy_build(&dir.path().join("dist-air"), &dir.path().join("builds"), &copies, Duration::from_secs(2)).unwrap();
  assert_eq!(files, Vec::new());
}

/// A build in place (`BRANCH_DECK_AIR_DIST`) is copied whole, each file by its path in the build
#[test]
fn a_build_in_place_is_copied_by_the_paths_of_its_files() {
  let dir = tempfile::tempdir().unwrap();
  let dist = dir.path().join("dist-air");
  write_build(&dist, 7, 3);
  let files = copy_build(&dist, &dir.path().join("builds"), &dir.path().join("copies"), Duration::from_secs(2)).unwrap();
  let names: Vec<_> = files.iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(names, ["assets/chunk-7-1.js", "assets/chunk-7-2.js", "index.html"]);
  assert_eq!(ids_of(&files), ["build 7".to_owned()].into());
}

/// `dist-air` replaced by a new build again and again while it is copied, each by one rename of a new link and the
/// build before the last pruned, as the publish of vite.config.ts does: every copy is of one whole build
#[cfg(unix)]
#[test]
fn every_copy_is_of_one_whole_build_while_new_builds_replace_it() {
  let dir = tempfile::tempdir().unwrap();
  let builds = dir.path().join("builds");
  let dist = dir.path().join("dist-air");
  let publish = |id: usize| {
    let build = builds.join(format!("{id:08}"));
    write_build(&build, id, 20);
    let link = builds.join(format!("link-{id}"));
    std::os::unix::fs::symlink(&build, &link).unwrap();
    std::fs::rename(&link, &dist).unwrap();
    if id >= 3 {
      std::fs::remove_dir_all(builds.join(format!("{:08}", id - 3))).unwrap();
    }
  };
  publish(0);
  let stop = AtomicBool::new(false);
  std::thread::scope(|scope| {
    scope.spawn(|| {
      let mut id = 1;
      while !stop.load(Ordering::Relaxed) {
        publish(id);
        id += 1;
      }
    });
    // the publisher stops also when an assertion fails, so the scope ends with the failure
    let _stop = StopOnDrop(&stop);
    for attempt in 0..50 {
      let files = copy_build(&dist, &builds, &dir.path().join("copies"), Duration::from_secs(2)).unwrap();
      assert_eq!(files.len(), 20, "copy {attempt}");
      assert_eq!(ids_of(&files).len(), 1, "copy {attempt} mixes builds");
    }
  });
}

/// Stops the publisher of a test as it drops, the end of the copies or a failed assertion
#[cfg(unix)]
struct StopOnDrop<'a>(&'a AtomicBool);

#[cfg(unix)]
impl Drop for StopOnDrop<'_> {
  fn drop(&mut self) {
    self.0.store(true, Ordering::Relaxed);
  }
}

/// A link to a build that is gone (`cargo clean` took `target/`) is no build
#[cfg(unix)]
#[test]
fn a_link_to_a_removed_build_is_no_build() {
  let dir = tempfile::tempdir().unwrap();
  let dist = dir.path().join("dist-air");
  std::os::unix::fs::symlink(dir.path().join("builds/gone"), &dist).unwrap();
  let files = copy_build(&dist, &dir.path().join("builds"), &dir.path().join("copies"), Duration::from_secs(2)).unwrap();
  assert_eq!(files, Vec::new());
}
