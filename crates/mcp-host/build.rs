//! The files of the Air build of the page (`pnpm build:air` into `dist-air/`, or the directory
//! `BRANCH_DECK_AIR_DIST` names) built into `brd mcp`: `resources.rs` of `OUT_DIR` lists each with its bytes. Without
//! the directory the list is empty, and `brd mcp` serves a stub page; a new build of the page builds the list again.
//!
//! rustc reads copies in `OUT_DIR`, never the build itself: a build of the page beside a cargo build (the global setup
//! of the e2e tests beside nextest, clippy or the e2e server) replaces `dist-air/` meanwhile. `scripts/build-air.mjs`
//! builds the next one into `target/dist-air.next/` and moves it into the place of `dist-air/` whole, the old one to
//! `target/dist-air.old/` first, so a copy is of one finished build: one whose files went while they were copied (the
//! old build moved away) is copied again, of the next build.

use std::fmt::Write;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// How many times a copy starts again after the build it copied was replaced: one replace takes two renames
const COPY_ATTEMPTS: usize = 8;

fn main() {
  println!("cargo::rerun-if-env-changed=BRANCH_DECK_AIR_DIST");
  let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
  let named = std::env::var_os("BRANCH_DECK_AIR_DIST").map(PathBuf::from);
  // where `pnpm build:air` moves the build it replaces, before the next one takes its place
  let old = named.is_none().then(|| manifest.join("../../target/dist-air.old"));
  let dist = named.unwrap_or_else(|| manifest.join("../../dist-air"));
  let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
  let files = copy_build(&dist, old.as_deref(), &out.join("air"));
  if dist.is_dir() {
    println!("cargo::rerun-if-changed={}", dist.display());
  }
  let mut source = String::from("/// The files of the Air build of the page, by their path in it\npub static FILES: &[(&str, &[u8])] = &[\n");
  for (path, file) in &files {
    let _ = writeln!(source, "  ({path:?}, include_bytes!({:?})),", file.display().to_string());
  }
  source.push_str("];\n");
  std::fs::write(out.join("resources.rs"), source).expect("OUT_DIR is writable");
}

/// The files of the build in `dist` copied into `copies`, by their path in it, sorted; none without a build. A build
/// replaced while it was copied (a file of it went, or it is in `old` and the next one not yet in its place) is copied
/// again
fn copy_build(dist: &Path, old: Option<&Path>, copies: &Path) -> Vec<(String, PathBuf)> {
  for _ in 0..COPY_ATTEMPTS {
    remove_copies(copies);
    if !dist.is_dir() {
      if old.is_some_and(Path::is_dir) {
        continue;
      }
      return Vec::new();
    }
    let mut files = Vec::new();
    match copy_dir(dist, dist, copies, &mut files) {
      Ok(()) => {
        files.sort();
        return files;
      }
      Err(error) if error.kind() == ErrorKind::NotFound => {}
      Err(error) => panic!("cannot copy the Air build {}: {error}", dist.display()),
    }
  }
  // a build left in `old` by a `pnpm build:air` that stopped between its renames: no build, as before the first one
  if !dist.is_dir() {
    remove_copies(copies);
    return Vec::new();
  }
  panic!("the Air build {} was replaced {COPY_ATTEMPTS} times while it was copied", dist.display())
}

fn remove_copies(copies: &Path) {
  match std::fs::remove_dir_all(copies) {
    Ok(()) => {}
    Err(error) if error.kind() == ErrorKind::NotFound => {}
    Err(error) => panic!("cannot remove the copy {} of the Air build: {error}", copies.display()),
  }
}

/// Every file under `dir` copied to its path from `root` under `copies`, with that path in `/`
fn copy_dir(root: &Path, dir: &Path, copies: &Path, files: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
  for entry in std::fs::read_dir(dir)? {
    let path = entry?.path();
    if path.is_dir() {
      copy_dir(root, &path, copies, files)?;
      continue;
    }
    let Ok(relative) = path.strip_prefix(root) else {
      continue;
    };
    let copy = copies.join(relative);
    std::fs::create_dir_all(copy.parent().expect("a file under the copies has a parent"))?;
    std::fs::copy(&path, &copy)?;
    let name = relative.components().map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
    files.push((name, copy));
  }
  Ok(())
}
