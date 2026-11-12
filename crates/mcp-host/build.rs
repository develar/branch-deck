//! The files of the Air build of the page (`vite build --mode air` into `dist-air/`, or the directory
//! `BRANCH_DECK_AIR_DIST` names) built into `brd mcp`: `resources.rs` of `OUT_DIR` lists each with its bytes. Without
//! the directory the list is empty, and `brd mcp` serves a stub page; a new build of the page builds the list again.

use std::fmt::Write;
use std::path::{Path, PathBuf};

fn main() {
  println!("cargo::rerun-if-env-changed=BRANCH_DECK_AIR_DIST");
  let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
  let dist = std::env::var_os("BRANCH_DECK_AIR_DIST").map_or_else(|| manifest.join("../../dist-air"), PathBuf::from);
  let mut files = Vec::new();
  if dist.is_dir() {
    println!("cargo::rerun-if-changed={}", dist.display());
    collect(&dist, &dist, &mut files);
  }
  files.sort();
  let mut source = String::from("/// The files of the Air build of the page, by their path in it\npub static FILES: &[(&str, &[u8])] = &[\n");
  for (path, file) in &files {
    let _ = writeln!(source, "  ({path:?}, include_bytes!({:?})),", file.display().to_string());
  }
  source.push_str("];\n");
  let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
  std::fs::write(out.join("resources.rs"), source).expect("OUT_DIR is writable");
}

/// Every file under `dir`, by its path from `root` with `/`
fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
  let Ok(entries) = std::fs::read_dir(dir) else {
    return;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      collect(root, &path, files);
    } else if let Ok(relative) = path.strip_prefix(root) {
      let name = relative.components().map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
      files.push((name, path.canonicalize().unwrap_or(path)));
    }
  }
}
