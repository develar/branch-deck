//! The links of a build output that a removal must take as links: Bazel's `bazel-*` links, junctions on Windows (made
//! without admin rights and without a `cmd /C mklink` process), symlinks elsewhere.

use std::path::Path;

/// A link at `link` to the directory `target`: on Windows a junction, as Bazel makes its `bazel-*` links there
#[cfg(windows)]
pub fn directory_link(target: &Path, link: &Path) {
  junction::create(target, link).unwrap();
}

/// A link at `link` to the directory `target`: a symlink, as Bazel makes its `bazel-*` links here
#[cfg(unix)]
pub fn directory_link(target: &Path, link: &Path) {
  std::os::unix::fs::symlink(target, link).unwrap();
}
