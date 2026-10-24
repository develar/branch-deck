//! A copy of a whole directory tree by APFS `clonefile(2)` (macOS): the clone shares the blocks of every file until one
//! side writes, and keeps the mtime and the size of each file, with new inodes and ctimes. The lease pool (`lease`)
//! adds a slot this way. Without skips it is one call for the whole tree; with them (the `delete` rules of
//! `.branchdeck.toml`, `lease_clean::clone_skips`) the directories on the way to a skipped path are made anew and the
//! rest of each is cloned entry by entry, so a clone of a main checkout leaves out its build output. Elsewhere the call
//! is unsupported, and the caller checks out instead.

use std::collections::BTreeMap;
use std::path::Path;

/// The paths a clone leaves out, as a tree of names from the root of the template
#[derive(Debug, Default)]
pub struct CloneSkips {
  children: BTreeMap<String, Self>,
  /// This path is left out, but for these entries of it
  skipped: Option<Vec<String>>,
}

impl CloneSkips {
  /// The tree of `skips`: relative, normalized paths (`a/b`), each with the names of its entries that are cloned all
  /// the same
  pub fn new(skips: &[(String, Vec<String>)]) -> Self {
    let mut root = Self::default();
    for (path, except) in skips {
      let node = path.split('/').fold(&mut root, |node, name| node.children.entry(name.to_string()).or_default());
      node.skipped = Some(except.clone());
    }
    root
  }

  fn is_empty(&self) -> bool {
    self.children.is_empty() && self.skipped.is_none()
  }
}

/// Clone the directory `from` to `to`, which must not exist and must be on the same volume, leaving out `skips`, and
/// the `.git` of the template when there are any (the caller replaces it). An error (`EXDEV`, `ENOTSUP`, `EACCES` for a
/// file it can't read, `Unsupported` outside macOS) may leave a part at `to`; the caller removes it
pub fn clone_directory(from: &Path, to: &Path, skips: &CloneSkips) -> std::io::Result<()> {
  if skips.is_empty() {
    return platform::clone(from, to);
  }
  clone_except(from, to, skips, true)
}

/// `from` to `to` without the entries `skips` leaves out: a skipped path is left out whole (a file, a symlink, a
/// directory without kept entries); a directory on the way, or a skipped one with kept entries, is made with the
/// permissions of the template; a symlink on the way is cloned as the link
fn clone_except(from: &Path, to: &Path, skips: &CloneSkips, root: bool) -> std::io::Result<()> {
  let metadata = from.symlink_metadata()?;
  if skips.skipped.as_ref().is_some_and(|except| except.is_empty() || !metadata.is_dir()) {
    return Ok(());
  }
  if !metadata.is_dir() {
    return platform::clone(from, to);
  }
  // the parent exists, and `to` must not: a directory left there by another run is an error, not a merge
  #[expect(clippy::create_dir, reason = "the clone must not merge into a directory that is there already")]
  std::fs::create_dir(to)?;
  std::fs::set_permissions(to, metadata.permissions())?;
  for entry in std::fs::read_dir(from)? {
    let entry = entry?;
    let name = entry.file_name().to_string_lossy().into_owned();
    if root && name == ".git" {
      continue;
    }
    let (source, target) = (entry.path(), to.join(&name));
    match &skips.skipped {
      Some(except) if except.contains(&name) => platform::clone(&source, &target)?,
      Some(_) => {}
      None => match skips.children.get(&name) {
        Some(child) => clone_except(&source, &target, child, false)?,
        None => platform::clone(&source, &target)?,
      },
    }
  }
  Ok(())
}

#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "clonefile(2), which std does not wrap")]
mod platform {
  use std::ffi::CString;
  use std::os::unix::ffi::OsStrExt;
  use std::path::Path;

  /// `CLONE_NOFOLLOW` of `<sys/clonefile.h>`, which the libc crate lacks: a symlink is cloned as the link
  const CLONE_NOFOLLOW: u32 = 0x0001;

  pub(super) fn clone(from: &Path, to: &Path) -> std::io::Result<()> {
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: both are NUL-terminated paths that live across the call
    let result = unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), CLONE_NOFOLLOW) };
    if result == 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
  }
}

#[cfg(not(target_os = "macos"))]
mod platform {
  use std::path::Path;

  pub(super) fn clone(_from: &Path, _to: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no directory clone on this platform"))
  }
}

#[cfg(test)]
#[path = "clone_dir_test.rs"]
mod clone_dir_test;
