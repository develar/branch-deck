//! The Air build of the page as the build script of mcp-host reads it (`build.rs`), a module of its own so that its
//! test (`src/air_build_test.rs`) runs it against builds replaced while it reads them.
//!
//! Each build of the page (`vite build --mode air`, the plugin `airBuild` of vite.config.ts) goes into a directory of
//! its own, `target/dist-air.builds/<time>-<pid>/`, written once; `dist-air` is a link to the newest, replaced by one
//! rename of a new link (a junction on Windows, removed first), and every publish touches `air-build.stamp` of the
//! crate. The build script resolves the link once and copies the files of that build into `OUT_DIR`, whose copies
//! rustc reads: a build of the page beside a cargo build (the global setup of the e2e tests beside nextest, clippy or
//! the e2e server) never gives it half a build.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The prefix of the link a publish creates beside the builds before it renames it into the place of `dist-air`: one
/// there while `dist-air` is missing is a publish between its two steps (Windows), which the copy waits for
pub(crate) const PUBLISH_LINK_PREFIX: &str = "link-";

/// How long a copy waits between two attempts: the next build is moved in at once, not written meanwhile
const ATTEMPT_WAIT: Duration = Duration::from_millis(25);

/// The paths the build script reruns on: the stamp, which every publish touches and which is in the repository, so the
/// script never reruns on every build while no build of the page is there; and the build itself when there is one, so
/// a build of the page in place (a directory, as `BRANCH_DECK_AIR_DIST` may name) is read again too
pub(crate) fn rerun_paths(dist: &Path, stamp: &Path) -> Vec<PathBuf> {
  let mut paths = vec![stamp.to_path_buf()];
  if dist.is_dir() {
    paths.push(dist.to_path_buf());
  }
  paths
}

/// The files of the build `dist` names copied into `copies`, by their path in it, sorted; none without a build (none
/// yet, or a link to a build `cargo clean` removed). The link is resolved once, so the copy is of one build; one whose
/// files went while they were copied (its build pruned, or a directory written in place) and a `dist` missing while a
/// publish link waits in `builds` are copied again after a short wait, for at most `patience`
pub(crate) fn copy_build(dist: &Path, builds: &Path, copies: &Path, patience: Duration) -> Result<Vec<(String, PathBuf)>, String> {
  let deadline = Instant::now() + patience;
  loop {
    remove_copies(copies)?;
    let attempt = match std::fs::canonicalize(dist) {
      Ok(root) if root.is_dir() => copy_tree(&root, copies),
      Ok(_) => return Ok(Vec::new()),
      Err(error) if error.kind() == ErrorKind::NotFound && !publishing(builds) => return Ok(Vec::new()),
      Err(error) => Err(error),
    };
    match attempt {
      Ok(mut files) => {
        files.sort();
        return Ok(files);
      }
      Err(error) if error.kind() == ErrorKind::NotFound && Instant::now() < deadline => std::thread::sleep(ATTEMPT_WAIT),
      Err(error) => return Err(format!("cannot copy the Air build {}: {error}", dist.display())),
    }
  }
}

/// A publish link waits in `builds`: `dist-air` is between its two steps
fn publishing(builds: &Path) -> bool {
  std::fs::read_dir(builds).is_ok_and(|entries| entries.flatten().any(|entry| entry.file_name().to_string_lossy().starts_with(PUBLISH_LINK_PREFIX)))
}

fn remove_copies(copies: &Path) -> Result<(), String> {
  match std::fs::remove_dir_all(copies) {
    Ok(()) => Ok(()),
    Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
    Err(error) => Err(format!("cannot remove the copy {} of the Air build: {error}", copies.display())),
  }
}

/// Every file under `root` copied to its path from `root` under `copies`, with that path in `/`
fn copy_tree(root: &Path, copies: &Path) -> std::io::Result<Vec<(String, PathBuf)>> {
  let mut files = Vec::new();
  let mut dirs = vec![root.to_path_buf()];
  while let Some(dir) = dirs.pop() {
    for entry in std::fs::read_dir(&dir)? {
      let path = entry?.path();
      if path.is_dir() {
        dirs.push(path);
        continue;
      }
      let Ok(relative) = path.strip_prefix(root) else {
        continue;
      };
      let copy = copies.join(relative);
      std::fs::create_dir_all(copy.parent().unwrap_or(copies))?;
      std::fs::copy(&path, &copy)?;
      let name = relative.components().map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
      files.push((name, copy));
    }
  }
  Ok(files)
}
